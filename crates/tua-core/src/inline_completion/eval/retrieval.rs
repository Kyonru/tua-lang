//! Phase 2 exact-retrieval evaluator index.
//!
//! This intentionally owns no VFS or host state. It measures immutable
//! per-file tables before production scheduling/adoption is introduced.

use std::collections::BTreeMap;
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Serialize;

use super::dataset::{display_name, EvalProject};
use super::metrics;
use crate::inline_completion::retrieval::{FilePatternTable, RetrievalLimits};

const NEAR_DUPLICATE_SIMILARITY: f64 = 0.90;
const NEAR_DUPLICATE_MIN_LINES: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum IndexState {
    Cold,
    Partial,
    Complete,
}

impl IndexState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::Partial => "partial",
            Self::Complete => "complete",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HoldoutMode {
    TargetFile,
    TargetLine,
}

impl HoldoutMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::TargetFile => "target-file",
            Self::TargetLine => "target-line",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RetrievalTier {
    CurrentFile,
    SameDirectory,
    Project,
    Bundled,
}

impl RetrievalTier {
    pub fn label(self) -> &'static str {
        match self {
            Self::CurrentFile => "current-file",
            Self::SameDirectory => "same-directory",
            Self::Project => "project",
            Self::Bundled => "bundled",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct NearDuplicateClusterReport {
    pub members: Vec<String>,
    pub representatives: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct RetrievalIndexReport {
    pub files_considered: usize,
    pub files_indexed: usize,
    pub files_skipped_by_limits: usize,
    pub files_excluded_as_near_duplicates: usize,
    pub partial_files: usize,
    pub complete_bytes: usize,
    pub partial_bytes: usize,
    /// Evaluator-only signatures and per-target holdout masks. These are not
    /// part of the production-shaped retrieval table byte totals above.
    pub evaluation_holdout_bytes: usize,
    pub build_ms: f64,
    pub table_build_ms: f64,
    pub signature_build_ms: f64,
    pub similarity_build_ms: f64,
    pub clustering_ms: f64,
    /// Expensive weighted-Jaccard comparisons, reused by every target view.
    pub pairwise_similarity_checks: usize,
    /// Temporary signatures and pairwise matrix retained only during build.
    pub clustering_working_bytes: usize,
    pub near_duplicate_clusters: Vec<NearDuplicateClusterReport>,
}

#[derive(Debug)]
struct IndexedFile {
    path: PathBuf,
    source_index: usize,
    table: FilePatternTable,
}

#[derive(Debug)]
struct TargetFileView {
    complete: Box<[bool]>,
    partial: Box<[bool]>,
}

struct SimilarityMatrix {
    rows: Vec<Box<[bool]>>,
    checks: usize,
}

impl SimilarityMatrix {
    fn build(
        files: &[IndexedFile],
        project: &EvalProject,
        signatures: &[BTreeMap<String, usize>],
    ) -> Self {
        let mut rows = Vec::with_capacity(files.len());
        let mut checks = 0;
        for left in 0..files.len() {
            let mut row = Vec::with_capacity(left);
            for right in 0..left {
                let left_file = &project.files[files[left].source_index];
                let right_file = &project.files[files[right].source_index];
                let similar = if left_file.kind == right_file.kind {
                    checks += 1;
                    near_duplicate(&signatures[left], &signatures[right])
                } else {
                    false
                };
                row.push(similar);
            }
            rows.push(row.into_boxed_slice());
        }
        Self { rows, checks }
    }

    fn get(&self, left: usize, right: usize) -> bool {
        if left == right {
            true
        } else if left > right {
            self.rows[left][right]
        } else {
            self.rows[right][left]
        }
    }

    fn approximate_bytes(&self) -> usize {
        size_of::<Self>()
            + self.rows.capacity() * size_of::<Box<[bool]>>()
            + self.rows.iter().map(|row| row.len()).sum::<usize>()
    }
}

#[derive(Debug)]
pub struct EvalProjectRetrievalIndex {
    files: Box<[IndexedFile]>,
    complete_representatives: Box<[bool]>,
    partial_representatives: Box<[bool]>,
    target_file_views: BTreeMap<PathBuf, TargetFileView>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetrievalScope {
    Combined,
    SameDirectory,
    Project,
}

impl RetrievalScope {
    fn includes(self, tier: RetrievalTier) -> bool {
        match self {
            Self::Combined => true,
            Self::SameDirectory => tier == RetrievalTier::SameDirectory,
            Self::Project => tier == RetrievalTier::Project,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectRetrievalHit {
    pub continuation: String,
    pub support: usize,
    pub tier: RetrievalTier,
}

impl EvalProjectRetrievalIndex {
    pub fn build(project: &EvalProject) -> (Self, RetrievalIndexReport) {
        let started = Instant::now();
        let table_started = Instant::now();
        let mut files = Vec::new();
        let mut skipped = 0;
        for (source_index, file) in project.files.iter().enumerate() {
            match FilePatternTable::build(&file.source, RetrievalLimits::default()) {
                Ok(table) => files.push(IndexedFile {
                    path: file.path.clone(),
                    source_index,
                    table,
                }),
                Err(_) => skipped += 1,
            }
        }
        files.sort_by(|left, right| left.path.cmp(&right.path));
        let table_build_ms = table_started.elapsed().as_secs_f64() * 1000.0;

        let signature_started = Instant::now();
        let signatures = files
            .iter()
            .map(|file| source_signature(&project.files[file.source_index].source))
            .collect::<Vec<_>>();
        let signature_build_ms = signature_started.elapsed().as_secs_f64() * 1000.0;
        let signature_bytes = signatures.iter().map(signature_bytes).sum::<usize>();

        let similarity_started = Instant::now();
        let similarities = SimilarityMatrix::build(&files, project, &signatures);
        let similarity_build_ms = similarity_started.elapsed().as_secs_f64() * 1000.0;
        let pairwise_similarity_checks = similarities.checks;
        let clustering_working_bytes = signature_bytes + similarities.approximate_bytes();

        let clustering_started = Instant::now();
        let all = (0..files.len()).collect::<Vec<_>>();
        let clusters = near_duplicate_clusters(&similarities, &all);
        let complete_representatives = representative_mask(files.len(), &clusters);
        let mut cluster_reports = Vec::new();
        for cluster in &clusters {
            let representatives = representative_indices(cluster);
            if cluster.len() > 1 {
                cluster_reports.push(NearDuplicateClusterReport {
                    members: cluster
                        .iter()
                        .map(|index| display_name(&files[*index].path))
                        .collect(),
                    representatives: representatives
                        .iter()
                        .map(|index| display_name(&files[*index].path))
                        .collect(),
                });
            }
        }
        let partial_files = if files.is_empty() {
            0
        } else {
            files.len().div_ceil(2)
        };
        let partial = (0..partial_files).collect::<Vec<_>>();
        let partial_representatives = representative_mask(
            files.len(),
            &near_duplicate_clusters(&similarities, &partial),
        );
        let target_file_views = files
            .iter()
            .map(|target| {
                let eligible = all
                    .iter()
                    .copied()
                    .filter(|index| files[*index].path != target.path)
                    .collect::<Vec<_>>();
                let partial_len = eligible.len().div_ceil(2);
                let complete = representative_mask(
                    files.len(),
                    &near_duplicate_clusters(&similarities, &eligible),
                );
                let partial = representative_mask(
                    files.len(),
                    &near_duplicate_clusters(&similarities, &eligible[..partial_len]),
                );
                (target.path.clone(), TargetFileView { complete, partial })
            })
            .collect::<BTreeMap<_, _>>();
        let clustering_ms = clustering_started.elapsed().as_secs_f64() * 1000.0;
        let files = files.into_boxed_slice();
        let bytes_for = |files: &[IndexedFile]| {
            size_of::<EvalProjectRetrievalIndex>()
                + size_of_val(files)
                + files
                    .iter()
                    .map(|file| {
                        file.path.as_os_str().len() + file.table.approximate_bytes()
                            - size_of::<FilePatternTable>()
                    })
                    .sum::<usize>()
        };
        let target_view_keys = target_file_views
            .keys()
            .map(|path| size_of::<PathBuf>() + path.as_os_str().len())
            .sum::<usize>();
        let complete_bytes = bytes_for(&files) + complete_representatives.len();
        let partial_bytes = bytes_for(&files[..partial_files]) + partial_representatives.len();
        let evaluation_holdout_bytes = target_view_keys
            + target_file_views
                .values()
                .map(|view| view.complete.len() + view.partial.len())
                .sum::<usize>();
        let report = RetrievalIndexReport {
            files_considered: project.files.len(),
            files_indexed: files.len(),
            files_skipped_by_limits: skipped,
            files_excluded_as_near_duplicates: files.len().saturating_sub(
                complete_representatives
                    .iter()
                    .filter(|keep| **keep)
                    .count(),
            ),
            partial_files,
            complete_bytes,
            partial_bytes,
            evaluation_holdout_bytes,
            build_ms: started.elapsed().as_secs_f64() * 1000.0,
            table_build_ms,
            signature_build_ms,
            similarity_build_ms,
            clustering_ms,
            pairwise_similarity_checks,
            clustering_working_bytes,
            near_duplicate_clusters: cluster_reports,
        };
        (
            Self {
                files,
                complete_representatives,
                partial_representatives,
                target_file_views,
            },
            report,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn query(
        &self,
        state: IndexState,
        holdout: HoldoutMode,
        active_path: &Path,
        active_line: usize,
        line_prefix: &str,
        allow_target_file_other_lines: bool,
        scope: RetrievalScope,
    ) -> Option<ProjectRetrievalHit> {
        let representatives = self.representatives(state, holdout, active_path)?;
        let active_parent = active_path.parent();
        let mut candidates = BTreeMap::<(RetrievalTier, String), usize>::new();
        for (index, file) in self.files.iter().enumerate() {
            if !representatives[index] {
                continue;
            }
            let active = file.path == active_path;
            if active && (holdout == HoldoutMode::TargetFile || !allow_target_file_other_lines) {
                continue;
            }
            let tier = if active {
                RetrievalTier::CurrentFile
            } else if file.path.parent() == active_parent {
                RetrievalTier::SameDirectory
            } else {
                RetrievalTier::Project
            };
            if !scope.includes(tier) {
                continue;
            }
            let excluded_line = active.then_some(active_line);
            for hit in file.table.query(line_prefix, excluded_line, usize::MAX) {
                *candidates.entry((tier, hit.continuation)).or_default() += hit.support();
            }
        }
        candidates
            .into_iter()
            .min_by(
                |((left_tier, left_text), left_support),
                 ((right_tier, right_text), right_support)| {
                    left_tier
                        .cmp(right_tier)
                        .then_with(|| right_support.cmp(left_support))
                        .then_with(|| left_text.cmp(right_text))
                },
            )
            .map(|((tier, continuation), support)| ProjectRetrievalHit {
                continuation,
                support,
                tier,
            })
    }

    fn representatives(
        &self,
        state: IndexState,
        holdout: HoldoutMode,
        active_path: &Path,
    ) -> Option<&[bool]> {
        match (holdout, state) {
            (_, IndexState::Cold) => None,
            (HoldoutMode::TargetFile, IndexState::Partial) => self
                .target_file_views
                .get(active_path)
                .map_or(Some(self.partial_representatives.as_ref()), |view| {
                    Some(view.partial.as_ref())
                }),
            (HoldoutMode::TargetFile, IndexState::Complete) => self
                .target_file_views
                .get(active_path)
                .map_or(Some(self.complete_representatives.as_ref()), |view| {
                    Some(view.complete.as_ref())
                }),
            (HoldoutMode::TargetLine, IndexState::Partial) => Some(&self.partial_representatives),
            (HoldoutMode::TargetLine, IndexState::Complete) => Some(&self.complete_representatives),
        }
    }

    /// Retrieve only evidence from the rest-of-project tier and return the
    /// conservative common prefix shared by every matching continuation.
    /// Same-directory evidence is deliberately excluded so the tuning
    /// candidate can measure the tiers independently.
    #[allow(clippy::too_many_arguments)]
    pub fn query_project_common_prefix(
        &self,
        state: IndexState,
        active_path: &Path,
        line_prefix: &str,
        min_support: usize,
        min_full_prefix_support: usize,
    ) -> Option<ProjectRetrievalHit> {
        let representatives = self.representatives(state, HoldoutMode::TargetFile, active_path)?;
        let active_parent = active_path.parent();
        let mut continuations = BTreeMap::<String, usize>::new();
        for (index, file) in self.files.iter().enumerate() {
            if file.path == active_path
                || file.path.parent() == active_parent
                || !representatives[index]
            {
                continue;
            }
            for hit in file.table.query(line_prefix, None, usize::MAX) {
                *continuations.entry(hit.continuation).or_default() += hit.support();
            }
        }

        let support = continuations.values().sum::<usize>();
        if support < min_support {
            return None;
        }
        let mut values = continuations.keys();
        let first = values.next()?;
        let mut common = first.len();
        for continuation in values {
            common = first
                .char_indices()
                .zip(continuation.chars())
                .find(|((_, left), right)| left != right)
                .map_or(common.min(continuation.len()), |((index, _), _)| {
                    index.min(common)
                });
        }
        let mut prefix = &first[..common];
        if continuations.keys().any(|continuation| {
            continuation[prefix.len()..]
                .chars()
                .next()
                .is_some_and(super::super::eligibility::is_identifier_char)
        }) {
            prefix = prefix.trim_end_matches(super::super::eligibility::is_identifier_char);
        }
        let mut prefix = prefix.trim_end();
        if prefix.is_empty() {
            return None;
        }
        if support < min_full_prefix_support {
            prefix = first_token(prefix)?;
        }
        Some(ProjectRetrievalHit {
            continuation: prefix.to_owned(),
            support,
            tier: RetrievalTier::Project,
        })
    }
}

fn first_token(text: &str) -> Option<&str> {
    let start = text.find(|character: char| !character.is_whitespace())?;
    let rest = &text[start..];
    let first = rest.chars().next()?;
    let end = if super::super::eligibility::is_identifier_char(first) {
        rest.char_indices()
            .find(|(_, character)| !super::super::eligibility::is_identifier_char(*character))
            .map_or(rest.len(), |(index, _)| index)
    } else if first == '.' && rest.starts_with("..") {
        2
    } else {
        first.len_utf8()
    };
    Some(&text[..start + end])
}

fn near_duplicate_clusters(
    similarities: &SimilarityMatrix,
    available: &[usize],
) -> Vec<Vec<usize>> {
    let mut clusters = Vec::<Vec<usize>>::new();
    for &index in available {
        if let Some(cluster) = clusters.iter_mut().find(|cluster| {
            cluster
                .iter()
                .all(|member| similarities.get(*member, index))
        }) {
            cluster.push(index);
        } else {
            clusters.push(vec![index]);
        }
    }
    clusters
}

fn source_signature(source: &str) -> BTreeMap<String, usize> {
    let mut signature = BTreeMap::new();
    for line in source
        .lines()
        .map(super::super::eligibility::strip_trailing_comment)
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| metrics::tokens(line).join(" "))
        .filter(|line| !line.is_empty())
    {
        *signature.entry(line).or_default() += 1;
    }
    signature
}

fn near_duplicate(left: &BTreeMap<String, usize>, right: &BTreeMap<String, usize>) -> bool {
    let left_lines = left.values().sum::<usize>();
    let right_lines = right.values().sum::<usize>();
    if left_lines.min(right_lines) < NEAR_DUPLICATE_MIN_LINES {
        return false;
    }
    let length_ratio = left_lines.min(right_lines) as f64 / left_lines.max(right_lines) as f64;
    if length_ratio < 0.8 {
        return false;
    }
    let intersection = left
        .iter()
        .map(|(line, count)| count.min(right.get(line).unwrap_or(&0)))
        .sum::<usize>();
    let union = left_lines + right_lines - intersection;
    intersection as f64 / union.max(1) as f64 >= NEAR_DUPLICATE_SIMILARITY
}

fn representative_mask(file_count: usize, clusters: &[Vec<usize>]) -> Box<[bool]> {
    let mut mask = vec![false; file_count];
    for cluster in clusters {
        for representative in representative_indices(cluster) {
            mask[representative] = true;
        }
    }
    mask.into_boxed_slice()
}

fn signature_bytes(signature: &BTreeMap<String, usize>) -> usize {
    signature
        .keys()
        .map(|line| size_of::<(String, usize)>() + line.capacity())
        .sum()
}

fn representative_indices(cluster: &[usize]) -> Vec<usize> {
    if cluster.len() <= 3 {
        return cluster.to_vec();
    }
    let mut representatives = vec![
        cluster[0],
        cluster[cluster.len() / 2],
        cluster[cluster.len() - 1],
    ];
    representatives.sort_unstable();
    representatives.dedup();
    representatives
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AnalysisSourceKind, ProjectConfig};

    fn project(files: &[(&str, &str)]) -> EvalProject {
        EvalProject {
            name: "fixture".to_owned(),
            config: ProjectConfig::default(),
            source_roots: vec![],
            output_roots: vec![],
            asset_roots: vec![],
            files: files
                .iter()
                .map(|(path, source)| super::super::dataset::EvalFile {
                    path: PathBuf::from(path),
                    kind: AnalysisSourceKind::Tua,
                    source: (*source).to_owned(),
                })
                .collect(),
        }
    }

    #[test]
    fn target_file_and_target_line_holdouts_do_not_leak() {
        let project = project(&[
            ("src/main.tua", "draw(player)\ndraw(pla"),
            ("src/other.tua", "draw(player)"),
        ]);
        let (index, _) = EvalProjectRetrievalIndex::build(&project);
        let target_file = index
            .query(
                IndexState::Complete,
                HoldoutMode::TargetFile,
                Path::new("src/main.tua"),
                1,
                "draw(pla",
                true,
                RetrievalScope::Combined,
            )
            .unwrap();
        assert_eq!(target_file.continuation, "yer)");
        assert_eq!(target_file.tier, RetrievalTier::SameDirectory);

        let target_line = index
            .query(
                IndexState::Complete,
                HoldoutMode::TargetLine,
                Path::new("src/main.tua"),
                1,
                "draw(pla",
                true,
                RetrievalScope::Combined,
            )
            .unwrap();
        assert_eq!(target_line.tier, RetrievalTier::CurrentFile);
        assert_eq!(target_line.continuation, "yer)");
        assert!(index
            .query(
                IndexState::Complete,
                HoldoutMode::TargetLine,
                Path::new("src/main.tua"),
                1,
                "draw(pla",
                false,
                RetrievalScope::Combined,
            )
            .is_some_and(|hit| hit.tier != RetrievalTier::CurrentFile));
    }

    #[test]
    fn cold_partial_and_complete_states_are_distinct_and_deterministic() {
        let project = project(&[
            ("a/one.tua", "draw(alpha)"),
            ("b/two.tua", "draw(beta)"),
            ("c/three.tua", "draw(gamma)"),
        ]);
        let (index, report) = EvalProjectRetrievalIndex::build(&project);
        assert_eq!(report.partial_files, 2);
        assert!(index
            .query(
                IndexState::Cold,
                HoldoutMode::TargetFile,
                Path::new("target.tua"),
                0,
                "draw(",
                true,
                RetrievalScope::Combined,
            )
            .is_none());
        assert_eq!(
            index
                .query(
                    IndexState::Partial,
                    HoldoutMode::TargetFile,
                    Path::new("target.tua"),
                    0,
                    "draw(",
                    true,
                    RetrievalScope::Combined,
                )
                .unwrap()
                .continuation,
            "alpha)"
        );
        assert!(report.partial_bytes < report.complete_bytes);
    }

    #[test]
    fn clusters_near_duplicates_and_keeps_checkpoints() {
        let common = (0..20)
            .map(|index| format!("value{index} = {index}"))
            .collect::<Vec<_>>();
        let files = (0..5)
            .map(|index| {
                let mut lines = common.clone();
                lines.push(format!("step = {index}"));
                (format!("step{index}.tua"), lines.join("\n"))
            })
            .collect::<Vec<_>>();
        let borrowed = files
            .iter()
            .map(|(path, source)| (path.as_str(), source.as_str()))
            .collect::<Vec<_>>();
        let project = project(&borrowed);
        let (_, report) = EvalProjectRetrievalIndex::build(&project);
        assert_eq!(report.near_duplicate_clusters.len(), 1);
        assert_eq!(report.near_duplicate_clusters[0].members.len(), 5);
        assert_eq!(report.near_duplicate_clusters[0].representatives.len(), 3);
        assert_eq!(report.files_indexed, 5);
        assert_eq!(report.files_excluded_as_near_duplicates, 2);
        assert!(report.table_build_ms >= 0.0);
        assert!(report.clustering_ms >= 0.0);
    }

    #[test]
    fn clustering_uses_multisets_and_does_not_chain_transitively() {
        let repeated_left = source_signature(&format!("{}\ny", "x\n".repeat(9)));
        let repeated_right = source_signature(&format!("x\n{}", "y\n".repeat(9)));
        assert!(!near_duplicate(&repeated_left, &repeated_right));

        let lines = |indices: &[usize]| {
            indices
                .iter()
                .map(|index| format!("value_{index}"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let a = (0..100).collect::<Vec<_>>();
        let mut b = (0..95).collect::<Vec<_>>();
        b.extend(100..105);
        let mut c = (0..90).collect::<Vec<_>>();
        c.extend(100..110);
        let project = project(&[
            ("a.tua", &lines(&a)),
            ("b.tua", &lines(&b)),
            ("c.tua", &lines(&c)),
        ]);
        let (_, report) = EvalProjectRetrievalIndex::build(&project);
        let sizes = report
            .near_duplicate_clusters
            .iter()
            .map(|cluster| cluster.members.len())
            .collect::<Vec<_>>();
        assert_eq!(sizes, vec![2]);
    }

    #[test]
    fn target_file_is_removed_before_clustering_and_partial_selection() {
        let common = (0..20)
            .map(|index| format!("value{index} = {index}"))
            .collect::<Vec<_>>();
        let source = |suffix: &str| {
            let mut lines = common.clone();
            lines.push(format!("draw({suffix})"));
            lines.join("\n")
        };
        let with_target = project(&[
            ("a/target.tua", &source("target")),
            ("b/one.tua", &source("one")),
            ("c/two.tua", &source("two")),
            ("d/three.tua", &source("three")),
            ("e/four.tua", &source("four")),
        ]);
        let without_target = project(&[
            ("b/one.tua", &source("one")),
            ("c/two.tua", &source("two")),
            ("d/three.tua", &source("three")),
            ("e/four.tua", &source("four")),
        ]);
        let (with_target, _) = EvalProjectRetrievalIndex::build(&with_target);
        let (without_target, _) = EvalProjectRetrievalIndex::build(&without_target);
        for state in [IndexState::Partial, IndexState::Complete] {
            let left = with_target.query(
                state,
                HoldoutMode::TargetFile,
                Path::new("a/target.tua"),
                20,
                "draw(",
                true,
                RetrievalScope::Project,
            );
            let right = without_target.query(
                state,
                HoldoutMode::TargetFile,
                Path::new("a/target.tua"),
                20,
                "draw(",
                true,
                RetrievalScope::Project,
            );
            assert_eq!(left, right);
        }
    }

    #[test]
    fn global_aggregation_sees_matches_beyond_each_files_first_sixteen() {
        let first = (0..16)
            .map(|index| format!("draw(a{index:02})"))
            .chain(std::iter::once("draw(shared)".to_owned()))
            .collect::<Vec<_>>()
            .join("\n");
        let project = project(&[("a/one.tua", &first), ("b/two.tua", "draw(shared)")]);
        let (index, _) = EvalProjectRetrievalIndex::build(&project);
        let hit = index
            .query(
                IndexState::Complete,
                HoldoutMode::TargetFile,
                Path::new("target.tua"),
                0,
                "draw(",
                true,
                RetrievalScope::Project,
            )
            .unwrap();
        assert_eq!(hit.continuation, "shared)");
        assert_eq!(hit.support, 2);
    }

    #[test]
    fn project_common_prefix_requires_support_and_excludes_same_directory() {
        let project = project(&[
            ("active/main.tua", "call(local)"),
            ("active/sibling.tua", "call(wrong)"),
            ("one/a.tua", "call(player.x)"),
            ("two/b.tua", "call(player.y)"),
        ]);
        let (index, _) = EvalProjectRetrievalIndex::build(&project);
        let hit = index
            .query_project_common_prefix(
                IndexState::Complete,
                Path::new("active/main.tua"),
                "call(pla",
                2,
                2,
            )
            .unwrap();
        assert_eq!(hit.continuation, "yer.");
        assert_eq!(hit.support, 2);
        assert_eq!(hit.tier, RetrievalTier::Project);
        assert!(index
            .query_project_common_prefix(
                IndexState::Complete,
                Path::new("active/main.tua"),
                "call(wr",
                2,
                2,
            )
            .is_none());
    }

    #[test]
    fn five_hundred_files_compute_each_expensive_similarity_only_once() {
        const FILES: usize = 500;
        let source = (0..8)
            .map(|index| format!("value_{index} = {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let project = EvalProject {
            name: "large-synthetic".to_owned(),
            config: ProjectConfig::default(),
            source_roots: vec![],
            output_roots: vec![],
            asset_roots: vec![],
            files: (0..FILES)
                .map(|index| super::super::dataset::EvalFile {
                    path: PathBuf::from(format!("steps/{index:03}.tua")),
                    kind: AnalysisSourceKind::Tua,
                    source: source.clone(),
                })
                .collect(),
        };
        let (_, report) = EvalProjectRetrievalIndex::build(&project);
        assert_eq!(report.pairwise_similarity_checks, FILES * (FILES - 1) / 2);
        assert!(report.signature_build_ms >= 0.0);
        assert!(report.similarity_build_ms >= 0.0);
        assert!(report.clustering_working_bytes > 0);
    }
}

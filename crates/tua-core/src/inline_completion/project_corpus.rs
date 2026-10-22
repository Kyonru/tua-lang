//! Immutable project-wide exact retrieval.
//!
//! Workers build [`PatternCorpusBuildOutcome`] values from captured VFS input.
//! A coordinator validates and publishes them through `AnalysisHost`; requests
//! only read the resulting immutable corpus and the live VFS versions.

use std::collections::{BTreeMap, BTreeSet};
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::module_link::{
    module_specifier_identities, module_specifier_literals, resolve_module_specifier_literals,
    ModuleSpecifierIdentity,
};
use crate::project_model::ProjectModel;
use crate::vfs::{normalize_path, VfsLineageId, VfsSourceVersion};
use crate::{parse_lua_source, parse_tua_source, AnalysisSourceKind};

use super::eligibility::strip_trailing_comment;
use super::recall::{continuation_after, normalize};
use super::retrieval::{FilePatternTable, RetrievalBuildError, RetrievalLimits};
use super::statistical::{
    self, StatisticalEvidence, StatisticalFileTable, StatisticalPrediction, StatisticalScopeFacts,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CorpusFileId(u32);

impl CorpusFileId {
    pub fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PatternCorpusTier {
    CurrentFile,
    RequiredModule,
    SameDirectory,
    Project,
}

impl PatternCorpusTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CurrentFile => "currentFile",
            Self::RequiredModule => "requiredModule",
            Self::SameDirectory => "sameDirectory",
            Self::Project => "project",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatternCorpusMatch {
    pub continuation: String,
    pub tier: PatternCorpusTier,
    pub file_id: CorpusFileId,
    pub source_path: PathBuf,
    pub source_kind: AnalysisSourceKind,
    pub support: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PatternCorpusLimits {
    pub max_bytes: usize,
    pub max_active_source_bytes: usize,
    pub max_query_patterns: usize,
    pub max_query_prefix_bytes: usize,
    pub max_related_targets: usize,
    pub max_callback_support_checks: usize,
    pub rank_same_directory: bool,
    /// Hard bound on coordinator-side copy-on-write metadata maps.
    pub max_files: usize,
    pub remerge_overlay_files: usize,
    pub remerge_overlay_bytes: usize,
}

impl Default for PatternCorpusLimits {
    fn default() -> Self {
        Self {
            max_bytes: 16 * 1024 * 1024,
            max_active_source_bytes: 256 * 1024,
            max_query_patterns: 4096,
            max_query_prefix_bytes: 16 * 1024,
            max_related_targets: 256,
            max_callback_support_checks: 512,
            rank_same_directory: false,
            max_files: 1024,
            remerge_overlay_files: 32,
            remerge_overlay_bytes: 2 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PatternCorpusBuildInput {
    path: PathBuf,
    source: String,
    kind: AnalysisSourceKind,
    version: VfsSourceVersion,
    include_exact: bool,
    include_statistical: bool,
}

impl PatternCorpusBuildInput {
    pub(crate) fn new(
        path: PathBuf,
        source: String,
        kind: AnalysisSourceKind,
        version: VfsSourceVersion,
        include_exact: bool,
        include_statistical: bool,
    ) -> Self {
        Self {
            path,
            source,
            kind,
            version,
            include_exact,
            include_statistical,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn version(&self) -> VfsSourceVersion {
        self.version
    }

    pub fn source_bytes(&self) -> usize {
        self.source.len()
    }

    pub fn source_kind(&self) -> AnalysisSourceKind {
        self.kind
    }

    pub fn build(
        self,
        project_model: &ProjectModel,
        limits: RetrievalLimits,
    ) -> PatternCorpusBuildOutcome {
        let table = if self.include_exact {
            match FilePatternTable::build(&self.source, limits) {
                Ok(table) => Some(Arc::new(table)),
                Err(error) => {
                    return PatternCorpusBuildOutcome::Skipped {
                        path: self.path,
                        version: self.version,
                        reason: error,
                    };
                }
            }
        } else {
            None
        };
        let literals = module_specifier_literals(&self.source);
        let resolved = resolve_module_specifier_literals(&self.path, &literals, project_model);
        let resolved_targets = resolved
            .links
            .into_iter()
            .map(|link| normalize_path(link.target))
            .collect::<BTreeSet<_>>();
        let build_statistical = self.include_statistical
            && self.kind == AnalysisSourceKind::Tua
            && self.source.len() <= statistical::MAX_SOURCE_BYTES;
        let parsed = match self.kind {
            AnalysisSourceKind::Tua => parse_tua_source(&self.path, self.source),
            AnalysisSourceKind::Lua => parse_lua_source(&self.path, self.source),
        };
        let statistical = build_statistical
            .then(|| {
                let parsed = parsed.as_ref().ok()?;
                let scope = crate::scope_map_parsed_source(parsed);
                StatisticalFileTable::build(parsed, &scope).map(Arc::new)
            })
            .flatten();
        let declared_love_callbacks = if self.include_exact {
            parsed
                .as_ref()
                .ok()
                .map(crate::love_callback::declared_love_callbacks)
                .into_iter()
                .flatten()
                .map(str::to_owned)
                .collect::<BTreeSet<_>>()
        } else {
            BTreeSet::new()
        };

        PatternCorpusBuildOutcome::Built(PatternFileBuild {
            path: self.path,
            version: self.version,
            kind: self.kind,
            table,
            specifier_identities: resolved.identities,
            resolved_targets,
            declared_love_callbacks,
            statistical,
        })
    }
}

#[derive(Clone, Debug)]
pub enum PatternCorpusBuildOutcome {
    Built(PatternFileBuild),
    Skipped {
        path: PathBuf,
        version: VfsSourceVersion,
        reason: RetrievalBuildError,
    },
}

impl PatternCorpusBuildOutcome {
    pub fn path(&self) -> &Path {
        match self {
            Self::Built(build) => &build.path,
            Self::Skipped { path, .. } => path,
        }
    }

    pub fn version(&self) -> VfsSourceVersion {
        match self {
            Self::Built(build) => build.version,
            Self::Skipped { version, .. } => *version,
        }
    }

    #[cfg(test)]
    pub(crate) fn add_required_target_for_test(&mut self, path: impl Into<PathBuf>) {
        let Self::Built(build) = self else {
            panic!("project-pattern fixture should build")
        };
        build.resolved_targets.insert(normalize_path(path.into()));
    }
}

#[derive(Clone, Debug)]
pub struct PatternFileBuild {
    path: PathBuf,
    version: VfsSourceVersion,
    kind: AnalysisSourceKind,
    table: Option<Arc<FilePatternTable>>,
    specifier_identities: Vec<ModuleSpecifierIdentity>,
    resolved_targets: BTreeSet<PathBuf>,
    declared_love_callbacks: BTreeSet<String>,
    statistical: Option<Arc<StatisticalFileTable>>,
}

#[derive(Clone, Debug)]
pub enum PatternCorpusBatchEntry {
    File(PatternCorpusBuildOutcome),
    Tombstone(PatternCorpusTombstone),
}

#[derive(Clone, Debug)]
pub struct PatternCorpusTombstone {
    pub(crate) path: PathBuf,
    pub(crate) expected_version: Option<VfsSourceVersion>,
    pub(crate) lineage: VfsLineageId,
}

impl PatternCorpusTombstone {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CorpusAdoptionResult {
    pub adopted: usize,
    pub tombstoned: usize,
    pub stale: usize,
    pub skipped: usize,
    pub over_budget: usize,
    pub file_limit: usize,
    pub generation_rejected: bool,
    pub lineage_rejected: bool,
    pub needs_remerge: bool,
    pub entries: Vec<CorpusEntryAdoption>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorpusEntryAdoption {
    pub path: PathBuf,
    pub status: CorpusEntryAdoptionStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorpusEntryAdoptionStatus {
    Adopted,
    Tombstoned,
    Skipped,
    Stale,
    GenerationMismatch,
    WrongLineage,
    OverBudget,
    FileLimit,
}

pub(crate) enum ValidatedCorpusMutation {
    File(PatternFileBuild),
    Tombstone { path: PathBuf, skipped: bool },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PatternCorpusStats {
    pub eligibility_generation: u64,
    pub files: usize,
    pub base_files: usize,
    pub overlay_files: usize,
    pub tombstones: usize,
    pub approximate_bytes: usize,
}

#[derive(Clone, Debug)]
struct CorpusFile {
    path: PathBuf,
    version: VfsSourceVersion,
    kind: AnalysisSourceKind,
    specifier_identities: Vec<ModuleSpecifierIdentity>,
    resolved_targets: BTreeSet<PathBuf>,
    declared_love_callbacks: BTreeSet<String>,
    statistical: Option<Arc<StatisticalFileTable>>,
}

#[derive(Clone, Debug)]
enum OverlayEntry {
    Table(Arc<FilePatternTable>),
    Metadata,
    Tombstone,
}

#[derive(Clone, Debug)]
struct BasePattern {
    normalized: Box<str>,
    original: Box<str>,
    file_id: CorpusFileId,
    version: VfsSourceVersion,
    line_indices: Box<[usize]>,
    max_continuation_bytes: usize,
}

#[derive(Clone, Debug)]
struct PatternBase {
    file_ids: BTreeSet<CorpusFileId>,
    patterns: Box<[BasePattern]>,
    approximate_bytes: usize,
}

impl Default for PatternBase {
    fn default() -> Self {
        Self {
            file_ids: BTreeSet::new(),
            patterns: Box::new([]),
            approximate_bytes: size_of::<Self>(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProjectPatternCorpus {
    base: Arc<PatternBase>,
    overlay: Arc<BTreeMap<CorpusFileId, OverlayEntry>>,
    files: Arc<BTreeMap<CorpusFileId, CorpusFile>>,
    path_ids: Arc<BTreeMap<PathBuf, CorpusFileId>>,
    callback_support: Arc<BTreeMap<String, BTreeSet<CorpusFileId>>>,
    eligibility_generation: u64,
    lineage: Option<VfsLineageId>,
    approximate_bytes: usize,
}

pub(crate) struct PatternCorpusQuery<'a> {
    pub generation: u64,
    pub active_path: &'a Path,
    pub active_source: &'a str,
    pub prefix: &'a str,
    pub active_line: usize,
    pub max_results: usize,
    pub max_active_source_bytes: usize,
    pub max_query_patterns: usize,
    pub max_query_prefix_bytes: usize,
    pub max_related_targets: usize,
    pub rank_same_directory: bool,
    pub rest_of_project_only: bool,
    pub budget: super::InlineCompletionBudget,
    pub cancelled: &'a AtomicBool,
}

pub(crate) struct ProjectCommonPrefixMatch {
    pub continuation: String,
    pub support: usize,
    pub tier: PatternCorpusTier,
}

#[derive(Clone, Debug)]
pub struct PreparedPatternCorpusRemerge {
    source: Arc<ProjectPatternCorpus>,
    generation: u64,
    lineage: VfsLineageId,
}

impl PreparedPatternCorpusRemerge {
    pub fn build(self) -> PatternCorpusRemergeOutcome {
        // Keep preparation O(1): this potentially corpus-wide walk belongs
        // on the background worker together with construction of the base.
        let versions = self
            .source
            .files
            .values()
            .map(|file| (file.path.clone(), file.version))
            .collect();
        let merged = self.source.build_remerged();
        PatternCorpusRemergeOutcome {
            source: self.source,
            merged,
            generation: self.generation,
            lineage: self.lineage,
            versions,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PatternCorpusRemergeOutcome {
    source: Arc<ProjectPatternCorpus>,
    merged: ProjectPatternCorpus,
    generation: u64,
    lineage: VfsLineageId,
    versions: Vec<(PathBuf, VfsSourceVersion)>,
}

/// Ownership token for corpus storage that should be released off the LSP
/// coordinator thread. Dropping this value may free the full corpus cap.
#[derive(Debug)]
pub struct RetiredProjectPatternCorpus(pub(crate) Arc<ProjectPatternCorpus>);

impl RetiredProjectPatternCorpus {
    pub fn approximate_bytes(&self) -> usize {
        self.0.approximate_bytes
    }
}

impl Default for ProjectPatternCorpus {
    fn default() -> Self {
        Self::empty(0, None)
    }
}

impl ProjectPatternCorpus {
    pub fn empty(eligibility_generation: u64, lineage: Option<VfsLineageId>) -> Self {
        let mut corpus = Self {
            base: Arc::new(PatternBase::default()),
            overlay: Arc::new(BTreeMap::new()),
            files: Arc::new(BTreeMap::new()),
            path_ids: Arc::new(BTreeMap::new()),
            callback_support: Arc::new(BTreeMap::new()),
            eligibility_generation,
            lineage,
            approximate_bytes: 0,
        };
        corpus.approximate_bytes = corpus.compute_approximate_bytes();
        corpus
    }

    pub fn eligibility_generation(&self) -> u64 {
        self.eligibility_generation
    }

    pub(crate) fn lineage(&self) -> Option<VfsLineageId> {
        self.lineage
    }

    pub(crate) fn prepare_remerge(
        source: Arc<Self>,
        lineage: VfsLineageId,
    ) -> Option<PreparedPatternCorpusRemerge> {
        if source.lineage != Some(lineage) || source.overlay.is_empty() {
            return None;
        }
        Some(PreparedPatternCorpusRemerge {
            generation: source.eligibility_generation,
            lineage,
            source,
        })
    }

    pub fn stats(&self) -> PatternCorpusStats {
        PatternCorpusStats {
            eligibility_generation: self.eligibility_generation,
            files: self.files.len(),
            base_files: self.base.file_ids.len(),
            overlay_files: self.overlay.len(),
            tombstones: self
                .overlay
                .values()
                .filter(|entry| matches!(entry, OverlayEntry::Tombstone))
                .count(),
            approximate_bytes: self.approximate_bytes,
        }
    }

    pub fn needs_remerge(&self, limits: PatternCorpusLimits) -> bool {
        self.overlay.len() >= limits.remerge_overlay_files
            || self.overlay_bytes() >= limits.remerge_overlay_bytes
    }

    pub(crate) fn with_batch(
        &self,
        mutations: Vec<ValidatedCorpusMutation>,
        limits: PatternCorpusLimits,
    ) -> (Self, Vec<CorpusEntryAdoption>) {
        let mut files = (*self.files).clone();
        let mut path_ids = (*self.path_ids).clone();
        let mut overlay = (*self.overlay).clone();
        let mut callback_support = (*self.callback_support).clone();
        let mut approximate_bytes = self.approximate_bytes;
        let mut results = Vec::with_capacity(mutations.len());
        let mut next_id = path_ids
            .values()
            .map(|id| id.0)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let mut tombstones = Vec::new();
        let mut builds = Vec::new();
        for mutation in mutations {
            match mutation {
                ValidatedCorpusMutation::Tombstone { path, skipped } => {
                    tombstones.push((normalize_path(path), skipped));
                }
                ValidatedCorpusMutation::File(build) => builds.push(build),
            }
        }
        tombstones.sort_by(|left, right| left.0.cmp(&right.0));
        builds.sort_by(|left, right| left.path.cmp(&right.path));

        for (path, skipped) in tombstones {
            let Some(id) = path_ids.get(&path).copied() else {
                results.push(CorpusEntryAdoption {
                    path,
                    status: if skipped {
                        CorpusEntryAdoptionStatus::Skipped
                    } else {
                        CorpusEntryAdoptionStatus::Tombstoned
                    },
                });
                continue;
            };
            let projected = approximate_bytes
                .saturating_sub(files.get(&id).map_or(0, file_map_entry_bytes))
                .saturating_sub(overlay.get(&id).map_or(0, overlay_map_entry_bytes))
                .saturating_add(overlay_map_entry_bytes(&OverlayEntry::Tombstone));
            if projected > limits.max_bytes {
                results.push(CorpusEntryAdoption {
                    path,
                    status: CorpusEntryAdoptionStatus::OverBudget,
                });
                continue;
            }
            approximate_bytes = projected;
            if let Some(old) = files.remove(&id) {
                remove_callback_support(&mut callback_support, id, &old.declared_love_callbacks);
            }
            overlay.insert(id, OverlayEntry::Tombstone);
            results.push(CorpusEntryAdoption {
                path,
                status: if skipped {
                    CorpusEntryAdoptionStatus::Skipped
                } else {
                    CorpusEntryAdoptionStatus::Tombstoned
                },
            });
        }

        for build in builds {
            let path = build.path.clone();
            let existing_id = path_ids.get(&path).copied();
            if existing_id.is_none() && files.len() >= limits.max_files {
                results.push(CorpusEntryAdoption {
                    path,
                    status: CorpusEntryAdoptionStatus::FileLimit,
                });
                continue;
            }
            let id = existing_id.unwrap_or(CorpusFileId(next_id));
            let table = build.table.as_ref().map(Arc::clone);
            let file = build.into_file();
            let overlay_entry = table
                .map(OverlayEntry::Table)
                .unwrap_or(OverlayEntry::Metadata);
            let projected = approximate_bytes
                .saturating_sub(files.get(&id).map_or(0, file_map_entry_bytes))
                .saturating_sub(overlay.get(&id).map_or(0, overlay_map_entry_bytes))
                .saturating_add(file_map_entry_bytes(&file))
                .saturating_add(overlay_map_entry_bytes(&overlay_entry))
                .saturating_add(if existing_id.is_none() {
                    path_map_entry_bytes(&path)
                } else {
                    0
                });
            if projected > limits.max_bytes {
                results.push(CorpusEntryAdoption {
                    path,
                    status: CorpusEntryAdoptionStatus::OverBudget,
                });
                continue;
            }
            if existing_id.is_none() {
                next_id = next_id.saturating_add(1);
                path_ids.insert(path.clone(), id);
            }
            approximate_bytes = projected;
            if let Some(old) = files.get(&id) {
                remove_callback_support(&mut callback_support, id, &old.declared_love_callbacks);
            }
            add_callback_support(&mut callback_support, id, &file.declared_love_callbacks);
            files.insert(id, file);
            overlay.insert(id, overlay_entry);
            results.push(CorpusEntryAdoption {
                path,
                status: CorpusEntryAdoptionStatus::Adopted,
            });
        }
        let next = Self {
            base: Arc::clone(&self.base),
            overlay: Arc::new(overlay),
            files: Arc::new(files),
            path_ids: Arc::new(path_ids),
            callback_support: Arc::new(callback_support),
            eligibility_generation: self.eligibility_generation,
            lineage: self.lineage,
            approximate_bytes,
        };
        debug_assert_eq!(next.approximate_bytes, next.compute_approximate_bytes());
        (next, results)
    }

    fn build_remerged(&self) -> Self {
        let base = PatternBase::merged(&self.base, &self.overlay, &self.files);
        let path_ids = self
            .files
            .iter()
            .map(|(id, file)| (file.path.clone(), *id))
            .collect::<BTreeMap<_, _>>();
        let mut next = Self {
            base: Arc::new(base),
            overlay: Arc::new(BTreeMap::new()),
            files: Arc::clone(&self.files),
            path_ids: Arc::new(path_ids),
            callback_support: Arc::clone(&self.callback_support),
            eligibility_generation: self.eligibility_generation,
            lineage: self.lineage,
            approximate_bytes: 0,
        };
        next.approximate_bytes = next.compute_approximate_bytes();
        next
    }

    pub(crate) fn query(
        &self,
        request: PatternCorpusQuery<'_>,
        current_version: impl Fn(&Path) -> Option<VfsSourceVersion>,
    ) -> Vec<PatternCorpusMatch> {
        if request.generation != self.eligibility_generation
            || request.max_results == 0
            || request.max_query_patterns == 0
            || request.prefix.len() > request.max_query_prefix_bytes
            || request.cancelled.load(Ordering::Relaxed)
            || request.budget.is_expired()
        {
            return Vec::new();
        }
        let active_path = normalize_path(request.active_path);
        let active_version = current_version(&active_path);
        if self.lineage.is_none() || active_version.map(VfsSourceVersion::lineage) != self.lineage {
            return Vec::new();
        }
        let active_id = self.path_ids.get(&active_path).copied();
        let required_targets = if request.active_source.len() <= request.max_active_source_bytes {
            let Some(targets) = self.related_targets(
                active_id,
                request.active_source,
                active_version,
                request.max_related_targets,
                request.budget,
                request.cancelled,
            ) else {
                return Vec::new();
            };
            targets
        } else {
            BTreeSet::new()
        };
        if request.cancelled.load(Ordering::Relaxed) || request.budget.is_expired() {
            return Vec::new();
        }
        let mut grouped = BTreeMap::<(PatternCorpusTier, String), AggregatedMatch>::new();
        let mut validity = BTreeMap::<CorpusFileId, bool>::new();
        if let Some(id) = active_id {
            validity.insert(
                id,
                self.files
                    .get(&id)
                    .is_some_and(|file| active_version == Some(file.version)),
            );
        }
        let mut scanned = 0usize;

        let (normalized_prefix, _) = normalize(request.prefix.trim_start());
        if request.cancelled.load(Ordering::Relaxed) || request.budget.is_expired() {
            return Vec::new();
        }
        if normalized_prefix.trim().is_empty() {
            return Vec::new();
        }
        let start = self
            .base
            .patterns
            .partition_point(|pattern| pattern.normalized.as_ref() < normalized_prefix.as_str());
        for pattern in &self.base.patterns[start..] {
            if !pattern.normalized.starts_with(&normalized_prefix) {
                break;
            }
            scanned += 1;
            if scanned > request.max_query_patterns
                || (scanned.is_multiple_of(32)
                    && (request.cancelled.load(Ordering::Relaxed) || request.budget.is_expired()))
            {
                return Vec::new();
            }
            if self.overlay.contains_key(&pattern.file_id) {
                continue;
            }
            let Some(file) = self.files.get(&pattern.file_id) else {
                continue;
            };
            let tier = tier_for(
                &file.path,
                &active_path,
                &required_targets,
                request.rank_same_directory,
            );
            if request.rest_of_project_only
                && matches!(
                    tier,
                    PatternCorpusTier::CurrentFile | PatternCorpusTier::SameDirectory
                )
            {
                continue;
            }
            let valid = *validity.entry(pattern.file_id).or_insert_with(|| {
                current_version(&file.path) == Some(file.version)
                    && file.version.lineage() == self.lineage.expect("checked lineage")
            });
            if !valid || file.version != pattern.version {
                continue;
            }
            let lines = pattern
                .line_indices
                .iter()
                .filter(|line| Some(pattern.file_id) != active_id || **line != request.active_line)
                .count();
            if lines == 0 {
                continue;
            }
            let code = strip_trailing_comment(&pattern.original);
            let Some(continuation) = continuation_after(code, &normalized_prefix) else {
                continue;
            };
            if continuation.is_empty() || continuation.len() > pattern.max_continuation_bytes {
                continue;
            }
            aggregate_match(
                &mut grouped,
                tier,
                continuation,
                pattern.file_id,
                &file.path,
                file.kind,
                lines,
            );
        }

        for (id, entry) in self.overlay.iter() {
            let OverlayEntry::Table(table) = entry else {
                continue;
            };
            let Some(file) = self.files.get(id) else {
                continue;
            };
            let tier = tier_for(
                &file.path,
                &active_path,
                &required_targets,
                request.rank_same_directory,
            );
            if request.rest_of_project_only
                && matches!(
                    tier,
                    PatternCorpusTier::CurrentFile | PatternCorpusTier::SameDirectory
                )
            {
                continue;
            }
            let valid = *validity.entry(*id).or_insert_with(|| {
                current_version(&file.path) == Some(file.version)
                    && file.version.lineage() == self.lineage.expect("checked lineage")
            });
            if !valid {
                continue;
            }
            let start = table.prefix_start(&normalized_prefix);
            for pattern in table.entries().skip(start) {
                if !pattern.normalized.starts_with(&normalized_prefix) {
                    break;
                }
                scanned += 1;
                if scanned > request.max_query_patterns
                    || (scanned.is_multiple_of(32)
                        && (request.cancelled.load(Ordering::Relaxed)
                            || request.budget.is_expired()))
                {
                    return Vec::new();
                }
                let supporting_lines = pattern
                    .line_indices
                    .iter()
                    .filter(|line| Some(*id) != active_id || **line != request.active_line)
                    .count();
                if supporting_lines == 0 {
                    continue;
                }
                let code = strip_trailing_comment(pattern.original);
                let Some(continuation) = continuation_after(code, &normalized_prefix) else {
                    continue;
                };
                if continuation.is_empty() || continuation.len() > table.max_continuation_bytes() {
                    continue;
                }
                aggregate_match(
                    &mut grouped,
                    tier,
                    continuation,
                    *id,
                    &file.path,
                    file.kind,
                    supporting_lines,
                );
            }
        }

        if request.cancelled.load(Ordering::Relaxed) || request.budget.is_expired() {
            return Vec::new();
        }

        let mut matches = grouped
            .into_iter()
            .map(|((tier, continuation), hit)| PatternCorpusMatch {
                continuation,
                tier,
                file_id: hit.file_id,
                source_path: hit.source_path,
                source_kind: hit.source_kind,
                support: hit.support,
            })
            .collect::<Vec<_>>();
        if request.cancelled.load(Ordering::Relaxed) || request.budget.is_expired() {
            return Vec::new();
        }
        matches.sort_by(|left, right| {
            left.tier
                .cmp(&right.tier)
                .then(right.support.cmp(&left.support))
                .then(left.continuation.cmp(&right.continuation))
        });
        if request.cancelled.load(Ordering::Relaxed) || request.budget.is_expired() {
            return Vec::new();
        }
        matches.truncate(request.max_results);
        matches
    }

    pub(crate) fn query_required_module_common_prefix(
        &self,
        mut request: PatternCorpusQuery<'_>,
        min_support: usize,
        min_full_prefix_support: usize,
        current_version: impl Fn(&Path) -> Option<VfsSourceVersion>,
    ) -> Option<ProjectCommonPrefixMatch> {
        // The measured policy holds out the active file and accepts only
        // directly required-module evidence. Same-directory and unrelated
        // project matches remain available to evaluator baselines, but are
        // too imprecise to enter the production candidate.
        request.rank_same_directory = true;
        request.rest_of_project_only = true;
        request.max_results = request.max_query_patterns;
        let budget = request.budget;
        let cancelled = request.cancelled;
        let mut by_tier = BTreeMap::<PatternCorpusTier, BTreeMap<String, usize>>::new();
        for (index, hit) in self.query(request, current_version).into_iter().enumerate() {
            if index.is_multiple_of(32)
                && (cancelled.load(Ordering::Relaxed) || budget.is_expired())
            {
                return None;
            }
            *by_tier
                .entry(hit.tier)
                .or_default()
                .entry(hit.continuation)
                .or_default() += hit.support;
        }
        // Evidence from a directly required module is more closely related to
        // the active file than an arbitrary project match. Never let a lower
        // tier shorten or veto a higher-tier continuation.
        let tier = PatternCorpusTier::RequiredModule;
        let continuations = by_tier.remove(&tier)?;
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
                .is_some_and(super::eligibility::is_identifier_char)
        }) {
            prefix = prefix.trim_end_matches(super::eligibility::is_identifier_char);
        }
        let mut prefix = prefix.trim_end();
        if prefix.is_empty() || cancelled.load(Ordering::Relaxed) || budget.is_expired() {
            return None;
        }
        if support < min_full_prefix_support {
            prefix = super::recall::first_token(prefix)?;
        }
        Some(ProjectCommonPrefixMatch {
            continuation: prefix.to_owned(),
            support,
            tier,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn query_statistical(
        &self,
        generation: u64,
        active_path: &Path,
        active_source: &str,
        offset: usize,
        scope: &StatisticalScopeFacts,
        budget: super::InlineCompletionBudget,
        cancelled: &AtomicBool,
        current_version: impl Fn(&Path) -> Option<VfsSourceVersion>,
    ) -> Option<StatisticalPrediction> {
        if generation != self.eligibility_generation
            || self.lineage.is_none()
            || cancelled.load(Ordering::Relaxed)
            || budget.is_expired()
        {
            return None;
        }
        let active_path = normalize_path(active_path);
        let active_version = current_version(&active_path)?;
        if Some(active_version.lineage()) != self.lineage {
            return None;
        }
        let active_id = self.path_ids.get(&active_path).copied();
        let required_targets = self.related_targets(
            active_id,
            active_source,
            Some(active_version),
            256,
            budget,
            cancelled,
        )?;
        let mut bytes = 0usize;
        let mut evidence = Vec::new();
        'tiers: for wanted_tier in [
            PatternCorpusTier::RequiredModule,
            PatternCorpusTier::SameDirectory,
            PatternCorpusTier::Project,
        ] {
            // Path order is stable across batching and adoption order. Corpus
            // file IDs are intentionally opaque and may be assigned in a
            // different order when background batches finish differently.
            for (index, file) in self.statistical_candidate_files().enumerate() {
                if index.is_multiple_of(32)
                    && (cancelled.load(Ordering::Relaxed) || budget.is_expired())
                {
                    return None;
                }
                if evidence.len() >= statistical::MAX_QUERY_FILES {
                    break 'tiers;
                }
                if file.path == active_path
                    || file.kind != AnalysisSourceKind::Tua
                    || current_version(&file.path) != Some(file.version)
                    || tier_for(&file.path, &active_path, &required_targets, true) != wanted_tier
                {
                    continue;
                }
                let Some(table) = file.statistical.as_deref() else {
                    continue;
                };
                if bytes.saturating_add(table.approximate_bytes()) > statistical::MAX_QUERY_BYTES {
                    continue;
                }
                bytes += table.approximate_bytes();
                evidence.push(StatisticalEvidence {
                    table,
                    weight: match wanted_tier {
                        PatternCorpusTier::RequiredModule => 3.0,
                        PatternCorpusTier::SameDirectory => 2.0,
                        PatternCorpusTier::Project => 1.0,
                        PatternCorpusTier::CurrentFile => unreachable!(),
                    },
                });
            }
        }
        statistical::predict(&evidence, active_source, offset, scope, budget, cancelled)
    }

    /// Iterate corpus files in path order so background batch/adoption order
    /// cannot change which files survive the statistical query cap.
    fn statistical_candidate_files(&self) -> impl Iterator<Item = &CorpusFile> {
        self.path_ids.values().filter_map(|id| self.files.get(id))
    }

    pub(crate) fn declared_love_callbacks(
        &self,
        generation: u64,
        lineage: VfsLineageId,
        max_support_checks: usize,
        budget: super::InlineCompletionBudget,
        cancelled: &AtomicBool,
        current_version: impl Fn(&Path) -> Option<VfsSourceVersion>,
    ) -> Option<BTreeSet<String>> {
        if generation != self.eligibility_generation || self.lineage != Some(lineage) {
            return None;
        }
        let mut result = BTreeSet::new();
        let mut checked = 0usize;
        for (callback, supporters) in self.callback_support.iter() {
            for id in supporters {
                checked += 1;
                if checked > max_support_checks
                    || cancelled.load(Ordering::Relaxed)
                    || budget.is_expired()
                {
                    return None;
                }
                let Some(file) = self.files.get(id) else {
                    continue;
                };
                if current_version(&file.path) == Some(file.version) {
                    result.insert(callback.clone());
                    break;
                }
            }
        }
        Some(result)
    }

    fn related_targets(
        &self,
        active_id: Option<CorpusFileId>,
        active_source: &str,
        active_version: Option<VfsSourceVersion>,
        max_targets: usize,
        budget: super::InlineCompletionBudget,
        cancelled: &AtomicBool,
    ) -> Option<BTreeSet<PathBuf>> {
        let Some(file) = active_id.and_then(|id| self.files.get(&id)) else {
            return Some(BTreeSet::new());
        };
        if active_version != Some(file.version) {
            if cancelled.load(Ordering::Relaxed) || budget.is_expired() {
                return None;
            }
            let current = module_specifier_identities(active_source);
            if cancelled.load(Ordering::Relaxed) || budget.is_expired() {
                return None;
            }
            if current != file.specifier_identities {
                return Some(BTreeSet::new());
            }
        }
        if file.resolved_targets.len() > max_targets {
            return None;
        }
        let mut result = BTreeSet::new();
        for (index, target) in file.resolved_targets.iter().enumerate() {
            if index.is_multiple_of(32)
                && (cancelled.load(Ordering::Relaxed) || budget.is_expired())
            {
                return None;
            }
            result.insert(target.clone());
        }
        Some(result)
    }

    fn overlay_bytes(&self) -> usize {
        self.overlay
            .values()
            .map(|entry| match entry {
                // Reserve the larger of the live overlay representation and
                // its eventual base representation. An admitted corpus can
                // therefore always be remerged under the same byte cap.
                OverlayEntry::Table(table) => table
                    .approximate_bytes()
                    .max(table_base_pattern_bytes(table)),
                OverlayEntry::Metadata => 0,
                OverlayEntry::Tombstone => 0,
            })
            .sum()
    }

    fn compute_approximate_bytes(&self) -> usize {
        size_of::<Self>()
            + self.base.approximate_bytes
            + self.overlay.len() * size_of::<(CorpusFileId, OverlayEntry)>()
            + self.overlay_bytes()
            + self.files.len() * size_of::<(CorpusFileId, CorpusFile)>()
            + self
                .files
                .values()
                .map(CorpusFile::metadata_bytes)
                .sum::<usize>()
            + self.path_ids.len() * size_of::<(PathBuf, CorpusFileId)>()
            + self
                .path_ids
                .keys()
                .map(|path| path.as_os_str().len())
                .sum::<usize>()
    }
}

impl PatternCorpusRemergeOutcome {
    pub(crate) fn source(&self) -> &Arc<ProjectPatternCorpus> {
        &self.source
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn lineage(&self) -> VfsLineageId {
        self.lineage
    }

    pub(crate) fn versions(&self) -> &[(PathBuf, VfsSourceVersion)] {
        &self.versions
    }

    pub(crate) fn merged_bytes(&self) -> usize {
        self.merged.approximate_bytes
    }

    pub(crate) fn into_merged(self) -> ProjectPatternCorpus {
        self.merged
    }
}

impl PatternFileBuild {
    fn into_file(self) -> CorpusFile {
        CorpusFile {
            path: self.path,
            version: self.version,
            kind: self.kind,
            specifier_identities: self.specifier_identities,
            resolved_targets: self.resolved_targets,
            declared_love_callbacks: self.declared_love_callbacks,
            statistical: self.statistical,
        }
    }
}

impl CorpusFile {
    fn metadata_bytes(&self) -> usize {
        self.path.as_os_str().len()
            + self.specifier_identities.capacity() * size_of::<ModuleSpecifierIdentity>()
            + self
                .specifier_identities
                .iter()
                .map(|identity| identity.module.capacity())
                .sum::<usize>()
            + self.resolved_targets.len() * size_of::<PathBuf>()
            + self
                .resolved_targets
                .iter()
                .map(|path| path.as_os_str().len())
                .sum::<usize>()
            + self.declared_love_callbacks.len() * size_of::<String>()
            + self
                .declared_love_callbacks
                .iter()
                // Also charge a conservative copy of the callback name and
                // one supporter node for the immutable aggregate index.
                .map(|name| name.capacity() * 2 + size_of::<CorpusFileId>() * 2)
                .sum::<usize>()
            + self
                .statistical
                .as_ref()
                .map_or(0, |table| table.approximate_bytes())
    }
}

fn remove_callback_support(
    support: &mut BTreeMap<String, BTreeSet<CorpusFileId>>,
    id: CorpusFileId,
    callbacks: &BTreeSet<String>,
) {
    for callback in callbacks {
        if let Some(ids) = support.get_mut(callback) {
            ids.remove(&id);
            if ids.is_empty() {
                support.remove(callback);
            }
        }
    }
}

fn add_callback_support(
    support: &mut BTreeMap<String, BTreeSet<CorpusFileId>>,
    id: CorpusFileId,
    callbacks: &BTreeSet<String>,
) {
    for callback in callbacks {
        support.entry(callback.clone()).or_default().insert(id);
    }
}

fn file_map_entry_bytes(file: &CorpusFile) -> usize {
    size_of::<(CorpusFileId, CorpusFile)>() + file.metadata_bytes()
}

fn overlay_map_entry_bytes(entry: &OverlayEntry) -> usize {
    size_of::<(CorpusFileId, OverlayEntry)>()
        + match entry {
            OverlayEntry::Table(table) => table
                .approximate_bytes()
                .max(table_base_pattern_bytes(table)),
            OverlayEntry::Metadata | OverlayEntry::Tombstone => 0,
        }
}

fn table_base_pattern_bytes(table: &FilePatternTable) -> usize {
    table
        .entries()
        .map(|entry| {
            size_of::<BasePattern>()
                + entry.normalized.len()
                + entry.original.len()
                + std::mem::size_of_val(entry.line_indices)
        })
        .sum()
}

fn path_map_entry_bytes(path: &Path) -> usize {
    size_of::<(PathBuf, CorpusFileId)>() + path.as_os_str().len()
}

impl PatternBase {
    fn merged(
        base: &PatternBase,
        overlay: &BTreeMap<CorpusFileId, OverlayEntry>,
        files: &BTreeMap<CorpusFileId, CorpusFile>,
    ) -> Self {
        let mut patterns = base
            .patterns
            .iter()
            .filter(|pattern| {
                !overlay.contains_key(&pattern.file_id) && files.contains_key(&pattern.file_id)
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut approximate_bytes = size_of::<Self>();
        for entry in &patterns {
            approximate_bytes += size_of::<BasePattern>()
                + entry.normalized.len()
                + entry.original.len()
                + std::mem::size_of_val(entry.line_indices.as_ref());
        }
        for (file_id, overlay_entry) in overlay {
            let OverlayEntry::Table(table) = overlay_entry else {
                continue;
            };
            let Some(file) = files.get(file_id) else {
                continue;
            };
            for entry in table.entries() {
                approximate_bytes += size_of::<BasePattern>()
                    + entry.normalized.len()
                    + entry.original.len()
                    + std::mem::size_of_val(entry.line_indices);
                patterns.push(BasePattern {
                    normalized: entry.normalized.into(),
                    original: entry.original.into(),
                    file_id: *file_id,
                    version: file.version,
                    line_indices: entry.line_indices.into(),
                    max_continuation_bytes: table.max_continuation_bytes(),
                });
            }
        }
        patterns.sort_by(|left, right| {
            left.normalized
                .cmp(&right.normalized)
                .then(left.file_id.cmp(&right.file_id))
                .then(left.original.cmp(&right.original))
        });
        Self {
            file_ids: files.keys().copied().collect(),
            patterns: patterns.into_boxed_slice(),
            approximate_bytes: approximate_bytes + files.len() * size_of::<CorpusFileId>(),
        }
    }
}

struct AggregatedMatch {
    file_id: CorpusFileId,
    source_path: PathBuf,
    source_kind: AnalysisSourceKind,
    support: usize,
}

fn aggregate_match(
    grouped: &mut BTreeMap<(PatternCorpusTier, String), AggregatedMatch>,
    tier: PatternCorpusTier,
    continuation: &str,
    file_id: CorpusFileId,
    source_path: &Path,
    source_kind: AnalysisSourceKind,
    support: usize,
) {
    let hit = grouped
        .entry((tier, continuation.to_owned()))
        .or_insert_with(|| AggregatedMatch {
            file_id,
            source_path: source_path.to_path_buf(),
            source_kind,
            support: 0,
        });
    hit.support += support;
}

fn tier_for(
    path: &Path,
    active_path: &Path,
    required_targets: &BTreeSet<PathBuf>,
    rank_same_directory: bool,
) -> PatternCorpusTier {
    if path == active_path {
        PatternCorpusTier::CurrentFile
    } else if required_targets.contains(path) {
        PatternCorpusTier::RequiredModule
    } else if rank_same_directory && path.parent() == active_path.parent() {
        PatternCorpusTier::SameDirectory
    } else {
        PatternCorpusTier::Project
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AnalysisHost;

    fn assert_send<T: Send>() {}
    fn assert_send_sync<T: Send + Sync>() {}

    fn add(host: &mut AnalysisHost, path: &str, source: &str) {
        let _ = host.update_file_with_kind(path, source, AnalysisSourceKind::Tua);
    }

    #[test]
    fn build_inputs_are_send_and_published_corpora_are_send_and_sync() {
        assert_send::<PatternCorpusBuildInput>();
        assert_send_sync::<ProjectPatternCorpus>();
        assert_send_sync::<PreparedPatternCorpusRemerge>();
        assert_send_sync::<PatternCorpusRemergeOutcome>();
        assert_send_sync::<RetiredProjectPatternCorpus>();
    }

    #[test]
    fn statistical_tables_are_not_built_until_the_source_is_enabled() {
        let mut host = AnalysisHost::new();
        add(
            &mut host,
            "main.tua",
            "local point = 0\npoint = point + 1\n",
        );
        let warmed = host.clone();
        assert!(warmed.warm_file("main.tua", crate::AnalysisWarmPriority::High));
        assert!(host.adopt_warmed_file(&warmed, "main.tua"));
        assert_eq!(host.retained_statistical_scope_files(), 0);
        let outcome = host
            .prepare_project_pattern_file("main.tua")
            .unwrap()
            .build(&ProjectModel::default(), RetrievalLimits::default());
        let PatternCorpusBuildOutcome::Built(build) = outcome else {
            panic!("expected a project corpus table")
        };
        assert!(build.statistical.is_none());
        let _ = host.snapshot();
        assert_eq!(host.cache_stats().inline_statistical_scope_files, 0);

        host.set_inline_statistical_enabled(true);
        let outcome = host
            .prepare_project_pattern_file("main.tua")
            .unwrap()
            .build(&ProjectModel::default(), RetrievalLimits::default());
        let PatternCorpusBuildOutcome::Built(build) = outcome else {
            panic!("expected a project corpus table")
        };
        assert!(build.statistical.is_some());
        let _ = host.snapshot();
        assert_eq!(
            host.cache_stats().inline_statistical_scope_files,
            0,
            "ordinary snapshots must not extract optional inline scope facts"
        );
        let warmed = host.clone();
        assert!(warmed.warm_file("main.tua", crate::AnalysisWarmPriority::High));
        assert!(host.adopt_warmed_file(&warmed, "main.tua"));

        host.set_file_with_kind("plain.lua", "local point = 1\n", AnalysisSourceKind::Lua);
        let outcome = host
            .prepare_project_pattern_file("plain.lua")
            .unwrap()
            .build(&ProjectModel::default(), RetrievalLimits::default());
        let PatternCorpusBuildOutcome::Built(build) = outcome else {
            panic!("expected a project corpus table")
        };
        assert!(build.statistical.is_none());

        host.set_file(
            "large.tua",
            format!(
                "local point = 0\n-- {}",
                "x".repeat(statistical::MAX_SOURCE_BYTES)
            ),
        );
        let outcome = host
            .prepare_project_pattern_file("large.tua")
            .unwrap()
            .build(&ProjectModel::default(), RetrievalLimits::default());
        let PatternCorpusBuildOutcome::Built(build) = outcome else {
            panic!("expected a project corpus table")
        };
        assert!(build.statistical.is_none());
        assert_eq!(host.cache_stats().inline_statistical_scope_files, 1);
        assert!(host
            .prepare_local_inline_completion_with_sources("plain.lua", false, true)
            .unwrap()
            .statistical_scope
            .is_none());
        assert!(host
            .prepare_local_inline_completion_with_sources("large.tua", false, true)
            .unwrap()
            .statistical_scope
            .is_none());
    }

    #[test]
    fn statistical_only_builds_omit_exact_tables_and_lua_files() {
        let mut host = AnalysisHost::new();
        host.set_inline_project_enabled(false);
        host.set_inline_statistical_enabled(true);
        add(
            &mut host,
            "train.tua",
            "local point = 0\npoint = point + 1\n",
        );

        let outcome = host
            .prepare_project_pattern_file("train.tua")
            .expect("Tua statistical input")
            .build(&ProjectModel::default(), RetrievalLimits::default());
        let PatternCorpusBuildOutcome::Built(build) = outcome else {
            panic!("expected a statistical corpus table")
        };
        assert!(build.table.is_none(), "exact retrieval must stay disabled");
        assert!(build.statistical.is_some());
        assert!(build.declared_love_callbacks.is_empty());

        host.set_file_with_kind("plain.lua", "local point = 1\n", AnalysisSourceKind::Lua);
        assert!(
            host.prepare_project_pattern_file("plain.lua").is_none(),
            "statistical-only indexing must not clone or parse Lua sources"
        );
    }

    #[test]
    fn metadata_only_overlay_remerges_and_tombstones_without_exact_patterns() {
        let mut host = AnalysisHost::new();
        host.set_inline_project_enabled(false);
        host.set_inline_statistical_enabled(true);
        add(
            &mut host,
            "train.tua",
            "local point = 0\npoint = point + 1\n",
        );
        add(&mut host, "active.tua", "point =");
        let generation = host.project_pattern_corpus_stats().eligibility_generation;
        let outcome = build(&host, "train.tua");
        adopt(&mut host, generation, outcome);

        let before = host.project_pattern_corpus_stats();
        assert_eq!(before.files, 1);
        assert_eq!(before.overlay_files, 1);
        assert!(
            query(&host, "active.tua", "point =", 0).is_empty(),
            "metadata-only overlays must never produce exact retrieval text"
        );

        let prepared = host
            .prepare_project_pattern_remerge()
            .expect("metadata overlay should be remergeable");
        let outcome = prepared.build();
        let retired = host
            .try_adopt_project_pattern_remerge(outcome)
            .expect("unchanged metadata remerge should adopt");
        drop(retired);
        let merged = host.project_pattern_corpus_stats();
        assert_eq!(merged.files, 1);
        assert_eq!(merged.base_files, 1);
        assert_eq!(merged.overlay_files, 0);
        assert!(query(&host, "active.tua", "point =", 0).is_empty());

        let tombstone = host.prepare_project_pattern_tombstone("train.tua");
        let adoption = host.adopt_project_pattern_batch(
            generation,
            [PatternCorpusBatchEntry::Tombstone(tombstone)],
        );
        assert_eq!(adoption.tombstoned, 1);
        assert_eq!(host.project_pattern_corpus_stats().files, 0);
        assert!(query(&host, "active.tua", "point =", 0).is_empty());
    }

    #[test]
    fn statistical_query_cap_is_independent_of_background_adoption_order() {
        fn selected_paths(reverse: bool) -> Vec<PathBuf> {
            let mut host = AnalysisHost::new();
            host.set_inline_project_enabled(false);
            host.set_inline_statistical_enabled(true);
            host.set_project_pattern_eligibility_generation(1);
            for index in 0..statistical::MAX_QUERY_FILES * 2 {
                add(
                    &mut host,
                    &format!("train/{index:03}.tua"),
                    "local point = 0\npoint = 1\n",
                );
            }
            let indices = if reverse {
                (0..statistical::MAX_QUERY_FILES * 2)
                    .rev()
                    .collect::<Vec<_>>()
            } else {
                (0..statistical::MAX_QUERY_FILES * 2).collect::<Vec<_>>()
            };
            for index in indices {
                let path = format!("train/{index:03}.tua");
                let outcome = build(&host, &path);
                adopt(&mut host, 1, outcome);
            }

            host.project_pattern_corpus()
                .statistical_candidate_files()
                .filter(|file| file.statistical.is_some())
                .take(statistical::MAX_QUERY_FILES)
                .map(|file| file.path.clone())
                .collect()
        }

        let ascending = selected_paths(false);
        let descending = selected_paths(true);
        assert_eq!(ascending, descending);
        assert_eq!(ascending.len(), statistical::MAX_QUERY_FILES);
        assert_eq!(ascending.first().unwrap(), Path::new("train/000.tua"));
        assert_eq!(ascending.last().unwrap(), Path::new("train/063.tua"));
    }

    fn build(host: &AnalysisHost, path: &str) -> PatternCorpusBuildOutcome {
        host.prepare_project_pattern_file(path)
            .expect("build input")
            .build(&ProjectModel::default(), RetrievalLimits::default())
    }

    fn adopt(host: &mut AnalysisHost, generation: u64, outcome: PatternCorpusBuildOutcome) {
        let result =
            host.adopt_project_pattern_batch(generation, [PatternCorpusBatchEntry::File(outcome)]);
        assert_eq!(result.adopted, 1, "{result:?}");
    }

    fn query(
        host: &AnalysisHost,
        path: &str,
        prefix: &str,
        line: usize,
    ) -> Vec<PatternCorpusMatch> {
        host.project_pattern_matches(
            path,
            prefix,
            line,
            8,
            &super::super::InlineCompletionBudget::unlimited(),
            &AtomicBool::new(false),
        )
    }

    fn remerge(host: &mut AnalysisHost) -> bool {
        let outcome = host
            .prepare_project_pattern_remerge()
            .expect("prepared remerge")
            .build();
        host.adopt_project_pattern_remerge(outcome)
    }

    #[test]
    fn adopts_queries_and_excludes_only_the_active_line() {
        let mut host = AnalysisHost::new();
        host.set_project_pattern_eligibility_generation(7);
        add(
            &mut host,
            "game/main.tua",
            "draw(player)\ndraw(player)\ndraw(pla",
        );
        let outcome = build(&host, "game/main.tua");
        adopt(&mut host, 7, outcome);

        let matches = query(&host, "game/main.tua", "draw(pla", 0);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].continuation, "yer)");
        assert_eq!(matches[0].support, 1);
        assert_eq!(matches[0].tier, PatternCorpusTier::CurrentFile);
        assert_eq!(matches[0].source_kind, AnalysisSourceKind::Tua);
    }

    #[test]
    fn stale_versions_disappear_immediately_and_are_rejected_on_adoption() {
        let mut host = AnalysisHost::new();
        host.set_project_pattern_eligibility_generation(1);
        add(&mut host, "game/main.tua", "print(value)\nprint(v");
        let stale = build(&host, "game/main.tua");
        adopt(&mut host, 1, stale.clone());
        assert!(!query(&host, "game/main.tua", "print(v", 1).is_empty());

        add(&mut host, "game/main.tua", "print(other)\nprint(v");
        assert!(query(&host, "game/main.tua", "print(v", 1).is_empty());
        let corpus_before_rejection = host.project_pattern_corpus();
        let rejected = host.adopt_project_pattern_batch(1, [PatternCorpusBatchEntry::File(stale)]);
        assert_eq!(rejected.stale, 1);
        assert_eq!(rejected.adopted, 0);
        assert!(Arc::ptr_eq(
            &corpus_before_rejection,
            &host.project_pattern_corpus()
        ));

        let fresh = build(&host, "game/main.tua");
        adopt(&mut host, 1, fresh);
        assert_eq!(
            query(&host, "game/main.tua", "print(", 1)[0].continuation,
            "other)"
        );
    }

    #[test]
    fn generation_changes_fail_closed_and_reject_old_batches() {
        let mut host = AnalysisHost::new();
        host.set_project_pattern_eligibility_generation(3);
        add(&mut host, "main.tua", "print(value)\nprint(");
        let build = build(&host, "main.tua");
        adopt(&mut host, 3, build.clone());
        host.set_project_pattern_eligibility_generation(4);
        assert!(query(&host, "main.tua", "print(", 1).is_empty());

        let result = host.adopt_project_pattern_batch(3, [PatternCorpusBatchEntry::File(build)]);
        assert!(result.generation_rejected);
        assert_eq!(result.entries.len(), 1);
        assert_eq!(
            result.entries[0].status,
            CorpusEntryAdoptionStatus::GenerationMismatch
        );
        assert_eq!(
            host.project_pattern_corpus_stats().eligibility_generation,
            3
        );
    }

    #[test]
    fn overlay_replaces_base_and_tombstones_remove_it() {
        let mut host = AnalysisHost::new();
        host.set_project_pattern_eligibility_generation(2);
        add(&mut host, "main.tua", "print(old)\nprint(");
        let first = build(&host, "main.tua");
        adopt(&mut host, 2, first);
        assert!(remerge(&mut host));
        assert_eq!(host.project_pattern_corpus_stats().base_files, 1);

        add(&mut host, "main.tua", "print(new)\nprint(");
        let replacement = build(&host, "main.tua");
        adopt(&mut host, 2, replacement);
        let matches = query(&host, "main.tua", "print(", 1);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].continuation, "new)");

        assert!(host.tombstone_project_pattern_file(2, "main.tua"));
        assert!(query(&host, "main.tua", "print(", 1).is_empty());
        assert_eq!(host.project_pattern_corpus_stats().tombstones, 1);
        assert!(remerge(&mut host));
        let stats = host.project_pattern_corpus_stats();
        assert_eq!(stats.files, 0);
        assert_eq!(stats.tombstones, 0);
    }

    #[test]
    fn ranks_same_directory_and_resolved_targets_as_related() {
        let mut host = AnalysisHost::new();
        host.set_project_pattern_eligibility_generation(9);
        add(
            &mut host,
            "game/main.tua",
            "local Other = require('pkg.other')\nprint(",
        );
        add(&mut host, "game/sibling.tua", "print(sibling)");
        add(&mut host, "lib/other.tua", "print(required)");
        add(&mut host, "elsewhere.tua", "print(project)");

        let mut active = build(&host, "game/main.tua");
        let PatternCorpusBuildOutcome::Built(active_build) = &mut active else {
            panic!("active build")
        };
        active_build
            .resolved_targets
            .insert(normalize_path("lib/other.tua"));
        let entries = [
            active,
            build(&host, "game/sibling.tua"),
            build(&host, "lib/other.tua"),
            build(&host, "elsewhere.tua"),
        ]
        .map(PatternCorpusBatchEntry::File);
        let result = host.adopt_project_pattern_batch(9, entries);
        assert_eq!(result.adopted, 4);

        let matches = query(&host, "game/main.tua", "print(", 1);
        let tier = |text: &str| {
            matches
                .iter()
                .find(|hit| hit.continuation == text)
                .map(|hit| hit.tier)
        };
        assert_eq!(tier("sibling)"), Some(PatternCorpusTier::Project));
        assert_eq!(tier("required)"), Some(PatternCorpusTier::RequiredModule));
        assert_eq!(tier("project)"), Some(PatternCorpusTier::Project));

        host.set_project_pattern_limits(PatternCorpusLimits {
            rank_same_directory: true,
            ..PatternCorpusLimits::default()
        });
        let matches = query(&host, "game/main.tua", "print(", 1);
        assert_eq!(
            matches
                .iter()
                .find(|hit| hit.continuation == "sibling)")
                .map(|hit| hit.tier),
            Some(PatternCorpusTier::SameDirectory)
        );
        host.set_project_pattern_limits(PatternCorpusLimits::default());

        // A non-specifier edit makes the active table stale but preserves the
        // exact lexical identity, so its resolved targets remain reusable.
        add(
            &mut host,
            "game/main.tua",
            "local Other = require('pkg.other')\nlocal x = 1\nprint(",
        );
        let matches = query(&host, "game/main.tua", "print(", 2);
        assert_eq!(
            matches
                .iter()
                .find(|hit| hit.continuation == "required)")
                .map(|hit| hit.tier),
            Some(PatternCorpusTier::RequiredModule)
        );

        host.set_project_pattern_limits(PatternCorpusLimits {
            max_active_source_bytes: 1,
            ..PatternCorpusLimits::default()
        });
        let matches = query(&host, "game/main.tua", "print(", 2);
        assert_eq!(
            matches
                .iter()
                .find(|hit| hit.continuation == "required)")
                .map(|hit| hit.tier),
            Some(PatternCorpusTier::Project)
        );
        host.set_project_pattern_limits(PatternCorpusLimits::default());

        add(
            &mut host,
            "game/main.tua",
            "local Other = require('different')\nprint(",
        );
        let matches = query(&host, "game/main.tua", "print(", 1);
        assert_eq!(
            matches
                .iter()
                .find(|hit| hit.continuation == "required)")
                .map(|hit| hit.tier),
            Some(PatternCorpusTier::Project)
        );
    }

    #[test]
    fn project_completion_keeps_and_prefers_a_same_directory_required_module() {
        let mut host = AnalysisHost::new();
        host.set_project_pattern_eligibility_generation(1);
        add(
            &mut host,
            "active/main.tua",
            "local Helper = require('./helper')\ndraw(player,",
        );
        add(
            &mut host,
            "active/helper.tua",
            "draw(player, score)\ndraw(player, score)",
        );
        add(
            &mut host,
            "elsewhere/noise.tua",
            "draw(player, stale)\ndraw(player, stale)",
        );

        let mut active = build(&host, "active/main.tua");
        let PatternCorpusBuildOutcome::Built(active_build) = &mut active else {
            panic!("active build")
        };
        active_build
            .resolved_targets
            .insert(normalize_path("active/helper.tua"));
        let entries = [
            active,
            build(&host, "active/helper.tua"),
            build(&host, "elsewhere/noise.tua"),
        ]
        .map(PatternCorpusBatchEntry::File);
        assert_eq!(host.adopt_project_pattern_batch(1, entries).adopted, 3);

        let candidates = host.local_inline_completion(
            "active/main.tua",
            crate::SourcePosition {
                line: 2,
                column: 13,
            },
            &crate::InlineCompletionOptions {
                semantic: false,
                current_file: false,
                project: true,
                ..crate::InlineCompletionOptions::default()
            },
            &super::super::InlineCompletionBudget::unlimited(),
            &AtomicBool::new(false),
        );
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].insert_text, " score)");
        assert_eq!(candidates[0].tier, Some(PatternCorpusTier::RequiredModule));
    }

    #[test]
    fn callbacks_require_a_current_file_version() {
        let mut host = AnalysisHost::new();
        host.set_project_pattern_eligibility_generation(1);
        add(&mut host, "callbacks.tua", "function love.draw()\nend\n");
        let outcome = build(&host, "callbacks.tua");
        adopt(&mut host, 1, outcome);
        assert!(host.project_declared_love_callbacks().contains("draw"));

        add(
            &mut host,
            "callbacks.tua",
            "function love.update(dt)\nend\n",
        );
        assert!(host.project_declared_love_callbacks().is_empty());
    }

    #[test]
    fn byte_cap_rejects_publication_without_replacing_the_corpus() {
        let mut host = AnalysisHost::new();
        host.set_project_pattern_eligibility_generation(1);
        host.set_project_pattern_limits(PatternCorpusLimits {
            max_bytes: 1,
            ..PatternCorpusLimits::default()
        });
        add(&mut host, "main.tua", "print(value)");
        let outcome = build(&host, "main.tua");
        let result = host.adopt_project_pattern_batch(1, [PatternCorpusBatchEntry::File(outcome)]);
        assert_eq!(result.over_budget, 1);
        assert_eq!(host.project_pattern_corpus_stats().files, 0);
    }

    #[test]
    fn host_clones_observe_only_the_immutable_value_they_captured() {
        let mut coordinator = AnalysisHost::new();
        coordinator.set_project_pattern_eligibility_generation(1);
        add(&mut coordinator, "main.tua", "print(value)\nprint(");
        let worker = coordinator.clone();
        let outcome = build(&coordinator, "main.tua");
        adopt(&mut coordinator, 1, outcome);

        assert!(!query(&coordinator, "main.tua", "print(", 1).is_empty());
        assert!(query(&worker, "main.tua", "print(", 1).is_empty());
    }

    #[test]
    fn clearing_publishes_empty_and_allows_off_thread_release() {
        let mut host = AnalysisHost::new();
        host.set_project_pattern_eligibility_generation(1);
        add(&mut host, "main.tua", "print(value)\nprint(");
        let outcome = build(&host, "main.tua");
        adopt(&mut host, 1, outcome);
        let retired = host.clear_project_pattern_corpus().expect("retired corpus");
        assert_eq!(host.project_pattern_corpus_stats().files, 0);
        assert!(query(&host, "main.tua", "print(", 1).is_empty());
        std::thread::spawn(move || drop(retired)).join().unwrap();
        assert!(host.clear_project_pattern_corpus().is_none());
    }

    #[test]
    fn skipped_current_build_tombstones_an_old_table() {
        let mut host = AnalysisHost::new();
        host.set_project_pattern_eligibility_generation(1);
        add(&mut host, "main.tua", "print(value)\nprint(");
        let outcome = build(&host, "main.tua");
        adopt(&mut host, 1, outcome);
        assert!(remerge(&mut host));

        add(
            &mut host,
            "main.tua",
            "a source that is deliberately too large",
        );
        let skipped = host
            .prepare_project_pattern_file("main.tua")
            .unwrap()
            .build(
                &ProjectModel::default(),
                RetrievalLimits {
                    max_source_bytes: 1,
                    ..RetrievalLimits::default()
                },
            );
        let result = host.adopt_project_pattern_batch(1, [PatternCorpusBatchEntry::File(skipped)]);
        assert_eq!(result.skipped, 1);
        assert_eq!(host.project_pattern_corpus_stats().files, 0);
    }

    #[test]
    fn rejects_builds_and_tombstones_from_another_vfs_lineage() {
        let mut first = AnalysisHost::new();
        let mut second = AnalysisHost::new();
        first.set_project_pattern_eligibility_generation(1);
        second.set_project_pattern_eligibility_generation(1);
        add(&mut first, "main.tua", "print(first)\nprint(");
        add(&mut second, "main.tua", "print(second)\nprint(");

        let result = second.adopt_project_pattern_batch(
            1,
            [PatternCorpusBatchEntry::File(build(&first, "main.tua"))],
        );
        assert!(result.lineage_rejected);
        assert_eq!(result.stale, 1);

        let tombstone = first.prepare_project_pattern_tombstone("main.tua");
        let result =
            second.adopt_project_pattern_batch(1, [PatternCorpusBatchEntry::Tombstone(tombstone)]);
        assert!(result.lineage_rejected);
    }

    #[test]
    fn stale_tombstone_tickets_cannot_remove_newer_content() {
        let mut host = AnalysisHost::new();
        host.set_project_pattern_eligibility_generation(1);
        add(&mut host, "main.tua", "print(old)\nprint(");
        let old = build(&host, "main.tua");
        adopt(&mut host, 1, old);
        let tombstone = host.prepare_project_pattern_tombstone("main.tua");

        add(&mut host, "main.tua", "print(new)\nprint(");
        let fresh = build(&host, "main.tua");
        adopt(&mut host, 1, fresh);
        let result =
            host.adopt_project_pattern_batch(1, [PatternCorpusBatchEntry::Tombstone(tombstone)]);
        assert_eq!(result.stale, 1);
        assert_eq!(
            query(&host, "main.tua", "print(", 1)[0].continuation,
            "new)"
        );

        host.remove_file("main.tua");
        let absent = host.prepare_project_pattern_tombstone("main.tua");
        add(&mut host, "main.tua", "print(readded)\nprint(");
        let readded = build(&host, "main.tua");
        adopt(&mut host, 1, readded);
        let result =
            host.adopt_project_pattern_batch(1, [PatternCorpusBatchEntry::Tombstone(absent)]);
        assert_eq!(result.stale, 1);
        assert_eq!(
            query(&host, "main.tua", "print(", 1)[0].continuation,
            "readded)"
        );
    }

    #[test]
    fn remerge_is_built_off_thread_and_conditionally_adopted() {
        let mut host = AnalysisHost::new();
        host.set_project_pattern_eligibility_generation(1);
        add(&mut host, "main.tua", "print(one)\nprint(");
        let first = build(&host, "main.tua");
        adopt(&mut host, 1, first);

        let stale_by_corpus = host.prepare_project_pattern_remerge().unwrap().build();
        add(&mut host, "other.tua", "print(two)");
        let other = build(&host, "other.tua");
        adopt(&mut host, 1, other);
        assert!(!host.adopt_project_pattern_remerge(stale_by_corpus));

        let stale_by_version = host.prepare_project_pattern_remerge().unwrap().build();
        add(&mut host, "main.tua", "print(changed)\nprint(");
        assert!(!host.adopt_project_pattern_remerge(stale_by_version));

        let changed = build(&host, "main.tua");
        adopt(&mut host, 1, changed);
        let outcome = host.prepare_project_pattern_remerge().unwrap().build();
        assert!(host.adopt_project_pattern_remerge(outcome));
        let stats = host.project_pattern_corpus_stats();
        assert_eq!(stats.overlay_files, 0);
        assert_eq!(stats.base_files, 2);
    }

    #[test]
    fn batch_adoption_keeps_the_base_shared_and_applies_all_files_once() {
        let mut host = AnalysisHost::new();
        host.set_project_pattern_eligibility_generation(1);
        add(&mut host, "seed.tua", "print(seed)");
        let seed = build(&host, "seed.tua");
        adopt(&mut host, 1, seed);
        assert!(remerge(&mut host));
        let before = host.project_pattern_corpus();
        let mut outcomes = Vec::new();
        for index in 0..128 {
            let path = format!("files/{index}.tua");
            add(&mut host, &path, &format!("print(value{index})"));
            outcomes.push(PatternCorpusBatchEntry::File(build(&host, &path)));
        }
        let result = host.adopt_project_pattern_batch(1, outcomes);
        assert_eq!(result.adopted, 128);
        let after = host.project_pattern_corpus();
        assert!(Arc::ptr_eq(&before.base, &after.base));
        assert_eq!(after.files.len(), 129);
    }

    #[test]
    fn query_cancellation_deadline_and_scan_cap_fail_closed() {
        let mut host = AnalysisHost::new();
        host.set_project_pattern_eligibility_generation(1);
        add(
            &mut host,
            "main.tua",
            "print(one)\nprint(two)\nprint(three)\nprint(",
        );
        let outcome = build(&host, "main.tua");
        adopt(&mut host, 1, outcome);

        assert!(host
            .project_pattern_matches(
                "main.tua",
                "print(",
                3,
                8,
                &super::super::InlineCompletionBudget::expired(),
                &AtomicBool::new(false),
            )
            .is_empty());
        assert!(host
            .project_pattern_matches(
                "main.tua",
                "print(",
                3,
                8,
                &super::super::InlineCompletionBudget::unlimited(),
                &AtomicBool::new(true),
            )
            .is_empty());

        host.set_project_pattern_limits(PatternCorpusLimits {
            max_query_patterns: 1,
            ..PatternCorpusLimits::default()
        });
        assert!(query(&host, "main.tua", "print(", 3).is_empty());

        host.set_project_pattern_limits(PatternCorpusLimits {
            max_query_prefix_bytes: 3,
            ..PatternCorpusLimits::default()
        });
        assert!(query(&host, "main.tua", "print(", 3).is_empty());
    }

    #[test]
    fn partial_admission_is_deterministic_and_keeps_later_small_files() {
        let mut host = AnalysisHost::new();
        host.set_project_pattern_eligibility_generation(1);
        add(
            &mut host,
            "large.tua",
            &format!("print({})", "large_value".repeat(128)),
        );
        add(&mut host, "small.tua", "print(small)");
        let PatternCorpusBuildOutcome::Built(large) = build(&host, "large.tua") else {
            panic!("large build")
        };
        let PatternCorpusBuildOutcome::Built(small) = build(&host, "small.tua") else {
            panic!("small build")
        };
        let lineage = small.version.lineage();
        let empty = ProjectPatternCorpus::empty(1, Some(lineage));
        let unlimited = PatternCorpusLimits::default();
        let (small_only, _) = empty.with_batch(
            vec![ValidatedCorpusMutation::File(small.clone())],
            unlimited,
        );
        let limits = PatternCorpusLimits {
            max_bytes: small_only.approximate_bytes,
            ..unlimited
        };
        let (corpus, results) = empty.with_batch(
            vec![
                ValidatedCorpusMutation::File(small),
                ValidatedCorpusMutation::File(large),
            ],
            limits,
        );
        assert_eq!(corpus.stats().files, 1);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].path, normalize_path("large.tua"));
        assert_eq!(results[0].status, CorpusEntryAdoptionStatus::OverBudget);
        assert_eq!(results[1].path, normalize_path("small.tua"));
        assert_eq!(results[1].status, CorpusEntryAdoptionStatus::Adopted);
    }

    #[test]
    fn mixed_batch_publishes_tombstone_and_fitting_file_before_overflow() {
        let mut host = AnalysisHost::new();
        host.set_project_pattern_eligibility_generation(1);
        add(&mut host, "a.tua", "print(old_value)");
        let old = build(&host, "a.tua");
        adopt(&mut host, 1, old);
        host.remove_file("a.tua");
        let tombstone = host.prepare_project_pattern_tombstone("a.tua");

        add(&mut host, "b.tua", "print(fits)");
        add(
            &mut host,
            "c.tua",
            &format!("print({})", "too_large".repeat(256)),
        );
        let PatternCorpusBuildOutcome::Built(b) = build(&host, "b.tua") else {
            panic!("b build")
        };
        let PatternCorpusBuildOutcome::Built(c) = build(&host, "c.tua") else {
            panic!("c build")
        };
        let before = host.project_pattern_corpus();
        let (fits, _) = before.with_batch(
            vec![
                ValidatedCorpusMutation::Tombstone {
                    path: normalize_path("a.tua"),
                    skipped: false,
                },
                ValidatedCorpusMutation::File(b.clone()),
            ],
            PatternCorpusLimits::default(),
        );
        host.set_project_pattern_limits(PatternCorpusLimits {
            max_bytes: fits.approximate_bytes,
            ..PatternCorpusLimits::default()
        });
        let result = host.adopt_project_pattern_batch(
            1,
            [
                PatternCorpusBatchEntry::File(PatternCorpusBuildOutcome::Built(c)),
                PatternCorpusBatchEntry::File(PatternCorpusBuildOutcome::Built(b)),
                PatternCorpusBatchEntry::Tombstone(tombstone),
            ],
        );
        assert_eq!(result.adopted, 1);
        assert_eq!(result.tombstoned, 1);
        assert_eq!(result.over_budget, 1);
        let corpus = host.project_pattern_corpus();
        assert!(!corpus
            .files
            .values()
            .any(|file| file.path == Path::new("a.tua")));
        assert!(corpus
            .files
            .values()
            .any(|file| file.path == Path::new("b.tua")));
        assert!(!corpus
            .files
            .values()
            .any(|file| file.path == Path::new("c.tua")));
    }

    #[test]
    fn tombstones_are_applied_before_builds_and_free_budget() {
        let mut host = AnalysisHost::new();
        host.set_project_pattern_eligibility_generation(1);
        add(&mut host, "old.tua", "print(an_old_and_fairly_long_value)");
        let PatternCorpusBuildOutcome::Built(old) = build(&host, "old.tua") else {
            panic!("old build")
        };
        let lineage = old.version.lineage();
        let limits = PatternCorpusLimits::default();
        let (corpus, _) = ProjectPatternCorpus::empty(1, Some(lineage))
            .with_batch(vec![ValidatedCorpusMutation::File(old)], limits);

        add(&mut host, "new.tua", "print(new)");
        let PatternCorpusBuildOutcome::Built(new) = build(&host, "new.tua") else {
            panic!("new build")
        };
        let (expected, _) = corpus.with_batch(
            vec![
                ValidatedCorpusMutation::Tombstone {
                    path: normalize_path("old.tua"),
                    skipped: false,
                },
                ValidatedCorpusMutation::File(new.clone()),
            ],
            limits,
        );
        let constrained = PatternCorpusLimits {
            max_bytes: expected.approximate_bytes,
            ..limits
        };
        let (next, results) = corpus.with_batch(
            vec![
                ValidatedCorpusMutation::File(new),
                ValidatedCorpusMutation::Tombstone {
                    path: normalize_path("old.tua"),
                    skipped: false,
                },
            ],
            constrained,
        );
        assert_eq!(next.stats().files, 1);
        assert_eq!(results[0].status, CorpusEntryAdoptionStatus::Tombstoned);
        assert_eq!(results[1].status, CorpusEntryAdoptionStatus::Adopted);
    }

    #[test]
    fn hard_file_limit_bounds_large_batch_metadata() {
        let mut host = AnalysisHost::new();
        add(&mut host, "template.tua", "print(value)");
        let PatternCorpusBuildOutcome::Built(template) = build(&host, "template.tua") else {
            panic!("template build")
        };
        let mut mutations = Vec::with_capacity(10_000);
        for index in 0..10_000 {
            let mut build = template.clone();
            build.path = PathBuf::from(format!("many/{index:05}.tua"));
            mutations.push(ValidatedCorpusMutation::File(build));
        }
        let limits = PatternCorpusLimits {
            max_bytes: usize::MAX,
            max_files: 1024,
            ..PatternCorpusLimits::default()
        };
        let lineage = template.version.lineage();
        let (corpus, results) =
            ProjectPatternCorpus::empty(0, Some(lineage)).with_batch(mutations, limits);
        assert_eq!(corpus.stats().files, 1024);
        assert_eq!(
            results
                .iter()
                .filter(|entry| entry.status == CorpusEntryAdoptionStatus::FileLimit)
                .count(),
            8976
        );
    }

    #[test]
    fn coordinator_work_is_structurally_bounded_at_the_file_cap() {
        let mut host = AnalysisHost::new();
        add(&mut host, "template.tua", "print(value)");
        let PatternCorpusBuildOutcome::Built(template) = build(&host, "template.tua") else {
            panic!("template build")
        };
        let lineage = template.version.lineage();
        let initial = (0..1024)
            .map(|index| {
                let mut build = template.clone();
                build.path = PathBuf::from(format!("many/{index:04}.tua"));
                ValidatedCorpusMutation::File(build)
            })
            .collect();
        let limits = PatternCorpusLimits {
            max_bytes: usize::MAX,
            ..PatternCorpusLimits::default()
        };
        let (corpus, _) = ProjectPatternCorpus::empty(0, Some(lineage)).with_batch(initial, limits);
        let replacements = (0..16)
            .map(|index| {
                let mut build = template.clone();
                build.path = PathBuf::from(format!("many/{index:04}.tua"));
                ValidatedCorpusMutation::File(build)
            })
            .collect();
        let (corpus, results) = corpus.with_batch(replacements, limits);
        assert_eq!(results.len(), 16);
        assert_eq!(corpus.files.len(), limits.max_files);
        assert_eq!(corpus.path_ids.len(), limits.max_files);

        let corpus = Arc::new(corpus);
        let prepared = ProjectPatternCorpus::prepare_remerge(Arc::clone(&corpus), lineage)
            .expect("overlay should prepare");
        // Preparation retains one immutable corpus pointer. It does not walk
        // or clone any of the 1,024 per-file metadata entries; that work is
        // intentionally deferred to `build` on the background worker.
        assert!(Arc::ptr_eq(&prepared.source, &corpus));
        assert_eq!(Arc::strong_count(&corpus), 2);
        // The expensive build and eventual release are explicitly worker
        // operations; merely transferring their ownership is constant time.
        let outcome = prepared.build();
        std::thread::spawn(move || drop(outcome)).join().unwrap();
    }

    /// Manual coordinator-cost probe. Kept out of ordinary test runs because
    /// wall-clock assertions are inherently flaky on loaded CI and sanitizer
    /// builds; the production file cap and structural test above provide the
    /// deterministic guardrail.
    #[test]
    #[ignore = "manual project-corpus coordinator latency probe"]
    fn profile_corpus_coordinator_work_at_file_cap() {
        use std::time::Instant;

        let mut host = AnalysisHost::new();
        add(&mut host, "template.tua", "print(value)");
        let PatternCorpusBuildOutcome::Built(template) = build(&host, "template.tua") else {
            panic!("template build")
        };
        let lineage = template.version.lineage();
        let limits = PatternCorpusLimits {
            max_bytes: usize::MAX,
            ..PatternCorpusLimits::default()
        };
        let initial = (0..limits.max_files)
            .map(|index| {
                let mut build = template.clone();
                build.path = PathBuf::from(format!("many/{index:04}.tua"));
                ValidatedCorpusMutation::File(build)
            })
            .collect();
        let (corpus, _) = ProjectPatternCorpus::empty(0, Some(lineage)).with_batch(initial, limits);
        let replacements = (0..16)
            .map(|index| {
                let mut build = template.clone();
                build.path = PathBuf::from(format!("many/{index:04}.tua"));
                ValidatedCorpusMutation::File(build)
            })
            .collect();

        let started = Instant::now();
        let (corpus, results) = corpus.with_batch(replacements, limits);
        let adoption_elapsed = started.elapsed();
        let corpus = Arc::new(corpus);
        let started = Instant::now();
        let prepared = ProjectPatternCorpus::prepare_remerge(Arc::clone(&corpus), lineage)
            .expect("overlay should prepare");
        let prepare_elapsed = started.elapsed();
        eprintln!(
            "project corpus coordinator probe: files={}, updates={}, adopt={adoption_elapsed:?}, prepare={prepare_elapsed:?}",
            corpus.files.len(),
            results.len()
        );
        std::thread::spawn(move || drop(prepared.build()))
            .join()
            .unwrap();
    }

    #[test]
    fn admitted_corpus_can_remerge_without_exceeding_its_cap() {
        let mut host = AnalysisHost::new();
        host.set_project_pattern_eligibility_generation(1);
        add(
            &mut host,
            "main.tua",
            "print(one)\nprint(two)\nprint(three)",
        );
        let outcome = build(&host, "main.tua");
        adopt(&mut host, 1, outcome);
        let admitted_bytes = host.project_pattern_corpus_stats().approximate_bytes;
        host.set_project_pattern_limits(PatternCorpusLimits {
            max_bytes: admitted_bytes,
            ..PatternCorpusLimits::default()
        });
        let outcome = host.prepare_project_pattern_remerge().unwrap().build();
        assert!(host.adopt_project_pattern_remerge(outcome));
        assert!(host.project_pattern_corpus_stats().approximate_bytes <= admitted_bytes);
        assert!(host.prepare_project_pattern_remerge().is_none());
    }

    #[test]
    fn stale_generation_is_not_prepared_for_remerge() {
        let mut host = AnalysisHost::new();
        host.set_project_pattern_eligibility_generation(1);
        add(&mut host, "main.tua", "print(value)");
        let outcome = build(&host, "main.tua");
        adopt(&mut host, 1, outcome);
        host.set_project_pattern_eligibility_generation(2);
        assert!(host.prepare_project_pattern_remerge().is_none());
    }

    #[test]
    fn callback_lookup_uses_bounded_immutable_support() {
        let mut host = AnalysisHost::new();
        host.set_project_pattern_eligibility_generation(1);
        add(&mut host, "draw.tua", "function love.draw()\nend");
        add(&mut host, "update.tua", "function love.update(dt)\nend");
        let outcomes = [build(&host, "draw.tua"), build(&host, "update.tua")]
            .map(PatternCorpusBatchEntry::File);
        assert_eq!(host.adopt_project_pattern_batch(1, outcomes).adopted, 2);
        host.set_project_pattern_limits(PatternCorpusLimits {
            max_callback_support_checks: 1,
            ..PatternCorpusLimits::default()
        });
        assert!(host.project_declared_love_callbacks().is_empty());
        host.set_project_pattern_limits(PatternCorpusLimits::default());
        assert_eq!(
            host.project_declared_love_callbacks(),
            BTreeSet::from(["draw".to_owned(), "update".to_owned()])
        );
    }
}

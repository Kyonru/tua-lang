//! Local, deterministic inline completion.
//!
//! Phase 0 of `better-autocompletion.md` provides the cursor-eligibility rules
//! and offline evaluator. Phase 1 exposes the measured current-file backend as
//! a host-level query that never builds semantic analysis or a snapshot.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::{AnalysisSourceKind, SourcePosition, SourceSpan};

#[doc(hidden)]
pub mod corpus;
pub mod eligibility;
#[cfg(any(test, feature = "test-fixture"))]
pub mod eval;
mod project_corpus;
mod recall;
#[doc(hidden)]
pub mod retrieval;
mod semantic;
mod validate;

pub use project_corpus::{
    CorpusAdoptionResult, CorpusEntryAdoption, CorpusEntryAdoptionStatus, CorpusFileId,
    PatternCorpusBatchEntry, PatternCorpusBuildInput, PatternCorpusBuildOutcome,
    PatternCorpusLimits, PatternCorpusMatch, PatternCorpusRemergeOutcome, PatternCorpusStats,
    PatternCorpusTier, PatternCorpusTombstone, PatternFileBuild, PreparedPatternCorpusRemerge,
    ProjectPatternCorpus, RetiredProjectPatternCorpus,
};
pub(crate) use project_corpus::{PatternCorpusQuery, ValidatedCorpusMutation};
pub use retrieval::{RetrievalBuildError, RetrievalLimits};

/// Sources that can produce a local inline-completion candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InlineCompletionSource {
    Semantic,
    CurrentFile,
    Project,
    Corpus,
}

impl InlineCompletionSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Semantic => "semantic",
            Self::CurrentFile => "currentFile",
            Self::Project => "project",
            Self::Corpus => "corpus",
        }
    }
}

/// A score meaningful only among candidates produced by the same source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InlineCompletionScore(pub f64);

/// Stable identity for future feedback-backed pattern ranking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatternId(pub String);

/// One protocol-neutral inline completion.
#[derive(Clone, Debug, PartialEq)]
pub struct InlineCompletionCandidate {
    pub insert_text: String,
    pub range: SourceSpan,
    pub is_snippet: bool,
    pub source: InlineCompletionSource,
    pub score: InlineCompletionScore,
    pub pattern_id: Option<PatternId>,
}

/// An immutable, source-local inline-completion request prepared by an
/// [`AnalysisHost`](crate::AnalysisHost).
///
/// Preparing a request copies only the active document and any already-cached
/// parse errors. It never parses, builds analysis, or takes a snapshot, so an
/// LSP coordinator can move this value to a worker without cloning the host.
pub struct PreparedInlineCompletion {
    pub(crate) path: PathBuf,
    pub(crate) source: String,
    pub(crate) kind: AnalysisSourceKind,
    pub(crate) include_ecs: bool,
    pub(crate) baseline_parse_errors: Option<Vec<crate::Diagnostic>>,
    pub(crate) project: Option<PreparedProjectCompletion>,
    pub(crate) stats: Arc<InlineCompletionRuntimeStats>,
}

impl PreparedInlineCompletion {
    /// The exact document source captured by the coordinator.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Generate and validate a candidate using the source captured when this
    /// request was prepared.
    pub fn complete(
        &self,
        position: SourcePosition,
        options: &InlineCompletionOptions,
        budget: &InlineCompletionBudget,
        cancelled: &AtomicBool,
    ) -> Vec<InlineCompletionCandidate> {
        let baseline_parse_errors = || self.baseline_parse_errors.clone();
        complete(
            InlineCompletionInput {
                path: &self.path,
                source: &self.source,
                kind: self.kind,
                position,
                include_ecs: self.include_ecs,
                stats: &self.stats,
            },
            self.project.as_ref().map(|project| ProjectCompletionInput {
                corpus: &project.corpus,
                versions: &project.versions,
                eligibility: &project.eligibility,
                generation: project.generation,
                eligibility_epoch: project.eligibility_epoch,
                limits: project.limits,
            }),
            options,
            budget,
            cancelled,
            &baseline_parse_errors,
        )
    }
}

/// Local source switches. Project retrieval remains experimental and disabled
/// by default until its aggregate gate passes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InlineCompletionOptions {
    pub semantic: bool,
    pub current_file: bool,
    pub project: bool,
}

impl Default for InlineCompletionOptions {
    fn default() -> Self {
        Self {
            semantic: true,
            current_file: true,
            project: false,
        }
    }
}

/// A request deadline with deterministic unlimited/expired constructors for
/// core tests. The parser is not interruptible, so callers must reserve enough
/// time before starting it.
#[derive(Clone, Copy, Debug)]
pub struct InlineCompletionBudget {
    deadline: Option<Instant>,
}

impl InlineCompletionBudget {
    pub fn new(duration: Duration) -> Self {
        Self {
            deadline: Some(Instant::now() + duration),
        }
    }

    pub fn unlimited() -> Self {
        Self { deadline: None }
    }

    pub fn expired() -> Self {
        Self {
            deadline: Some(Instant::now()),
        }
    }

    pub fn is_expired(self) -> bool {
        self.deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
    }

    pub(crate) fn has_time_for(self, duration: Duration) -> bool {
        self.deadline
            .is_none_or(|deadline| Instant::now() + duration < deadline)
    }
}

pub(crate) struct InlineCompletionInput<'a> {
    pub path: &'a std::path::Path,
    pub source: &'a str,
    pub kind: AnalysisSourceKind,
    pub position: SourcePosition,
    pub include_ecs: bool,
    pub stats: &'a InlineCompletionRuntimeStats,
}

pub(crate) struct ProjectCompletionInput<'a> {
    corpus: &'a ProjectPatternCorpus,
    versions: &'a crate::vfs::VfsSourceVersionIndex,
    eligibility: &'a ProjectPatternEligibility,
    generation: u64,
    eligibility_epoch: u64,
    limits: PatternCorpusLimits,
}

pub(crate) struct PreparedProjectCompletion {
    pub corpus: Arc<ProjectPatternCorpus>,
    pub versions: Arc<crate::vfs::VfsSourceVersionIndex>,
    pub eligibility: Arc<ProjectPatternEligibility>,
    pub generation: u64,
    pub eligibility_epoch: u64,
    pub limits: PatternCorpusLimits,
}

#[derive(Debug, Default)]
pub(crate) struct ProjectPatternEligibility {
    generation: std::sync::atomic::AtomicU64,
    epoch: std::sync::atomic::AtomicU64,
}

impl ProjectPatternEligibility {
    pub(crate) fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    pub(crate) fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }

    pub(crate) fn set_generation(&self, generation: u64) {
        self.generation.store(generation, Ordering::Release);
        self.invalidate();
    }

    pub(crate) fn invalidate(&self) {
        self.epoch.fetch_add(1, Ordering::AcqRel);
    }

    pub(crate) fn matches(&self, generation: u64, epoch: u64) -> bool {
        let before = self.epoch.load(Ordering::Acquire);
        before == epoch
            && self.generation.load(Ordering::Acquire) == generation
            && self.epoch.load(Ordering::Acquire) == before
    }
}

#[derive(Debug, Default)]
pub(crate) struct InlineCompletionRuntimeStats {
    pub requests: AtomicUsize,
    pub cache_peek_hits: AtomicUsize,
    pub cache_peek_misses: AtomicUsize,
    pub validation_attempts: AtomicUsize,
    pub candidate_parses: AtomicUsize,
    pub baseline_parses: AtomicUsize,
    pub edited_parse_sufficient: AtomicUsize,
    pub candidates_returned: AtomicUsize,
    pub returned_after_zero_parses: AtomicUsize,
    pub returned_after_one_parse: AtomicUsize,
    pub returned_after_two_parses: AtomicUsize,
}

pub(crate) fn complete(
    input: InlineCompletionInput<'_>,
    project: Option<ProjectCompletionInput<'_>>,
    options: &InlineCompletionOptions,
    budget: &InlineCompletionBudget,
    cancelled: &AtomicBool,
    baseline_parse_errors: &dyn Fn() -> Option<Vec<crate::Diagnostic>>,
) -> Vec<InlineCompletionCandidate> {
    if cancelled.load(Ordering::Relaxed) || budget.is_expired() {
        return Vec::new();
    }
    let Some(offset) = crate::source_text::byte_for_position(input.source, input.position) else {
        return Vec::new();
    };
    let Some(context) = eligibility::cursor_context(input.source, offset) else {
        return Vec::new();
    };
    let line_end = input.source[offset..]
        .find('\n')
        .map_or(input.source.len(), |relative| offset + relative);
    let suffix = &input.source[offset..line_end];
    if !eligibility::suffix_allows_inline(suffix) {
        return Vec::new();
    }

    let range = SourceSpan {
        start: input.position,
        end: input.position,
    };
    // One epoch view spans every project-dependent stage, including parse
    // validation. Each lookup is non-blocking and takes no long-lived guard;
    // the final epoch check rejects a candidate if any VFS source changed at
    // any point during this completion.
    let project_versions = options
        .project
        .then(|| {
            let project = project.as_ref()?;
            project
                .eligibility
                .matches(project.generation, project.eligibility_epoch)
                .then(|| project.versions.begin_read())
                .flatten()
        })
        .flatten();
    let semantic = if options.semantic {
        let _profile = crate::profile::span("inline.semantic");
        // LÖVE callback ghost text remains evaluator-only: the pinned Phase 1
        // policy disables it, and Phase 2 retrieval evaluation does not add
        // it. Keep production semantic behavior limited to the measured
        // statement-snippet source until callbacks pass their own gate.
        semantic::statement_snippet(input.source, offset, context, input.include_ecs, 3)
            .map(|text| (text, true))
            .map(|(text, is_snippet)| InlineCompletionCandidate {
                insert_text: text,
                range,
                is_snippet,
                source: InlineCompletionSource::Semantic,
                score: InlineCompletionScore(1.0),
                pattern_id: None,
            })
    } else {
        None
    };
    let raw = semantic
        .or_else(|| {
            let _profile = crate::profile::span("inline.recall");
            (options.current_file
                && !matches!(
                    context,
                    eligibility::CursorContext::Assignment | eligibility::CursorContext::Member
                ))
            .then(|| {
                recall::common_prefix_checked(
                    input.source,
                    offset,
                    input.position.line.saturating_sub(1),
                    recall::RecallOptions::phase1(),
                    || cancelled.load(Ordering::Relaxed) || budget.is_expired(),
                )
                .map(|suggestion| InlineCompletionCandidate {
                    insert_text: suggestion.text,
                    range,
                    is_snippet: false,
                    source: InlineCompletionSource::CurrentFile,
                    score: InlineCompletionScore(suggestion.score),
                    pattern_id: None,
                })
            })
            .flatten()
        })
        .or_else(|| {
            let project = project.as_ref()?;
            if !project
                .eligibility
                .matches(project.generation, project.eligibility_epoch)
            {
                return None;
            }
            let line_start = input.source[..offset]
                .rfind('\n')
                .map_or(0, |index| index + 1);
            let prefix = &input.source[line_start..offset];
            if !options.project
                || !matches!(
                    context,
                    eligibility::CursorContext::Comma
                        | eligibility::CursorContext::Operator
                        | eligibility::CursorContext::Other
                )
            {
                return None;
            }
            if prefix
                .chars()
                .filter(|character| !character.is_whitespace())
                .count()
                < 6
            {
                return None;
            }
            let versions = project_versions.as_ref()?;
            let hit = project.corpus.query_project_common_prefix(
                PatternCorpusQuery {
                    generation: project.generation,
                    active_path: input.path,
                    active_source: input.source,
                    prefix,
                    active_line: input.position.line.saturating_sub(1),
                    max_results: project.limits.max_query_patterns,
                    max_active_source_bytes: project.limits.max_active_source_bytes,
                    max_query_patterns: project.limits.max_query_patterns,
                    max_query_prefix_bytes: project.limits.max_query_prefix_bytes,
                    max_related_targets: project.limits.max_related_targets,
                    rank_same_directory: project.limits.rank_same_directory,
                    rest_of_project_only: false,
                    budget: *budget,
                    cancelled,
                },
                2,
                2,
                |path| versions.get(path),
            )?;
            Some(InlineCompletionCandidate {
                insert_text: hit.continuation,
                range,
                is_snippet: false,
                source: InlineCompletionSource::Project,
                score: InlineCompletionScore(hit.support as f64),
                pattern_id: None,
            })
        });

    let Some(mut candidate) = raw else {
        return Vec::new();
    };
    if cancelled.load(Ordering::Relaxed) || budget.is_expired() {
        return Vec::new();
    }
    let Some(adjusted) = validate::agree_with_suffix(&candidate.insert_text, suffix) else {
        return Vec::new();
    };
    candidate.insert_text = adjusted;
    let _profile = crate::profile::span("inline.validate");
    let Some(parse_count) = validate::validate(
        &input,
        &candidate,
        offset,
        budget,
        cancelled,
        baseline_parse_errors,
    ) else {
        return Vec::new();
    };
    if options.project
        && (project.as_ref().is_some_and(|project| {
            !project
                .eligibility
                .matches(project.generation, project.eligibility_epoch)
        }) || project_versions.is_some_and(|versions| !versions.is_current()))
    {
        return Vec::new();
    }
    match parse_count {
        0 => &input.stats.returned_after_zero_parses,
        1 => &input.stats.returned_after_one_parse,
        _ => &input.stats.returned_after_two_parses,
    }
    .fetch_add(1, Ordering::Relaxed);
    input
        .stats
        .candidates_returned
        .fetch_add(1, Ordering::Relaxed);
    vec![candidate]
}

#[cfg(test)]
mod tests;

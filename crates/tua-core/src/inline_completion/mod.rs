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

pub mod eligibility;
#[cfg(any(test, feature = "test-fixture"))]
pub mod eval;
mod recall;
mod semantic;
mod validate;

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
            options,
            budget,
            cancelled,
            &baseline_parse_errors,
        )
    }
}

/// Phase 1 source switches. More settings are added only when later phases
/// actually ship their corresponding source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InlineCompletionOptions {
    pub semantic: bool,
    pub current_file: bool,
}

impl Default for InlineCompletionOptions {
    fn default() -> Self {
        Self {
            semantic: true,
            current_file: true,
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
    let semantic = if options.semantic {
        let _profile = crate::profile::span("inline.semantic");
        semantic::statement_snippet(input.source, offset, context, input.include_ecs, 3).map(
            |text| InlineCompletionCandidate {
                insert_text: text,
                range,
                is_snippet: true,
                source: InlineCompletionSource::Semantic,
                score: InlineCompletionScore(1.0),
                pattern_id: None,
            },
        )
    } else {
        None
    };
    let raw = semantic.or_else(|| {
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

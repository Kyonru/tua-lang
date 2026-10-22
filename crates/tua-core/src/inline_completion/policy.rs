//! The shipping inline-completion policy.
//!
//! The host query in [`super::complete`] reads these values, and the
//! evaluator's pinned configurations (`Phase1Config::production` and
//! `Phase2TuningConfig::pinned`) are built from them. A gate file that matches
//! those configurations therefore describes exactly what the host does.

use super::eligibility::CursorContext;
use super::recall::RecallOptions;

/// Minimum typed characters before a unique statement snippet is offered.
pub(crate) const SNIPPET_MIN_PREFIX_CHARS: usize = 3;

/// Current-file recall thresholds.
pub(crate) const RECALL: RecallOptions = RecallOptions::phase1();

/// Current-file recall is suppressed where tuning data was substantially less
/// precise.
pub(crate) fn recall_allowed(context: CursorContext) -> bool {
    !matches!(context, CursorContext::Assignment | CursorContext::Member)
}

/// Cursor contexts where experimental project retrieval may answer after the
/// current-file sources abstain.
pub(crate) const PROJECT_CONTEXTS: [CursorContext; 3] = [
    CursorContext::Comma,
    CursorContext::Operator,
    CursorContext::Other,
];

/// Minimum non-whitespace characters typed on the line before project
/// retrieval is consulted.
pub(crate) const PROJECT_MIN_PREFIX_CHARS: usize = 6;

/// Supporting project lines required before any project continuation.
pub(crate) const PROJECT_MIN_SUPPORT: usize = 2;

/// Supporting project lines required before the full common prefix, rather
/// than only its first token, is returned.
pub(crate) const PROJECT_MIN_FULL_PREFIX_SUPPORT: usize = 2;

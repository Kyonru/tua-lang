use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::{parse_lua_source, parse_tua_source, AnalysisSourceKind, Diagnostic};

use super::{InlineCompletionBudget, InlineCompletionCandidate, InlineCompletionInput};

const LUA_PARSE_CAP: usize = 31_428;
const TUA_PARSE_CAP: usize = 6_855;
const PARSE_BUDGET: Duration = Duration::from_millis(4);

pub(crate) fn validate(
    input: &InlineCompletionInput<'_>,
    candidate: &InlineCompletionCandidate,
    offset: usize,
    budget: &InlineCompletionBudget,
    cancelled: &AtomicBool,
    baseline_parse_errors: &dyn Fn() -> Option<Vec<Diagnostic>>,
) -> Option<usize> {
    input
        .stats
        .validation_attempts
        .fetch_add(1, Ordering::Relaxed);
    validate_with_parser(
        input,
        candidate,
        offset,
        budget,
        cancelled,
        baseline_parse_errors,
        &mut parse_errors,
    )
}

fn validate_with_parser(
    input: &InlineCompletionInput<'_>,
    candidate: &InlineCompletionCandidate,
    offset: usize,
    budget: &InlineCompletionBudget,
    cancelled: &AtomicBool,
    baseline_parse_errors: &dyn Fn() -> Option<Vec<Diagnostic>>,
    parse: &mut impl FnMut(AnalysisSourceKind, &std::path::Path, String) -> Vec<Diagnostic>,
) -> Option<usize> {
    if candidate.insert_text.is_empty()
        || (!candidate.is_snippet && candidate.insert_text.contains(['\n', '\r']))
        || candidate.insert_text.chars().any(|character| {
            character.is_control()
                && !(candidate.is_snippet && matches!(character, '\n' | '\r' | '\t'))
        })
    {
        return None;
    }
    let insertion = if candidate.is_snippet {
        snippet_plain_text(&candidate.insert_text)?
    } else {
        candidate.insert_text.clone()
    };

    if cancelled.load(Ordering::Relaxed) || !can_parse(input, budget) {
        return None;
    }
    let mut edited = String::with_capacity(input.source.len() + insertion.len());
    edited.push_str(&input.source[..offset]);
    edited.push_str(&insertion);
    edited.push_str(&input.source[offset..]);
    input.stats.candidate_parses.fetch_add(1, Ordering::Relaxed);
    let candidate_errors = parse(input.kind, input.path, edited);
    let mut parse_count = 1;
    if candidate_errors.is_empty() {
        input
            .stats
            .edited_parse_sufficient
            .fetch_add(1, Ordering::Relaxed);
    }
    if cancelled.load(Ordering::Relaxed) || budget.is_expired() {
        return None;
    }
    if candidate_errors.is_empty() {
        return Some(parse_count);
    }

    if cancelled.load(Ordering::Relaxed) || !can_parse(input, budget) {
        return None;
    }
    let baseline_errors = if let Some(errors) = baseline_parse_errors() {
        errors
    } else {
        input.stats.baseline_parses.fetch_add(1, Ordering::Relaxed);
        parse_count += 1;
        parse(input.kind, input.path, input.source.to_owned())
    };
    if cancelled.load(Ordering::Relaxed) || budget.is_expired() {
        return None;
    }
    let baseline = crate::model_completion::baseline_error_multiset(baseline_errors);
    let candidate = crate::model_completion::candidate_error_multiset(
        candidate_errors,
        input.position,
        &insertion,
    );
    (!candidate
        .iter()
        .any(|(signature, count)| *count > baseline.get(signature).copied().unwrap_or(0)))
    .then_some(parse_count)
}

/// Whether a document of this size can ever be validated. Validation must
/// parse, so callers check this before spending time generating a candidate.
pub(crate) fn within_parse_cap(kind: AnalysisSourceKind, source_len: usize) -> bool {
    source_len <= parse_cap(kind)
}

fn parse_cap(kind: AnalysisSourceKind) -> usize {
    match kind {
        AnalysisSourceKind::Lua => LUA_PARSE_CAP,
        AnalysisSourceKind::Tua => TUA_PARSE_CAP,
    }
}

fn can_parse(input: &InlineCompletionInput<'_>, budget: &InlineCompletionBudget) -> bool {
    let cap = parse_cap(input.kind);
    input.source.len() <= cap
        && budget.has_time_for(PARSE_BUDGET.mul_f64(input.source.len() as f64 / cap as f64))
}

fn parse_errors(
    kind: AnalysisSourceKind,
    path: &std::path::Path,
    source: String,
) -> Vec<Diagnostic> {
    match kind {
        AnalysisSourceKind::Lua => parse_lua_source(path, source),
        AnalysisSourceKind::Tua => parse_tua_source(path, source),
    }
    .err()
    .unwrap_or_default()
}

/// Convert VS Code's `${n:default}` and `${n}` forms to parseable defaults.
/// Phase 1 source snippets do not use choices or variables.
fn snippet_plain_text(snippet: &str) -> Option<String> {
    let mut plain = String::with_capacity(snippet.len());
    let mut rest = snippet;
    while let Some(start) = rest.find("${") {
        plain.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after.find('}')?;
        let placeholder = &after[..end];
        if let Some((index, default)) = placeholder.split_once(':') {
            index.parse::<u32>().ok()?;
            plain.push_str(default);
        } else {
            placeholder.parse::<u32>().ok()?;
        }
        rest = &after[end + 1..];
    }
    plain.push_str(rest);
    Some(plain)
}

pub(crate) fn agree_with_suffix(text: &str, suffix: &str) -> Option<String> {
    let closers = suffix
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    if closers.is_empty() {
        return Some(text.to_owned());
    }
    if closes_outer_brackets(text) == 0 {
        return Some(text.to_owned());
    }
    let trimmed = text.trim_end();
    let kept = trimmed.strip_suffix(&closers)?.len();
    (!text[..kept].trim().is_empty()).then(|| text[..kept].to_owned())
}

fn closes_outer_brackets(text: &str) -> usize {
    let mut depth = 0usize;
    let mut outer = 0;
    for (index, character) in text.char_indices() {
        if !matches!(character, '(' | '[' | '{' | ')' | ']' | '}')
            || super::eligibility::lexical_region_at(text, index)
                != super::eligibility::LexicalRegion::Code
        {
            continue;
        }
        if matches!(character, '(' | '[' | '{') {
            depth += 1;
        } else if depth == 0 {
            outer += 1;
        } else {
            depth -= 1;
        }
    }
    outer
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InlineCompletionScore, InlineCompletionSource, SourcePosition, SourceSpan};

    #[test]
    fn materializes_supported_snippet_placeholders() {
        assert_eq!(
            snippet_plain_text("al ${1:name} = ${0:nil}").as_deref(),
            Some("al name = nil")
        );
        assert_eq!(snippet_plain_text("${name}"), None);
    }

    #[test]
    fn existing_auto_closer_is_not_duplicated() {
        assert_eq!(agree_with_suffix("a, b)", ")").as_deref(), Some("a, b"));
        assert_eq!(agree_with_suffix("foo()", ")").as_deref(), Some("foo()"));
        assert_eq!(agree_with_suffix("a) + b", ")"), None);
        assert_eq!(agree_with_suffix("a, b)", "  ) ").as_deref(), Some("a, b"));
        assert_eq!(agree_with_suffix("a, b)", "   ").as_deref(), Some("a, b)"));
    }

    #[test]
    fn parse_delta_rejects_a_new_error() {
        let stats = super::super::InlineCompletionRuntimeStats::default();
        let path = std::path::Path::new("src/main.lua");
        let source = "local value = 1";
        let position = SourcePosition {
            line: 1,
            column: source.chars().count() + 1,
        };
        let candidate = InlineCompletionCandidate {
            insert_text: ")".to_owned(),
            range: SourceSpan {
                start: position,
                end: position,
            },
            is_snippet: false,
            source: InlineCompletionSource::CurrentFile,
            score: InlineCompletionScore(1.0),
            pattern_id: None,
        };
        assert!(validate(
            &InlineCompletionInput {
                path,
                source,
                kind: AnalysisSourceKind::Lua,
                position,
                include_ecs: false,
                stats: &stats,
            },
            &candidate,
            source.len(),
            &InlineCompletionBudget::unlimited(),
            &AtomicBool::new(false),
            &|| Some(Vec::new()),
        )
        .is_none());
    }

    #[test]
    fn an_unrelated_existing_parse_error_does_not_block_a_candidate() {
        let stats = super::super::InlineCompletionRuntimeStats::default();
        let path = std::path::Path::new("src/main.lua");
        let source = "local broken = )\nprint(";
        let position = SourcePosition { line: 2, column: 7 };
        let baseline = parse_errors(AnalysisSourceKind::Lua, path, source.to_owned());
        assert!(!baseline.is_empty());
        let candidate = InlineCompletionCandidate {
            insert_text: "1)".to_owned(),
            range: SourceSpan {
                start: position,
                end: position,
            },
            is_snippet: false,
            source: InlineCompletionSource::CurrentFile,
            score: InlineCompletionScore(1.0),
            pattern_id: None,
        };
        assert_eq!(
            validate(
                &InlineCompletionInput {
                    path,
                    source,
                    kind: AnalysisSourceKind::Lua,
                    position,
                    include_ecs: false,
                    stats: &stats,
                },
                &candidate,
                source.len(),
                &InlineCompletionBudget::unlimited(),
                &AtomicBool::new(false),
                &|| Some(baseline.clone()),
            ),
            Some(1)
        );
        assert_eq!(
            validate(
                &InlineCompletionInput {
                    path,
                    source,
                    kind: AnalysisSourceKind::Lua,
                    position,
                    include_ecs: false,
                    stats: &stats,
                },
                &candidate,
                source.len(),
                &InlineCompletionBudget::unlimited(),
                &AtomicBool::new(false),
                &|| None,
            ),
            Some(2)
        );
    }

    #[test]
    fn cancellation_during_a_parse_abstains_even_when_the_edit_parses() {
        let stats = super::super::InlineCompletionRuntimeStats::default();
        let path = std::path::Path::new("src/main.lua");
        let source = "print(";
        let position = SourcePosition { line: 1, column: 7 };
        let candidate = InlineCompletionCandidate {
            insert_text: "1)".to_owned(),
            range: SourceSpan {
                start: position,
                end: position,
            },
            is_snippet: false,
            source: InlineCompletionSource::CurrentFile,
            score: InlineCompletionScore(1.0),
            pattern_id: None,
        };
        let cancelled = AtomicBool::new(false);
        let mut parse_then_cancel = |kind, path: &std::path::Path, source| {
            let errors = parse_errors(kind, path, source);
            cancelled.store(true, Ordering::Relaxed);
            errors
        };
        assert!(validate_with_parser(
            &InlineCompletionInput {
                path,
                source,
                kind: AnalysisSourceKind::Lua,
                position,
                include_ecs: false,
                stats: &stats,
            },
            &candidate,
            source.len(),
            &InlineCompletionBudget::unlimited(),
            &cancelled,
            &|| Some(Vec::new()),
            &mut parse_then_cancel,
        )
        .is_none());
    }

    #[test]
    fn multiline_recall_is_rejected_while_semantic_snippets_are_explicitly_exempt() {
        let stats = super::super::InlineCompletionRuntimeStats::default();
        let path = std::path::Path::new("src/main.lua");
        let source = "print(";
        let position = SourcePosition { line: 1, column: 7 };
        let candidate = |is_snippet| InlineCompletionCandidate {
            insert_text: "1)\nprint(2)".to_owned(),
            range: SourceSpan {
                start: position,
                end: position,
            },
            is_snippet,
            source: if is_snippet {
                InlineCompletionSource::Semantic
            } else {
                InlineCompletionSource::CurrentFile
            },
            score: InlineCompletionScore(1.0),
            pattern_id: None,
        };
        let input = InlineCompletionInput {
            path,
            source,
            kind: AnalysisSourceKind::Lua,
            position,
            include_ecs: false,
            stats: &stats,
        };
        assert!(validate(
            &input,
            &candidate(false),
            source.len(),
            &InlineCompletionBudget::unlimited(),
            &AtomicBool::new(false),
            &|| Some(Vec::new()),
        )
        .is_none());
        assert!(validate(
            &input,
            &candidate(true),
            source.len(),
            &InlineCompletionBudget::unlimited(),
            &AtomicBool::new(false),
            &|| Some(Vec::new()),
        )
        .is_some());
    }
}

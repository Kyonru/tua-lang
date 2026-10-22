use std::collections::BTreeMap;

use super::eligibility::{is_identifier_char, strip_trailing_comment};

const MAX_RECALL_LINE_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct RecallOptions {
    pub min_prefix_chars: usize,
    pub dominance_ratio: f64,
    pub min_support: usize,
    pub min_full_prefix_support: usize,
    pub max_document_bytes: usize,
}

impl RecallOptions {
    pub(crate) const fn phase1() -> Self {
        Self {
            min_prefix_chars: 6,
            dominance_ratio: 2.0,
            min_support: 1,
            min_full_prefix_support: 2,
            max_document_bytes: 256 * 1024,
        }
    }
}

impl Default for RecallOptions {
    fn default() -> Self {
        Self {
            min_prefix_chars: 6,
            dominance_ratio: 2.0,
            min_support: 1,
            min_full_prefix_support: 1,
            max_document_bytes: 256 * 1024,
        }
    }
}

pub(crate) struct Suggestion {
    pub text: String,
    pub score: f64,
}

#[cfg(any(test, feature = "test-fixture"))]
pub(crate) fn unique(
    document: &str,
    offset: usize,
    line_index: usize,
    options: RecallOptions,
) -> Option<Suggestion> {
    let continuations = continuations(document, offset, line_index, options, &mut || false)?;
    let (best, best_count) = *continuations.first()?;
    let runner_up = continuations.get(1).map_or(0, |(_, count)| *count);
    if runner_up > 0 && (best_count as f64) < options.dominance_ratio * runner_up as f64 {
        return None;
    }
    let total = continuations.iter().map(|(_, count)| count).sum::<usize>();
    (total >= options.min_support).then(|| Suggestion {
        text: best.to_owned(),
        score: best_count as f64 / total as f64,
    })
}

#[cfg(any(test, feature = "test-fixture"))]
pub(crate) fn common_prefix(
    document: &str,
    offset: usize,
    line_index: usize,
    options: RecallOptions,
) -> Option<Suggestion> {
    common_prefix_checked(document, offset, line_index, options, || false)
}

pub(crate) fn common_prefix_checked(
    document: &str,
    offset: usize,
    line_index: usize,
    options: RecallOptions,
    mut should_stop: impl FnMut() -> bool,
) -> Option<Suggestion> {
    let continuations = continuations(document, offset, line_index, options, &mut should_stop)?;
    let (first, _) = *continuations.first()?;
    let mut common = first.len();
    for (continuation, _) in &continuations[1..] {
        common = first
            .char_indices()
            .zip(continuation.chars())
            .find(|((_, left), right)| left != right)
            .map_or(common.min(continuation.len()), |((index, _), _)| {
                index.min(common)
            });
    }
    let mut prefix = &first[..common];
    if continuations.iter().any(|(continuation, _)| {
        continuation[prefix.len()..]
            .chars()
            .next()
            .is_some_and(is_identifier_char)
    }) {
        prefix = prefix.trim_end_matches(is_identifier_char);
    }
    let mut prefix = prefix.trim_end();
    if prefix.is_empty() {
        return None;
    }
    let support = continuations.iter().map(|(_, count)| count).sum::<usize>();
    if support < options.min_support {
        return None;
    }
    if support < options.min_full_prefix_support {
        prefix = first_token(prefix)?;
    }
    Some(Suggestion {
        text: prefix.to_owned(),
        score: support as f64,
    })
}

fn continuations<'a>(
    document: &'a str,
    offset: usize,
    line_index: usize,
    options: RecallOptions,
    should_stop: &mut impl FnMut() -> bool,
) -> Option<Vec<(&'a str, usize)>> {
    if document.len() > options.max_document_bytes {
        return None;
    }
    let line_start = document[..offset].rfind('\n').map_or(0, |index| index + 1);
    let prefix = &document[line_start..offset];
    if prefix.len() > MAX_RECALL_LINE_BYTES {
        return None;
    }
    let (normalized_prefix, _) = normalize(prefix.trim_start());
    if normalized_prefix
        .chars()
        .filter(|character| !character.is_whitespace())
        .count()
        < options.min_prefix_chars
    {
        return None;
    }

    let mut continuations = BTreeMap::<&str, usize>::new();
    for (index, line) in document.split('\n').enumerate() {
        if should_stop() {
            return None;
        }
        if line.len() > MAX_RECALL_LINE_BYTES {
            continue;
        }
        if index == line_index {
            continue;
        }
        let line = line.strip_suffix('\r').unwrap_or(line);
        let Some(continuation) = continuation_after(line, &normalized_prefix) else {
            continue;
        };
        let continuation = strip_trailing_comment(continuation);
        if !continuation.is_empty() {
            *continuations.entry(continuation).or_default() += 1;
        }
    }
    let mut ranked = continuations.into_iter().collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(right.0)));
    Some(ranked)
}

pub(super) fn normalize(text: &str) -> (String, Vec<usize>) {
    let mut normalized = String::with_capacity(text.len());
    let mut origins = Vec::with_capacity(text.len());
    let mut in_space = false;
    for (offset, character) in text.char_indices() {
        if character.is_whitespace() {
            if !in_space {
                normalized.push(' ');
                origins.push(offset);
            }
            in_space = true;
        } else {
            in_space = false;
            let start = normalized.len();
            normalized.push(character);
            origins.extend(std::iter::repeat_n(offset, normalized.len() - start));
        }
    }
    (normalized, origins)
}

pub(super) fn continuation_after<'line>(line: &'line str, prefix: &str) -> Option<&'line str> {
    let indent = line.len() - line.trim_start().len();
    let body = &line[indent..];
    let (normalized, origins) = normalize(body);
    if !normalized.starts_with(prefix) || normalized.len() == prefix.len() {
        return None;
    }
    let mut start = origins[prefix.len()];
    if prefix.ends_with(' ') {
        start += body[start..].len() - body[start..].trim_start().len();
    }
    Some(&body[start..])
}

pub(super) fn first_token(text: &str) -> Option<&str> {
    let start = text.find(|character: char| !character.is_whitespace())?;
    let rest = &text[start..];
    let first = rest.chars().next()?;
    let end = if is_identifier_char(first) {
        rest.char_indices()
            .find(|(_, character)| !is_identifier_char(*character))
            .map_or(rest.len(), |(index, _)| index)
    } else if first == '.' && rest.starts_with("..") {
        2
    } else {
        first.len_utf8()
    };
    Some(&text[..start + end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_original_text_and_excludes_active_line() {
        let source = "  draw(foo, 1)\n\tdraw(foo, 2)\n  draw(fo";
        let suggestion = common_prefix(source, source.len(), 2, RecallOptions::phase1()).unwrap();
        assert_eq!(suggestion.text, "o,");
    }

    #[test]
    fn one_supporting_line_stops_after_the_first_token() {
        let source = "print(player.name, player.score)\nprint(pl";
        let suggestion = common_prefix(source, source.len(), 1, RecallOptions::phase1()).unwrap();
        assert_eq!(suggestion.text, "ayer");
    }

    #[test]
    fn unicode_common_prefix_ends_on_a_character_boundary() {
        let source = "print(éclair, 1)\nprint(éclair, 2)\nprint(é";
        let suggestion = common_prefix(source, source.len(), 2, RecallOptions::phase1()).unwrap();
        assert_eq!(suggestion.text, "clair,");
    }

    #[test]
    fn a_long_scan_stops_when_the_request_is_cancelled() {
        let source = "print(player)\nprint(player)\nprint(pla";
        let mut checks = 0;
        let suggestion =
            common_prefix_checked(source, source.len(), 2, RecallOptions::phase1(), || {
                checks += 1;
                checks > 1
            });
        assert!(suggestion.is_none());
        assert_eq!(checks, 2);
    }

    #[test]
    fn pathological_single_lines_are_skipped_before_normalization() {
        let long = "a".repeat(MAX_RECALL_LINE_BYTES + 1);
        let source = format!("{long}\n{long}");
        assert!(
            common_prefix_checked(&source, source.len(), 1, RecallOptions::phase1(), || false,)
                .is_none()
        );
    }
}

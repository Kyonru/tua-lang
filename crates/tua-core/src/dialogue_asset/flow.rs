//! Recover flow names from incomplete scripts without treating prose or payloads as code.
use super::{identifier, span, DialogueName};
use crate::{SourcePosition, SourceSpan};

pub fn labels(source: &str) -> Vec<DialogueName> {
    source
        .lines()
        .enumerate()
        .filter_map(|(index, raw)| {
            let text = raw.trim();
            let name = text.strip_prefix('[')?.strip_suffix(']')?;
            if !identifier(name) || matches!(name, "else" | "endif") {
                return None;
            }
            let column = raw.chars().count() - raw.trim_start().chars().count() + 2;
            Some(DialogueName {
                value: name.into(),
                span: span(index + 1, column, name.len()),
            })
        })
        .collect()
}

/// The complete name range, including the suffix after the cursor. Names are ASCII;
/// all preceding prose is measured in Unicode scalars, as in the strict parser.
pub fn name_at(source: &str, position: SourcePosition) -> Option<DialogueName> {
    let raw = source.lines().nth(position.line.checked_sub(1)?)?;
    let text = raw.trim();
    let base = raw.len() - raw.trim_start().len();
    let mut ranges = Vec::new();
    if let Some(rest) = raw.trim_start().strip_prefix("@fallback ") {
        let rest = rest.trim_start();
        ranges.push((raw.len() - rest.len(), rest.trim_end().len()));
    } else if text.starts_with('[') {
        let name = text.strip_prefix('[')?.strip_suffix(']')?;
        if matches!(name, "else" | "endif") {
            return None;
        }
        ranges.push((base + 1, name.len()));
    } else if text.starts_with("-> ") || text.starts_with("@wait ") {
        // A choice's only flow reference is the final target tag, never its prose.
        let offset = text.rfind(" [target:")? + " [target:".len();
        let rest = text[offset..].trim_end();
        let name = rest.strip_suffix(']').unwrap_or(rest);
        ranges.push((base + offset, name.len()));
    } else if text.starts_with("@branch ") {
        // Conditions may contain quoted text. Only recover the final target pair.
        let offset = text.rfind(" [target:")? + " [target:".len();
        let rest = &text[offset..];
        if let Some((yes, no)) = rest.split_once("] [else:") {
            ranges.push((base + offset, yes.len()));
            let no = no.trim_end();
            ranges.push((
                base + offset + yes.len() + "] [else:".len(),
                no.strip_suffix(']').unwrap_or(no).len(),
            ));
        } else {
            let yes = rest.trim_end();
            ranges.push((base + offset, yes.strip_suffix(']').unwrap_or(yes).len()));
        }
    } else if text.starts_with("@action ") {
        // Positional header words stop before the JSON payload, even if it contains tags.
        let words: Vec<_> = text.split_whitespace().take(5).collect();
        for (index, prefix) in [(3, "[success:"), (4, "[failure:")] {
            let Some(word) = words.get(index) else {
                continue;
            };
            let Some(name) = word.strip_prefix(prefix) else {
                continue;
            };
            let offset = word.as_ptr() as usize - raw.as_ptr() as usize + prefix.len();
            ranges.push((offset, name.strip_suffix(']').unwrap_or(name).len()));
        }
    }
    for (offset, length) in ranges {
        let value = raw.get(offset..offset + length)?;
        if !value.is_empty() && !identifier(value) {
            continue;
        }
        let range = span(
            position.line,
            raw[..offset].chars().count() + 1,
            value.len(),
        );
        if position.column >= range.start.column && position.column <= range.end.column {
            return Some(DialogueName {
                value: value.into(),
                span: range,
            });
        }
    }
    None
}

pub fn replacement_span(source: &str, position: SourcePosition) -> Option<SourceSpan> {
    name_at(source, position).map(|name| name.span)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovered_flow_spans_agree_with_strict_parser() {
        let source = "@dialogue 1\n  [start]\n// @id hello\n: Hello\n// @id go\n-> é🦆 Go [target:finish.good-end]\n[finish.good-end]  \n// @id done\n: Bye (end)\n[waiting]\n@wait 1 [target:start]\n";
        let parsed = super::super::parse_dialogue_source("scene.tdialogue", source).unwrap();
        let recovered = labels(source);
        assert_eq!(
            recovered,
            parsed
                .sections
                .iter()
                .map(|section| section.label.clone())
                .collect::<Vec<_>>()
        );
        for label in &recovered {
            assert_eq!(name_at(source, label.span.start).as_ref(), Some(label));
        }
        let choice = &parsed.sections[0].lines[0].choices[0].target;
        assert_eq!(name_at(source, choice.span.start).as_ref(), Some(choice));
        let wait = &parsed.sections[2].wait.as_ref().unwrap().target;
        assert_eq!(name_at(source, wait.span.start).as_ref(), Some(wait));
        for empty in ["@wait 1 [target:", "  @fallback ", "@fallback   "] {
            assert_eq!(
                name_at(
                    empty,
                    SourcePosition {
                        line: 1,
                        column: empty.len() + 1
                    }
                )
                .unwrap()
                .value,
                ""
            );
        }
    }
}

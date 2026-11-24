//! Source ranges for named text substitutions, including unfinished names.
use super::{identifier, rich_text, span, DialogueName};
use crate::{SourcePosition, SourceSpan};

pub fn name_at(source: &str, position: SourcePosition) -> Option<DialogueName> {
    let (text, column, _) = rich_text::text_field(source, position.line)?;
    let relative = position.column.checked_sub(column)?;
    if relative > text.chars().count() {
        return None;
    }
    let cursor = text
        .char_indices()
        .nth(relative)
        .map_or(text.len(), |(byte, _)| byte);
    let rich = rich_text::authoring_field(source, position.line).is_some();
    let mut chars = text.char_indices().peekable();
    while let Some((_, ch)) = chars.next() {
        if rich && ch == '\\' {
            chars.next();
            continue;
        }
        if rich && ch == '[' {
            rich_text::mark_end(&mut chars, text.len());
            continue;
        }
        if ch != '%' {
            continue;
        }
        if chars.peek().is_some_and(|(_, ch)| *ch == '%') {
            chars.next();
            continue;
        }
        let Some((open, '{')) = chars.peek().copied() else {
            continue;
        };
        chars.next();
        let start = open + 1;
        let mut end = text.len();
        for (at, ch) in chars.by_ref() {
            if ch == '}' {
                end = at;
                break;
            }
        }
        let raw = &text[start..end];
        let value = raw.trim();
        if !value.is_empty() && (!identifier(value) || value.contains(['.', '-'])) {
            continue;
        }
        let start = start + raw.len() - raw.trim_start().len();
        let end = start + value.len();
        if cursor >= start && cursor <= end {
            return Some(DialogueName {
                value: value.into(),
                span: span(
                    position.line,
                    column + text[..start].chars().count(),
                    value.len(),
                ),
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
    fn binding_recovery_matches_strict_names_and_defaults() {
        let source = "@dialogue 1\n@context \"context\"\n@bind   player context.name default \"é\\nfriend\"\n[start]\n// @id hello\n: Hello %{ player } (end)\n";
        let strict =
            crate::dialogue_asset::parse_dialogue_source("scene.tdialogue", source).unwrap();
        let recovered = crate::dialogue_asset::binding_declarations(
            std::path::Path::new("scene.tdialogue"),
            source,
        )
        .unwrap();
        assert_eq!(recovered, strict.bindings);
        assert_eq!(recovered[0].name.span.start.column, 9);
        let position = SourcePosition {
            line: 6,
            column: 14,
        };
        let name = name_at(source, position).unwrap();
        assert_eq!(name.value, "player");
        assert_eq!(name.span.start.column, 12);
        assert_eq!(name.span.end.column, 18);
        assert!(crate::dialogue_asset::binding_declarations(
            std::path::Path::new("bad.tdialogue"),
            &source.replace("@context \"context\"\n", "")
        )
        .is_none());
    }
}

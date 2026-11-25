//! Source-only voice cue recognition. Cue IDs are host identifiers, not file paths.
use super::{span, DialogueName};
use crate::{SourcePosition, SourceSpan};

/// Recover the cue's JSON string content, including an unfinished string. Other
/// values are consumed by serde, so nested objects and escaped quotes cannot
/// masquerade as a top-level cue field.
fn cue_on_line(raw: &str, line: usize) -> Option<(DialogueName, usize)> {
    let mut rest = raw
        .trim_start()
        .strip_prefix("@voice ")?
        .trim_start()
        .strip_prefix('{')?;
    let mut keys = std::collections::BTreeSet::new();
    loop {
        rest = rest.trim_start();
        let mut key = serde_json::Deserializer::from_str(rest).into_iter::<String>();
        let key_name = key.next()?.ok()?;
        if !keys.insert(key_name.clone()) {
            return None;
        }
        rest = rest
            .get(key.byte_offset()..)?
            .trim_start()
            .strip_prefix(':')?
            .trim_start();
        if key_name == "cue" {
            let content = rest.strip_prefix('"')?;
            let mut end = content.len();
            let mut chars = content.char_indices();
            while let Some((offset, ch)) = chars.next() {
                if ch == '\\' {
                    chars.next();
                } else if ch == '"' {
                    end = offset;
                    break;
                }
            }
            let value = serde_json::from_str::<String>(&format!("\"{}\"", &content[..end])).ok()?;
            let offset = content.as_ptr() as usize - raw.as_ptr() as usize;
            return Some((
                DialogueName {
                    value,
                    span: span(
                        line,
                        raw[..offset].chars().count() + 1,
                        content[..end].chars().count(),
                    ),
                },
                offset,
            ));
        }
        let mut value = serde_json::Deserializer::from_str(rest).into_iter::<serde_json::Value>();
        value.next()?.ok()?;
        rest = rest
            .get(value.byte_offset()..)?
            .trim_start()
            .strip_prefix(',')?;
    }
}

pub(crate) fn cues(source: &str) -> Vec<DialogueName> {
    source
        .lines()
        .enumerate()
        .filter_map(|(index, raw)| cue_on_line(raw, index + 1).map(|(name, _)| name))
        .collect()
}

pub(crate) fn cue_at(source: &str, position: SourcePosition, prefix: bool) -> Option<DialogueName> {
    let raw = source.lines().nth(position.line.checked_sub(1)?)?;
    let (mut name, offset) = cue_on_line(raw, position.line)?;
    if position.column < name.span.start.column || position.column > name.span.end.column {
        return None;
    }
    if prefix {
        let count = position.column - name.span.start.column;
        let content: String = raw[offset..].chars().take(count).collect();
        name.value = serde_json::from_str(&format!("\"{content}\"")).ok()?;
    }
    Some(name)
}

pub fn replacement_span(source: &str, position: SourcePosition) -> Option<SourceSpan> {
    cue_at(source, position, false).map(|name| name.span)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marked(source: &str) -> (String, SourcePosition) {
        let (before, after) = source.split_once('|').unwrap();
        (
            format!("{before}{after}"),
            SourcePosition {
                line: 1,
                column: before.chars().count() + 1,
            },
        )
    }

    #[test]
    fn voice_cue_ranges_decode_json_and_preserve_quotes() {
        let (source, position) = marked(
            r#"  @voice {"sourceLocale":"é🦆", "cue":"guide.|\"hi\"", "fallback":"silent"}"#,
        );
        let name = cue_at(&source, position, false).unwrap();
        assert_eq!(name.value, "guide.\"hi\"");
        assert_eq!(cue_at(&source, position, true).unwrap().value, "guide.");
        let selected: String = source
            .chars()
            .skip(name.span.start.column - 1)
            .take(name.span.end.column - name.span.start.column)
            .collect();
        assert_eq!(selected, r#"guide.\"hi\""#);
        let (unfinished, cursor) = marked(r#"@voice {"cue":"guide.|"#);
        assert_eq!(cue_at(&unfinished, cursor, true).unwrap().value, "guide.");
        for text in [
            r#"@voice {"sourceLocale":"gu|ide", "cue":"real"}"#,
            r#"@voice {"extra":{"cue":"gu|ide"}, "cue":"real"}"#,
            r#"@voice {"extra":"\"cue\":\"gu|ide\"", "cue":"real"}"#,
            r#"// @voice {"cue":"gu|ide"}"#,
            r#": {"cue":"gu|ide"}"#,
            r#"@emit shown 0 event {"cue":"gu|ide"}"#,
        ] {
            let (source, position) = marked(text);
            assert!(cue_at(&source, position, false).is_none(), "{text}");
        }
    }
}

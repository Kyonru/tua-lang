//! Shared source ranges for typed action/event authoring, including incomplete buffers.
use crate::{SourcePosition, SourceSpan};

// Name completion and top-level JSON argument keys also work in incomplete buffers.
pub fn token_at(
    source: &str,
    position: SourcePosition,
) -> Option<(String, bool, String, SourceSpan)> {
    let line = source.lines().nth(position.line.checked_sub(1)?)?;
    let chars: Vec<_> = line.chars().collect();
    let cursor = position.column.checked_sub(1)?;
    if cursor > chars.len() {
        return None;
    }
    let name_index = if line.trim_start().starts_with("@action ") {
        1
    } else {
        3
    };
    if !line.trim_start().starts_with("@emit ") && name_index != 1 {
        return None;
    }
    let words: Vec<_> = line.split_whitespace().collect();
    let name = words.get(name_index).copied().unwrap_or("").to_owned();
    let mut start = cursor;
    while start > 0
        && (chars[start - 1].is_alphanumeric() || matches!(chars[start - 1], '_' | '.' | '-'))
    {
        start -= 1;
    }
    let mut end = cursor;
    while end < chars.len()
        && (chars[end].is_alphanumeric() || matches!(chars[end], '_' | '.' | '-'))
    {
        end += 1;
    }
    let before: String = chars[..start].iter().collect();
    let field = before.contains('{');
    if !field && before.split_whitespace().count() != name_index {
        return None;
    }
    let key_position = before
        .strip_suffix('"')
        .is_some_and(|prefix| prefix.trim_end().ends_with(['{', ',']));
    if field && (!key_position || !top_level_object(before.strip_suffix('"')?)) {
        return None;
    }
    let token: String = chars[start..end].iter().collect();
    Some((
        name,
        field,
        token,
        SourceSpan {
            start: SourcePosition {
                line: position.line,
                column: start + 1,
            },
            end: SourcePosition {
                line: position.line,
                column: end + 1,
            },
        },
    ))
}

fn top_level_object(source: &str) -> bool {
    let mut stack = Vec::new();
    let mut quoted = false;
    let mut escaped = false;
    for character in source.chars() {
        if quoted {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                quoted = false;
            }
        } else {
            match character {
                '"' => quoted = true,
                '{' | '[' => stack.push(character),
                '}' | ']' => {
                    stack.pop();
                }
                _ => {}
            }
        }
    }
    stack == ['{'] && !quoted
}

pub fn replacement_span(source: &str, position: SourcePosition) -> Option<SourceSpan> {
    token_at(source, position).map(|(_, _, _, span)| span)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at_marker(source: &str) -> Option<(String, bool, String, SourceSpan)> {
        let (before, after) = source.split_once('|').unwrap();
        token_at(
            &format!("{before}{after}"),
            SourcePosition {
                line: 1,
                column: before.chars().count() + 1,
            },
        )
    }

    #[test]
    fn contract_tokens_keep_whole_names_and_ignore_values_and_nested_keys() {
        let token = at_marker("  @action shop.gr|ant [id:a] [success:s] [failure:s] {}").unwrap();
        assert_eq!(token.0, "shop.grant");
        assert!(!token.1);
        assert_eq!(token.2, "shop.grant");
        assert_eq!(token.3.start.column, 11);
        assert_eq!(token.3.end.column, 21);
        assert_eq!(
            at_marker(r#"@emit shown 0 reward {"note":"é🦆", "co|ins":1}"#)
                .unwrap()
                .2,
            "coins"
        );
        assert_eq!(at_marker(r#"@emit shown 0 reward {"|"#).unwrap().2, "");
        for source in [
            r#"@emit shown 0 reward {"note":"co|ins"}"#,
            r#"@emit shown 0 reward {"note":"fake, \"co|ins"}"#,
            r#"@emit shown 0 reward {"child":{"co|ins":1}}"#,
            r#"@emit shown 0 reward {"list":[{"co|ins":1}]}"#,
            r#"// @emit shown 0 re|ward {}"#,
            r#": @action re|ward"#,
            r#"@action reward [id:re|ward] [success:s] [failure:s] {}"#,
        ] {
            assert!(at_marker(source).is_none(), "{source}");
        }
    }
}

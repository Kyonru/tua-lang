//! Non-executing context-path recovery shared by typed queries and editor edits.
use crate::{SourcePosition, SourceSpan};

fn condition_token(
    source: &str,
    position: SourcePosition,
    prefix: bool,
) -> Option<(String, SourceSpan)> {
    let line = source.lines().nth(position.line.checked_sub(1)?)?;
    let trimmed = line.trim_start();
    if !["@branch ", "@visible ", "@enabled "]
        .iter()
        .any(|p| trimmed.starts_with(p))
    {
        return None;
    }
    let chars: Vec<char> = line.chars().collect();
    let mut cursor = 0;
    while cursor < chars.len() {
        if chars[cursor] == '[' {
            // Branch target tags are flow labels, never context expressions.
            break;
        }
        if chars[cursor] == '"' {
            cursor += 1;
            while cursor < chars.len() {
                let ch = chars[cursor];
                cursor += 1;
                if ch == '\\' {
                    cursor = (cursor + 1).min(chars.len());
                } else if ch == '"' {
                    break;
                }
            }
        } else if chars[cursor].is_ascii_alphabetic() || chars[cursor] == '_' {
            let start = cursor;
            cursor += 1;
            while cursor < chars.len()
                && (chars[cursor].is_ascii_alphanumeric() || matches!(chars[cursor], '_' | '.'))
            {
                cursor += 1;
            }
            let token: String = chars[start..cursor].iter().collect();
            if (token.starts_with("context.") || token.starts_with("locals."))
                && position.column > start
                && position.column <= cursor + 1
            {
                let value = if prefix {
                    token.chars().take(position.column - start - 1).collect()
                } else {
                    token
                };
                return Some((
                    value,
                    SourceSpan {
                        start: SourcePosition {
                            line: position.line,
                            column: start + 1,
                        },
                        end: SourcePosition {
                            line: position.line,
                            column: cursor + 1,
                        },
                    },
                ));
            }
        } else {
            cursor += 1;
        }
    }
    None
}

pub fn token_at(
    source: &str,
    position: SourcePosition,
    prefix: bool,
) -> Option<(String, SourceSpan)> {
    if let Some(token) = condition_token(source, position, prefix) {
        return Some(token);
    }
    let line = source.lines().nth(position.line.checked_sub(1)?)?;
    let rest = line.trim_start().strip_prefix("@bind ")?.trim_start();
    let (_, rest) = rest.split_once(char::is_whitespace)?;
    let path = rest.split_whitespace().next()?;
    if !(path.starts_with("context.") || path.starts_with("locals."))
        || !path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.'))
    {
        return None;
    }
    let offset = line.len() - rest.trim_start().len();
    let start = line[..offset].chars().count() + 1;
    let end = start + path.chars().count();
    if position.column < start || position.column > end {
        return None;
    }
    Some((
        if prefix {
            path.chars().take(position.column - start).collect()
        } else {
            path.into()
        },
        SourceSpan {
            start: SourcePosition {
                line: position.line,
                column: start,
            },
            end: SourcePosition {
                line: position.line,
                column: end,
            },
        },
    ))
}

/// Replace only the member at the cursor, retaining its parent and trailing path.
pub fn replacement_span(source: &str, position: SourcePosition) -> Option<SourceSpan> {
    let (path, span) = token_at(source, position, false)?;
    let cursor = position.column.checked_sub(span.start.column)?;
    let start = path.get(..cursor)?.rfind('.')? + 1;
    let end = path
        .get(cursor..)?
        .find('.')
        .map_or(path.len(), |offset| cursor + offset);
    Some(SourceSpan {
        start: SourcePosition {
            line: position.line,
            column: span.start.column + start,
        },
        end: SourcePosition {
            line: position.line,
            column: span.start.column + end,
        },
    })
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
    fn context_path_ranges_preserve_parents_suffixes_and_unicode() {
        for text in [
            "@bind name context.player.na|me default \"friend\"",
            "@visible context.player.na|me != \"\"",
            "@enabled locals.na|me != nil",
            "@branch \"é🦆 [target:fake]\" != \"\" and context.player.na|me != nil [target:end] [else:end]",
            "@bind name context.na|me.first",
            "@bind name context.|name",
        ] {
            let (source, position) = marked(text);
            let range = replacement_span(&source, position).unwrap();
            let selected: String = source.chars().skip(range.start.column - 1).take(range.end.column - range.start.column).collect();
            assert_eq!(selected, "name", "{text}");
        }
        let (source, position) = marked("@enabled context.player.|");
        let range = replacement_span(&source, position).unwrap();
        assert_eq!(range.start, range.end);
    }

    #[test]
    fn context_paths_ignore_literals_targets_and_non_expression_fields() {
        for text in [
            "@branch true [target:context.na|me] [else:start]",
            "@branch true [target:start] [else:locals.na|me]",
            "@visible context.name == \"context.na|me\"",
            r#"@enabled "escaped \" context.na|me" == """#,
            "@bind name context.name default \"locals.na|me\"",
            "@bind context.na|me context.name",
            ": context.na|me",
            "// @visible context.na|me",
            "@action run [id:x] [success:s] [failure:s] {\"key\":\"context.na|me\"}",
        ] {
            let (source, position) = marked(text);
            assert!(token_at(&source, position, false).is_none(), "{text}");
            assert!(replacement_span(&source, position).is_none(), "{text}");
        }
    }
}

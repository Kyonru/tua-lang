use full_moon::ast::LuaVersion;
use full_moon::tokenizer::{Lexer, LexerResult, TokenType};

use crate::{SourcePosition, SourceSpan};

pub(crate) fn leading_doc_comment(source: &str, declaration_line: usize) -> Option<String> {
    let mut docs = Vec::new();
    let mut line_number = declaration_line.checked_sub(1)?;

    while line_number > 0 {
        let Some(line) = source.lines().nth(line_number - 1) else {
            break;
        };
        let trimmed = line.trim_start();

        if !trimmed.starts_with("---") {
            break;
        }

        if !trimmed.starts_with("---@") {
            let text = trimmed.trim_start_matches("---").trim_start();

            if !text.is_empty() {
                docs.push(text.to_owned());
            }
        }

        line_number -= 1;
    }

    docs.reverse();

    (!docs.is_empty()).then(|| docs.join("\n"))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DottedChain {
    pub parts: Vec<IdentifierPart>,
    pub cursor_index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IdentifierPart {
    pub text: String,
    pub span: SourceSpan,
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceIdentifier {
    pub text: String,
    pub span: SourceSpan,
}

/// Immutable byte-to-source-position index. Building it is linear in the
/// source size; every later lookup is logarithmic even for a single enormous
/// line containing many candidate declarations.
pub(crate) struct SourcePositionIndex {
    char_bytes: Vec<usize>,
    line_start_chars: Vec<usize>,
}

impl SourcePositionIndex {
    pub(crate) fn new(source: &str) -> Self {
        let mut char_bytes = Vec::with_capacity(source.len().saturating_add(1));
        let mut line_start_chars = vec![0];

        for (char_index, (byte, character)) in source.char_indices().enumerate() {
            char_bytes.push(byte);
            if character == '\n' {
                line_start_chars.push(char_index + 1);
            }
        }
        char_bytes.push(source.len());

        Self {
            char_bytes,
            line_start_chars,
        }
    }

    pub(crate) fn position(&self, byte: usize) -> SourcePosition {
        let char_index = self.char_bytes.partition_point(|offset| *offset < byte);
        let line_index = self
            .line_start_chars
            .partition_point(|start| *start <= char_index)
            .saturating_sub(1);

        SourcePosition {
            line: line_index + 1,
            column: char_index - self.line_start_chars[line_index] + 1,
        }
    }
}

pub(crate) fn source_identifiers(source: &str) -> Vec<SourceIdentifier> {
    let tokens = match Lexer::new(source, LuaVersion::luau()).collect() {
        LexerResult::Ok(tokens) | LexerResult::Recovered(tokens, _) => tokens,
        LexerResult::Fatal(_) => return Vec::new(),
    };

    tokens
        .into_iter()
        .filter_map(|token| {
            let TokenType::Identifier { identifier } = token.token_type() else {
                return None;
            };

            Some(SourceIdentifier {
                text: identifier.to_string(),
                span: SourceSpan::from_range(token.start_position(), token.end_position()),
            })
        })
        .collect()
}

pub(crate) fn line_at(source: &str, line: usize) -> Option<&str> {
    source
        .split('\n')
        .nth(line.checked_sub(1)?)
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
}

pub(crate) fn byte_for_column(line: &str, column: usize) -> usize {
    let target = column.saturating_sub(1);

    line.char_indices()
        .nth(target)
        .map(|(index, _)| index)
        .unwrap_or(line.len())
}

pub(crate) fn byte_for_position(source: &str, position: SourcePosition) -> Option<usize> {
    let mut offset = 0usize;

    for (line_index, line) in source.split_inclusive('\n').enumerate() {
        if line_index + 1 == position.line {
            let line = line.trim_end_matches(['\n', '\r']);
            return Some(offset + byte_for_column(line, position.column));
        }

        offset += line.len();
    }

    (position.line == source.lines().count() + 1).then_some(source.len())
}

pub(crate) fn column_for_byte(line: &str, byte: usize) -> usize {
    line[..byte.min(line.len())].chars().count() + 1
}

pub(crate) fn lookup_identifier_byte(line: &str, column: usize) -> Option<usize> {
    let byte = byte_for_column(line, column);
    let bytes = line.as_bytes();

    if byte < bytes.len() && is_identifier_continue(bytes[byte]) {
        return Some(byte);
    }

    byte.checked_sub(1)
        .filter(|previous| is_identifier_continue(bytes[*previous]))
}

pub(crate) fn dotted_chain_at_position(
    source: &str,
    position: SourcePosition,
) -> Option<DottedChain> {
    let line = line_at(source, position.line)?;
    let lookup_byte = lookup_identifier_byte(line, position.column)?;
    let mut index = 0;

    while index < line.len() {
        if !is_identifier_start(line.as_bytes()[index]) {
            index += 1;
            continue;
        }

        let mut parts = Vec::new();
        let mut current = index;

        loop {
            let start = current;
            let end = identifier_end(line, start);
            let span = SourceSpan {
                start: SourcePosition {
                    line: position.line,
                    column: column_for_byte(line, start),
                },
                end: SourcePosition {
                    line: position.line,
                    column: column_for_byte(line, end),
                },
            };

            parts.push(IdentifierPart {
                text: line[start..end].to_owned(),
                span,
                start_byte: start,
                end_byte: end,
            });

            current = end;

            if current >= line.len() || !matches!(line.as_bytes()[current], b'.' | b':') {
                break;
            }

            let next = current + 1;

            if next >= line.len() || !is_identifier_start(line.as_bytes()[next]) {
                break;
            }

            current = next;
        }

        if let Some(cursor_index) = parts
            .iter()
            .position(|part| lookup_byte >= part.start_byte && lookup_byte < part.end_byte)
        {
            return Some(DottedChain {
                parts,
                cursor_index,
            });
        }

        index = current.max(index + 1);
    }

    None
}

pub(crate) fn function_name_table_field(name: &str) -> Option<(String, String, bool)> {
    let colon = name.rfind(':');
    let dot = name.rfind('.');
    let separator = match (colon, dot) {
        (Some(colon), Some(dot)) => Some((colon.max(dot), colon > dot)),
        (Some(colon), None) => Some((colon, true)),
        (None, Some(dot)) => Some((dot, false)),
        (None, None) => None,
    }?;
    let owner = name[..separator.0].trim();
    let field = name[separator.0 + 1..].trim();

    (!owner.is_empty() && !field.is_empty())
        .then(|| (owner.to_owned(), field.to_owned(), separator.1))
}

#[cfg(test)]
pub(crate) fn position_at(source: &str, needle: &str) -> SourcePosition {
    let index = source.find(needle).expect("needle should exist in source");
    let before = &source[..index];
    let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = before
        .rsplit_once('\n')
        .map(|(_, line)| line.len())
        .unwrap_or(before.len())
        + 1;

    SourcePosition { line, column }
}

pub(crate) fn is_identifier_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

pub(crate) fn is_identifier_continue(byte: u8) -> bool {
    is_identifier_start(byte) || byte.is_ascii_digit()
}

pub(crate) fn is_identifier_continue_char(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

fn identifier_end(line: &str, start: usize) -> usize {
    line[start..]
        .char_indices()
        .find_map(|(offset, character)| {
            let index = start + offset;

            (!is_identifier_continue_char(character)).then_some(index)
        })
        .unwrap_or(line.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_position_index_handles_unicode_and_line_boundaries() {
        let source = "one\nλx\nlast";
        let positions = SourcePositionIndex::new(source);

        assert_eq!(
            positions.position(source.find('λ').unwrap()),
            SourcePosition { line: 2, column: 1 }
        );
        assert_eq!(
            positions.position(source.find('x').unwrap()),
            SourcePosition { line: 2, column: 2 }
        );
        assert_eq!(
            positions.position(source.len()),
            SourcePosition { line: 3, column: 5 }
        );
    }
}

use crate::{SourcePosition, SourceSpan};

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

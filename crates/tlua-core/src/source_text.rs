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

#[cfg(test)]
pub(crate) fn position_at(source: &str, needle: &str) -> crate::SourcePosition {
    let index = source.find(needle).expect("needle should exist in source");
    let before = &source[..index];
    let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = before
        .rsplit_once('\n')
        .map(|(_, line)| line.len())
        .unwrap_or(before.len())
        + 1;

    crate::SourcePosition { line, column }
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

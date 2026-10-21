//! Where inline completion may run, from source text alone.
//!
//! These rules need no parse: the document is usually invalid while a line is
//! being typed, so they lex only as far as the cursor.

/// What precedes the cursor on its line, which decides which inline sources
/// are worth asking.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CursorContext {
    /// Directly after `(`.
    OpenParen,
    /// Directly after `,`.
    Comma,
    /// Directly after a single `=` assignment.
    Assignment,
    /// Directly after another operator, such as `+`, `..`, or `==`.
    Operator,
    /// After whitespace that follows code on the same line.
    Whitespace,
    /// Directly after `.` or `:`.
    Member,
    /// Inside or at the end of the first word on the line, such as `loc`.
    StatementPrefix,
    /// Inside or at the end of a later word on the line.
    Identifier,
    /// After any other token, such as `{`, `[`, `]`, or a closing quote.
    Other,
}

impl CursorContext {
    pub const ALL: [Self; 9] = [
        Self::OpenParen,
        Self::Comma,
        Self::Assignment,
        Self::Operator,
        Self::Whitespace,
        Self::Member,
        Self::StatementPrefix,
        Self::Identifier,
        Self::Other,
    ];

    /// The context named by `label`, such as `open-paren`.
    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|context| context.label() == label)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::OpenParen => "open-paren",
            Self::Comma => "comma",
            Self::Assignment => "assignment",
            Self::Operator => "operator",
            Self::Whitespace => "whitespace",
            Self::Member => "member",
            Self::StatementPrefix => "statement-prefix",
            Self::Identifier => "identifier",
            Self::Other => "other",
        }
    }

    /// Whether VS Code would normally have its suggestion menu open here:
    /// while typing a word, or right after a member-access trigger.
    pub fn menu_may_be_open(self) -> bool {
        matches!(
            self,
            Self::Member | Self::StatementPrefix | Self::Identifier
        )
    }
}

/// The lexical region a byte offset falls in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LexicalRegion {
    Code,
    String,
    Comment,
}

/// The region at `offset`, lexing `source` from its start. An unterminated
/// short string ends at the newline, as Lua reports it.
pub fn lexical_region_at(source: &str, offset: usize) -> LexicalRegion {
    let bytes = source.as_bytes();
    let end = offset.min(bytes.len());
    let mut index = 0;

    while index < end {
        match bytes[index] {
            b'-' if bytes.get(index + 1) == Some(&b'-') => {
                if let Some(level) = long_bracket_level(bytes, index + 2) {
                    match find_long_bracket_close(bytes, index + 2 + level + 2, level) {
                        Some(close) if close <= end => index = close,
                        _ => return LexicalRegion::Comment,
                    }
                } else {
                    match memchr_newline(bytes, index) {
                        Some(newline) if newline < end => index = newline + 1,
                        _ => return LexicalRegion::Comment,
                    }
                }
            }
            b'[' => {
                if let Some(level) = long_bracket_level(bytes, index) {
                    match find_long_bracket_close(bytes, index + level + 2, level) {
                        Some(close) if close <= end => index = close,
                        _ => return LexicalRegion::String,
                    }
                } else {
                    index += 1;
                }
            }
            quote @ (b'"' | b'\'') => {
                let mut cursor = index + 1;
                loop {
                    if cursor >= end {
                        return LexicalRegion::String;
                    }
                    match bytes[cursor] {
                        b'\\' => cursor += 2,
                        b'\n' => break,
                        byte if byte == quote => break,
                        _ => cursor += 1,
                    }
                }
                index = cursor + 1;
            }
            _ => index += 1,
        }
    }

    LexicalRegion::Code
}

/// The cursor context for `offset`, or `None` when inline completion must not
/// run: inside a string or comment, or with nothing but indentation before
/// the cursor on its line.
pub fn cursor_context(source: &str, offset: usize) -> Option<CursorContext> {
    let offset = offset.min(source.len());
    if lexical_region_at(source, offset) != LexicalRegion::Code {
        return None;
    }

    let line_start = source[..offset].rfind('\n').map_or(0, |index| index + 1);
    let prefix = &source[line_start..offset];
    if prefix.trim().is_empty() {
        return None;
    }

    let last = prefix.chars().next_back()?;
    if last.is_whitespace() {
        return Some(CursorContext::Whitespace);
    }
    if is_identifier_char(last) {
        let word_start = prefix
            .char_indices()
            .rev()
            .take_while(|(_, character)| is_identifier_char(*character))
            .last()
            .map_or(prefix.len(), |(index, _)| index);
        return Some(if prefix[..word_start].trim().is_empty() {
            CursorContext::StatementPrefix
        } else {
            CursorContext::Identifier
        });
    }

    let before = prefix[..prefix.len() - last.len_utf8()].chars().next_back();
    Some(match last {
        '(' => CursorContext::OpenParen,
        ',' => CursorContext::Comma,
        '.' if before == Some('.') => CursorContext::Operator,
        '.' | ':' => CursorContext::Member,
        '=' if matches!(before, Some('=' | '<' | '>' | '~')) => CursorContext::Operator,
        '=' => CursorContext::Assignment,
        '+' | '-' | '*' | '/' | '%' | '^' | '#' | '<' | '>' => CursorContext::Operator,
        _ => CursorContext::Other,
    })
}

/// The word immediately before `offset`, which a menu item or snippet would
/// replace.
pub fn word_before(source: &str, offset: usize) -> &str {
    let offset = offset.min(source.len());
    let line_start = source[..offset].rfind('\n').map_or(0, |index| index + 1);
    let prefix = &source[line_start..offset];
    let start = prefix
        .char_indices()
        .rev()
        .take_while(|(_, character)| is_identifier_char(*character))
        .last()
        .map_or(prefix.len(), |(index, _)| index);

    &prefix[start..]
}

/// The closing brackets an editor with auto-closing pairs would have placed
/// after the cursor: one for each bracket opened earlier on the line and not
/// yet closed, innermost first. Brackets inside strings and comments don't
/// count.
pub fn auto_close_suffix(line_prefix: &str) -> String {
    let mut open = Vec::new();
    for (index, character) in line_prefix.char_indices() {
        let opens_long_bracket =
            character == '[' && matches!(line_prefix[index + 1..].chars().next(), Some('[' | '='));
        if !matches!(character, '(' | '[' | '{' | ')' | ']' | '}')
            || opens_long_bracket
            || lexical_region_at(line_prefix, index) != LexicalRegion::Code
        {
            continue;
        }
        match character {
            '(' => open.push(')'),
            '[' => open.push(']'),
            '{' => open.push('}'),
            closer => {
                if open.last() == Some(&closer) {
                    open.pop();
                } else {
                    // Unbalanced on this line; there is nothing to auto-close.
                    return String::new();
                }
            }
        }
    }
    open.iter().rev().collect()
}

/// Whether text after the cursor is nothing but closing delimiters and
/// whitespace, the only suffix inline completion may continue in front of.
pub fn suffix_allows_inline(suffix: &str) -> bool {
    suffix
        .chars()
        .all(|character| matches!(character, ')' | ']' | '}') || character.is_whitespace())
}

/// The code part of a line, without a trailing `--` comment or trailing
/// whitespace. Text inside strings is kept as written.
pub fn strip_trailing_comment(line: &str) -> &str {
    let mut cut = line.len();
    let mut search = 0;
    while let Some(found) = line[search..].find("--") {
        let at = search + found;
        if lexical_region_at(line, at) == LexicalRegion::Code {
            cut = at;
            break;
        }
        search = at + 2;
    }

    line[..cut].trim_end()
}

pub(crate) fn is_identifier_char(character: char) -> bool {
    character == '_' || character.is_alphanumeric()
}

/// The level of a long bracket opening at `index` (`[[` is level 0, `[==[`
/// level 2), or `None` when `index` does not open one.
fn long_bracket_level(bytes: &[u8], index: usize) -> Option<usize> {
    if bytes.get(index) != Some(&b'[') {
        return None;
    }
    let mut cursor = index + 1;
    while bytes.get(cursor) == Some(&b'=') {
        cursor += 1;
    }

    (bytes.get(cursor) == Some(&b'[')).then_some(cursor - index - 1)
}

/// The offset just after the `]=*]` closing a long bracket of `level`,
/// searching from `from`.
fn find_long_bracket_close(bytes: &[u8], from: usize, level: usize) -> Option<usize> {
    let mut index = from;
    while index < bytes.len() {
        if bytes[index] == b']' {
            let mut cursor = index + 1;
            let mut equals = 0;
            while bytes.get(cursor) == Some(&b'=') {
                cursor += 1;
                equals += 1;
            }
            if equals == level && bytes.get(cursor) == Some(&b']') {
                return Some(cursor + 1);
            }
        }
        index += 1;
    }

    None
}

fn memchr_newline(bytes: &[u8], from: usize) -> Option<usize> {
    bytes[from..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map(|position| from + position)
}

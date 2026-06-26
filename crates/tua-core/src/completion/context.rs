use crate::source_text::{byte_for_column, is_identifier_continue, is_identifier_start, line_at};
use crate::SourcePosition;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CompletionContext {
    AnnotationTag {
        prefix: String,
    },
    DocumentationSnippet {
        position: SourcePosition,
    },
    TypeAnnotation {
        prefix: String,
    },
    FieldAccess {
        receiver_parts: Vec<String>,
        prefix: String,
        position: SourcePosition,
        separator: FieldAccessSeparator,
        separator_span: crate::SourceSpan,
    },
    StringArgument {
        callee_parts: Vec<String>,
        active_parameter: usize,
        prefix: String,
        position: SourcePosition,
    },
    Identifier {
        prefix: String,
        position: SourcePosition,
    },
    RequirePath {
        prefix: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FieldAccessSeparator {
    Dot,
    Colon,
}

pub(crate) fn completion_context(
    source: &str,
    position: SourcePosition,
) -> Option<CompletionContext> {
    let line = line_at(source, position.line)?;
    let cursor = byte_for_column(line, position.column);

    annotation_tag_context(line, cursor)
        .or_else(|| documentation_snippet_context(line, cursor, position))
        .or_else(|| lua_doc_type_annotation_context(line, cursor))
        .or_else(|| require_path_context(line, cursor))
        .or_else(|| string_argument_context(source, line, cursor, position))
        .or_else(|| field_access_context(line, cursor, position))
        .or_else(|| type_annotation_context(line, cursor))
        .or_else(|| identifier_context(line, cursor, position))
}

fn annotation_tag_context(line: &str, cursor: usize) -> Option<CompletionContext> {
    let cursor = cursor.min(line.len());
    let prefix = &line[..cursor];
    let leading = prefix.len() - prefix.trim_start().len();
    let trimmed = &prefix[leading..];

    if !trimmed.starts_with("---@") {
        return None;
    }

    let tag_prefix = &trimmed[4..];

    if !tag_prefix
        .chars()
        .all(|character| character == '_' || character.is_ascii_alphabetic())
    {
        return None;
    }

    Some(CompletionContext::AnnotationTag {
        prefix: tag_prefix.to_owned(),
    })
}

fn documentation_snippet_context(
    line: &str,
    cursor: usize,
    position: SourcePosition,
) -> Option<CompletionContext> {
    let cursor = cursor.min(line.len());

    if line[..cursor].trim() != "---" {
        return None;
    }

    if !line[cursor..].trim().is_empty() {
        return None;
    }

    Some(CompletionContext::DocumentationSnippet { position })
}

fn lua_doc_type_annotation_context(line: &str, cursor: usize) -> Option<CompletionContext> {
    let cursor = cursor.min(line.len());
    let prefix = &line[..cursor];
    let leading = prefix.len() - prefix.trim_start().len();
    let trimmed = &prefix[leading..];

    if !trimmed.starts_with("---@") {
        return None;
    }

    let after_marker = &trimmed[4..];
    let mut parts = after_marker.split_whitespace();
    let tag = parts.next()?;
    let rest_start = after_marker.find(tag)? + tag.len();
    let rest = &after_marker[rest_start..];
    let rest = rest.trim_start();

    let type_text = match tag {
        "type" | "return" => rest,
        "param" | "field" => second_annotation_argument(rest)?,
        "alias" => second_annotation_argument(rest)?,
        "class" => rest.rsplit_once(':').map(|(_, ty)| ty.trim_start())?,
        "generic" => rest.rsplit_once(':').map(|(_, ty)| ty.trim_start())?,
        _ => return None,
    };
    let prefix = current_type_prefix(type_text);

    if !prefix
        .chars()
        .all(|character| character == '_' || character.is_ascii_alphanumeric())
    {
        return None;
    }

    Some(CompletionContext::TypeAnnotation {
        prefix: prefix.to_owned(),
    })
}

fn current_type_prefix(type_text: &str) -> &str {
    let type_text = type_text.trim_end();
    let start = reverse_identifier_start(type_text, type_text.len());

    &type_text[start..]
}

fn second_annotation_argument(rest: &str) -> Option<&str> {
    let trimmed = rest.trim_start();
    let first_len = trimmed.find(char::is_whitespace)?;

    Some(trimmed[first_len..].trim_start())
}

fn field_access_context(
    line: &str,
    cursor: usize,
    position: SourcePosition,
) -> Option<CompletionContext> {
    let cursor = cursor.min(line.len());
    let prefix_start = reverse_identifier_start(line, cursor);

    if prefix_start == 0 || !matches!(line.as_bytes().get(prefix_start - 1), Some(b'.' | b':')) {
        return None;
    }

    let receiver_end = prefix_start - 1;
    let separator = match line.as_bytes()[receiver_end] {
        b'.' => FieldAccessSeparator::Dot,
        b':' => FieldAccessSeparator::Colon,
        _ => return None,
    };
    let separator_column = line[..receiver_end].chars().count() + 1;
    let receiver_start = reverse_field_chain_start(line, receiver_end)?;
    let receiver = &line[receiver_start..receiver_end];
    let receiver_parts = receiver.split('.').map(str::to_owned).collect::<Vec<_>>();

    if receiver_parts.is_empty() || !receiver_parts.iter().all(|part| is_identifier(part)) {
        return None;
    }

    Some(CompletionContext::FieldAccess {
        receiver_parts,
        prefix: line[prefix_start..cursor].to_owned(),
        position,
        separator,
        separator_span: crate::SourceSpan {
            start: SourcePosition {
                line: position.line,
                column: separator_column,
            },
            end: SourcePosition {
                line: position.line,
                column: separator_column + 1,
            },
        },
    })
}

fn require_path_context(line: &str, cursor: usize) -> Option<CompletionContext> {
    let cursor = cursor.min(line.len());
    let prefix = &line[..cursor];
    let quote_start = prefix.rfind(['"', '\''])?;
    let quote = prefix.as_bytes()[quote_start];

    if prefix[quote_start + 1..].contains(quote as char) {
        return None;
    }

    let before_quote = prefix[..quote_start].trim_end();

    if before_quote.ends_with("require(") || before_quote.ends_with("require") {
        return Some(CompletionContext::RequirePath {
            prefix: prefix[quote_start + 1..].to_owned(),
        });
    }

    None
}

fn string_argument_context(
    source: &str,
    line: &str,
    cursor: usize,
    position: SourcePosition,
) -> Option<CompletionContext> {
    let cursor = cursor.min(line.len());

    if line[..cursor].trim_start().starts_with("--") {
        return None;
    }

    let (prefix, quote_start) = string_prefix_at(line, cursor)?;
    let cursor = byte_offset_for_position(source, position)?;
    let (callee_parts, active_parameter) = if let Some(open) = active_call_open(source, cursor) {
        (
            callee_parts_before(source, open)?,
            active_parameter_index(source, open + 1, cursor),
        )
    } else {
        let callee = callee_parts_before(line, quote_start)?;

        (callee, 0)
    };

    Some(CompletionContext::StringArgument {
        callee_parts,
        active_parameter,
        prefix,
        position,
    })
}

fn type_annotation_context(line: &str, cursor: usize) -> Option<CompletionContext> {
    let cursor = cursor.min(line.len());
    let prefix = &line[..cursor];
    let annotation_start = prefix.rfind([':', '|']).map(|index| index + 1)?;
    let marker = prefix[..annotation_start].chars().last()?;

    if marker == ':' && !looks_like_annotation_prefix(&prefix[..annotation_start - 1]) {
        return None;
    }

    if marker == '|' && !prefix[..annotation_start - 1].contains(':') {
        return None;
    }

    let typed_prefix = prefix[annotation_start..].trim_start();

    if !typed_prefix
        .chars()
        .all(|character| character == '_' || character.is_ascii_alphanumeric())
    {
        return None;
    }

    Some(CompletionContext::TypeAnnotation {
        prefix: typed_prefix.to_owned(),
    })
}

fn identifier_context(
    line: &str,
    cursor: usize,
    position: SourcePosition,
) -> Option<CompletionContext> {
    let cursor = cursor.min(line.len());

    if is_inside_string(line, cursor) {
        return None;
    }

    if line[..cursor].trim_start().starts_with("--") {
        return None;
    }

    let prefix_start = reverse_identifier_start(line, cursor);

    if prefix_start > 0 && matches!(line.as_bytes().get(prefix_start - 1), Some(b'.' | b':')) {
        return None;
    }

    if cursor < line.len() && is_identifier_continue(line.as_bytes()[cursor]) {
        return None;
    }

    Some(CompletionContext::Identifier {
        prefix: line[prefix_start..cursor].to_owned(),
        position,
    })
}

fn looks_like_annotation_prefix(prefix: &str) -> bool {
    let trimmed = prefix.trim_end();

    trimmed.contains("local ")
        || trimmed.contains("function ")
        || trimmed.contains("type ")
        || trimmed.ends_with(')')
        || trimmed.ends_with(',')
        || trimmed.ends_with('{')
}

fn reverse_identifier_start(line: &str, cursor: usize) -> usize {
    let mut start = cursor.min(line.len());

    while start > 0 && is_identifier_continue(line.as_bytes()[start - 1]) {
        start -= 1;
    }

    start
}

fn reverse_field_chain_start(line: &str, end: usize) -> Option<usize> {
    let mut start = end;

    while start > 0 {
        let byte = line.as_bytes()[start - 1];

        if byte == b'.' || is_identifier_continue(byte) {
            start -= 1;
        } else {
            break;
        }
    }

    Some(start).filter(|start| *start < end)
}

pub(super) fn is_identifier(value: &str) -> bool {
    let Some(first) = value.as_bytes().first() else {
        return false;
    };

    is_identifier_start(*first)
        && value
            .as_bytes()
            .iter()
            .skip(1)
            .all(|byte| is_identifier_continue(*byte))
}

fn is_inside_string(line: &str, cursor: usize) -> bool {
    let mut quote = None;
    let mut escaped = false;

    for character in line[..cursor.min(line.len())].chars() {
        if escaped {
            escaped = false;
            continue;
        }

        if character == '\\' {
            escaped = true;
            continue;
        }

        match quote {
            Some(current) if character == current => quote = None,
            None if character == '"' || character == '\'' => quote = Some(character),
            _ => {}
        }
    }

    quote.is_some()
}

fn string_prefix_at(line: &str, cursor: usize) -> Option<(String, usize)> {
    let mut quote = None;
    let mut escaped = false;
    let cursor = cursor.min(line.len());

    for (index, character) in line[..cursor].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        if character == '\\' {
            escaped = true;
            continue;
        }

        match quote {
            Some((current, _)) if character == current => quote = None,
            None if character == '"' || character == '\'' => quote = Some((character, index)),
            _ => {}
        }
    }

    let (_, start) = quote?;

    Some((line[start + 1..cursor].to_owned(), start))
}

fn byte_offset_for_position(source: &str, position: SourcePosition) -> Option<usize> {
    let mut current_line = 1;
    let mut line_start = 0;

    for (index, character) in source.char_indices() {
        if character != '\n' {
            continue;
        }

        if current_line == position.line {
            let line = source[line_start..index]
                .strip_suffix('\r')
                .unwrap_or(&source[line_start..index]);

            return Some(line_start + byte_for_column(line, position.column));
        }

        current_line += 1;
        line_start = index + 1;
    }

    if current_line == position.line {
        let line = source[line_start..]
            .strip_suffix('\r')
            .unwrap_or(&source[line_start..]);

        return Some(line_start + byte_for_column(line, position.column));
    }

    None
}

fn active_call_open(source: &str, cursor: usize) -> Option<usize> {
    let mut stack = Vec::new();
    let mut scanner = Scanner::new(source, cursor);

    while let Some((index, byte)) = scanner.next_code_byte() {
        match byte {
            b'(' => stack.push(index),
            b')' => {
                stack.pop();
            }
            _ => {}
        }
    }

    stack
        .into_iter()
        .rev()
        .find(|open| callee_parts_before(source, *open).is_some())
}

fn callee_parts_before(source: &str, open: usize) -> Option<Vec<String>> {
    let bytes = source.as_bytes();
    let mut end = trim_ascii_whitespace_before(bytes, open);
    let mut parts = Vec::new();

    loop {
        let start = reverse_identifier_start_bytes(bytes, end)?;

        parts.push(source[start..end].to_owned());
        end = trim_ascii_whitespace_before(bytes, start);

        if end == 0 || !matches!(bytes[end - 1], b'.' | b':') {
            break;
        }

        end = trim_ascii_whitespace_before(bytes, end - 1);
    }

    parts.reverse();
    (!parts.is_empty()).then_some(parts)
}

fn active_parameter_index(source: &str, start: usize, cursor: usize) -> usize {
    let mut active = 0;
    let mut depth = 0usize;
    let mut scanner = Scanner::new(
        &source[start.min(source.len())..],
        cursor.saturating_sub(start),
    );

    while let Some((_, byte)) = scanner.next_code_byte() {
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => active += 1,
            _ => {}
        }
    }

    active
}

fn trim_ascii_whitespace_before(bytes: &[u8], mut end: usize) -> usize {
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }

    end
}

fn reverse_identifier_start_bytes(bytes: &[u8], end: usize) -> Option<usize> {
    let mut start = end;

    while start > 0 && is_identifier_continue(bytes[start - 1]) {
        start -= 1;
    }

    (start < end && is_identifier_start(bytes[start])).then_some(start)
}

struct Scanner<'a> {
    source: &'a str,
    cursor: usize,
    index: usize,
    quote: Option<u8>,
    escaped: bool,
    line_comment: bool,
}

impl<'a> Scanner<'a> {
    fn new(source: &'a str, cursor: usize) -> Self {
        Self {
            source,
            cursor: cursor.min(source.len()),
            index: 0,
            quote: None,
            escaped: false,
            line_comment: false,
        }
    }

    fn next_code_byte(&mut self) -> Option<(usize, u8)> {
        let bytes = self.source.as_bytes();

        while self.index < self.cursor {
            let index = self.index;
            let byte = bytes[index];

            self.index += 1;

            if self.line_comment {
                if byte == b'\n' {
                    self.line_comment = false;
                }
                continue;
            }

            if let Some(quote) = self.quote {
                if self.escaped {
                    self.escaped = false;
                } else if byte == b'\\' {
                    self.escaped = true;
                } else if byte == quote {
                    self.quote = None;
                }
                continue;
            }

            if byte == b'-' && self.index < self.cursor && bytes[self.index] == b'-' {
                self.index += 1;
                self.line_comment = true;
                continue;
            }

            if byte == b'"' || byte == b'\'' {
                self.quote = Some(byte);
                continue;
            }

            return Some((index, byte));
        }

        None
    }
}

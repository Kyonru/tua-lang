use crate::source_text::{byte_for_column, is_identifier_continue, is_identifier_start, line_at};
use crate::SourcePosition;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CompletionContext {
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
    },
    Identifier {
        prefix: String,
        position: SourcePosition,
    },
    RequirePath {
        prefix: String,
    },
}

pub(super) fn completion_context(
    source: &str,
    position: SourcePosition,
) -> Option<CompletionContext> {
    let line = line_at(source, position.line)?;
    let cursor = byte_for_column(line, position.column);

    annotation_tag_context(line, cursor)
        .or_else(|| documentation_snippet_context(line, cursor, position))
        .or_else(|| lua_doc_type_annotation_context(line, cursor))
        .or_else(|| require_path_context(line, cursor))
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

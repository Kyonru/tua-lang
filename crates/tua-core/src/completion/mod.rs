use std::collections::BTreeMap;
use std::path::Path;

use crate::semantic::{resolve_aliases, resolve_field_chain_type};
use crate::source_text::{byte_for_column, is_identifier_continue, is_identifier_start, line_at};
use crate::types::{build_type_environment_with_diagnostics, TableShape, Type, TypeEnvironment};
use crate::{parse_tua_source, Diagnostic, ParsedSource, SourcePosition};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Completion {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionKind {
    TypeAlias,
    Field,
}

pub fn complete_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    position: SourcePosition,
) -> Result<Vec<Completion>, Vec<Diagnostic>> {
    let path = path.as_ref().to_path_buf();
    let source = source.into();
    let Some(context) = completion_context(&source, position) else {
        return Ok(Vec::new());
    };
    let parsed = parse_completion_source(&path, &source, position)?;

    Ok(complete_parsed_source_with_context(&parsed, context))
}

pub fn complete_parsed_source(
    parsed: &ParsedSource,
    position: SourcePosition,
) -> Result<Vec<Completion>, Vec<Diagnostic>> {
    let Some(context) = completion_context(&parsed.source, position) else {
        return Ok(Vec::new());
    };

    Ok(complete_parsed_source_with_context(parsed, context))
}

fn complete_parsed_source_with_context(
    parsed: &ParsedSource,
    context: CompletionContext,
) -> Vec<Completion> {
    match context {
        CompletionContext::TypeAnnotation { prefix } => {
            complete_type_aliases(parsed, prefix.as_str())
        }
        CompletionContext::FieldAccess {
            receiver_parts,
            prefix,
        } => {
            let environment = build_completion_environment(parsed);

            complete_fields(&environment, &receiver_parts, prefix.as_str())
        }
    }
}

fn complete_type_aliases(parsed: &ParsedSource, prefix: &str) -> Vec<Completion> {
    parsed
        .type_aliases
        .iter()
        .filter(|alias| alias.name.starts_with(prefix))
        .map(|alias| Completion {
            label: alias.name.clone(),
            kind: CompletionKind::TypeAlias,
            detail: Some("type alias".to_owned()),
        })
        .collect()
}

fn complete_fields(
    environment: &TypeEnvironment,
    receiver_parts: &[String],
    prefix: &str,
) -> Vec<Completion> {
    let Some(receiver_type) = resolve_field_chain_type(environment, receiver_parts) else {
        return Vec::new();
    };
    let mut fields = BTreeMap::new();

    collect_table_fields(environment, &receiver_type, &mut fields);

    fields
        .into_iter()
        .filter(|(name, _)| name.starts_with(prefix))
        .map(|(name, detail)| Completion {
            label: name,
            kind: CompletionKind::Field,
            detail: Some(detail),
        })
        .collect()
}

fn collect_table_fields(
    environment: &TypeEnvironment,
    ty: &Type,
    fields: &mut BTreeMap<String, String>,
) {
    match resolve_aliases(environment, ty, 0) {
        Type::Table(table) => collect_shape_fields(&table, fields),
        Type::Union(types) => {
            for ty in types {
                collect_table_fields(environment, &ty, fields);
            }
        }
        Type::Any | Type::Alias(_) | Type::Function(_) | Type::Primitive(_) => {}
    }
}

fn collect_shape_fields(table: &TableShape, fields: &mut BTreeMap<String, String>) {
    for (name, field) in table.fields() {
        let suffix = if field.optional { " optional" } else { "" };

        fields.insert(name.to_owned(), format!("field{suffix}: {}", field.ty));
    }
}

fn build_completion_environment(parsed: &ParsedSource) -> TypeEnvironment {
    let (environment, _) = build_type_environment_with_diagnostics(parsed);

    environment
}

fn parse_completion_source(
    path: &Path,
    source: &str,
    position: SourcePosition,
) -> Result<ParsedSource, Vec<Diagnostic>> {
    parse_tua_source(path, source.to_owned())
        .or_else(|_| parse_tua_source(path, source_with_line_removed(source, position.line)))
}

fn source_with_line_removed(source: &str, line_to_remove: usize) -> String {
    let mut output = String::with_capacity(source.len());
    let mut current_line = 1;
    let mut line_start = 0;

    for (index, character) in source.char_indices() {
        if character != '\n' {
            continue;
        }

        if current_line == line_to_remove {
            output.push('\n');
        } else {
            output.push_str(&source[line_start..=index]);
        }

        current_line += 1;
        line_start = index + 1;
    }

    if line_start < source.len() {
        if current_line == line_to_remove {
            output.push('\n');
        } else {
            output.push_str(&source[line_start..]);
        }
    }

    output
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CompletionContext {
    TypeAnnotation {
        prefix: String,
    },
    FieldAccess {
        receiver_parts: Vec<String>,
        prefix: String,
    },
}

fn completion_context(source: &str, position: SourcePosition) -> Option<CompletionContext> {
    let line = line_at(source, position.line)?;
    let cursor = byte_for_column(line, position.column);

    field_access_context(line, cursor).or_else(|| type_annotation_context(line, cursor))
}

fn field_access_context(line: &str, cursor: usize) -> Option<CompletionContext> {
    let cursor = cursor.min(line.len());
    let prefix_start = reverse_identifier_start(line, cursor);

    if prefix_start == 0 || line.as_bytes().get(prefix_start - 1) != Some(&b'.') {
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

fn is_identifier(value: &str) -> bool {
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

#[cfg(test)]
mod tests;

use std::path::Path;

use crate::types::{build_type_environment, Type, TypeEnvironment, TypedFunction};
use crate::{parse_tlua_source, Diagnostic, ParsedSource, SourcePosition, SourceSpan};

const MAX_TYPE_RESOLUTION_DEPTH: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Hover {
    pub contents: String,
    pub span: Option<SourceSpan>,
}

pub fn hover_tlua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    position: SourcePosition,
) -> Result<Option<Hover>, Vec<Diagnostic>> {
    let parsed = parse_tlua_source(path, source)?;

    hover_parsed_source(&parsed, position)
}

pub fn hover_parsed_source(
    parsed: &ParsedSource,
    position: SourcePosition,
) -> Result<Option<Hover>, Vec<Diagnostic>> {
    let environment = build_type_environment(parsed)?;
    let resolver = HoverResolver {
        parsed,
        environment: &environment,
    };

    Ok(resolver.hover(position))
}

struct HoverResolver<'a> {
    parsed: &'a ParsedSource,
    environment: &'a TypeEnvironment,
}

impl<'a> HoverResolver<'a> {
    fn hover(&self, position: SourcePosition) -> Option<Hover> {
        let chain = dotted_chain_at_position(&self.parsed.source, position)?;
        let current_part = chain.parts.get(chain.cursor_index)?;

        if chain.cursor_index > 0 {
            let field_parts = chain
                .parts
                .iter()
                .take(chain.cursor_index + 1)
                .map(|part| part.text.clone())
                .collect::<Vec<_>>();

            if let Some(ty) = self.resolve_field_chain_type(&field_parts) {
                return Some(Hover {
                    contents: format!("(field) {}: {ty}", current_part.text),
                    span: Some(current_part.span),
                });
            }
        }

        if let Some(ty) = self.environment.alias(&current_part.text) {
            return Some(Hover {
                contents: format!("type {} = {ty}", current_part.text),
                span: Some(current_part.span),
            });
        }

        if let Some(ty) = self.environment.local(&current_part.text) {
            return Some(Hover {
                contents: format!("local {}: {ty}", current_part.text),
                span: Some(current_part.span),
            });
        }

        self.environment
            .function(&current_part.text)
            .map(|function| Hover {
                contents: format_function(function),
                span: Some(current_part.span),
            })
    }

    fn resolve_field_chain_type(&self, parts: &[String]) -> Option<Type> {
        let (root, fields) = parts.split_first()?;
        let mut current = self.environment.local(root)?.clone();

        for field in fields {
            current = self.resolve_table_field_type(&current, field)?;
        }

        Some(current)
    }

    fn resolve_table_field_type(&self, ty: &Type, field: &str) -> Option<Type> {
        match resolve_aliases(ty, self.environment, 0) {
            Type::Table(table) => table.field(field).map(|field| field.ty.clone()),
            Type::Union(types) => {
                let mut resolved = Vec::new();

                for ty in types {
                    if let Some(field_type) = self.resolve_table_field_type(&ty, field) {
                        resolved.push(field_type);
                    }
                }

                match resolved.len() {
                    0 => None,
                    1 => resolved.into_iter().next(),
                    _ => Some(Type::union(resolved)),
                }
            }
            _ => None,
        }
    }
}

fn format_function(function: &TypedFunction) -> String {
    let name = function.name.as_deref().unwrap_or("<anonymous>");
    let mut parameters = function
        .parameters
        .iter()
        .map(|parameter| format!("{}: {}", parameter.name, parameter.ty))
        .collect::<Vec<_>>();

    if function.has_varargs {
        parameters.push("...".to_owned());
    }

    format!(
        "function {name}({}): {}",
        parameters.join(", "),
        function.return_type_or_any()
    )
}

fn resolve_aliases(ty: &Type, environment: &TypeEnvironment, depth: usize) -> Type {
    if depth >= MAX_TYPE_RESOLUTION_DEPTH {
        return ty.clone();
    }

    match ty {
        Type::Alias(name) => environment
            .alias(name)
            .map(|resolved| resolve_aliases(resolved, environment, depth + 1))
            .unwrap_or_else(|| ty.clone()),
        _ => ty.clone(),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DottedChain {
    parts: Vec<ChainPart>,
    cursor_index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ChainPart {
    text: String,
    span: SourceSpan,
    start_byte: usize,
    end_byte: usize,
}

fn dotted_chain_at_position(source: &str, position: SourcePosition) -> Option<DottedChain> {
    let line = source_line(source, position.line)?;
    let lookup_byte = lookup_byte(line, position.column)?;
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

            parts.push(ChainPart {
                text: line[start..end].to_owned(),
                span,
                start_byte: start,
                end_byte: end,
            });

            current = end;

            if current >= line.len() || line.as_bytes()[current] != b'.' {
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

fn source_line(source: &str, line: usize) -> Option<&str> {
    source
        .split('\n')
        .nth(line.checked_sub(1)?)
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
}

fn lookup_byte(line: &str, column: usize) -> Option<usize> {
    let byte = byte_for_column(line, column);
    let bytes = line.as_bytes();

    if byte < bytes.len() && is_identifier_continue(bytes[byte]) {
        return Some(byte);
    }

    byte.checked_sub(1)
        .filter(|previous| is_identifier_continue(bytes[*previous]))
}

fn byte_for_column(line: &str, column: usize) -> usize {
    let target = column.saturating_sub(1);

    line.char_indices()
        .nth(target)
        .map(|(index, _)| index)
        .unwrap_or(line.len())
}

fn column_for_byte(line: &str, byte: usize) -> usize {
    line[..byte.min(line.len())].chars().count() + 1
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

fn is_identifier_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

fn is_identifier_continue(byte: u8) -> bool {
    is_identifier_start(byte) || byte.is_ascii_digit()
}

fn is_identifier_continue_char(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests;

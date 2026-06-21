use std::path::{Path, PathBuf};

use crate::parser::{ParsedSource, SourcePosition, SourceSpan, TuaType};
use crate::source_text::{byte_for_column, dotted_chain_at_position, line_at};
use crate::types::{TableField, Type};
use crate::{parse_tua_source, Diagnostic};

const MAX_FIELD_DEFINITION_DEPTH: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Definition {
    pub path: PathBuf,
    pub span: SourceSpan,
    pub kind: DefinitionKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefinitionKind {
    TypeAlias,
    Local,
    Function,
    Parameter,
    TableField,
}

pub fn definition_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    position: SourcePosition,
) -> Result<Option<Definition>, Vec<Diagnostic>> {
    let parsed = parse_tua_source(path, source)?;

    definition_parsed_source(&parsed, position)
}

pub fn definition_parsed_source(
    parsed: &ParsedSource,
    position: SourcePosition,
) -> Result<Option<Definition>, Vec<Diagnostic>> {
    let resolver = DefinitionResolver { parsed };

    Ok(resolver.definition(position))
}

struct DefinitionResolver<'a> {
    parsed: &'a ParsedSource,
}

impl<'a> DefinitionResolver<'a> {
    fn definition(&self, position: SourcePosition) -> Option<Definition> {
        let chain = dotted_chain_at_position(&self.parsed.source, position)?;
        let current_part = chain.parts.get(chain.cursor_index)?;

        if chain.cursor_index > 0 {
            let parts = chain
                .parts
                .iter()
                .take(chain.cursor_index + 1)
                .map(|part| part.text.clone())
                .collect::<Vec<_>>();

            if let Some(definition) = self.field_definition(&parts, position) {
                return Some(definition);
            }
        }

        if self.is_type_reference(position) {
            return self
                .alias_definition(&current_part.text)
                .or_else(|| self.value_definition(&current_part.text, position));
        }

        self.value_definition(&current_part.text, position)
            .or_else(|| self.alias_definition(&current_part.text))
    }

    fn value_definition(&self, name: &str, position: SourcePosition) -> Option<Definition> {
        let mut candidates = Vec::new();

        candidates.extend(self.parsed.local_annotations.iter().filter_map(|local| {
            let span = local.name_span.or(local.span)?;

            (local.name == name && starts_before_or_at(span, position)).then(|| Definition {
                path: self.parsed.path.clone(),
                span,
                kind: DefinitionKind::Local,
            })
        }));

        for function in &self.parsed.function_annotations {
            if let Some(span) = function.name_span.or(function.span) {
                if function.name.as_deref() == Some(name) && starts_before_or_at(span, position) {
                    candidates.push(Definition {
                        path: self.parsed.path.clone(),
                        span,
                        kind: DefinitionKind::Function,
                    });
                }
            }

            candidates.extend(function.parameters.iter().filter_map(|parameter| {
                let span = parameter.name_span.or(parameter.span)?;

                (parameter.name == name && starts_before_or_at(span, position)).then(|| {
                    Definition {
                        path: self.parsed.path.clone(),
                        span,
                        kind: DefinitionKind::Parameter,
                    }
                })
            }));
        }

        candidates
            .into_iter()
            .max_by_key(|definition| position_key(definition.span.start))
    }

    fn alias_definition(&self, name: &str) -> Option<Definition> {
        self.parsed
            .type_aliases
            .iter()
            .find(|alias| alias.name == name)
            .and_then(|alias| {
                let span = alias.name_span.or(alias.span)?;

                Some(Definition {
                    path: self.parsed.path.clone(),
                    span,
                    kind: DefinitionKind::TypeAlias,
                })
            })
    }

    fn is_type_reference(&self, position: SourcePosition) -> bool {
        let Some(line) = line_at(&self.parsed.source, position.line) else {
            return false;
        };
        let cursor = byte_for_column(line, position.column).min(line.len());
        let prefix = &line[..cursor];
        let Some(marker_index) = prefix.rfind([':', '|', '=']) else {
            return false;
        };
        let marker = prefix.as_bytes()[marker_index];
        let before_marker = &prefix[..marker_index];

        match marker {
            b':' => true,
            b'|' => before_marker.contains(':') || before_marker.contains("type "),
            b'=' => before_marker.contains("type "),
            _ => false,
        }
    }

    fn field_definition(&self, parts: &[String], position: SourcePosition) -> Option<Definition> {
        let (root, fields) = parts.split_first()?;
        let mut current = self.value_type(root, position)?;

        for (index, field) in fields.iter().enumerate() {
            let target = self.field_target(&current, field, 0)?;

            current = target.ty;

            if index == fields.len() - 1 {
                return Some(Definition {
                    path: self.parsed.path.clone(),
                    span: target.span,
                    kind: DefinitionKind::TableField,
                });
            }
        }

        None
    }

    fn value_type(&self, name: &str, position: SourcePosition) -> Option<Type> {
        let mut candidates = Vec::new();

        candidates.extend(self.parsed.local_annotations.iter().filter_map(|local| {
            let span = local.name_span.or(local.span)?;

            (local.name == name && starts_before_or_at(span, position))
                .then(|| (span.start, type_from_tua_type(&local.ty)))
        }));

        for function in &self.parsed.function_annotations {
            candidates.extend(function.parameters.iter().filter_map(|parameter| {
                let span = parameter.name_span.or(parameter.span)?;

                (parameter.name == name && starts_before_or_at(span, position))
                    .then(|| (span.start, type_from_tua_type(&parameter.ty)))
            }));
        }

        candidates
            .into_iter()
            .max_by_key(|(position, _)| position_key(*position))
            .map(|(_, ty)| ty)
    }

    fn field_target(&self, ty: &Type, field: &str, depth: usize) -> Option<FieldTarget> {
        if depth >= MAX_FIELD_DEFINITION_DEPTH {
            return None;
        }

        match ty {
            Type::Alias(name) => {
                let alias = self
                    .parsed
                    .type_aliases
                    .iter()
                    .find(|alias| alias.name == *name)?;

                match &alias.ty {
                    TuaType::Table(fields) => fields
                        .iter()
                        .find(|candidate| candidate.name == field)
                        .and_then(|candidate| {
                            let span = candidate.name_span.or(candidate.span)?;

                            Some(FieldTarget {
                                span,
                                ty: type_from_tua_type(&candidate.ty),
                            })
                        }),
                    other => self.field_target(&type_from_tua_type(other), field, depth + 1),
                }
            }
            Type::Union(types) => types
                .iter()
                .find_map(|ty| self.field_target(ty, field, depth + 1)),
            Type::Any | Type::Function(_) | Type::Primitive(_) | Type::Table(_) => None,
        }
    }
}

struct FieldTarget {
    span: SourceSpan,
    ty: Type,
}

fn type_from_tua_type(ty: &TuaType) -> Type {
    match ty {
        TuaType::Named(name) => Type::named(name),
        TuaType::Union(types) => Type::union(types.iter().map(type_from_tua_type)),
        TuaType::Table(fields) => Type::table(fields.iter().map(|field| {
            let field_type = type_from_tua_type(&field.ty);
            let table_field = if field.optional {
                TableField::optional(field_type)
            } else {
                TableField::required(field_type)
            };

            (field.name.clone(), table_field)
        })),
    }
}

fn starts_before_or_at(span: SourceSpan, position: SourcePosition) -> bool {
    position_key(span.start) <= position_key(position)
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

#[cfg(test)]
mod tests;

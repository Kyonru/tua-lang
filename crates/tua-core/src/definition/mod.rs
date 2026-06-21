use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use full_moon::ast::{
    self, Expression, Field, FunctionArgs, Index, LastStmt, Prefix, Suffix, TableConstructor, Var,
};
use full_moon::node::Node;

use crate::module_index::resolve_required_module;
use crate::module_link::require_module_at_position;
use crate::parser::{parse_lua_source, ParsedSource, SourcePosition, SourceSpan, TuaType};
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
    Module,
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
    let path = path.as_ref().to_path_buf();
    let source = source.into();
    let parsed = parse_definition_source(&path, &source, position)?;

    if let Some(definition) = require_module_definition_at_position(&parsed, position) {
        return Ok(Some(definition));
    }

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

            if let Some(definition) = self.required_module_field_definition(&parts, position) {
                return Some(definition);
            }

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
            .or_else(|| self.required_module_definition(&current_part.text, position))
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

        candidates.extend(self.local_assignment_definitions(name, position));

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

    fn local_assignment_definitions(
        &self,
        name: &str,
        position: SourcePosition,
    ) -> Vec<Definition> {
        self.parsed
            .ast
            .nodes()
            .stmts()
            .filter_map(|stmt| {
                let ast::Stmt::LocalAssignment(local) = stmt else {
                    return None;
                };

                Some(local.names().iter().filter_map(|local_name| {
                    let span = node_span(local_name)?;

                    (token_text(local_name) == name && starts_before_or_at(span, position)).then(
                        || Definition {
                            path: self.parsed.path.clone(),
                            span,
                            kind: DefinitionKind::Local,
                        },
                    )
                }))
            })
            .flatten()
            .collect()
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

    fn required_module_definition(
        &self,
        name: &str,
        position: SourcePosition,
    ) -> Option<Definition> {
        let binding = self.require_binding(name, position)?;
        let path = resolve_required_module(&self.parsed.path, &binding.module)?;

        module_file_definition(&path)
    }

    fn required_module_field_definition(
        &self,
        parts: &[String],
        position: SourcePosition,
    ) -> Option<Definition> {
        let (root, fields) = parts.split_first()?;
        let field = fields.first()?;
        let binding = self.require_binding(root, position)?;
        let path = resolve_required_module(&self.parsed.path, &binding.module)?;
        let parsed = parse_module_file(&path)?;

        exported_field_definition(&parsed, field).or_else(|| module_file_definition(&path))
    }

    fn require_binding(&self, name: &str, position: SourcePosition) -> Option<RequireBinding> {
        require_bindings(self.parsed)
            .into_iter()
            .filter(|binding| binding.receiver == name)
            .filter(|binding| starts_before_or_at(binding.receiver_span, position))
            .max_by_key(|binding| position_key(binding.receiver_span.start))
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

#[derive(Clone, Debug)]
struct RequireBinding {
    receiver: String,
    receiver_span: SourceSpan,
    module: String,
}

fn require_module_definition_at_position(
    parsed: &ParsedSource,
    position: SourcePosition,
) -> Option<Definition> {
    let module = require_module_at_position(&parsed.source, position)?;
    let path = resolve_required_module(&parsed.path, &module)?;

    module_file_definition(&path)
}

fn require_bindings(parsed: &ParsedSource) -> Vec<RequireBinding> {
    let mut bindings = Vec::new();

    for stmt in parsed.ast.nodes().stmts() {
        let ast::Stmt::LocalAssignment(local) = stmt else {
            continue;
        };

        for (name, expression) in local.names().iter().zip(local.expressions()) {
            let Some(module) = require_module_from_expression(expression) else {
                continue;
            };
            let Some(receiver_span) = node_span(name) else {
                continue;
            };

            bindings.push(RequireBinding {
                receiver: token_text(name),
                receiver_span,
                module,
            });
        }
    }

    bindings
}

fn require_module_from_expression(expression: &Expression) -> Option<String> {
    let Expression::FunctionCall(call) = expression else {
        return None;
    };
    let Prefix::Name(name) = call.prefix() else {
        return None;
    };

    if token_text(name) != "require" {
        return None;
    }

    let first_call = call.suffixes().find_map(|suffix| match suffix {
        Suffix::Call(call) => Some(call),
        _ => None,
    })?;
    let ast::Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }) = first_call else {
        return None;
    };
    let Expression::String(module) = arguments.iter().next()? else {
        return None;
    };

    Some(unquote_string(&module.to_string()))
}

fn parse_module_file(path: &Path) -> Option<ParsedSource> {
    let source = fs::read_to_string(path).ok()?;

    match path.extension().and_then(|extension| extension.to_str()) {
        Some("lua") => parse_lua_source(path, source).ok(),
        _ => parse_tua_source(path, source).ok(),
    }
}

fn module_file_definition(path: &Path) -> Option<Definition> {
    let parsed = parse_module_file(path)?;
    let span = first_module_span(&parsed).unwrap_or_else(default_span);

    Some(Definition {
        path: path.to_path_buf(),
        span,
        kind: DefinitionKind::Module,
    })
}

fn first_module_span(parsed: &ParsedSource) -> Option<SourceSpan> {
    parsed
        .type_aliases
        .iter()
        .filter_map(|alias| alias.name_span.or(alias.span))
        .chain(
            parsed
                .function_annotations
                .iter()
                .filter_map(|function| function.name_span.or(function.span)),
        )
        .chain(
            parsed
                .local_annotations
                .iter()
                .filter_map(|local| local.name_span.or(local.span)),
        )
        .min_by_key(|span| position_key(span.start))
}

fn exported_field_definition(parsed: &ParsedSource, field: &str) -> Option<Definition> {
    let index = ModuleDefinitionIndex::new(parsed);

    match returned_expression(parsed)? {
        Expression::TableConstructor(table) => {
            exported_table_field_definition(parsed, &index, table, field)
        }
        Expression::Var(Var::Name(name)) => {
            returned_local_field_definition(parsed, &index, &token_text(name), field)
        }
        Expression::Parentheses { expression, .. } => match expression.as_ref() {
            Expression::TableConstructor(table) => {
                exported_table_field_definition(parsed, &index, table, field)
            }
            Expression::Var(Var::Name(name)) => {
                returned_local_field_definition(parsed, &index, &token_text(name), field)
            }
            _ => None,
        },
        _ => None,
    }
}

fn returned_expression(parsed: &ParsedSource) -> Option<&Expression> {
    let LastStmt::Return(return_stmt) = parsed.ast.nodes().last_stmt()? else {
        return None;
    };

    return_stmt.returns().iter().next()
}

fn exported_table_field_definition(
    parsed: &ParsedSource,
    index: &ModuleDefinitionIndex,
    table: &TableConstructor,
    field: &str,
) -> Option<Definition> {
    for table_field in table.fields() {
        let Field::NameKey { key, value, .. } = table_field else {
            continue;
        };

        if token_text(key) != field {
            continue;
        }

        if let Expression::Var(Var::Name(name)) = value {
            if let Some(definition) = index.value(&token_text(name)) {
                return Some(definition);
            }
        }

        let span = node_span(key).or_else(|| node_span(table_field))?;

        return Some(Definition {
            path: parsed.path.clone(),
            span,
            kind: DefinitionKind::TableField,
        });
    }

    None
}

fn returned_local_field_definition(
    parsed: &ParsedSource,
    index: &ModuleDefinitionIndex,
    local: &str,
    field: &str,
) -> Option<Definition> {
    index
        .table_member(local, field)
        .or_else(|| {
            index
                .local_table_field(local, field)
                .map(|span| Definition {
                    path: parsed.path.clone(),
                    span,
                    kind: DefinitionKind::TableField,
                })
        })
        .or_else(|| {
            let ty = index.local_type(local)?;
            let target = ModuleDefinitionIndex::field_target(parsed, ty, field, 0)?;

            Some(Definition {
                path: parsed.path.clone(),
                span: target.span,
                kind: DefinitionKind::TableField,
            })
        })
}

struct ModuleDefinitionIndex<'a> {
    parsed: &'a ParsedSource,
    values: BTreeMap<String, Definition>,
    table_members: BTreeMap<(String, String), Definition>,
    local_tables: BTreeMap<String, BTreeMap<String, SourceSpan>>,
    local_types: BTreeMap<String, Type>,
}

impl<'a> ModuleDefinitionIndex<'a> {
    fn new(parsed: &'a ParsedSource) -> Self {
        let mut index = Self {
            parsed,
            values: BTreeMap::new(),
            table_members: BTreeMap::new(),
            local_tables: BTreeMap::new(),
            local_types: BTreeMap::new(),
        };

        index.collect_annotations();
        index.collect_statements();
        index
    }

    fn collect_annotations(&mut self) {
        for local in &self.parsed.local_annotations {
            let Some(span) = local.name_span.or(local.span) else {
                continue;
            };

            self.values.entry(local.name.clone()).or_insert(Definition {
                path: self.parsed.path.clone(),
                span,
                kind: DefinitionKind::Local,
            });
            self.local_types
                .insert(local.name.clone(), type_from_tua_type(&local.ty));
        }

        for function in &self.parsed.function_annotations {
            let Some(name) = &function.name else {
                continue;
            };
            let Some(span) = function.name_span.or(function.span) else {
                continue;
            };
            let definition = Definition {
                path: self.parsed.path.clone(),
                span,
                kind: DefinitionKind::Function,
            };

            if let Some((table, field)) = name.split_once('.') {
                self.table_members
                    .entry((table.trim().to_owned(), field.trim().to_owned()))
                    .or_insert(definition);
            } else {
                self.values.entry(name.clone()).or_insert(definition);
            }
        }
    }

    fn collect_statements(&mut self) {
        for stmt in self.parsed.ast.nodes().stmts() {
            match stmt {
                ast::Stmt::Assignment(assignment) => {
                    for variable in assignment.variables() {
                        let Some(parts) = chain_parts_from_var(variable) else {
                            continue;
                        };

                        if let [table, field] = parts.as_slice() {
                            self.table_members
                                .entry((table.text.clone(), field.text.clone()))
                                .or_insert(Definition {
                                    path: self.parsed.path.clone(),
                                    span: field.span,
                                    kind: DefinitionKind::TableField,
                                });
                        }
                    }
                }
                ast::Stmt::LocalAssignment(local) => {
                    for (name, expression) in local.names().iter().zip(local.expressions()) {
                        if let Some(span) = node_span(name) {
                            self.values.entry(token_text(name)).or_insert(Definition {
                                path: self.parsed.path.clone(),
                                span,
                                kind: DefinitionKind::Local,
                            });
                        }

                        let Expression::TableConstructor(table) = expression else {
                            continue;
                        };

                        self.local_tables
                            .insert(token_text(name), table_field_spans(table));
                    }
                }
                ast::Stmt::LocalFunction(function) => {
                    if let Some(span) = node_span(function.name()) {
                        self.values
                            .entry(token_text(function.name()))
                            .or_insert(Definition {
                                path: self.parsed.path.clone(),
                                span,
                                kind: DefinitionKind::Function,
                            });
                    }
                }
                ast::Stmt::FunctionDeclaration(function) => {
                    let parts = function
                        .name()
                        .to_string()
                        .split('.')
                        .map(|part| part.trim().to_owned())
                        .collect::<Vec<_>>();
                    let Some(span) = node_span(function.name()) else {
                        continue;
                    };
                    let definition = Definition {
                        path: self.parsed.path.clone(),
                        span,
                        kind: DefinitionKind::Function,
                    };

                    if let [table, field] = parts.as_slice() {
                        self.table_members
                            .entry((table.clone(), field.clone()))
                            .or_insert(definition);
                    } else {
                        self.values
                            .entry(function.name().to_string())
                            .or_insert(definition);
                    }
                }
                _ => {}
            }
        }
    }

    fn value(&self, name: &str) -> Option<Definition> {
        self.values.get(name).cloned()
    }

    fn table_member(&self, table: &str, field: &str) -> Option<Definition> {
        self.table_members
            .get(&(table.to_owned(), field.to_owned()))
            .cloned()
    }

    fn local_table_field(&self, local: &str, field: &str) -> Option<SourceSpan> {
        self.local_tables.get(local)?.get(field).copied()
    }

    fn local_type(&self, local: &str) -> Option<&Type> {
        self.local_types.get(local)
    }

    fn field_target(
        parsed: &ParsedSource,
        ty: &Type,
        field: &str,
        depth: usize,
    ) -> Option<FieldTarget> {
        if depth >= MAX_FIELD_DEFINITION_DEPTH {
            return None;
        }

        match ty {
            Type::Alias(name) => {
                let alias = parsed
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
                    other => {
                        Self::field_target(parsed, &type_from_tua_type(other), field, depth + 1)
                    }
                }
            }
            Type::Union(types) => types
                .iter()
                .find_map(|ty| Self::field_target(parsed, ty, field, depth + 1)),
            Type::Table(table) => table.field(field).map(|field| FieldTarget {
                span: default_span(),
                ty: field.ty.clone(),
            }),
            Type::Any | Type::Function(_) | Type::Primitive(_) => None,
        }
    }
}

#[derive(Clone, Debug)]
struct ChainPart {
    text: String,
    span: SourceSpan,
}

fn chain_parts_from_var(var: &Var) -> Option<Vec<ChainPart>> {
    match var {
        Var::Name(name) => Some(vec![ChainPart {
            text: token_text(name),
            span: node_span(name)?,
        }]),
        Var::Expression(expression) => chain_parts_from_var_expression(expression),
        _ => None,
    }
}

fn chain_parts_from_var_expression(expression: &ast::VarExpression) -> Option<Vec<ChainPart>> {
    let Prefix::Name(name) = expression.prefix() else {
        return None;
    };

    let mut parts = vec![ChainPart {
        text: token_text(name),
        span: node_span(name)?,
    }];

    for suffix in expression.suffixes() {
        let Suffix::Index(Index::Dot { name, .. }) = suffix else {
            return None;
        };

        parts.push(ChainPart {
            text: token_text(name),
            span: node_span(name)?,
        });
    }

    Some(parts)
}

fn table_field_spans(table: &TableConstructor) -> BTreeMap<String, SourceSpan> {
    table
        .fields()
        .iter()
        .filter_map(|field| {
            let Field::NameKey { key, .. } = field else {
                return None;
            };

            Some((token_text(key), node_span(key)?))
        })
        .collect()
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.to_string().trim().to_owned()
}

fn unquote_string(value: &str) -> String {
    value.trim().trim_matches('"').trim_matches('\'').to_owned()
}

fn default_span() -> SourceSpan {
    SourceSpan {
        start: SourcePosition { line: 1, column: 1 },
        end: SourcePosition { line: 1, column: 2 },
    }
}

fn starts_before_or_at(span: SourceSpan, position: SourcePosition) -> bool {
    position_key(span.start) <= position_key(position)
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

fn parse_definition_source(
    path: &Path,
    source: &str,
    position: SourcePosition,
) -> Result<ParsedSource, Vec<Diagnostic>> {
    match parse_tua_source(path, source.to_owned()) {
        Ok(parsed) => Ok(parsed),
        Err(original_diagnostics) => {
            if let Ok(parsed) = parse_tua_source_with_original_source(
                path,
                source,
                source_with_line_blank(source, position.line),
            ) {
                return Ok(parsed);
            }

            for line in 1..=source.lines().count().max(1) {
                if line == position.line {
                    continue;
                }

                if let Ok(parsed) = parse_tua_source_with_original_source(
                    path,
                    source,
                    source_with_line_blank(source, line),
                ) {
                    return Ok(parsed);
                }
            }

            Err(original_diagnostics)
        }
    }
}

fn parse_tua_source_with_original_source(
    path: &Path,
    original_source: &str,
    parser_source: String,
) -> Result<ParsedSource, Vec<Diagnostic>> {
    let mut parsed = parse_tua_source(path, parser_source)?;

    parsed.source = original_source.to_owned();

    Ok(parsed)
}

fn source_with_line_blank(source: &str, line_to_blank: usize) -> String {
    let mut output = String::with_capacity(source.len());
    let mut current_line = 1;
    let mut line_start = 0;

    for (index, character) in source.char_indices() {
        if character != '\n' {
            continue;
        }

        if current_line == line_to_blank {
            output.push('\n');
        } else {
            output.push_str(&source[line_start..=index]);
        }

        current_line += 1;
        line_start = index + 1;
    }

    if line_start < source.len() {
        if current_line == line_to_blank {
            output.push('\n');
        } else {
            output.push_str(&source[line_start..]);
        }
    }

    output
}

#[cfg(test)]
mod tests;

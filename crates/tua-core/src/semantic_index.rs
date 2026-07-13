use std::collections::BTreeMap;
use std::path::PathBuf;

use full_moon::ast::{self, Expression, FunctionArgs, Prefix, Suffix};
use full_moon::node::Node;

use crate::hir::{HirModule, HirSymbol, HirSymbolKind};
use crate::module_link::module_specifier_at_position;
use crate::source_text::{byte_for_column, dotted_chain_at_position, line_at};
use crate::types::type_from_tua_type;
use crate::{
    lua, Definition, DefinitionKind, ParsedSource, SourcePosition, SourceSpan, TuaType, Type,
    TypeEnvironment,
};

const MAX_FIELD_DEPTH: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticIndex {
    path: PathBuf,
    definitions: BTreeMap<SpanKey, DefinitionTarget>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DefinitionTarget {
    Symbol {
        path: PathBuf,
        span: SourceSpan,
        kind: DefinitionKind,
    },
    TypeField {
        path: PathBuf,
        span: SourceSpan,
    },
    ModuleExport {
        path: PathBuf,
        span: SourceSpan,
        kind: DefinitionKind,
    },
    Builtin {
        path: PathBuf,
        span: SourceSpan,
    },
    ModuleFile {
        path: PathBuf,
        span: SourceSpan,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct SpanKey {
    start_line: usize,
    start_column: usize,
    end_line: usize,
    end_column: usize,
}

impl SemanticIndex {
    pub fn build(
        parsed: &ParsedSource,
        hir: Option<&HirModule>,
        environment: &TypeEnvironment,
    ) -> Self {
        let mut index = Self {
            path: parsed.path.clone(),
            definitions: BTreeMap::new(),
        };

        index.index_scope_symbols(hir);
        index.index_identifier_references(parsed, hir, environment);
        index
    }

    pub fn definition_at(&self, position: SourcePosition) -> Option<Definition> {
        self.definitions
            .iter()
            .find(|(span, _)| span.contains(position))
            .map(|(_, target)| target.definition())
    }

    pub fn cache_len(&self) -> usize {
        self.definitions.len()
    }

    fn index_scope_symbols(&mut self, hir: Option<&HirModule>) {
        let Some(hir) = hir else {
            return;
        };

        for symbol in hir.symbols() {
            let Some(target) = target_for_hir_symbol(&self.path, symbol) else {
                continue;
            };

            self.insert(symbol.name_span, target);
        }
    }

    fn index_identifier_references(
        &mut self,
        parsed: &ParsedSource,
        hir: Option<&HirModule>,
        environment: &TypeEnvironment,
    ) {
        for token in parsed.ast.tokens() {
            let Some(span) = node_span(token) else {
                continue;
            };
            if self.definitions.contains_key(&SpanKey::from(span)) {
                continue;
            }

            if let Some(target) = self.definition_target_at(parsed, hir, environment, span.start) {
                self.insert(span, target);
            }
        }
    }

    fn definition_target_at(
        &self,
        parsed: &ParsedSource,
        hir: Option<&HirModule>,
        environment: &TypeEnvironment,
        position: SourcePosition,
    ) -> Option<DefinitionTarget> {
        if let Some(specifier) = module_specifier_at_position(&parsed.source, position) {
            return module_file_target(parsed, &specifier.module, specifier.kind);
        }

        let chain = dotted_chain_at_position(&parsed.source, position)?;
        let current = chain.parts.get(chain.cursor_index)?;

        if chain.cursor_index > 0 {
            let parts = chain
                .parts
                .iter()
                .take(chain.cursor_index + 1)
                .map(|part| part.text.clone())
                .collect::<Vec<_>>();

            if let Some(target) = field_definition_target(parsed, environment, &parts, position) {
                return Some(target);
            }

            if let Some(target) = metatable_method_target(parsed, &parts, position) {
                return Some(target);
            }

            if let Some(target) = builtin_target(parsed, environment, &parts, position) {
                return Some(target);
            }
        }

        if is_type_reference(parsed, position) {
            return hir
                .and_then(|hir| hir.definition_for_type_alias(&current.text))
                .and_then(|symbol| target_for_hir_symbol(&parsed.path, symbol));
        }

        hir.and_then(|hir| hir.definition_for_value(&current.text, position))
            .and_then(|symbol| target_for_hir_symbol(&parsed.path, symbol))
            .or_else(|| {
                builtin_target(
                    parsed,
                    environment,
                    std::slice::from_ref(&current.text),
                    position,
                )
            })
            .or_else(|| {
                hir.and_then(|hir| hir.definition_for_type_alias(&current.text))
                    .and_then(|symbol| target_for_hir_symbol(&parsed.path, symbol))
            })
    }

    fn insert(&mut self, span: SourceSpan, target: DefinitionTarget) {
        self.definitions.insert(SpanKey::from(span), target);
    }
}

impl DefinitionTarget {
    pub fn definition(&self) -> Definition {
        match self {
            Self::Symbol { path, span, kind } | Self::ModuleExport { path, span, kind } => {
                Definition {
                    path: path.clone(),
                    span: *span,
                    kind: *kind,
                }
            }
            Self::TypeField { path, span } => Definition {
                path: path.clone(),
                span: *span,
                kind: DefinitionKind::TableField,
            },
            Self::Builtin { path, span } => Definition {
                path: path.clone(),
                span: *span,
                kind: DefinitionKind::Builtin,
            },
            Self::ModuleFile { path, span } => Definition {
                path: path.clone(),
                span: *span,
                kind: DefinitionKind::Module,
            },
        }
    }
}

impl SpanKey {
    fn contains(self, position: SourcePosition) -> bool {
        (self.start_line, self.start_column) <= (position.line, position.column)
            && (position.line, position.column) < (self.end_line, self.end_column)
    }
}

impl From<SourceSpan> for SpanKey {
    fn from(span: SourceSpan) -> Self {
        Self {
            start_line: span.start.line,
            start_column: span.start.column,
            end_line: span.end.line,
            end_column: span.end.column,
        }
    }
}

fn target_for_hir_symbol(path: &std::path::Path, symbol: &HirSymbol) -> Option<DefinitionTarget> {
    let kind = match symbol.kind {
        HirSymbolKind::TypeAlias => DefinitionKind::TypeAlias,
        HirSymbolKind::Local => DefinitionKind::Local,
        HirSymbolKind::Function => DefinitionKind::Function,
        HirSymbolKind::Parameter => DefinitionKind::Parameter,
        HirSymbolKind::TableField => DefinitionKind::TableField,
    };

    Some(match symbol.kind {
        HirSymbolKind::TableField => DefinitionTarget::TypeField {
            path: path.to_path_buf(),
            span: symbol.name_span,
        },
        _ => DefinitionTarget::Symbol {
            path: path.to_path_buf(),
            span: symbol.name_span,
            kind,
        },
    })
}

fn field_definition_target(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    parts: &[String],
    position: SourcePosition,
) -> Option<DefinitionTarget> {
    let (root, fields) = parts.split_first()?;
    let Some(mut current) = receiver_type(parsed, environment, root, position) else {
        return Some(DefinitionTarget::TypeField {
            path: parsed.path.clone(),
            span: local_table_field_span(parsed, parts, position)?,
        });
    };

    for (index, field) in fields.iter().enumerate() {
        let target = field_target(parsed, &current, field, 0)?;
        let source_span = local_table_field_span(parsed, &parts[..index + 2], position);

        current = target.ty;

        if index == fields.len() - 1 {
            return Some(DefinitionTarget::TypeField {
                path: parsed.path.clone(),
                span: target.span.or(source_span)?,
            });
        }
    }

    None
}

fn local_table_field_span(
    parsed: &ParsedSource,
    parts: &[String],
    position: SourcePosition,
) -> Option<SourceSpan> {
    let (root, fields) = parts.split_first()?;
    let expression = latest_table_expression_for_name(parsed, root, position)?;

    table_field_span_in_expression(expression, fields)
}

fn latest_table_expression_for_name<'a>(
    parsed: &'a ParsedSource,
    name: &str,
    position: SourcePosition,
) -> Option<&'a Expression> {
    let mut candidates = Vec::new();

    for stmt in parsed.ast.nodes().stmts() {
        let Some(span) = node_span(stmt) else {
            continue;
        };
        if !starts_before_or_at(span, position) {
            continue;
        }

        match stmt {
            ast::Stmt::LocalAssignment(local) => {
                for (local_name, expression) in local.names().iter().zip(local.expressions()) {
                    if token_text(local_name) == name
                        && table_constructor_expression(expression).is_some()
                    {
                        candidates.push((span.start, expression));
                    }
                }
            }
            ast::Stmt::Assignment(assignment) => {
                for (variable, expression) in
                    assignment.variables().iter().zip(assignment.expressions())
                {
                    if root_assignment_name(variable).as_deref() == Some(name)
                        && table_constructor_expression(expression).is_some()
                    {
                        candidates.push((span.start, expression));
                    }
                }
            }
            _ => {}
        }
    }

    candidates
        .into_iter()
        .max_by_key(|(position, _)| (position.line, position.column))
        .map(|(_, expression)| expression)
}

fn table_field_span_in_expression(
    expression: &Expression,
    fields: &[String],
) -> Option<SourceSpan> {
    let (field, remaining) = fields.split_first()?;
    let table = table_constructor_expression(expression)?;

    for table_field in table.fields() {
        let ast::Field::NameKey { key, value, .. } = table_field else {
            continue;
        };
        if token_text(key) != *field {
            continue;
        }

        if remaining.is_empty() {
            return node_span(key).or_else(|| node_span(table_field));
        }

        return table_field_span_in_expression(value, remaining);
    }

    None
}

fn table_constructor_expression(expression: &Expression) -> Option<&ast::TableConstructor> {
    match expression {
        Expression::TableConstructor(table) => Some(table),
        Expression::Parentheses { expression, .. } => table_constructor_expression(expression),
        _ => None,
    }
}

fn root_assignment_name(variable: &ast::Var) -> Option<String> {
    match variable {
        ast::Var::Name(name) => Some(token_text(name)),
        _ => None,
    }
}

fn receiver_type(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    root: &str,
    position: SourcePosition,
) -> Option<Type> {
    if root == "self" {
        if let Some(owner) = enclosing_method_owner(parsed, position) {
            if environment.alias(&owner).is_some() {
                return Some(Type::alias(owner));
            }
        }
    }

    environment.local(root).cloned().or_else(|| {
        parsed
            .function_annotations
            .iter()
            .flat_map(|function| function.parameters.iter())
            .filter(|parameter| parameter.name == root)
            .filter_map(|parameter| {
                let span = parameter.name_span.or(parameter.span)?;

                (span.start.line <= position.line).then(|| type_from_tua_type(&parameter.ty))
            })
            .next()
    })
}

fn field_target(
    parsed: &ParsedSource,
    ty: &Type,
    field: &str,
    depth: usize,
) -> Option<FieldTarget> {
    if depth >= MAX_FIELD_DEPTH {
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
                            span: Some(span),
                            ty: type_from_tua_type(&candidate.ty),
                        })
                    }),
                other => field_target(parsed, &type_from_tua_type(other), field, depth + 1),
            }
        }
        Type::Union(types) => types
            .iter()
            .find_map(|ty| field_target(parsed, ty, field, depth + 1)),
        Type::Table(table) => table.semantic_field(field).map(|field| FieldTarget {
            span: None,
            ty: field.ty.clone(),
        }),
        Type::Any
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::StringLiteral(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. } => None,
    }
}

#[derive(Clone, Debug)]
struct FieldTarget {
    span: Option<SourceSpan>,
    ty: Type,
}

fn enclosing_method_owner(parsed: &ParsedSource, position: SourcePosition) -> Option<String> {
    parsed.ast.nodes().stmts().find_map(|stmt| {
        let ast::Stmt::FunctionDeclaration(function) = stmt else {
            return None;
        };
        let body_span = node_span(function.body())?;
        if !span_contains(body_span, position) {
            return None;
        }

        let name = function.name().to_string();
        let (owner, _, is_method) = function_name_table_field(&name)?;

        is_method.then_some(owner)
    })
}

fn metatable_method_target(
    parsed: &ParsedSource,
    parts: &[String],
    position: SourcePosition,
) -> Option<DefinitionTarget> {
    let (field, receiver_parts) = parts.split_last()?;
    let [receiver] = receiver_parts else {
        return None;
    };
    let prototype = setmetatable_prototype_for_receiver(parsed, receiver, position)?;
    let target = table_member_definition(parsed, &prototype, field)?;

    Some(DefinitionTarget::Symbol {
        path: target.path,
        span: target.span,
        kind: target.kind,
    })
}

fn table_member_definition(parsed: &ParsedSource, table: &str, field: &str) -> Option<Definition> {
    for function in &parsed.function_annotations {
        let name = function.name.as_deref()?;
        let (owner, member, _) = function_name_table_field(name)?;
        if owner == table && member == field {
            return Some(Definition {
                path: parsed.path.clone(),
                span: function.name_span.or(function.span)?,
                kind: DefinitionKind::Function,
            });
        }
    }

    None
}

fn setmetatable_prototype_for_receiver(
    parsed: &ParsedSource,
    receiver: &str,
    position: SourcePosition,
) -> Option<String> {
    parsed
        .ast
        .nodes()
        .stmts()
        .filter_map(|stmt| {
            let span = node_span(stmt)?;
            if !starts_before_or_at(span, position) {
                return None;
            }

            let ast::Stmt::LocalAssignment(local) = stmt else {
                return None;
            };

            for (name, expression) in local.names().iter().zip(local.expressions()) {
                if token_text(name) != receiver {
                    continue;
                }
                let Expression::FunctionCall(call) = expression else {
                    continue;
                };
                let (_, meta) = setmetatable_arguments(call)?;
                let Expression::Var(ast::Var::Name(meta_name)) = meta else {
                    continue;
                };

                return Some(token_text(meta_name));
            }

            None
        })
        .last()
}

fn builtin_target(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    parts: &[String],
    position: SourcePosition,
) -> Option<DefinitionTarget> {
    let item = if parts.first().is_some_and(|part| part == "love") {
        lua::hover_item(parts)
    } else {
        let semantic = crate::semantic::SemanticModel::new(parsed, environment);

        semantic
            .lua_function_item_for_call_parts(parts, position)
            .or_else(|| lua::hover_item(parts))
    }?;
    let definition = lua::builtin_definition_for_item(item)?;

    Some(DefinitionTarget::Builtin {
        path: definition.path,
        span: definition.span,
    })
}

fn module_file_target(
    parsed: &ParsedSource,
    module: &str,
    kind: crate::ModuleSpecifierKind,
) -> Option<DefinitionTarget> {
    let path = crate::resolve_module_specifier(&parsed.path, module, kind)?;

    Some(DefinitionTarget::ModuleFile {
        path,
        span: SourceSpan {
            start: SourcePosition { line: 1, column: 1 },
            end: SourcePosition { line: 1, column: 2 },
        },
    })
}

fn is_type_reference(parsed: &ParsedSource, position: SourcePosition) -> bool {
    let Some(line) = line_at(&parsed.source, position.line) else {
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

fn function_name_table_field(name: &str) -> Option<(String, String, bool)> {
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

fn setmetatable_arguments(
    function_call: &ast::FunctionCall,
) -> Option<(&ast::Expression, &ast::Expression)> {
    let Prefix::Name(name) = function_call.prefix() else {
        return None;
    };

    if token_text(name) != "setmetatable" {
        return None;
    }

    let Some(Suffix::Call(ast::Call::AnonymousCall(FunctionArgs::Parentheses {
        arguments, ..
    }))) = function_call.suffixes().next()
    else {
        return None;
    };

    Some((arguments.iter().next()?, arguments.iter().nth(1)?))
}

fn span_contains(span: SourceSpan, position: SourcePosition) -> bool {
    (span.start.line, span.start.column) <= (position.line, position.column)
        && (position.line, position.column) <= (span.end.line, span.end.column)
}

fn starts_before_or_at(span: SourceSpan, position: SourcePosition) -> bool {
    (span.start.line, span.start.column) <= (position.line, position.column)
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.to_string().trim().to_owned()
}

#[cfg(test)]
mod tests;

use std::fs;
use std::path::Path;

use full_moon::ast::{
    self, Call, Expression, Field, FunctionArgs, FunctionCall, Index, Prefix, Suffix,
    TableConstructor, Var,
};
use full_moon::node::Node;

use crate::lua::{self, LuaItem};
use crate::module_index::{
    required_module_export_for_fields, required_module_export_type_for_fields,
    required_module_for_receiver, resolve_required_module,
};
use crate::module_link::require_module_at_position;
use crate::semantic::resolve_field_chain_type;
use crate::source_text::dotted_chain_at_position;
use crate::types::{
    build_type_environment, FunctionType, TableField, Type, TypeEnvironment, TypedFunction,
};
use crate::{
    definition_parsed_source, parse_tua_source, Definition, DefinitionKind, Diagnostic,
    ParameterAnnotation, ParsedSource, SourcePosition, SourceSpan, TuaType,
};

const MAX_LOCAL_INFERENCE_DEPTH: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Hover {
    pub contents: String,
    pub span: Option<SourceSpan>,
    pub documentation: Option<String>,
    pub context: Vec<String>,
}

pub fn hover_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    position: SourcePosition,
) -> Result<Option<Hover>, Vec<Diagnostic>> {
    let parsed = parse_tua_source(path, source)?;

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
        if let Some(hover) = self.required_module_literal_hover(position) {
            return Some(hover);
        }

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
                return Some(hover(
                    format!("(field) {}: {ty}", current_part.text),
                    Some(current_part.span),
                ));
            }

            if let Some(hover) =
                self.required_module_field_hover(&field_parts, position, current_part.span)
            {
                return Some(hover);
            }

            if self.environment.local(&field_parts[0]).is_none() {
                if let Some(item) = lua::hover_item(&field_parts) {
                    return Some(lua_item_hover(item, Some(current_part.span)));
                }
            }
        }

        if let Some(hover) =
            self.required_module_binding_hover(&current_part.text, current_part.span)
        {
            return Some(hover);
        }

        if let Some(ty) = self.environment.alias(&current_part.text) {
            let contents = format!("type {} = {ty}", current_part.text);

            if let Some(documentation) = self.environment.alias_documentation(&current_part.text) {
                let mut hover = hover(contents, Some(current_part.span));
                hover.documentation = Some(documentation.to_owned());
                hover.context = self.declaration_context(position, DefinitionKind::TypeAlias);

                return Some(hover);
            }

            return Some(hover(contents, Some(current_part.span)));
        }

        if self.definition_kind_at(position) == Some(DefinitionKind::Parameter) {
            if let Some(hover) =
                self.parameter_hover(&current_part.text, position, current_part.span)
            {
                return Some(hover);
            }
        }

        if let Some(ty) = self.environment.local(&current_part.text) {
            let contents = format!("local {}: {ty}", current_part.text);

            if let Some(documentation) = self.environment.local_documentation(&current_part.text) {
                let mut hover = hover(contents, Some(current_part.span));
                hover.documentation = Some(documentation.to_owned());
                hover.context = self.declaration_context(position, DefinitionKind::Local);

                return Some(hover);
            }

            return Some(hover(contents, Some(current_part.span)));
        }

        if let Some(hover) =
            self.unannotated_local_hover(&current_part.text, position, current_part.span)
        {
            return Some(hover);
        }

        self.environment
            .function(&current_part.text)
            .map(|function| self.function_hover(function, position, current_part.span))
            .or_else(|| {
                lua::hover_item(std::slice::from_ref(&current_part.text))
                    .map(|item| lua_item_hover(item, Some(current_part.span)))
            })
    }

    fn definition_kind_at(&self, position: SourcePosition) -> Option<DefinitionKind> {
        definition_parsed_source(self.parsed, position)
            .ok()
            .flatten()
            .map(|definition| definition.kind)
    }

    fn parameter_hover(
        &self,
        name: &str,
        position: SourcePosition,
        hover_span: SourceSpan,
    ) -> Option<Hover> {
        let parameter = self.parameter_at_position(name, position)?;
        let ty = type_from_tua_type(&parameter.ty);

        Some(Hover {
            contents: format!("parameter {name}: {ty}"),
            span: Some(hover_span),
            documentation: None,
            context: Vec::new(),
        })
    }

    fn parameter_at_position(
        &self,
        name: &str,
        position: SourcePosition,
    ) -> Option<&'a ParameterAnnotation> {
        self.parsed
            .function_annotations
            .iter()
            .filter_map(|function| {
                let function_span = function.span?;

                if !span_contains(function_span, position) {
                    return None;
                }

                let parameter = function
                    .parameters
                    .iter()
                    .find(|parameter| parameter.name == name)?;
                let parameter_span = parameter.name_span.or(parameter.span)?;

                starts_before_or_at(parameter_span, position).then_some((
                    function_span,
                    parameter_span,
                    parameter,
                ))
            })
            .max_by_key(|(function_span, parameter_span, _)| {
                (
                    position_key(function_span.start),
                    position_key(parameter_span.start),
                )
            })
            .map(|(_, _, parameter)| parameter)
    }

    fn unannotated_local_hover(
        &self,
        name: &str,
        position: SourcePosition,
        hover_span: SourceSpan,
    ) -> Option<Hover> {
        let local = self.local_assignment_value(name, position)?;
        let contents = local
            .expression
            .and_then(|expression| self.infer_expression(expression, local.span.start, 0))
            .map(|ty| format!("local {name}: {ty}"))
            .unwrap_or_else(|| format!("local {name}"));

        Some(Hover {
            contents,
            span: Some(hover_span),
            documentation: None,
            context: Vec::new(),
        })
    }

    fn local_assignment_value(
        &self,
        name: &str,
        position: SourcePosition,
    ) -> Option<LocalAssignmentValue<'a>> {
        let mut candidates = Vec::new();

        for stmt in self.parsed.ast.nodes().stmts() {
            let ast::Stmt::LocalAssignment(local) = stmt else {
                continue;
            };
            let expressions = local.expressions().iter().collect::<Vec<_>>();

            for (index, local_name) in local.names().iter().enumerate() {
                let Some(span) = node_span(local_name) else {
                    continue;
                };

                if token_text(local_name) != name || !starts_before_or_at(span, position) {
                    continue;
                }

                candidates.push(LocalAssignmentValue {
                    span,
                    expression: expressions.get(index).copied(),
                });
            }
        }

        candidates
            .into_iter()
            .max_by_key(|candidate| position_key(candidate.span.start))
    }

    fn infer_expression(
        &self,
        expression: &Expression,
        position: SourcePosition,
        depth: usize,
    ) -> Option<Type> {
        if depth >= MAX_LOCAL_INFERENCE_DEPTH {
            return None;
        }

        match expression {
            Expression::Number(_) => Some(Type::number()),
            Expression::String(_) => Some(Type::string()),
            Expression::Symbol(symbol) => infer_symbol_type(&token_text(symbol)),
            Expression::TableConstructor(table) => {
                Some(self.infer_table_constructor(table, position, depth + 1))
            }
            Expression::FunctionCall(function_call) => {
                self.infer_function_call_return_type(function_call)
            }
            Expression::Parentheses { expression, .. }
            | Expression::TypeAssertion { expression, .. } => {
                self.infer_expression(expression, position, depth + 1)
            }
            Expression::Var(var) => self.infer_var_value(var, position, depth + 1),
            _ => None,
        }
    }

    fn infer_table_constructor(
        &self,
        table: &TableConstructor,
        position: SourcePosition,
        depth: usize,
    ) -> Type {
        Type::table(table.fields().iter().filter_map(|field| match field {
            Field::NameKey { key, value, .. } => {
                let ty = self
                    .infer_expression(value, position, depth + 1)
                    .unwrap_or(Type::Any);

                Some((token_text(key), TableField::required(ty)))
            }
            _ => None,
        }))
    }

    fn infer_function_call_return_type(&self, function_call: &FunctionCall) -> Option<Type> {
        let parts = field_chain_before_first_call(function_call)?;
        let (root, fields) = parts.split_first()?;

        if fields.is_empty() {
            if let Some(function) = self.environment.function(root) {
                return Some(function.return_type_or_any());
            }
        }

        if let Some(Type::Function(function)) = resolve_field_chain_type(self.environment, &parts) {
            return Some(*function.return_type);
        }

        if let Some(Type::Function(function)) =
            required_module_export_type_for_fields(self.parsed, root, fields)
        {
            return Some(*function.return_type);
        }

        lua::hover_item(&parts).and_then(|item| return_type_from_function_detail(item.detail))
    }

    fn infer_var_value(&self, var: &Var, position: SourcePosition, depth: usize) -> Option<Type> {
        match var {
            Var::Name(name) => self.infer_named_value(&token_text(name), position, depth + 1),
            Var::Expression(expression) => resolve_field_chain_type(
                self.environment,
                &plain_field_chain_from_var_expression(expression)?,
            ),
            _ => None,
        }
    }

    fn infer_named_value(
        &self,
        name: &str,
        position: SourcePosition,
        depth: usize,
    ) -> Option<Type> {
        if depth >= MAX_LOCAL_INFERENCE_DEPTH {
            return None;
        }

        self.environment
            .local(name)
            .cloned()
            .or_else(|| {
                self.environment
                    .function(name)
                    .and_then(TypedFunction::as_type)
            })
            .or_else(|| {
                let local = self.local_assignment_value(name, position)?;

                local.expression.and_then(|expression| {
                    self.infer_expression(expression, local.span.start, depth + 1)
                })
            })
    }

    fn resolve_field_chain_type(&self, parts: &[String]) -> Option<Type> {
        resolve_field_chain_type(self.environment, parts)
    }

    fn required_module_literal_hover(&self, position: SourcePosition) -> Option<Hover> {
        let module = require_module_at_position(&self.parsed.source, position)?;
        let path = resolve_required_module(&self.parsed.path, &module)?;

        Some(hover(
            format!("(module) {module} -> {}", path.display()),
            None,
        ))
    }

    fn required_module_binding_hover(&self, receiver: &str, span: SourceSpan) -> Option<Hover> {
        let required = required_module_for_receiver(self.parsed, receiver)?;

        Some(hover(
            format!(
                "(module) {receiver} = require(\"{}\") -> {}",
                required.module,
                required.path.display()
            ),
            Some(span),
        ))
    }

    fn required_module_field_hover(
        &self,
        parts: &[String],
        position: SourcePosition,
        span: SourceSpan,
    ) -> Option<Hover> {
        let (receiver, fields) = parts.split_first()?;
        let required = required_module_for_receiver(self.parsed, receiver)?;
        let export = required_module_export_for_fields(self.parsed, receiver, fields)?;
        let definition = definition_parsed_source(self.parsed, position)
            .ok()
            .flatten();
        let mut hover = hover(format_export_hover(parts, &export.ty), Some(span));
        let source_documentation = definition
            .as_ref()
            .and_then(|definition| documentation_for_definition(self.parsed, definition));

        hover.documentation = merge_documentation([
            source_documentation.as_deref(),
            export.documentation.as_deref(),
        ]);
        hover.context = export_context(&required.path, definition.as_ref(), &export.ty);

        Some(hover)
    }

    fn function_hover(
        &self,
        function: &TypedFunction,
        position: SourcePosition,
        span: SourceSpan,
    ) -> Hover {
        let definition = definition_parsed_source(self.parsed, position)
            .ok()
            .flatten();
        let source_documentation = definition
            .as_ref()
            .and_then(|definition| documentation_for_definition(self.parsed, definition));
        let mut hover = hover(format_function_signature(function), Some(span));

        hover.documentation = merge_documentation([
            source_documentation.as_deref(),
            function.documentation.as_deref(),
        ]);

        if hover.documentation.is_some() {
            hover.context = declaration_context(definition.as_ref(), Some("function"));
        }

        hover
    }

    fn declaration_context(
        &self,
        position: SourcePosition,
        expected: DefinitionKind,
    ) -> Vec<String> {
        let definition = definition_parsed_source(self.parsed, position)
            .ok()
            .flatten()
            .filter(|definition| definition.kind == expected);

        declaration_context(definition.as_ref(), Some(definition_kind_label(expected)))
    }
}

struct LocalAssignmentValue<'a> {
    span: SourceSpan,
    expression: Option<&'a Expression>,
}

fn hover(contents: impl Into<String>, span: Option<SourceSpan>) -> Hover {
    Hover {
        contents: contents.into(),
        span,
        documentation: None,
        context: Vec::new(),
    }
}

fn lua_item_hover(item: LuaItem, span: Option<SourceSpan>) -> Hover {
    let mut hover = hover(item.detail.to_owned(), span);

    if !item.documentation.is_empty() {
        hover.documentation = Some(item.documentation.to_owned());
    }

    hover
}

fn format_function_signature(function: &TypedFunction) -> String {
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

fn merge_documentation<'a>(parts: impl IntoIterator<Item = Option<&'a str>>) -> Option<String> {
    let mut lines = Vec::new();

    for part in parts.into_iter().flatten() {
        let part = part.trim();

        if !part.is_empty() && !lines.iter().any(|line| line == part) {
            lines.push(part.to_owned());
        }
    }

    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn documentation_for_definition(parsed: &ParsedSource, definition: &Definition) -> Option<String> {
    let source;
    let source = if definition.path == parsed.path {
        parsed.source.as_str()
    } else {
        source = fs::read_to_string(&definition.path).ok()?;
        source.as_str()
    };

    leading_doc_comment(source, definition.span.start.line)
}

fn leading_doc_comment(source: &str, declaration_line: usize) -> Option<String> {
    let mut docs = Vec::new();
    let mut line_number = declaration_line.checked_sub(1)?;

    while line_number > 0 {
        let Some(line) = source.lines().nth(line_number - 1) else {
            break;
        };
        let trimmed = line.trim_start();

        if !trimmed.starts_with("---") {
            break;
        }

        if !trimmed.starts_with("---@") {
            let text = trimmed.trim_start_matches("---").trim_start();

            if !text.is_empty() {
                docs.push(text.to_owned());
            }
        }

        line_number -= 1;
    }

    docs.reverse();

    (!docs.is_empty()).then(|| docs.join("\n"))
}

fn declaration_context(definition: Option<&Definition>, kind: Option<&str>) -> Vec<String> {
    let mut context = Vec::new();

    if let Some(kind) = kind {
        context.push(format!("kind: {kind}"));
    }

    if let Some(definition) = definition {
        context.push(format!(
            "declared: {}",
            source_location(&definition.path, definition.span.start)
        ));
    }

    context
}

fn export_context(module_path: &Path, definition: Option<&Definition>, ty: &Type) -> Vec<String> {
    let mut context = Vec::new();

    context.push(format!("kind: {}", export_kind_label(ty)));
    context.push(format!("module: {}", module_path.display()));

    if let Some(definition) = definition {
        context.push(format!(
            "declared: {}",
            source_location(&definition.path, definition.span.start)
        ));
    }

    context
}

fn source_location(path: &Path, position: SourcePosition) -> String {
    format!("{}:{}:{}", path.display(), position.line, position.column)
}

fn definition_kind_label(kind: DefinitionKind) -> &'static str {
    match kind {
        DefinitionKind::Module => "module",
        DefinitionKind::TypeAlias => "type alias",
        DefinitionKind::Local => "local",
        DefinitionKind::Function => "function",
        DefinitionKind::Parameter => "parameter",
        DefinitionKind::TableField => "field",
    }
}

fn export_kind_label(ty: &Type) -> &'static str {
    match ty {
        Type::Function(_) => "function",
        _ => "field",
    }
}

fn format_export_hover(parts: &[String], ty: &Type) -> String {
    let qualified_name = parts.join(".");

    match ty {
        Type::Function(function) => format_export_function(&qualified_name, function),
        _ => format!("(field) {qualified_name}: {ty}"),
    }
}

fn format_export_function(name: &str, function: &FunctionType) -> String {
    let parameters = function
        .parameters
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");

    format!("function {name}({parameters}): {}", function.return_type)
}

fn infer_symbol_type(symbol: &str) -> Option<Type> {
    match symbol {
        "true" | "false" => Some(Type::boolean()),
        "nil" => Some(Type::nil()),
        _ => None,
    }
}

fn return_type_from_function_detail(detail: &str) -> Option<Type> {
    let start = detail.rfind("): ")? + "): ".len();
    let ty = detail[start..].trim();

    type_from_label(ty)
}

fn type_from_label(label: &str) -> Option<Type> {
    match label {
        "any" => Some(Type::Any),
        "boolean" => Some(Type::boolean()),
        "nil" => Some(Type::nil()),
        "number" => Some(Type::number()),
        "string" => Some(Type::string()),
        "table" => Some(Type::empty_table()),
        other if !other.is_empty() => Some(Type::alias(other)),
        _ => None,
    }
}

fn type_from_tua_type(ty: &TuaType) -> Type {
    match ty {
        TuaType::Named(name) => Type::named(name),
        TuaType::Union(types) => Type::union(types.iter().map(type_from_tua_type)),
        TuaType::Table(fields) => Type::table(fields.iter().map(|field| {
            let ty = type_from_tua_type(&field.ty);
            let table_field = if field.optional {
                TableField::optional(ty)
            } else {
                TableField::required(ty)
            };

            (field.name.clone(), table_field)
        })),
    }
}

fn field_chain_before_first_call(function_call: &FunctionCall) -> Option<Vec<String>> {
    let Prefix::Name(name) = function_call.prefix() else {
        return None;
    };

    let mut parts = vec![token_text(name)];

    for suffix in function_call.suffixes() {
        match suffix {
            Suffix::Index(Index::Dot { name, .. }) => parts.push(token_text(name)),
            Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses { .. }))
            | Suffix::Call(Call::AnonymousCall(FunctionArgs::String(_)))
            | Suffix::Call(Call::AnonymousCall(FunctionArgs::TableConstructor(_)))
            | Suffix::Call(Call::MethodCall(_)) => return Some(parts),
            _ => return None,
        }
    }

    None
}

fn plain_field_chain_from_var_expression(expression: &ast::VarExpression) -> Option<Vec<String>> {
    let Prefix::Name(name) = expression.prefix() else {
        return None;
    };

    let mut parts = vec![token_text(name)];

    for suffix in expression.suffixes() {
        let Suffix::Index(Index::Dot { name, .. }) = suffix else {
            return None;
        };

        parts.push(token_text(name));
    }

    Some(parts)
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.to_string().trim().to_owned()
}

fn starts_before_or_at(span: SourceSpan, position: SourcePosition) -> bool {
    position_key(span.start) <= position_key(position)
}

fn span_contains(span: SourceSpan, position: SourcePosition) -> bool {
    position_key(span.start) <= position_key(position)
        && position_key(position) <= position_key(span.end)
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::lua::{self, LuaItem};
use crate::module_index::{
    required_module_export_for_fields, required_module_for_receiver, resolve_module_specifier,
};
use crate::module_link::module_specifier_at_position;
use crate::narrowing::narrowed_type_at_position;
use crate::semantic::{resolve_aliases, SemanticModel};
use crate::source_text::dotted_chain_at_position;
use crate::types::{
    build_type_environment, FunctionType, TableField, Type, TypeEnvironment, TypedFunction,
    TypedFunctionOverload,
};
use crate::{
    definition_parsed_source, parse_tua_source, Definition, DefinitionKind, Diagnostic,
    ParameterAnnotation, ParsedSource, SourcePosition, SourceSpan, TuaType,
};

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

    Ok(hover_parsed_source_with_environment(
        parsed,
        position,
        &environment,
    ))
}

pub(crate) fn hover_parsed_source_with_environment(
    parsed: &ParsedSource,
    position: SourcePosition,
    environment: &TypeEnvironment,
) -> Option<Hover> {
    let resolver = HoverResolver {
        parsed,
        environment,
        semantic: SemanticModel::new(parsed, environment),
    };

    resolver.hover(position)
}

struct HoverResolver<'a> {
    parsed: &'a ParsedSource,
    environment: &'a TypeEnvironment,
    semantic: SemanticModel<'a>,
}

impl<'a> HoverResolver<'a> {
    fn hover(&self, position: SourcePosition) -> Option<Hover> {
        if let Some(hover) = self.required_module_literal_hover(position) {
            return Some(hover);
        }

        let Some(chain) = dotted_chain_at_position(&self.parsed.source, position) else {
            return self.expression_hover(position);
        };
        let current_part = chain.parts.get(chain.cursor_index)?;

        if chain.cursor_index > 0 {
            let field_parts = chain
                .parts
                .iter()
                .take(chain.cursor_index + 1)
                .map(|part| part.text.clone())
                .collect::<Vec<_>>();

            if let Some(hover) =
                self.required_module_field_hover(&field_parts, position, current_part.span)
            {
                return Some(hover);
            }

            if let Some(ty) = self.semantic.type_of_field_chain(&field_parts, position) {
                return Some(hover(
                    format!("(field) {}: {ty}", current_part.text),
                    Some(current_part.span),
                ));
            }

            if let Some(item) =
                self.love_object_field_item(&field_parts, position, &current_part.text)
            {
                return Some(lua_item_hover(item, Some(current_part.span)));
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
            .or_else(|| self.expression_hover(position))
    }

    fn expression_hover(&self, position: SourcePosition) -> Option<Hover> {
        let (ty, span) = self.semantic.type_of_expression_at_position(position)?;

        Some(hover(format!("expression: {ty}"), Some(span)))
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
        let base = type_from_tua_type(&parameter.ty);
        let aliases = self
            .environment
            .aliases()
            .map(|(name, ty)| (name.to_owned(), ty.clone()))
            .collect::<BTreeMap<_, _>>();
        let ty = narrowed_type_at_position(self.parsed, &aliases, name, position, |lookup_name| {
            if lookup_name == name {
                Some(base.clone())
            } else {
                self.semantic.type_of_name(lookup_name, position)
            }
        })
        .unwrap_or(base);

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
        let ty = self.semantic.type_of_local_assignment(name, position)?;
        let contents = format!("local {name}: {ty}");

        Some(Hover {
            contents,
            span: Some(hover_span),
            documentation: None,
            context: Vec::new(),
        })
    }

    fn love_object_field_item(
        &self,
        parts: &[String],
        position: SourcePosition,
        field: &str,
    ) -> Option<LuaItem> {
        let receiver_parts = parts.get(..parts.len().checked_sub(1)?)?;
        let receiver_type = self
            .semantic
            .type_of_field_chain(receiver_parts, position)?;

        lua_type_field_item(self.environment, &receiver_type, field)
    }

    fn required_module_literal_hover(&self, position: SourcePosition) -> Option<Hover> {
        let specifier = module_specifier_at_position(&self.parsed.source, position)?;
        let path = resolve_module_specifier(&self.parsed.path, &specifier.module, specifier.kind)?;

        Some(hover(
            format!("(module) {} -> {}", specifier.module, path.display()),
            None,
        ))
    }

    fn required_module_binding_hover(&self, receiver: &str, span: SourceSpan) -> Option<Hover> {
        let required = required_module_for_receiver(self.parsed, receiver)?;

        Some(hover(
            format!(
                "(module) {receiver} = {}(\"{}\") -> {}",
                required.kind.label(),
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
    let documentation = lua::documentation_with_love_wiki_link(item);

    if !documentation.is_empty() {
        hover.documentation = Some(documentation);
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
        parameters.push(
            function
                .vararg_type
                .as_ref()
                .map(|ty| format!("...: {ty}"))
                .unwrap_or_else(|| "...".to_owned()),
        );
    }

    let mut signatures = Vec::new();

    if should_show_primary_typed_signature(function) {
        signatures.push(format!(
            "function {name}({}): {}",
            parameters.join(", "),
            format_type_list(&function.return_types_or_any())
        ));
    }

    signatures.extend(
        function
            .overloads
            .iter()
            .map(|overload| format_function_overload_signature(name, overload)),
    );

    signatures.join("\n")
}

fn should_show_primary_typed_signature(function: &TypedFunction) -> bool {
    function.overloads.is_empty()
        || function
            .parameters
            .iter()
            .any(|parameter| parameter.ty != Type::Any)
        || function.vararg_type.is_some()
}

fn format_function_overload_signature(name: &str, overload: &TypedFunctionOverload) -> String {
    let mut parameters = overload
        .parameters
        .iter()
        .map(|parameter| {
            if parameter.name.is_empty() {
                parameter.ty.to_string()
            } else {
                format!("{}: {}", parameter.name, parameter.ty)
            }
        })
        .collect::<Vec<_>>();

    if overload.has_varargs {
        parameters.push(
            overload
                .vararg_type
                .as_ref()
                .map(|ty| format!("...: {ty}"))
                .unwrap_or_else(|| "...".to_owned()),
        );
    }

    format!(
        "function {name}({}): {}",
        parameters.join(", "),
        format_type_list(&overload.return_types_or_any())
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
        DefinitionKind::Builtin => "builtin",
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

    format!(
        "function {name}({parameters}): {}",
        format_type_list(function.return_types_or_any())
    )
}

fn format_type_list(types: &[Type]) -> String {
    types
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn type_from_tua_type(ty: &TuaType) -> Type {
    match ty {
        TuaType::Named(name) => Type::named(name),
        TuaType::StringLiteral(value) => Type::string_literal(value),
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
        TuaType::Array(item) => Type::array(type_from_tua_type(item)),
        TuaType::Tuple(items) => Type::tuple(items.iter().map(type_from_tua_type)),
        TuaType::Dictionary { key, value } => {
            Type::dictionary(type_from_tua_type(key), type_from_tua_type(value))
        }
        TuaType::Function {
            parameters,
            returns,
        } => Type::function_returns(
            parameters.iter().map(|parameter| {
                let ty = type_from_tua_type(&parameter.ty);

                if parameter.optional {
                    Type::union([ty, Type::nil()])
                } else {
                    ty
                }
            }),
            returns.iter().map(type_from_tua_type),
        ),
    }
}

fn lua_type_field_item(environment: &TypeEnvironment, ty: &Type, field: &str) -> Option<LuaItem> {
    match resolve_aliases(environment, ty, 0) {
        Type::Alias(name) => lua::type_field_items(&name, field)
            .into_iter()
            .find(|item| item.label == field),
        Type::Union(types) => types
            .iter()
            .find_map(|ty| lua_type_field_item(environment, ty, field)),
        Type::Any
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::Table(_)
        | Type::StringLiteral(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. } => None,
    }
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

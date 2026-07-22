use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::deprecation::deprecated_declarations;
use crate::love_callback;
use crate::lua::{self, LuaItem};
use crate::module_index::{
    required_module_export_for_fields, required_module_for_receiver, resolve_module_specifier,
};
use crate::module_link::module_specifier_at_position;
use crate::narrowing::narrowed_type_at_position;
use crate::semantic::{resolve_aliases, SemanticModel};
use crate::source_text::dotted_chain_at_position;
use crate::types::{
    build_type_environment, format_named_function_pretty, format_type_with_prefix,
    type_from_tua_type, FunctionParameter, FunctionType, Type, TypeEnvironment, TypedFunction,
    TypedFunctionOverload,
};
use crate::{
    definition_parsed_source, ecs, input, object_pool, parse_tua_source, state_machine, store,
    Definition, DefinitionKind, Diagnostic, ParameterAnnotation, ParsedSource, SourcePosition,
    SourceSpan,
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
    let environment = ecs::augment_environment(build_type_environment(parsed)?);

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

            if field_parts.len() == 2 && field_parts[0] == "love" {
                let callback_name = format!("love.{}", current_part.text);

                if self.environment.local("love").is_none()
                    || self.environment.function(&callback_name).is_some()
                {
                    if let Some(item) = love_callback::item(&current_part.text) {
                        return Some(lua_item_hover(item, Some(current_part.span)));
                    }
                }
            }

            if let Some(receiver_type) = self
                .semantic
                .type_of_field_chain(&field_parts[..field_parts.len() - 1], position)
            {
                if let Some(function) =
                    ecs::typed_member(self.environment, &receiver_type, &current_part.text)
                        .or_else(|| {
                            object_pool::typed_member(
                                self.environment,
                                &receiver_type,
                                &current_part.text,
                            )
                        })
                        .or_else(|| {
                            state_machine::typed_member(
                                self.environment,
                                &receiver_type,
                                &current_part.text,
                            )
                        })
                        .or_else(|| {
                            store::typed_member(
                                self.environment,
                                &receiver_type,
                                &current_part.text,
                            )
                        })
                        .or_else(|| {
                            input::typed_member(
                                self.environment,
                                &receiver_type,
                                &current_part.text,
                            )
                        })
                {
                    return Some(self.function_hover(&function, position, current_part.span));
                }
            }

            if let Some(item) =
                self.love_object_field_item(&field_parts, position, &current_part.text)
            {
                return Some(lua_item_hover(item, Some(current_part.span)));
            }

            if let Some(ty) = self.semantic.type_of_field_chain(&field_parts, position) {
                return Some(hover(
                    format_type_with_prefix(format!("(field) {}: ", current_part.text), &ty),
                    Some(current_part.span),
                ));
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

        if current_part.text == ecs::MODULE_NAME
            && self
                .environment
                .local(ecs::MODULE_NAME)
                .is_some_and(|ty| ecs::is_module_type(self.environment, ty))
        {
            let mut hover = hover("(built-in module) ecs", Some(current_part.span));
            hover.documentation = Some(ecs::module_documentation().to_owned());

            return Some(hover);
        }

        if let Some(ty) = self.environment.alias(&current_part.text) {
            let contents = format_type_with_prefix(format!("type {} = ", current_part.text), ty);

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
            if let Some(hover) =
                object_pool_local_hover(self.environment, &current_part.text, ty, current_part.span)
            {
                return Some(hover);
            }
            if let Some(hover) = state_machine_local_hover(
                self.environment,
                &current_part.text,
                ty,
                current_part.span,
            ) {
                return Some(hover);
            }
            if let Some(hover) =
                store_local_hover(self.environment, &current_part.text, ty, current_part.span)
            {
                return Some(hover);
            }
            if let Some(hover) =
                input_local_hover(self.environment, &current_part.text, ty, current_part.span)
            {
                return Some(hover);
            }

            let contents = format_type_with_prefix(format!("local {}: ", current_part.text), ty);
            let deprecation = self
                .deprecation_at_position(position)
                .map(|item| item.markdown());
            let documentation = merge_documentation([
                deprecation.as_deref(),
                self.environment.local_documentation(&current_part.text),
            ]);

            if documentation.is_some() {
                let mut hover = hover(contents, Some(current_part.span));
                hover.documentation = documentation;
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

        Some(hover(
            format_type_with_prefix("expression: ", &ty),
            Some(span),
        ))
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
            contents: format_type_with_prefix(format!("parameter {name}: "), &ty),
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
        if let Some(hover) = object_pool_local_hover(self.environment, name, &ty, hover_span) {
            return Some(hover);
        }
        if let Some(hover) = state_machine_local_hover(self.environment, name, &ty, hover_span) {
            return Some(hover);
        }
        if let Some(hover) = store_local_hover(self.environment, name, &ty, hover_span) {
            return Some(hover);
        }
        if let Some(hover) = input_local_hover(self.environment, name, &ty, hover_span) {
            return Some(hover);
        }

        let contents = format_type_with_prefix(format!("local {name}: "), &ty);

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
        let deprecation = export
            .deprecation
            .as_ref()
            .map(crate::Deprecation::markdown);

        hover.documentation = merge_documentation([
            deprecation.as_deref(),
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
        let deprecation = definition
            .as_ref()
            .and_then(|definition| self.deprecation_for_definition(definition))
            .map(|deprecation| deprecation.markdown());
        let mut hover = hover(format_function_signature(function), Some(span));

        hover.documentation = merge_documentation([
            deprecation.as_deref(),
            source_documentation.as_deref(),
            function.documentation.as_deref(),
        ]);

        if hover.documentation.is_some() {
            hover.context = declaration_context(definition.as_ref(), Some("function"));
        }

        hover
    }

    fn deprecation_at_position(&self, position: SourcePosition) -> Option<crate::Deprecation> {
        let definition = definition_parsed_source(self.parsed, position)
            .ok()
            .flatten()?;

        self.deprecation_for_definition(&definition)
    }

    fn deprecation_for_definition(&self, definition: &Definition) -> Option<crate::Deprecation> {
        deprecated_declarations(self.parsed)
            .into_iter()
            .find(|declaration| declaration.name_span == definition.span)
            .map(|declaration| declaration.deprecation)
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
    let parameter_start = usize::from(
        name.contains(':')
            && function
                .parameters
                .first()
                .is_some_and(|parameter| parameter.name == "self"),
    );

    let mut signatures = Vec::new();

    if should_show_primary_typed_signature(function) {
        let parameters = function
            .parameters
            .iter()
            .skip(parameter_start)
            .map(|parameter| FunctionParameter::named(&parameter.name, parameter.ty.clone()))
            .collect::<Vec<_>>();
        signatures.push(format_named_function_pretty(
            name,
            &parameters,
            function.has_varargs,
            function.vararg_type.as_ref(),
            &function.return_types_or_any(),
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
    let parameters = overload
        .parameters
        .iter()
        .map(|parameter| {
            if parameter.name.is_empty() {
                FunctionParameter::anonymous(parameter.ty.clone())
            } else {
                FunctionParameter::named(&parameter.name, parameter.ty.clone())
            }
        })
        .collect::<Vec<_>>();

    format_named_function_pretty(
        name,
        &parameters,
        overload.has_varargs,
        overload.vararg_type.as_ref(),
        &overload.return_types_or_any(),
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
        DefinitionKind::Asset => "asset",
        DefinitionKind::SignalEvent => "signal event",
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
        _ => format_type_with_prefix(format!("(field) {qualified_name}: "), ty),
    }
}

fn format_export_function(name: &str, function: &FunctionType) -> String {
    format_named_function_pretty(
        name,
        &function.parameters,
        function.vararg_type.is_some(),
        function.vararg_type.as_deref(),
        function.return_types_or_any(),
    )
}

fn lua_type_field_item(environment: &TypeEnvironment, ty: &Type, field: &str) -> Option<LuaItem> {
    if object_pool::pool_item_type(environment, ty).is_some() {
        return lua::type_field_items(object_pool::TYPE_NAME, field)
            .into_iter()
            .find(|item| item.label == field);
    }

    if state_machine::type_is_state_machine(environment, ty) {
        if let Some(item) = lua::type_field_items(state_machine::TYPE_NAME, field)
            .into_iter()
            .find(|item| item.label == field)
        {
            return Some(item);
        }
    }

    if store::store_state_type(environment, ty).is_some() {
        if let Some(item) = lua::type_field_items(store::TYPE_NAME, field)
            .into_iter()
            .find(|item| item.label == field)
        {
            return Some(item);
        }
    }

    if input::type_is_input(environment, ty) {
        if let Some(item) = lua::type_field_items(input::TYPE_NAME, field)
            .into_iter()
            .find(|item| item.label == field)
        {
            return Some(item);
        }
    }

    if let Type::Alias(name) = ty {
        if let Some(item) = lua::type_field_items(name, field)
            .into_iter()
            .find(|item| item.label == field)
        {
            return Some(item);
        }
    }

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

fn object_pool_local_hover(
    environment: &TypeEnvironment,
    name: &str,
    ty: &Type,
    span: SourceSpan,
) -> Option<Hover> {
    let item_type = object_pool::pool_item_type(environment, ty)?;
    let documentation = (item_type != Type::Any)
        .then(|| format!("Acquires and releases values of type `{item_type}`."));

    Some(Hover {
        contents: format!("local {name}: {}", object_pool::TYPE_NAME),
        span: Some(span),
        documentation,
        context: Vec::new(),
    })
}

fn state_machine_local_hover(
    environment: &TypeEnvironment,
    name: &str,
    ty: &Type,
    span: SourceSpan,
) -> Option<Hover> {
    let (states, events) = state_machine::type_summary(environment, ty)?;

    Some(Hover {
        contents: format!("local {name}: {}", state_machine::TYPE_NAME),
        span: Some(span),
        documentation: Some(format!("States: `{states}`. Events: `{events}`.")),
        context: Vec::new(),
    })
}

fn store_local_hover(
    environment: &TypeEnvironment,
    name: &str,
    ty: &Type,
    span: SourceSpan,
) -> Option<Hover> {
    let state_type = store::store_state_type(environment, ty)?;
    let documentation =
        (state_type != Type::Any).then(|| format!("Holds runtime state of type `{state_type}`."));

    Some(Hover {
        contents: format!("local {name}: {}", store::TYPE_NAME),
        span: Some(span),
        documentation,
        context: Vec::new(),
    })
}

fn input_local_hover(
    environment: &TypeEnvironment,
    name: &str,
    ty: &Type,
    span: SourceSpan,
) -> Option<Hover> {
    let (spec, exact) = input::type_summary(environment, ty)?;
    let documentation = exact.then(|| {
        let controls = if spec.controls.is_empty() {
            "none".to_owned()
        } else {
            spec.controls.join(", ")
        };
        let pairs = if spec.pairs.is_empty() {
            "none".to_owned()
        } else {
            spec.pairs.join(", ")
        };
        format!("Controls: `{controls}`. Pairs: `{pairs}`.")
    });

    Some(Hover {
        contents: format!("local {name}: {}", input::TYPE_NAME),
        span: Some(span),
        documentation,
        context: Vec::new(),
    })
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

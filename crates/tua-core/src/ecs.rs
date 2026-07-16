use std::collections::BTreeMap;

use crate::plugin::{PluginContext, TuaPlugin};
use crate::types::{FunctionType, TableField, TableShape, Type};
use crate::{Diagnostic, DiagnosticCode, DiagnosticSeverity};

pub(crate) const WORLD_TYPE: &str = "EcsWorld";
pub(crate) const ENTITY_ID_TYPE: &str = "EcsEntityId";
pub(crate) const COMPONENT_TYPE: &str = "EcsComponent";
pub(crate) const SYSTEM_TYPE: &str = "EcsSystem";
pub(crate) const QUERY_TYPE: &str = "EcsQuery";

const BUILTIN_TYPE_NAMES: &[&str] = &[
    WORLD_TYPE,
    ENTITY_ID_TYPE,
    COMPONENT_TYPE,
    SYSTEM_TYPE,
    QUERY_TYPE,
];

pub(crate) fn builtin_type_names() -> &'static [&'static str] {
    BUILTIN_TYPE_NAMES
}

pub(crate) fn builtin_aliases() -> Vec<(String, Type)> {
    [
        (WORLD_TYPE.to_owned(), world_type()),
        (ENTITY_ID_TYPE.to_owned(), Type::number()),
        (COMPONENT_TYPE.to_owned(), component_type()),
        (SYSTEM_TYPE.to_owned(), system_type()),
        (QUERY_TYPE.to_owned(), query_type()),
    ]
    .into()
}

pub(crate) fn builtin_locals() -> Vec<(String, Type)> {
    vec![("ecs".to_owned(), ecs_module_type())]
}

#[derive(Clone, Debug, Default)]
pub(crate) struct EcsPlugin;

impl TuaPlugin for EcsPlugin {
    fn name(&self) -> &'static str {
        "tua.ecs"
    }

    fn diagnostics(&self, context: PluginContext<'_>) -> Vec<Diagnostic> {
        diagnostics_for_source(context)
    }
}

fn ecs_module_type() -> Type {
    Type::table([
        (
            "world",
            TableField::required(Type::Function(FunctionType::new(
                [],
                Type::alias(WORLD_TYPE),
            ))),
        ),
        (
            "component",
            TableField::required(Type::Function(FunctionType::new(
                [Type::string(), Type::empty_table()],
                Type::alias(COMPONENT_TYPE),
            ))),
        ),
        (
            "system",
            TableField::required(Type::Function(FunctionType::new(
                [
                    Type::alias(WORLD_TYPE),
                    Type::array(Type::alias(COMPONENT_TYPE)),
                    Type::function([], Type::nil()),
                ],
                Type::alias(SYSTEM_TYPE),
            ))),
        ),
    ])
}

fn world_type() -> Type {
    Type::Table(TableShape::from_fields([
        (
            "create",
            TableField::required(Type::Function(FunctionType::new(
                [Type::alias(WORLD_TYPE)],
                Type::alias(ENTITY_ID_TYPE),
            ))),
        ),
        (
            "destroy",
            TableField::required(Type::Function(FunctionType::new(
                [Type::alias(WORLD_TYPE), Type::alias(ENTITY_ID_TYPE)],
                Type::nil(),
            ))),
        ),
        (
            "add",
            TableField::required(Type::Function(FunctionType::new(
                [
                    Type::alias(WORLD_TYPE),
                    Type::alias(ENTITY_ID_TYPE),
                    Type::alias(COMPONENT_TYPE),
                    Type::empty_table(),
                ],
                Type::nil(),
            ))),
        ),
        (
            "get",
            TableField::required(Type::Function(FunctionType::new(
                [
                    Type::alias(WORLD_TYPE),
                    Type::alias(ENTITY_ID_TYPE),
                    Type::alias(COMPONENT_TYPE),
                ],
                Type::empty_table(),
            ))),
        ),
        (
            "has",
            TableField::required(Type::Function(FunctionType::new(
                [
                    Type::alias(WORLD_TYPE),
                    Type::alias(ENTITY_ID_TYPE),
                    Type::alias(COMPONENT_TYPE),
                ],
                Type::boolean(),
            ))),
        ),
        (
            "remove",
            TableField::required(Type::Function(FunctionType::new(
                [
                    Type::alias(WORLD_TYPE),
                    Type::alias(ENTITY_ID_TYPE),
                    Type::alias(COMPONENT_TYPE),
                ],
                Type::nil(),
            ))),
        ),
        (
            "query",
            TableField::required(Type::Function(FunctionType::new(
                [
                    Type::alias(WORLD_TYPE),
                    Type::array(Type::alias(COMPONENT_TYPE)),
                ],
                Type::alias(QUERY_TYPE),
            ))),
        ),
    ]))
}

fn component_type() -> Type {
    Type::table([
        ("name", TableField::required(Type::string())),
        ("schema", TableField::required(Type::empty_table())),
    ])
}

fn system_type() -> Type {
    Type::table([
        ("enabled", TableField::required(Type::boolean())),
        (
            "update",
            TableField::required(Type::function([], Type::nil())),
        ),
    ])
}

fn query_type() -> Type {
    Type::table([
        (
            "each",
            TableField::required(Type::function(
                [Type::function([], Type::nil())],
                Type::nil(),
            )),
        ),
        (
            "count",
            TableField::required(Type::function([], Type::number())),
        ),
    ])
}

fn diagnostics_for_source(context: PluginContext<'_>) -> Vec<Diagnostic> {
    let schemas = collect_component_schemas(context.source);
    let mut diagnostics = Vec::new();

    for (line_index, line) in context.source.lines().enumerate() {
        let line_number = line_index + 1;

        if let Some(add_call) = parse_add_call(line) {
            match schemas.get(add_call.component.as_str()) {
                Some(schema) => diagnostics.extend(component_value_diagnostics(
                    context,
                    line_number,
                    line,
                    &add_call,
                    schema,
                )),
                None => diagnostics.push(ecs_query_diagnostic(
                    context,
                    line_number,
                    add_call.component_column,
                    format!("unknown ECS component `{}`", add_call.component),
                )),
            }
        }

        if let Some(query) = parse_query_components(line) {
            if query.components.is_empty() {
                diagnostics.push(ecs_query_diagnostic(
                    context,
                    line_number,
                    query.column,
                    "ECS system/query requires at least one component".to_owned(),
                ));
            }

            for component in query.components {
                if !schemas.contains_key(component.name.as_str()) {
                    diagnostics.push(ecs_query_diagnostic(
                        context,
                        line_number,
                        component.column,
                        format!("unknown ECS component `{}` in system/query", component.name),
                    ));
                }
            }
        }
    }

    diagnostics
}

#[derive(Clone, Debug)]
struct ComponentSchema {
    fields: BTreeMap<String, SchemaField>,
}

#[derive(Clone, Debug)]
struct SchemaField {
    ty: String,
}

#[derive(Clone, Debug)]
struct AddCall {
    component: String,
    component_column: usize,
    values: BTreeMap<String, ValueField>,
    values_column: usize,
}

#[derive(Clone, Debug)]
struct ValueField {
    ty: String,
    column: usize,
}

#[derive(Clone, Debug)]
struct QueryComponents {
    components: Vec<QueryComponent>,
    column: usize,
}

#[derive(Clone, Debug)]
struct QueryComponent {
    name: String,
    column: usize,
}

fn collect_component_schemas(source: &str) -> BTreeMap<String, ComponentSchema> {
    source
        .lines()
        .filter_map(parse_component_schema)
        .collect::<BTreeMap<_, _>>()
}

fn parse_component_schema(line: &str) -> Option<(String, ComponentSchema)> {
    let local_name = line.trim_start().strip_prefix("local ")?;
    let (local_name, rest) = local_name.split_once('=')?;
    let local_name = local_name.trim();
    let rest = rest.trim();
    let args = rest.strip_prefix("ecs.component(")?;
    let args = args.strip_suffix(')')?;
    let schema_source = split_top_level_args(args).get(1).copied()?;
    let fields = parse_schema_fields(schema_source);

    Some((local_name.to_owned(), ComponentSchema { fields }))
}

fn parse_add_call(line: &str) -> Option<AddCall> {
    let call_start = line.find(":add(")?;
    let args_start = call_start + ":add(".len();
    let args = line.get(args_start..)?.strip_suffix(')')?;
    let args = split_top_level_args(args);
    let component = args.get(1)?.trim();
    let values_source = args.get(2)?.trim();
    let component_column = line.find(component).map(|index| index + 1).unwrap_or(1);
    let values_column = line.find(values_source).map(|index| index + 1).unwrap_or(1);

    Some(AddCall {
        component: component.to_owned(),
        component_column,
        values: parse_value_fields(values_source),
        values_column,
    })
}

fn parse_query_components(line: &str) -> Option<QueryComponents> {
    let query_start = line
        .find("ecs.system(")
        .map(|index| (index, "ecs.system(".len(), 1))
        .or_else(|| {
            line.find(":query(")
                .map(|index| (index, ":query(".len(), 0))
        })?;
    let args_start = query_start.0 + query_start.1;
    let args = line.get(args_start..)?.strip_suffix(')')?;
    let args = split_top_level_args(args);
    let components_source = args.get(query_start.2)?.trim();
    let column = line
        .find(components_source)
        .map(|index| index + 1)
        .unwrap_or(1);
    let inner = table_inner(components_source)?;
    let components = split_top_level_args(inner)
        .into_iter()
        .filter_map(|component| {
            let name = component.trim();

            if name.is_empty() {
                return None;
            }

            Some(QueryComponent {
                name: name.to_owned(),
                column: line.find(name).map(|index| index + 1).unwrap_or(column),
            })
        })
        .collect();

    Some(QueryComponents { components, column })
}

fn parse_schema_fields(source: &str) -> BTreeMap<String, SchemaField> {
    let Some(inner) = table_inner(source.trim()) else {
        return BTreeMap::new();
    };

    split_top_level_args(inner)
        .into_iter()
        .filter_map(|field| {
            let (name, value) = field.split_once('=')?;
            let name = name.trim().to_owned();
            let value = value.trim();
            let ty = value.trim_matches('"').trim_matches('\'').to_owned();
            Some((name, SchemaField { ty }))
        })
        .collect()
}

fn parse_value_fields(source: &str) -> BTreeMap<String, ValueField> {
    let Some(inner) = table_inner(source.trim()) else {
        return BTreeMap::new();
    };

    split_top_level_args(inner)
        .into_iter()
        .filter_map(|field| {
            let (name, value) = field.split_once('=')?;
            let name = name.trim().to_owned();
            let value = value.trim();
            let ty = infer_literal_type(value);
            let column = source
                .find(name.as_str())
                .map(|index| index + 1)
                .unwrap_or(1);

            Some((name, ValueField { ty, column }))
        })
        .collect()
}

fn component_value_diagnostics(
    context: PluginContext<'_>,
    line_number: usize,
    line: &str,
    add_call: &AddCall,
    schema: &ComponentSchema,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for (name, field) in &schema.fields {
        match add_call.values.get(name) {
            Some(value) if value.ty == field.ty || field.ty == "any" || value.ty == "any" => {}
            Some(value) => diagnostics.push(ecs_component_value_diagnostic(
                context,
                line_number,
                line.find(name)
                    .map(|index| index + 1)
                    .unwrap_or(value.column),
                format!(
                    "ECS component field `{name}` expected {}, found {}",
                    field.ty, value.ty
                ),
            )),
            None => diagnostics.push(ecs_component_value_diagnostic(
                context,
                line_number,
                add_call.values_column,
                format!("ECS component value is missing required field `{name}`"),
            )),
        }
    }

    for (name, value) in &add_call.values {
        if !schema.fields.contains_key(name) {
            diagnostics.push(ecs_component_value_diagnostic(
                context,
                line_number,
                line.find(name)
                    .map(|index| index + 1)
                    .unwrap_or(value.column),
                format!("ECS component value includes unknown field `{name}`"),
            ));
        }
    }

    diagnostics
}

fn split_top_level_args(source: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut quote: Option<char> = None;

    for (index, ch) in source.char_indices() {
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => quote = Some(ch),
            '{' | '(' | '[' => depth += 1,
            '}' | ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                args.push(source[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }

    args.push(source[start..].trim());
    args
}

fn table_inner(source: &str) -> Option<&str> {
    source.strip_prefix('{')?.strip_suffix('}')
}

fn infer_literal_type(value: &str) -> String {
    let value = value.trim();

    if value.starts_with('"') || value.starts_with('\'') {
        "string".to_owned()
    } else if value == "true" || value == "false" {
        "boolean".to_owned()
    } else if value.parse::<f64>().is_ok() {
        "number".to_owned()
    } else if value == "nil" {
        "nil".to_owned()
    } else {
        "any".to_owned()
    }
}

fn ecs_component_value_diagnostic(
    context: PluginContext<'_>,
    line: usize,
    column: usize,
    message: String,
) -> Diagnostic {
    Diagnostic::new(
        context.path,
        line,
        column,
        DiagnosticSeverity::Error,
        message,
    )
    .with_code(DiagnosticCode::PLUGIN_ECS_COMPONENT_VALUE)
}

fn ecs_query_diagnostic(
    context: PluginContext<'_>,
    line: usize,
    column: usize,
    message: String,
) -> Diagnostic {
    Diagnostic::new(
        context.path,
        line,
        column,
        DiagnosticSeverity::Error,
        message,
    )
    .with_code(DiagnosticCode::PLUGIN_ECS_QUERY)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn diagnostics_check_component_values_and_queries() {
        let source = r#"local Position = ecs.component("Position", { x = "number", y = "number" })
local world = ecs.world()
local entity = world:create()
world:add(entity, Position, { x = 1, name = "wrong" })
ecs.system(world, { Position, Missing }, function() end)
"#;

        let diagnostics = diagnostics_for_source(PluginContext {
            path: Path::new("src/ecs.tua"),
            source,
            parsed: None,
            assets: &[],
        });

        assert_eq!(diagnostics.len(), 3);
        assert_eq!(
            diagnostics[0].code,
            Some(DiagnosticCode::PLUGIN_ECS_COMPONENT_VALUE)
        );
        assert_eq!(diagnostics[2].code, Some(DiagnosticCode::PLUGIN_ECS_QUERY));
    }
}

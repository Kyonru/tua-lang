use std::collections::BTreeMap;
use std::path::Path;

use crate::types::{TableField, TableShape, Type, TypeEnvironment};
use crate::{Diagnostic, DiagnosticCode, DiagnosticSeverity, TypedFunction, TypedParameter};

pub(crate) const MODULE_NAME: &str = "ecs";
pub(crate) const RUNTIME_MODULE: &str = "tuaecs";

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
    vec![(MODULE_NAME.to_owned(), ecs_module_type())]
}

pub(crate) fn augment_environment(environment: TypeEnvironment) -> TypeEnvironment {
    environment
        .with_alias_types(builtin_aliases())
        .with_local_types(builtin_locals())
}

fn ecs_module_type() -> Type {
    Type::table([
        (
            "world",
            TableField::required(typed_module_function("world").unwrap().as_type().unwrap()),
        ),
        (
            "component",
            TableField::required(
                typed_module_function("component")
                    .unwrap()
                    .as_type()
                    .unwrap(),
            ),
        ),
        (
            "system",
            TableField::required(typed_module_function("system").unwrap().as_type().unwrap()),
        ),
    ])
}

fn world_type() -> Type {
    Type::Table(TableShape::from_fields([
        (
            "create",
            TableField::required(
                typed_method(WORLD_TYPE, "create")
                    .unwrap()
                    .as_type()
                    .unwrap(),
            ),
        ),
        (
            "destroy",
            TableField::required(
                typed_method(WORLD_TYPE, "destroy")
                    .unwrap()
                    .as_type()
                    .unwrap(),
            ),
        ),
        (
            "add",
            TableField::required(typed_method(WORLD_TYPE, "add").unwrap().as_type().unwrap()),
        ),
        (
            "get",
            TableField::required(typed_method(WORLD_TYPE, "get").unwrap().as_type().unwrap()),
        ),
        (
            "has",
            TableField::required(typed_method(WORLD_TYPE, "has").unwrap().as_type().unwrap()),
        ),
        (
            "remove",
            TableField::required(
                typed_method(WORLD_TYPE, "remove")
                    .unwrap()
                    .as_type()
                    .unwrap(),
            ),
        ),
        (
            "query",
            TableField::required(
                typed_method(WORLD_TYPE, "query")
                    .unwrap()
                    .as_type()
                    .unwrap(),
            ),
        ),
        (
            "clear",
            TableField::required(
                typed_method(WORLD_TYPE, "clear")
                    .unwrap()
                    .as_type()
                    .unwrap(),
            ),
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
        ("query", TableField::required(Type::alias(QUERY_TYPE))),
        (
            "update",
            TableField::required(
                typed_method(SYSTEM_TYPE, "update")
                    .unwrap()
                    .as_type()
                    .unwrap(),
            ),
        ),
    ])
}

fn query_type() -> Type {
    Type::table([
        (
            "each",
            TableField::required(typed_method(QUERY_TYPE, "each").unwrap().as_type().unwrap()),
        ),
        (
            "count",
            TableField::required(
                typed_method(QUERY_TYPE, "count")
                    .unwrap()
                    .as_type()
                    .unwrap(),
            ),
        ),
    ])
}

pub(crate) fn typed_module_function(name: &str) -> Option<TypedFunction> {
    match name {
        "world" => Some(typed_function(
            name,
            [],
            Type::alias(WORLD_TYPE),
            "Create an empty ECS world that owns entities and component storage.",
        )),
        "component" => Some(typed_function(
            name,
            [
                parameter("name", Type::string()),
                parameter("schema", Type::empty_table()),
            ],
            Type::alias(COMPONENT_TYPE),
            "Declare a reusable component and its editor-checked data schema.",
        )),
        "system" => Some(typed_function(
            name,
            [
                parameter("world", Type::alias(WORLD_TYPE)),
                parameter("components", Type::array(Type::alias(COMPONENT_TYPE))),
                parameter("update", Type::Any),
            ],
            Type::alias(SYSTEM_TYPE),
            "Create a system that runs a callback for every matching entity when updated.",
        )),
        _ => None,
    }
}

pub(crate) fn typed_method(type_name: &str, method: &str) -> Option<TypedFunction> {
    let self_parameter = parameter("self", Type::alias(type_name));
    let entity = || parameter("entity", Type::alias(ENTITY_ID_TYPE));
    let component = || parameter("component", Type::alias(COMPONENT_TYPE));

    match (type_name, method) {
        (WORLD_TYPE, "create") => Some(typed_function(
            method,
            [self_parameter],
            Type::alias(ENTITY_ID_TYPE),
            "Create an entity ID in this world.",
        )),
        (WORLD_TYPE, "destroy") => Some(typed_function(
            method,
            [self_parameter, entity()],
            Type::nil(),
            "Destroy an entity and remove all of its component data.",
        )),
        (WORLD_TYPE, "add") => Some(typed_function(
            method,
            [
                self_parameter,
                entity(),
                component(),
                parameter("value", Type::Any),
            ],
            Type::Any,
            "Attach component data to an entity and return that data.",
        )),
        (WORLD_TYPE, "get") => Some(typed_function(
            method,
            [self_parameter, entity(), component()],
            Type::Any,
            "Return an entity's component data, or nil when it is absent.",
        )),
        (WORLD_TYPE, "has") => Some(typed_function(
            method,
            [self_parameter, entity(), component()],
            Type::boolean(),
            "Return whether an entity has a component.",
        )),
        (WORLD_TYPE, "remove") => Some(typed_function(
            method,
            [self_parameter, entity(), component()],
            Type::Any,
            "Remove and return an entity's component data.",
        )),
        (WORLD_TYPE, "query") => Some(typed_function(
            method,
            [
                self_parameter,
                parameter("components", Type::array(Type::alias(COMPONENT_TYPE))),
            ],
            Type::alias(QUERY_TYPE),
            "Create a reusable query matching entities with every listed component.",
        )),
        (WORLD_TYPE, "clear") => Some(typed_function(
            method,
            [self_parameter],
            Type::nil(),
            "Destroy every entity and clear all component storage.",
        )),
        (QUERY_TYPE, "each") => Some(typed_function(
            method,
            [self_parameter, parameter("visit", Type::Any)],
            Type::nil(),
            "Visit matching entities as visit(entity, componentValue...).",
        )),
        (QUERY_TYPE, "count") => Some(typed_function(
            method,
            [self_parameter],
            Type::number(),
            "Count entities currently matched by this query.",
        )),
        (SYSTEM_TYPE, "update") => Some(typed_function(
            method,
            [self_parameter, parameter("dt", Type::number())],
            Type::nil(),
            "Run this system once for all matching entities when enabled.",
        )),
        _ => None,
    }
}

pub(crate) fn typed_member(
    environment: &TypeEnvironment,
    receiver_type: &Type,
    member: &str,
) -> Option<TypedFunction> {
    if is_module_type(environment, receiver_type) {
        return typed_module_function(member);
    }

    [WORLD_TYPE, QUERY_TYPE, SYSTEM_TYPE]
        .into_iter()
        .find(|type_name| {
            type_matches(
                environment,
                receiver_type,
                &Type::alias(*type_name),
                Some(type_name),
            )
        })
        .and_then(|type_name| typed_method(type_name, member))
}

pub(crate) fn is_module_type(environment: &TypeEnvironment, candidate: &Type) -> bool {
    type_matches(environment, candidate, &ecs_module_type(), None)
}

pub(crate) const fn module_documentation() -> &'static str {
    "Built-in data-oriented ECS helpers. Define components, create a world, then update matching entities through queries or systems."
}

pub(crate) const fn workflow_snippet() -> &'static str {
    "local ${1:Position} = ecs.component(\"${1:Position}\", { ${2:x} = \"${3:number}\" })\nlocal ${4:world} = ecs.world()\nlocal ${5:entity} = ${4:world}:create()\n${4:world}:add(${5:entity}, ${1:Position}, { ${2:x} = ${6:0} })\n${0}"
}

pub(crate) fn runtime_source() -> &'static str {
    include_str!("../../../vendor/runtime/tuaecs.lua")
}

fn typed_function(
    name: &str,
    parameters: impl IntoIterator<Item = TypedParameter>,
    return_type: Type,
    documentation: &str,
) -> TypedFunction {
    let parameters = parameters.into_iter().collect::<Vec<_>>();

    TypedFunction {
        name: Some(name.to_owned()),
        parameter_count: parameters.len(),
        required_parameter_count: parameters.len(),
        parameters,
        has_varargs: false,
        vararg_type: None,
        return_type: Some(return_type.clone()),
        return_types: vec![return_type],
        documentation: Some(documentation.to_owned()),
        overloads: Vec::new(),
    }
}

fn parameter(name: &str, ty: Type) -> TypedParameter {
    TypedParameter {
        name: name.to_owned(),
        ty,
        documentation: None,
    }
}

fn type_matches(
    environment: &TypeEnvironment,
    candidate: &Type,
    expected: &Type,
    alias_name: Option<&str>,
) -> bool {
    if candidate == expected {
        return true;
    }

    match candidate {
        Type::Alias(name) => alias_name.is_some_and(|expected_name| name == expected_name),
        Type::Union(types) => types
            .iter()
            .any(|ty| type_matches(environment, ty, expected, alias_name)),
        _ => alias_name
            .and_then(|name| environment.alias(name))
            .is_some_and(|resolved| candidate == resolved),
    }
}

pub(crate) fn diagnostics(path: &Path, source: &str) -> Vec<Diagnostic> {
    let schemas = collect_component_schemas(source);
    let mut diagnostics = Vec::new();

    for (line_index, line) in source.lines().enumerate() {
        let line_number = line_index + 1;

        if let Some(add_call) = parse_add_call(line) {
            match schemas.get(add_call.component.as_str()) {
                Some(schema) => diagnostics.extend(component_value_diagnostics(
                    path,
                    line_number,
                    line,
                    &add_call,
                    schema,
                )),
                None => diagnostics.push(ecs_query_diagnostic(
                    path,
                    line_number,
                    add_call.component_column,
                    format!("unknown ECS component `{}`", add_call.component),
                )),
            }
        }

        if let Some(query) = parse_query_components(line) {
            if query.components.is_empty() {
                diagnostics.push(ecs_query_diagnostic(
                    path,
                    line_number,
                    query.column,
                    "ECS system/query requires at least one component".to_owned(),
                ));
            }

            for component in query.components {
                if !schemas.contains_key(component.name.as_str()) {
                    diagnostics.push(ecs_query_diagnostic(
                        path,
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
    path: &Path,
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
                path,
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
                path,
                line_number,
                add_call.values_column,
                format!("ECS component value is missing required field `{name}`"),
            )),
        }
    }

    for (name, value) in &add_call.values {
        if !schema.fields.contains_key(name) {
            diagnostics.push(ecs_component_value_diagnostic(
                path,
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
    path: &Path,
    line: usize,
    column: usize,
    message: String,
) -> Diagnostic {
    Diagnostic::new(path, line, column, DiagnosticSeverity::Error, message)
        .with_code(DiagnosticCode::DOMAIN_ECS_COMPONENT_VALUE)
}

fn ecs_query_diagnostic(path: &Path, line: usize, column: usize, message: String) -> Diagnostic {
    Diagnostic::new(path, line, column, DiagnosticSeverity::Error, message)
        .with_code(DiagnosticCode::DOMAIN_ECS_QUERY)
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

        let diagnostics = diagnostics(Path::new("src/ecs.tua"), source);

        assert_eq!(diagnostics.len(), 3);
        assert_eq!(
            diagnostics[0].code,
            Some(DiagnosticCode::DOMAIN_ECS_COMPONENT_VALUE)
        );
        assert_eq!(diagnostics[2].code, Some(DiagnosticCode::DOMAIN_ECS_QUERY));
    }
}

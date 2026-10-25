use full_moon::ast::{
    Call, Expression, Field, FunctionArgs, FunctionCall, Suffix, TableConstructor,
};
use full_moon::node::Node;
use full_moon::visitors::Visitor;

use crate::ast_utils::{
    field_chain_before_first_call, plain_field_chain_from_expression, token_text,
};
use crate::semantic::SemanticModel;
use crate::types::{
    AssignabilityFailure, AssignabilityFailureKind, AssignabilityPathSegment, TableField,
    TableShape, Type, TypeEnvironment,
};
use crate::{
    Diagnostic, DiagnosticCode, DiagnosticSeverity, ParsedSource, SourcePosition, SourceSpan,
    TypedFunction, TypedParameter,
};

mod inspection;

pub(crate) use inspection::{ecs_file_index, ecs_graph, EcsFileIndex};
pub use inspection::{
    EcsAccessKind, EcsComponent, EcsComponentField, EcsComponentUse, EcsGraph, EcsInsight,
    EcsInsightKind, EcsQuery, EcsQueryKind, EcsSystem,
};

pub(crate) const MODULE_NAME: &str = "ecs";
pub(crate) const RUNTIME_MODULE: &str = "tuaecs";

pub(crate) const WORLD_TYPE: &str = "EcsWorld";
pub(crate) const ENTITY_ID_TYPE: &str = "EcsEntityId";
pub(crate) const COMPONENT_TYPE: &str = "EcsComponent";
pub(crate) const SYSTEM_TYPE: &str = "EcsSystem";
pub(crate) const QUERY_TYPE: &str = "EcsQuery";

const COMPONENT_VALUE_FIELD: &str = "__tua_ecs_component_value";

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
        (COMPONENT_TYPE.to_owned(), component_type(Type::Any)),
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

fn component_type(value_type: Type) -> Type {
    let mut table = TableShape::from_fields([
        ("name", TableField::required(Type::string())),
        ("schema", TableField::required(Type::empty_table())),
        (COMPONENT_VALUE_FIELD, TableField::required(value_type)),
    ]);
    table.mark_nominal(COMPONENT_TYPE, COMPONENT_TYPE, []);

    Type::Table(table)
}

pub(crate) fn inferred_method_return_types(
    receiver_type: &Type,
    call: &FunctionCall,
    component_argument_type: Option<&Type>,
) -> Option<Vec<Type>> {
    let parts = field_chain_before_first_call(call)?;
    let method = parts.last()?.as_str();

    if is_module_type_direct(receiver_type) && method == "component" {
        return Some(vec![
            component_type_from_call(call).unwrap_or_else(|| Type::alias(COMPONENT_TYPE))
        ]);
    }

    if !is_world_type_direct(receiver_type) || !matches!(method, "add" | "get" | "remove") {
        return None;
    }

    Some(vec![component_argument_type
        .and_then(component_value_type)
        .unwrap_or(Type::Any)])
}

pub(crate) fn component_argument(call: &FunctionCall) -> Option<&Expression> {
    call_argument(call, 1)
}

fn component_type_from_call(call: &FunctionCall) -> Option<Type> {
    let Expression::TableConstructor(schema) = call_argument(call, 1)? else {
        return None;
    };
    let value_type = component_value_type_from_schema(schema)?;

    Some(component_type(value_type))
}

fn component_value_type_from_schema(schema: &TableConstructor) -> Option<Type> {
    let mut fields = Vec::new();

    for field in schema.fields().iter() {
        let Field::NameKey { key, value, .. } = field else {
            return None;
        };
        let Expression::String(type_name) = value else {
            return None;
        };
        let type_name = token_text(type_name).trim_matches(['\'', '"']).to_owned();

        fields.push((
            token_text(key),
            TableField::required(Type::named(type_name)),
        ));
    }

    Some(Type::table(fields))
}

fn component_value_type(candidate: &Type) -> Option<Type> {
    match candidate {
        Type::Alias(name) if name == COMPONENT_TYPE => Some(Type::Any),
        Type::Union(types) => {
            let values = types
                .iter()
                .filter_map(component_value_type)
                .collect::<Vec<_>>();

            match values.len() {
                0 => None,
                1 => values.into_iter().next(),
                _ => Some(Type::union(values)),
            }
        }
        Type::Table(table) if table.nominal_identity() == Some(COMPONENT_TYPE) => table
            .field(COMPONENT_VALUE_FIELD)
            .map(|field| field.ty.clone()),
        _ => None,
    }
}

fn is_module_type_direct(candidate: &Type) -> bool {
    candidate == &ecs_module_type()
}

fn is_world_type_direct(candidate: &Type) -> bool {
    match candidate {
        Type::Alias(name) => name == WORLD_TYPE,
        Type::Table(_) => candidate == &world_type(),
        Type::Union(types) => types.iter().any(is_world_type_direct),
        _ => false,
    }
}

fn call_argument(call: &FunctionCall, index: usize) -> Option<&Expression> {
    call.suffixes().find_map(|suffix| match suffix {
        Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. })) => {
            arguments.iter().nth(index)
        }
        Suffix::Call(Call::MethodCall(method)) => match method.args() {
            FunctionArgs::Parentheses { arguments, .. } => arguments.iter().nth(index),
            _ => None,
        },
        _ => None,
    })
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
        guarded_return_types: None,
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

pub(crate) fn diagnostics(parsed: &ParsedSource, environment: &TypeEnvironment) -> Vec<Diagnostic> {
    let mut visitor = EcsDiagnosticVisitor {
        parsed,
        environment,
        semantic: SemanticModel::new(parsed, environment),
        diagnostics: Vec::new(),
    };

    visitor.visit_ast(&parsed.ast);
    visitor.diagnostics
}

struct EcsDiagnosticVisitor<'a> {
    parsed: &'a ParsedSource,
    environment: &'a TypeEnvironment,
    semantic: SemanticModel<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl Visitor for EcsDiagnosticVisitor<'_> {
    fn visit_function_call(&mut self, call: &FunctionCall) {
        let Some(parts) = field_chain_before_first_call(call) else {
            return;
        };
        let Some(span) = node_span(call) else {
            return;
        };
        let Some((member, receiver)) = parts.split_last() else {
            return;
        };
        let Some(receiver_type) = self.semantic.type_of_field_chain(receiver, span.start) else {
            return;
        };

        if typed_member(self.environment, &receiver_type, member).is_none() {
            return;
        }

        match member.as_str() {
            "add" => self.check_add(call, span.start),
            "system" => self.check_component_list(argument_expression(call, 1)),
            "query" => self.check_component_list(argument_expression(call, 0)),
            _ => {}
        }
    }
}

impl EcsDiagnosticVisitor<'_> {
    fn check_add(&mut self, call: &FunctionCall, position: SourcePosition) {
        let Some(component) = argument_expression(call, 1) else {
            return;
        };
        let Some(expected) = self.component_value_type(component, position) else {
            let name = plain_field_chain_from_expression(component)
                .map(|parts| parts.join("."))
                .unwrap_or_else(|| component.to_string());
            self.push_query_diagnostic(component, format!("unknown ECS component `{name}`"));
            return;
        };
        if expected == Type::Any {
            return;
        }
        let Some(value) = argument_expression(call, 2) else {
            return;
        };
        let Some(found) = self.semantic.type_of_expression_return(value, position, 0) else {
            return;
        };
        if found == Type::Any {
            return;
        }

        let context = self.environment.to_type_context();
        if let Some(failure) = context.assignability_failure(&found, &expected) {
            self.push_component_value_diagnostic(value, component_mismatch_message(&failure));
        }

        let (Expression::TableConstructor(table), Type::Table(expected_fields)) =
            (value, self.environment.resolve_aliases(&expected))
        else {
            return;
        };

        for field in table.fields().iter() {
            let Field::NameKey { key, .. } = field else {
                continue;
            };
            let name = token_text(key);
            if expected_fields.semantic_field(&name).is_none() {
                self.push_component_value_diagnostic(
                    key,
                    format!("ECS component value includes unknown field `{name}`"),
                );
            }
        }
    }

    fn check_component_list(&mut self, expression: Option<&Expression>) {
        let Some(Expression::TableConstructor(table)) = expression else {
            return;
        };
        let entries = table.fields().iter().collect::<Vec<_>>();

        if entries.is_empty() {
            self.push_query_diagnostic(
                table,
                "ECS system/query requires at least one component".to_owned(),
            );
            return;
        }

        for field in entries {
            let Field::NoKey(component) = field else {
                continue;
            };
            let Some(position) = node_span(component).map(|span| span.start) else {
                continue;
            };

            if self.component_value_type(component, position).is_none() {
                let name = plain_field_chain_from_expression(component)
                    .map(|parts| parts.join("."))
                    .unwrap_or_else(|| component.to_string());
                self.push_query_diagnostic(
                    component,
                    format!("unknown ECS component `{name}` in system/query"),
                );
            }
        }
    }

    fn component_value_type(
        &mut self,
        component: &Expression,
        position: SourcePosition,
    ) -> Option<Type> {
        let component_type = self
            .semantic
            .type_of_expression_return(component, position, 0)?;
        if component_type == Type::Any {
            return Some(Type::Any);
        }

        component_value_type(&self.environment.resolve_aliases(&component_type))
    }

    fn push_component_value_diagnostic(&mut self, node: &impl Node, message: String) {
        let Some(span) = node_span(node) else {
            return;
        };

        self.diagnostics.push(
            Diagnostic::new(
                &self.parsed.path,
                span.start.line,
                span.start.column,
                DiagnosticSeverity::Error,
                message,
            )
            .with_code(DiagnosticCode::DOMAIN_ECS_COMPONENT_VALUE),
        );
    }

    fn push_query_diagnostic(&mut self, node: &impl Node, message: String) {
        let Some(span) = node_span(node) else {
            return;
        };

        self.diagnostics.push(
            Diagnostic::new(
                &self.parsed.path,
                span.start.line,
                span.start.column,
                DiagnosticSeverity::Error,
                message,
            )
            .with_code(DiagnosticCode::DOMAIN_ECS_QUERY),
        );
    }
}

fn component_mismatch_message(failure: &AssignabilityFailure) -> String {
    let fields = failure
        .path
        .iter()
        .filter_map(|segment| match segment {
            AssignabilityPathSegment::Field(name) => Some(name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let field = fields.join(".");

    if matches!(failure.kind, AssignabilityFailureKind::MissingField) && !field.is_empty() {
        return format!("ECS component value is missing required field `{field}`");
    }
    if !field.is_empty() {
        return format!(
            "ECS component field `{field}` expected {}, found {}",
            failure.expected, failure.found
        );
    }

    format!(
        "ECS component value expected {}, found {}",
        failure.expected, failure.found
    )
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn argument_expression(call: &FunctionCall, index: usize) -> Option<&Expression> {
    let call = call.suffixes().find_map(|suffix| match suffix {
        Suffix::Call(call) => Some(call),
        _ => None,
    })?;

    match call {
        Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }) => {
            arguments.iter().nth(index)
        }
        Call::MethodCall(method) => match method.args() {
            FunctionArgs::Parentheses { arguments, .. } => arguments.iter().nth(index),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::parse_tua_source;

    use super::*;

    fn diagnostics_for(source: &str) -> Vec<Diagnostic> {
        let parsed = parse_tua_source(Path::new("src/ecs.tua"), source)
            .expect("ECS diagnostic fixture should parse");
        let (environment, _) = crate::types::build_type_environment_with_domain_registry(
            &parsed,
            crate::semantic_domains::BuiltinDomainRegistry::with_ecs(true),
        );

        diagnostics(&parsed, &environment)
    }

    #[test]
    fn diagnostics_check_component_values_and_queries() {
        let source = r#"local Position = ecs.component("Position", { x = "number", y = "number" })
local world = ecs.world()
local entity = world:create()
world:add(entity, Position, { x = 1, name = "wrong" })
ecs.system(world, { Position, Missing }, function() end)
"#;

        let diagnostics = diagnostics_for(source);

        assert_eq!(diagnostics.len(), 3);
        assert_eq!(
            diagnostics[0].code,
            Some(DiagnosticCode::DOMAIN_ECS_COMPONENT_VALUE)
        );
        assert_eq!(diagnostics[2].code, Some(DiagnosticCode::DOMAIN_ECS_QUERY));
    }

    #[test]
    fn diagnostics_accept_typed_values_and_multiline_ecs_calls() {
        let source = r#"local Position = ecs.component(
  "Position",
  { x = "number", y = "number" }
)
local world = ecs.world()
local entity = world:create()
local payload: { x: number, y: number } = { x = 1, y = 2 }
world:add(
  entity,
  Position,
  payload
)
ecs.system(
  world,
  {
    Position,
  },
  function() end
)
"#;

        let diagnostics = diagnostics_for(source);

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    }

    #[test]
    fn diagnostics_reject_wrong_typed_non_literal_values() {
        let source = r#"local Position = ecs.component("Position", { x = "number", y = "number" })
local world = ecs.world()
local entity = world:create()
local payload: { x: number, y: string } = { x = 1, y = "wrong" }
world:add(entity, Position, payload)
"#;

        let diagnostics = diagnostics_for(source);

        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code == Some(DiagnosticCode::DOMAIN_ECS_COMPONENT_VALUE)
                    && diagnostic
                        .message
                        .contains("field `y` expected number, found string")
            }),
            "{diagnostics:#?}"
        );
    }

    #[test]
    fn diagnostics_report_invalid_static_components_but_allow_dynamic_values() {
        let source = r#"local world = ecs.world()
local entity = world:create()
local NotAComponent = 1
world:add(entity, NotAComponent, {})

local function addDynamic(component, value)
  world:add(entity, component, value)
end
"#;

        let diagnostics = diagnostics_for(source);

        assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
        assert_eq!(diagnostics[0].code, Some(DiagnosticCode::DOMAIN_ECS_QUERY));
        assert!(diagnostics[0].message.contains("NotAComponent"));
    }
}

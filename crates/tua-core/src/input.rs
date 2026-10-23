use std::collections::BTreeSet;
use std::path::Path;

use full_moon::ast::{
    Call, Expression, Field, FunctionArgs, FunctionCall, Suffix, TableConstructor,
};
use full_moon::node::Node;
use full_moon::visitors::Visitor;

use crate::ast_utils::{field_chain_before_first_call, token_text};
use crate::types::{
    FunctionParameter, FunctionType, TableField, Type, TypeEnvironment, TypedFunctionOverload,
};
use crate::{
    lua, Diagnostic, DiagnosticCode, ParsedSource, SourcePosition, SourceSpan, TypedFunction,
    TypedParameter,
};

pub(crate) const TYPE_NAME: &str = "Input";
pub(crate) const CONFIG_TYPE_NAME: &str = "InputConfig";
pub(crate) const SOURCE_TYPE_NAME: &str = "InputSource";
pub(crate) const CONSTRUCTOR_NAME: &str = "input";
pub(crate) const RUNTIME_MODULE: &str = "baton";
pub(crate) const EMITTED_CONSTRUCTOR_LOCAL: &str = "__tua_input";

const BUILTIN_TYPE_NAMES: &[&str] = &[TYPE_NAME, CONFIG_TYPE_NAME, SOURCE_TYPE_NAME];
const METHOD_NAMES: &[&str] = &[
    "update",
    "get",
    "getRaw",
    "down",
    "pressed",
    "released",
    "getActiveDevice",
    "pointer",
    "pointerDelta",
];

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct InputSpec {
    pub(crate) controls: Vec<String>,
    pub(crate) pairs: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConfigContext {
    Options,
    Controls,
    Pairs,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StringValueContext {
    Source,
    PairControl,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConfigCompletionItem {
    pub(crate) label: String,
    pub(crate) detail: String,
    pub(crate) documentation: String,
    pub(crate) insert_text: String,
}

#[derive(Clone, Debug)]
struct InputDeclaration {
    spec: InputSpec,
    config_span: SourceSpan,
    controls_span: Option<SourceSpan>,
    pairs_span: Option<SourceSpan>,
    string_values: Vec<(SourceSpan, StringValueContext, String)>,
}

pub(crate) fn builtin_type_names() -> &'static [&'static str] {
    BUILTIN_TYPE_NAMES
}

pub(crate) fn builtin_aliases() -> Vec<(String, Type)> {
    vec![
        (TYPE_NAME.to_owned(), base_input_type()),
        (CONFIG_TYPE_NAME.to_owned(), config_type()),
        (SOURCE_TYPE_NAME.to_owned(), Type::string()),
    ]
}

pub(crate) fn is_constructor(name: &str) -> bool {
    name == CONSTRUCTOR_NAME
}

pub(crate) fn constructor_type() -> FunctionType {
    FunctionType::new([Type::alias(CONFIG_TYPE_NAME)], Type::alias(TYPE_NAME))
}

pub(crate) fn constructor_typed_function() -> TypedFunction {
    typed_function(
        CONSTRUCTOR_NAME,
        [parameter("config", Type::alias(CONFIG_TYPE_NAME))],
        [Type::alias(TYPE_NAME)],
        documentation(),
    )
}

pub(crate) const fn detail() -> &'static str {
    "function input(config: InputConfig): Input"
}

pub(crate) const fn documentation() -> &'static str {
    "Create a typed Baton input player whose configured controls, pairs, optional pointer state, and LOVE input sources are available to editor tooling."
}

pub(crate) const fn workflow_snippet() -> &'static str {
    "local ${1:controls} = input({\n  controls = {\n    left = { \"key:left\", \"key:a\", \"axis:leftx-\" },\n    right = { \"key:right\", \"key:d\", \"axis:leftx+\" },\n    up = { \"key:up\", \"key:w\", \"axis:lefty-\" },\n    down = { \"key:down\", \"key:s\", \"axis:lefty+\" },\n    confirm = { \"key:return\", \"button:a\" },\n  },\n  pairs = {\n    move = { \"left\", \"right\", \"up\", \"down\" },\n  },\n  deadzone = ${2:0.25},\n})\n\nfunction love.update(dt: number)\n  ${1:controls}:update()\n  local x, y = ${1:controls}:get(\"move\")\n  ${0}\nend"
}

pub(crate) fn runtime_source() -> &'static str {
    include_str!("../../../vendor/runtime/baton.lua")
}

pub(crate) fn input_type_from_call(call: &FunctionCall) -> Option<Type> {
    let config = config_from_call(call)?;
    if !config_has_static_names(config) {
        return Some(Type::alias(TYPE_NAME));
    }

    Some(input_type(&spec_from_config(config)))
}

pub(crate) fn type_is_input(environment: &TypeEnvironment, candidate: &Type) -> bool {
    input_table(environment, candidate, 0).is_some()
}

pub(crate) fn type_summary(
    environment: &TypeEnvironment,
    candidate: &Type,
) -> Option<(InputSpec, bool)> {
    let table = input_table(environment, candidate, 0)?;
    Some(spec_from_input_table(environment, table))
}

pub(crate) fn typed_member(
    environment: &TypeEnvironment,
    receiver_type: &Type,
    member: &str,
) -> Option<TypedFunction> {
    let (spec, exact) = type_summary(environment, receiver_type)?;
    typed_member_for_spec(member, &spec, exact)
}

pub(crate) fn method_return_types_from_call(
    environment: &TypeEnvironment,
    receiver_type: &Type,
    call: &FunctionCall,
) -> Option<Vec<Type>> {
    let (spec, exact) = type_summary(environment, receiver_type)?;
    method_return_types(&spec, exact, call)
}

pub(crate) fn method_return_types_from_direct_type(
    receiver_type: &Type,
    call: &FunctionCall,
) -> Option<Vec<Type>> {
    let Type::Table(table) = receiver_type else {
        return None;
    };
    if !table_is_input(table) {
        return None;
    }

    let (spec, exact) = spec_from_direct_input_table(table);
    method_return_types(&spec, exact, call)
}

pub(crate) fn config_completion_items(
    parsed: &ParsedSource,
    position: SourcePosition,
) -> Option<Vec<ConfigCompletionItem>> {
    let declaration = declaration_at(parsed, position)?;
    let items = match declaration.context_at(position) {
        ConfigContext::Options => vec![
            completion_item(
                "controls",
                "{ [string]: InputSource[] }",
                "Named controls and the Baton sources that can activate each one.",
                "controls = {\n  ${1:confirm} = { \"${2:key:return}\", \"${3:button:a}\" },\n},",
            ),
            completion_item(
                "pairs",
                "{ [string]: { string, string, string, string } }",
                "Directional pairs in left, right, up, down order.",
                "pairs = {\n  ${1:move} = { \"${2:left}\", \"${3:right}\", \"${4:up}\", \"${5:down}\" },\n},",
            ),
            completion_item(
                "joystick",
                "Joystick | nil",
                "LOVE Joystick used by axis, button, and hat sources.",
                "joystick = ${1:love.joystick.getJoysticks()[1]},",
            ),
            completion_item(
                "deadzone",
                "number",
                "Analog activation threshold between 0 and 1. Baton defaults to 0.5.",
                "deadzone = ${1:0.25},",
            ),
            completion_item(
                "squareDeadzone",
                "boolean",
                "Apply the deadzone independently to pair axes instead of radially.",
                "squareDeadzone = ${1:false},",
            ),
            completion_item(
                "pointer",
                "boolean",
                "Cache mouse position and per-update movement, and treat mouse movement as keyboard/mouse activity.",
                "pointer = ${1:true},",
            ),
        ],
        ConfigContext::Controls => vec![completion_item(
            "control",
            "InputSource[]",
            "Named control with one or more keyboard, mouse, or joystick sources.",
            "${1:confirm} = { \"${2:key:return}\", \"${3:button:a}\" },",
        )],
        ConfigContext::Pairs => vec![completion_item(
            "pair",
            "{ string, string, string, string }",
            "Directional pair in left, right, up, down order.",
            "${1:move} = { \"${2:left}\", \"${3:right}\", \"${4:up}\", \"${5:down}\" },",
        )],
    };

    Some(items)
}

pub(crate) fn string_value_completion_items(
    parsed: &ParsedSource,
    position: SourcePosition,
) -> Option<Vec<ConfigCompletionItem>> {
    let declaration = declaration_at(parsed, position)?;
    let (_, context, value) = declaration
        .string_values
        .iter()
        .find(|(span, _, _)| span_contains_position(*span, position))?;

    match context {
        StringValueContext::Source => Some(source_completion_items(value)),
        StringValueContext::PairControl => Some(
            declaration
                .spec
                .controls
                .iter()
                .map(|control| ConfigCompletionItem {
                    label: control.clone(),
                    detail: "input control".to_owned(),
                    documentation: format!("Configured input control `{control}`."),
                    insert_text: control.clone(),
                })
                .collect(),
        ),
    }
}

pub(crate) fn method_string_completion_items(
    environment: &TypeEnvironment,
    receiver_type: &Type,
    method: &str,
    active_parameter: usize,
) -> Option<Vec<ConfigCompletionItem>> {
    if active_parameter != 0
        || !matches!(method, "get" | "getRaw" | "down" | "pressed" | "released")
    {
        return None;
    }

    let (spec, exact) = type_summary(environment, receiver_type)?;
    if !exact {
        return None;
    }

    Some(
        spec.controls
            .iter()
            .map(|name| name_completion_item(name, "input control"))
            .chain(
                spec.pairs
                    .iter()
                    .map(|name| name_completion_item(name, "input pair")),
            )
            .collect(),
    )
}

pub(crate) fn config_diagnostics(parsed: &ParsedSource) -> Vec<Diagnostic> {
    let mut visitor = DiagnosticVisitor {
        path: &parsed.path,
        diagnostics: Vec::new(),
    };
    visitor.visit_ast(&parsed.ast);
    visitor.diagnostics
}

fn config_type() -> Type {
    Type::table([
        ("controls", TableField::optional(Type::Any)),
        ("pairs", TableField::optional(Type::Any)),
        ("joystick", TableField::optional(Type::alias("Joystick"))),
        ("deadzone", TableField::optional(Type::number())),
        ("squareDeadzone", TableField::optional(Type::boolean())),
        ("pointer", TableField::optional(Type::boolean())),
    ])
}

fn base_input_type() -> Type {
    let Type::Table(mut table) = input_type(&InputSpec::default()) else {
        unreachable!("input type is always a table");
    };
    table.insert("config", TableField::required(config_type()));
    Type::Table(table)
}

fn input_type(spec: &InputSpec) -> Type {
    let exact = !spec.controls.is_empty() || !spec.pairs.is_empty();
    let name_type = literal_union_or_string(
        &spec
            .controls
            .iter()
            .chain(&spec.pairs)
            .cloned()
            .collect::<Vec<_>>(),
    );
    let second_number = if exact && spec.pairs.is_empty() {
        None
    } else {
        Some(Type::union([Type::number(), Type::nil()]))
    };
    let mut get_returns = vec![Type::number()];
    get_returns.extend(second_number);
    let config = Type::table([
        (
            "controls",
            TableField::required(named_config_table(
                &spec.controls,
                Type::array(Type::string()),
            )),
        ),
        (
            "pairs",
            TableField::required(named_config_table(&spec.pairs, Type::array(Type::string()))),
        ),
        ("joystick", TableField::optional(Type::alias("Joystick"))),
        ("deadzone", TableField::required(Type::number())),
        ("squareDeadzone", TableField::required(Type::boolean())),
        ("pointer", TableField::required(Type::boolean())),
    ]);

    Type::table([
        ("config", TableField::required(config)),
        ("update", method_field([("self", Type::Any)], [Type::nil()])),
        (
            "get",
            method_field(
                [("self", Type::Any), ("name", name_type.clone())],
                get_returns.clone(),
            ),
        ),
        (
            "getRaw",
            method_field(
                [("self", Type::Any), ("name", name_type.clone())],
                get_returns,
            ),
        ),
        (
            "down",
            method_field(
                [("self", Type::Any), ("name", name_type.clone())],
                [Type::boolean()],
            ),
        ),
        (
            "pressed",
            method_field(
                [("self", Type::Any), ("name", name_type.clone())],
                [Type::boolean()],
            ),
        ),
        (
            "released",
            method_field(
                [("self", Type::Any), ("name", name_type)],
                [Type::boolean()],
            ),
        ),
        (
            "getActiveDevice",
            method_field(
                [("self", Type::Any)],
                [Type::union([
                    Type::string_literal("kbm"),
                    Type::string_literal("joy"),
                    Type::string_literal("none"),
                ])],
            ),
        ),
        (
            "pointer",
            method_field([("self", Type::Any)], [Type::number(), Type::number()]),
        ),
        (
            "pointerDelta",
            method_field([("self", Type::Any)], [Type::number(), Type::number()]),
        ),
    ])
}

fn named_config_table(names: &[String], value_type: Type) -> Type {
    Type::table(
        names
            .iter()
            .map(|name| (name.clone(), TableField::required(value_type.clone()))),
    )
}

fn method_field(
    parameters: impl IntoIterator<Item = (&'static str, Type)>,
    returns: impl IntoIterator<Item = Type>,
) -> TableField {
    let parameters = parameters
        .into_iter()
        .map(|(name, ty)| FunctionParameter::named(name, ty))
        .collect::<Vec<_>>();

    TableField::required(Type::Function(FunctionType::new_signature(
        parameters.clone(),
        parameters.len(),
        None,
        returns,
    )))
}

fn typed_member_for_spec(member: &str, spec: &InputSpec, exact: bool) -> Option<TypedFunction> {
    let all_names = spec
        .controls
        .iter()
        .chain(&spec.pairs)
        .cloned()
        .collect::<Vec<_>>();
    let name_type = literal_union_or_string(&all_names);
    let self_parameter = parameter("self", Type::Any);

    match member {
        "update" => Some(typed_function(
            "Input:update",
            [self_parameter],
            [Type::nil()],
            "Poll configured Baton sources and update control, pair, and enabled pointer state.",
        )),
        "get" | "getRaw" if exact => {
            let mut overloads = Vec::new();
            if !spec.controls.is_empty() {
                overloads.push(method_overload(
                    [
                        parameter("self", Type::Any),
                        parameter("name", literal_union_or_string(&spec.controls)),
                    ],
                    [Type::number()],
                    "Return the scalar value of a configured input control.",
                ));
            }
            if !spec.pairs.is_empty() {
                overloads.push(method_overload(
                    [
                        parameter("self", Type::Any),
                        parameter("name", literal_union_or_string(&spec.pairs)),
                    ],
                    [Type::number(), Type::number()],
                    "Return the x and y values of a configured directional pair.",
                ));
            }

            Some(TypedFunction {
                name: Some(format!("Input:{member}")),
                parameters: vec![parameter("self", Type::Any), parameter("name", Type::Any)],
                parameter_count: 2,
                required_parameter_count: 2,
                has_varargs: false,
                vararg_type: None,
                return_type: Some(Type::number()),
                return_types: vec![Type::number()],
                documentation: Some(if member == "getRaw" {
                    "Return configured input values before applying the deadzone.".to_owned()
                } else {
                    "Return configured input values after applying the deadzone.".to_owned()
                }),
                overloads,
            })
        }
        "get" | "getRaw" => Some(typed_function(
            &format!("Input:{member}"),
            [self_parameter, parameter("name", Type::string())],
            [Type::number(), Type::union([Type::number(), Type::nil()])],
            if member == "getRaw" {
                "Return a control value or pair values before applying the deadzone."
            } else {
                "Return a control value or pair values after applying the deadzone."
            },
        )),
        "down" | "pressed" | "released" if exact => Some(TypedFunction {
            name: Some(format!("Input:{member}")),
            parameters: vec![parameter("self", Type::Any), parameter("name", Type::Any)],
            parameter_count: 2,
            required_parameter_count: 2,
            has_varargs: false,
            vararg_type: None,
            return_type: Some(Type::boolean()),
            return_types: vec![Type::boolean()],
            documentation: Some(member_documentation(member).to_owned()),
            overloads: vec![method_overload(
                [parameter("self", Type::Any), parameter("name", name_type)],
                [Type::boolean()],
                member_documentation(member),
            )],
        }),
        "down" | "pressed" | "released" => Some(typed_function(
            &format!("Input:{member}"),
            [self_parameter, parameter("name", Type::string())],
            [Type::boolean()],
            member_documentation(member),
        )),
        "getActiveDevice" => Some(typed_function(
            "Input:getActiveDevice",
            [self_parameter],
            [Type::union([
                Type::string_literal("kbm"),
                Type::string_literal("joy"),
                Type::string_literal("none"),
            ])],
            "Return the most recently active Baton device category.",
        )),
        "pointer" => Some(typed_function(
            "Input:pointer",
            [self_parameter],
            [Type::number(), Type::number()],
            "Return the cached mouse position in window coordinates. Requires `pointer = true`.",
        )),
        "pointerDelta" => Some(typed_function(
            "Input:pointerDelta",
            [self_parameter],
            [Type::number(), Type::number()],
            "Return mouse movement since the previous input update. Requires `pointer = true`.",
        )),
        _ => None,
    }
}

fn method_return_types(spec: &InputSpec, exact: bool, call: &FunctionCall) -> Option<Vec<Type>> {
    let parts = field_chain_before_first_call(call)?;
    let method = parts.last()?.as_str();

    match method {
        "update" => Some(vec![Type::nil()]),
        "down" | "pressed" | "released" => Some(vec![Type::boolean()]),
        "getActiveDevice" => Some(vec![Type::union([
            Type::string_literal("kbm"),
            Type::string_literal("joy"),
            Type::string_literal("none"),
        ])]),
        "pointer" | "pointerDelta" => Some(vec![Type::number(), Type::number()]),
        "get" | "getRaw" => {
            let name = first_parenthesized_argument(call).and_then(string_expression);
            if exact && name.as_ref().is_some_and(|name| spec.pairs.contains(name)) {
                return Some(vec![Type::number(), Type::number()]);
            }
            if exact
                && name
                    .as_ref()
                    .is_some_and(|name| spec.controls.contains(name))
            {
                return Some(vec![Type::number()]);
            }

            Some(vec![
                Type::number(),
                Type::union([Type::number(), Type::nil()]),
            ])
        }
        _ => None,
    }
}

fn member_documentation(member: &str) -> &'static str {
    match member {
        "down" => "Return whether the selected control or pair is currently held down.",
        "pressed" => "Return whether the selected control or pair was pressed this frame.",
        "released" => "Return whether the selected control or pair was released this frame.",
        _ => "Baton input method.",
    }
}

fn input_table<'a>(
    environment: &'a TypeEnvironment,
    candidate: &'a Type,
    depth: usize,
) -> Option<&'a crate::TableShape> {
    if depth >= 16 {
        return None;
    }

    match candidate {
        Type::Alias(name) => environment
            .alias(name)
            .filter(|resolved| *resolved != candidate)
            .and_then(|resolved| input_table(environment, resolved, depth + 1)),
        Type::Table(table) if table_is_input(table) => Some(table),
        Type::Union(types) => types
            .iter()
            .find_map(|ty| input_table(environment, ty, depth + 1)),
        _ => None,
    }
}

fn table_is_input(table: &crate::TableShape) -> bool {
    table.semantic_field("config").is_some()
        && METHOD_NAMES
            .iter()
            .all(|method| table.semantic_field(method).is_some())
}

fn spec_from_input_table(
    environment: &TypeEnvironment,
    table: &crate::TableShape,
) -> (InputSpec, bool) {
    let Some(config) = table
        .semantic_field("config")
        .and_then(|field| table_from_type(environment, &field.ty, 0))
    else {
        return (InputSpec::default(), false);
    };
    let controls = config
        .semantic_field("controls")
        .and_then(|field| table_from_type(environment, &field.ty, 0));
    let pairs = config
        .semantic_field("pairs")
        .and_then(|field| table_from_type(environment, &field.ty, 0));
    let exact = controls.is_some();

    (
        InputSpec {
            controls: controls.map(field_names).unwrap_or_default(),
            pairs: pairs.map(field_names).unwrap_or_default(),
        },
        exact,
    )
}

fn spec_from_direct_input_table(table: &crate::TableShape) -> (InputSpec, bool) {
    let Some(Type::Table(config)) = table.semantic_field("config").map(|field| &field.ty) else {
        return (InputSpec::default(), false);
    };
    let controls = config
        .semantic_field("controls")
        .and_then(|field| match &field.ty {
            Type::Table(table) => Some(table),
            _ => None,
        });
    let pairs = config
        .semantic_field("pairs")
        .and_then(|field| match &field.ty {
            Type::Table(table) => Some(table),
            _ => None,
        });

    (
        InputSpec {
            controls: controls.map(field_names).unwrap_or_default(),
            pairs: pairs.map(field_names).unwrap_or_default(),
        },
        controls.is_some(),
    )
}

fn table_from_type<'a>(
    environment: &'a TypeEnvironment,
    ty: &'a Type,
    depth: usize,
) -> Option<&'a crate::TableShape> {
    if depth >= 16 {
        return None;
    }

    match ty {
        Type::Table(table) => Some(table),
        Type::Alias(name) => environment
            .alias(name)
            .filter(|resolved| *resolved != ty)
            .and_then(|resolved| table_from_type(environment, resolved, depth + 1)),
        _ => None,
    }
}

fn field_names(table: &crate::TableShape) -> Vec<String> {
    table.fields().map(|(name, _)| name.to_owned()).collect()
}

fn literal_union_or_string(values: &[String]) -> Type {
    match values {
        [] => Type::string(),
        [value] => Type::string_literal(value),
        _ => Type::union(values.iter().cloned().map(Type::string_literal)),
    }
}

fn typed_function(
    name: &str,
    parameters: impl IntoIterator<Item = TypedParameter>,
    returns: impl IntoIterator<Item = Type>,
    documentation: &str,
) -> TypedFunction {
    let parameters = parameters.into_iter().collect::<Vec<_>>();
    let returns = returns.into_iter().collect::<Vec<_>>();

    TypedFunction {
        name: Some(name.to_owned()),
        parameter_count: parameters.len(),
        required_parameter_count: parameters.len(),
        parameters,
        has_varargs: false,
        vararg_type: None,
        return_type: returns.first().cloned(),
        return_types: returns,
        documentation: Some(documentation.to_owned()),
        overloads: Vec::new(),
    }
}

fn method_overload(
    parameters: impl IntoIterator<Item = TypedParameter>,
    returns: impl IntoIterator<Item = Type>,
    documentation: &str,
) -> TypedFunctionOverload {
    TypedFunctionOverload {
        parameters: parameters.into_iter().collect(),
        has_varargs: false,
        vararg_type: None,
        return_types: returns.into_iter().collect(),
        documentation: Some(documentation.to_owned()),
    }
}

fn parameter(name: &str, ty: Type) -> TypedParameter {
    TypedParameter {
        name: name.to_owned(),
        ty,
        documentation: None,
    }
}

fn name_completion_item(name: &str, detail: &str) -> ConfigCompletionItem {
    ConfigCompletionItem {
        label: name.to_owned(),
        detail: detail.to_owned(),
        documentation: format!("Configured {detail} `{name}`."),
        insert_text: name.to_owned(),
    }
}

fn source_completion_items(value: &str) -> Vec<ConfigCompletionItem> {
    let source_types = [
        ("key:", "LOVE key input"),
        ("sc:", "LOVE scancode input"),
        ("mouse:", "LOVE mouse button input"),
        ("axis:", "LOVE joystick axis input"),
        ("button:", "LOVE gamepad button input"),
        ("hat:", "LOVE joystick hat input"),
    ];
    let mut items = source_types
        .iter()
        .map(|(label, detail)| ConfigCompletionItem {
            label: (*label).to_owned(),
            detail: (*detail).to_owned(),
            documentation: format!("Baton `{}` source.", label.trim_end_matches(':')),
            insert_text: (*label).to_owned(),
        })
        .collect::<Vec<_>>();

    let Some((kind, _)) = value.split_once(':') else {
        return items;
    };

    let candidates = match kind {
        "key" => enum_source_values("key", "KeyConstant", "LOVE key input"),
        "sc" => enum_source_values("sc", "Scancode", "LOVE scancode input"),
        "axis" => lua::enum_value_items("GamepadAxis", "")
            .into_iter()
            .flat_map(|item| {
                ['-', '+'].map(|direction| {
                    source_value_item(
                        format!("axis:{}{direction}", item.label),
                        "LOVE gamepad axis input",
                    )
                })
            })
            .collect(),
        "button" => enum_source_values("button", "GamepadButton", "LOVE gamepad button input"),
        "mouse" => (1..=5)
            .map(|button| source_value_item(format!("mouse:{button}"), "LOVE mouse button input"))
            .collect(),
        "hat" => lua::enum_value_items("JoystickHat", "")
            .into_iter()
            .map(|item| {
                source_value_item(format!("hat:1{}", item.label), "LOVE joystick hat input")
            })
            .collect(),
        _ => Vec::new(),
    };
    items.extend(candidates);
    items
}

fn enum_source_values(kind: &str, enum_type: &str, detail: &str) -> Vec<ConfigCompletionItem> {
    lua::enum_value_items(enum_type, "")
        .into_iter()
        .map(|item| source_value_item(format!("{kind}:{}", item.label), detail))
        .collect()
}

fn source_value_item(value: String, detail: &str) -> ConfigCompletionItem {
    ConfigCompletionItem {
        label: value.clone(),
        detail: detail.to_owned(),
        documentation: format!("Baton source `{value}`."),
        insert_text: value,
    }
}

fn completion_item(
    label: &str,
    detail: &str,
    documentation: &str,
    insert_text: &str,
) -> ConfigCompletionItem {
    ConfigCompletionItem {
        label: label.to_owned(),
        detail: detail.to_owned(),
        documentation: documentation.to_owned(),
        insert_text: insert_text.to_owned(),
    }
}

fn declaration_at(parsed: &ParsedSource, position: SourcePosition) -> Option<InputDeclaration> {
    let mut visitor = DeclarationVisitor::default();
    visitor.visit_ast(&parsed.ast);

    visitor
        .declarations
        .into_iter()
        .filter(|declaration| span_contains_position(declaration.config_span, position))
        .min_by_key(|declaration| span_size(declaration.config_span))
}

impl InputDeclaration {
    fn context_at(&self, position: SourcePosition) -> ConfigContext {
        if self
            .controls_span
            .is_some_and(|span| span_contains_position(span, position))
        {
            return ConfigContext::Controls;
        }
        if self
            .pairs_span
            .is_some_and(|span| span_contains_position(span, position))
        {
            return ConfigContext::Pairs;
        }

        ConfigContext::Options
    }
}

#[derive(Default)]
struct DeclarationVisitor {
    declarations: Vec<InputDeclaration>,
}

impl Visitor for DeclarationVisitor {
    fn visit_function_call(&mut self, call: &FunctionCall) {
        if let Some(declaration) = declaration_from_call(call) {
            self.declarations.push(declaration);
        }
    }
}

fn declaration_from_call(call: &FunctionCall) -> Option<InputDeclaration> {
    let config = config_from_call(call)?;
    let controls = named_field(config, "controls").and_then(table_expression);
    let pairs = named_field(config, "pairs").and_then(table_expression);
    let mut string_values = Vec::new();

    if let Some(controls) = controls {
        for (_, sources) in named_table_fields(controls) {
            let Some(sources) = table_expression(sources) else {
                continue;
            };
            for source in
                table_values(sources).filter(|value| matches!(value, Expression::String(_)))
            {
                if let (Some(span), Some(value)) = (node_span(source), string_expression(source)) {
                    string_values.push((span, StringValueContext::Source, value));
                }
            }
        }
    }

    if let Some(pairs) = pairs {
        for (_, controls) in named_table_fields(pairs) {
            let Some(controls) = table_expression(controls) else {
                continue;
            };
            for control in
                table_values(controls).filter(|value| matches!(value, Expression::String(_)))
            {
                if let (Some(span), Some(value)) = (node_span(control), string_expression(control))
                {
                    string_values.push((span, StringValueContext::PairControl, value));
                }
            }
        }
    }

    Some(InputDeclaration {
        spec: spec_from_config(config),
        config_span: node_span(config)?,
        controls_span: controls.and_then(node_span),
        pairs_span: pairs.and_then(node_span),
        string_values,
    })
}

fn spec_from_config(config: &TableConstructor) -> InputSpec {
    InputSpec {
        controls: named_field(config, "controls")
            .and_then(table_expression)
            .map(static_field_names)
            .unwrap_or_default(),
        pairs: named_field(config, "pairs")
            .and_then(table_expression)
            .map(static_field_names)
            .unwrap_or_default(),
    }
}

fn config_has_static_names(config: &TableConstructor) -> bool {
    let Some(controls) = named_field(config, "controls").and_then(table_expression) else {
        return false;
    };
    if table_has_dynamic_keys(controls) {
        return false;
    }

    named_field(config, "pairs")
        .and_then(table_expression)
        .is_none_or(|pairs| !table_has_dynamic_keys(pairs))
}

fn table_has_dynamic_keys(table: &TableConstructor) -> bool {
    table.fields().iter().any(|field| match field {
        Field::NameKey { .. } => false,
        Field::ExpressionKey { key, .. } => string_expression(key).is_none(),
        _ => true,
    })
}

fn config_from_call(call: &FunctionCall) -> Option<&TableConstructor> {
    let parts = field_chain_before_first_call(call)?;
    if parts.as_slice() != [CONSTRUCTOR_NAME] {
        return None;
    }
    let Expression::TableConstructor(config) = first_parenthesized_argument(call)? else {
        return None;
    };
    Some(config)
}

fn first_parenthesized_argument(call: &FunctionCall) -> Option<&Expression> {
    call.suffixes().find_map(|suffix| match suffix {
        Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. })) => {
            arguments.iter().next()
        }
        Suffix::Call(Call::MethodCall(method)) => match method.args() {
            FunctionArgs::Parentheses { arguments, .. } => arguments.iter().next(),
            _ => None,
        },
        _ => None,
    })
}

fn named_field<'a>(table: &'a TableConstructor, name: &str) -> Option<&'a Expression> {
    named_table_fields(table).find_map(|(field_name, value)| (field_name == name).then_some(value))
}

fn named_table_fields(table: &TableConstructor) -> impl Iterator<Item = (String, &Expression)> {
    table.fields().iter().filter_map(|field| match field {
        Field::NameKey { key, value, .. } => Some((token_text(key), value)),
        Field::ExpressionKey { key, value, .. } => string_expression(key).map(|key| (key, value)),
        _ => None,
    })
}

fn static_field_names(table: &TableConstructor) -> Vec<String> {
    named_table_fields(table)
        .map(|(name, _)| name)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn table_values(table: &TableConstructor) -> impl Iterator<Item = &Expression> {
    table.fields().iter().filter_map(|field| match field {
        Field::NoKey(value) => Some(value),
        _ => None,
    })
}

fn table_expression(expression: &Expression) -> Option<&TableConstructor> {
    match expression {
        Expression::TableConstructor(table) => Some(table),
        _ => None,
    }
}

fn string_expression(expression: &Expression) -> Option<String> {
    match expression {
        Expression::String(value) => Some(unquote_string(&value.to_string())),
        _ => None,
    }
}

fn unquote_string(value: &str) -> String {
    value
        .trim()
        .strip_prefix(['"', '\''])
        .and_then(|value| value.strip_suffix(['"', '\'']))
        .unwrap_or_else(|| value.trim())
        .to_owned()
}

struct DiagnosticVisitor<'a> {
    path: &'a Path,
    diagnostics: Vec<Diagnostic>,
}

impl Visitor for DiagnosticVisitor<'_> {
    fn visit_function_call(&mut self, call: &FunctionCall) {
        let Some(config) = config_from_call(call) else {
            return;
        };
        validate_config(self.path, config, &mut self.diagnostics);
    }
}

fn validate_config(path: &Path, config: &TableConstructor, diagnostics: &mut Vec<Diagnostic>) {
    let Some(controls_expression) = named_field(config, "controls") else {
        diagnostics.push(input_diagnostic(
            path,
            config,
            "input config requires a `controls` table",
        ));
        return;
    };
    let Some(controls) = table_expression(controls_expression) else {
        diagnostics.push(input_diagnostic(
            path,
            controls_expression,
            "input `controls` must be a table",
        ));
        return;
    };
    let control_names = static_field_names(controls)
        .into_iter()
        .collect::<BTreeSet<_>>();
    let control_names_are_static = !table_has_dynamic_keys(controls);

    for (name, sources_expression) in named_table_fields(controls) {
        let Some(sources) = table_expression(sources_expression) else {
            diagnostics.push(input_diagnostic(
                path,
                sources_expression,
                format!("input control `{name}` sources must be a table"),
            ));
            continue;
        };

        for field in sources
            .fields()
            .iter()
            .filter(|field| !matches!(field, Field::NoKey(_)))
        {
            diagnostics.push(input_diagnostic(
                path,
                field,
                format!("input control `{name}` sources must be an array"),
            ));
        }

        for source in table_values(sources) {
            let Some(source_value) = string_expression(source) else {
                if expression_is_definitely_not_string(source) {
                    diagnostics.push(input_diagnostic(
                        path,
                        source,
                        format!("input control `{name}` source must be a string"),
                    ));
                }
                continue;
            };
            if let Some(message) = validate_source(&source_value) {
                diagnostics.push(input_diagnostic(path, source, message));
            }
        }
    }

    if let Some(deadzone) = named_field(config, "deadzone") {
        if let Expression::Number(number) = deadzone {
            if number
                .to_string()
                .trim()
                .parse::<f64>()
                .is_ok_and(|value| !(0.0..=1.0).contains(&value))
            {
                diagnostics.push(input_diagnostic(
                    path,
                    deadzone,
                    "input `deadzone` must be between 0 and 1",
                ));
            }
        }
    }

    let Some(pairs_expression) = named_field(config, "pairs") else {
        return;
    };
    let Some(pairs) = table_expression(pairs_expression) else {
        diagnostics.push(input_diagnostic(
            path,
            pairs_expression,
            "input `pairs` must be a table",
        ));
        return;
    };

    for (name, pair_expression) in named_table_fields(pairs) {
        if control_names.contains(&name) {
            diagnostics.push(input_diagnostic(
                path,
                pair_expression,
                format!("input name `{name}` cannot be both a control and a pair"),
            ));
        }

        let Some(pair) = table_expression(pair_expression) else {
            diagnostics.push(input_diagnostic(
                path,
                pair_expression,
                format!("input pair `{name}` must be a table in left, right, up, down order"),
            ));
            continue;
        };
        let values = table_values(pair).collect::<Vec<_>>();
        if values.len() != 4 {
            diagnostics.push(input_diagnostic(
                path,
                pair,
                format!("input pair `{name}` requires exactly four controls in left, right, up, down order"),
            ));
            continue;
        }

        for value in values {
            let Some(control) = string_expression(value) else {
                if expression_is_definitely_not_string(value) {
                    diagnostics.push(input_diagnostic(
                        path,
                        value,
                        format!("input pair `{name}` control reference must be a string"),
                    ));
                }
                continue;
            };
            if control_names_are_static && !control_names.contains(&control) {
                diagnostics.push(input_diagnostic(
                    path,
                    value,
                    format!("input pair `{name}` references unknown control `{control}`"),
                ));
            }
        }
    }
}

fn expression_is_definitely_not_string(expression: &Expression) -> bool {
    matches!(
        expression,
        Expression::Function(_)
            | Expression::Number(_)
            | Expression::Symbol(_)
            | Expression::TableConstructor(_)
    )
}

fn validate_source(source: &str) -> Option<String> {
    let Some((kind, value)) = source.split_once(':') else {
        return Some(format!(
            "invalid Baton source `{source}`; expected `key:`, `sc:`, `mouse:`, `axis:`, `button:`, or `hat:`"
        ));
    };
    if value.is_empty() {
        return Some(format!(
            "Baton source `{source}` requires a value after `:`"
        ));
    }

    let valid = match kind {
        "key" => enum_contains("KeyConstant", value),
        "sc" => enum_contains("Scancode", value),
        "mouse" => positive_integer(value),
        "axis" => value
            .strip_suffix(['-', '+'])
            .is_some_and(|axis| positive_integer(axis) || enum_contains("GamepadAxis", axis)),
        "button" => positive_integer(value) || enum_contains("GamepadButton", value),
        "hat" => {
            let (hat, direction) =
                value.split_at(value.chars().next().map(char::len_utf8).unwrap_or(0));
            positive_integer(hat) && enum_contains("JoystickHat", direction)
        }
        _ => {
            return Some(format!(
                "unknown Baton source type `{kind}`; expected key, sc, mouse, axis, button, or hat"
            ));
        }
    };

    (!valid).then(|| format!("invalid Baton source `{source}`"))
}

fn enum_contains(type_name: &str, value: &str) -> bool {
    lua::enum_value_items(type_name, "")
        .iter()
        .any(|item| item.label == value)
}

fn positive_integer(value: &str) -> bool {
    value.parse::<u32>().is_ok_and(|value| value > 0)
}

fn input_diagnostic(path: &Path, node: &impl Node, message: impl Into<String>) -> Diagnostic {
    let span = node_span(node).unwrap_or(SourceSpan {
        start: SourcePosition { line: 1, column: 1 },
        end: SourcePosition { line: 1, column: 2 },
    });

    Diagnostic::error(path, span.start.line, span.start.column, message)
        .with_code(DiagnosticCode::DOMAIN_INPUT_CONFIG)
        .with_range(
            span.start.line,
            span.start.column,
            span.end.line,
            span.end.column,
        )
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn span_contains_position(span: SourceSpan, position: SourcePosition) -> bool {
    position_key(span.start) <= position_key(position)
        && position_key(position) <= position_key(span.end)
}

fn span_size(span: SourceSpan) -> (usize, usize) {
    (
        span.end.line.saturating_sub(span.start.line),
        span.end.column.saturating_sub(span.start.column),
    )
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_tua_source;

    const SOURCE: &str = r#"local controls = input({
  controls = {
    left = { "key:left", "axis:leftx-" },
    right = { "key:right", "axis:leftx+" },
    up = { "key:up" },
    down = { "key:down" },
    confirm = { "key:return", "button:a" },
  },
  pairs = {
    move = { "left", "right", "up", "down" },
  },
})"#;

    #[test]
    fn derives_controls_pairs_and_return_arity() {
        let parsed = parse_tua_source("main.tua", SOURCE).expect("input should parse");
        let call = parsed
            .ast
            .nodes()
            .stmts()
            .find_map(|stmt| match stmt {
                full_moon::ast::Stmt::LocalAssignment(local) => local
                    .expressions()
                    .iter()
                    .find_map(|expression| match expression {
                        Expression::FunctionCall(call) => Some(call),
                        _ => None,
                    }),
                _ => None,
            })
            .expect("input call");
        let spec = spec_from_config(config_from_call(call).expect("input config"));

        assert_eq!(spec.controls, ["confirm", "down", "left", "right", "up"]);
        assert_eq!(spec.pairs, ["move"]);
        let Type::Table(input) = input_type(&spec) else {
            panic!("input should be a table");
        };
        assert!(input.field("pressed").is_some());
        assert!(input.field("getActiveDevice").is_some());
        assert!(input.field("pointer").is_some());
        assert!(input.field("pointerDelta").is_some());
    }

    #[test]
    fn validates_sources_and_pairs() {
        let source = r#"local controls = input({
  controls = {
    left = { "keyboard:left" },
    right = { "axis:leftx" },
  },
  pairs = {
    move = { "left", "right", "up" },
    left = { "left", "right", "left", "right" },
    invalid = { "left", "right", 1, "left" },
  },
  deadzone = 1.5,
})"#;
        let parsed = parse_tua_source("main.tua", source).expect("input should parse");
        let messages = config_diagnostics(&parsed)
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>();

        assert!(messages
            .iter()
            .any(|message| message.contains("unknown Baton source type")));
        assert!(messages
            .iter()
            .any(|message| message.contains("invalid Baton source `axis:leftx`")));
        assert!(messages
            .iter()
            .any(|message| message.contains("exactly four controls")));
        assert!(messages
            .iter()
            .any(|message| message.contains("both a control and a pair")));
        assert!(messages
            .iter()
            .any(|message| message.contains("control reference must be a string")));
        assert!(messages
            .iter()
            .any(|message| message.contains("between 0 and 1")));
    }

    #[test]
    fn completes_pair_controls_and_baton_sources() {
        let parsed = parse_tua_source("main.tua", SOURCE).expect("input should parse");
        let pair_labels = string_value_completion_items(
            &parsed,
            SourcePosition {
                line: 10,
                column: 16,
            },
        )
        .expect("pair completion")
        .into_iter()
        .map(|item| item.label)
        .collect::<BTreeSet<_>>();
        let source_labels = string_value_completion_items(
            &parsed,
            SourcePosition {
                line: 3,
                column: 20,
            },
        )
        .expect("source completion")
        .into_iter()
        .map(|item| item.label)
        .collect::<BTreeSet<_>>();

        assert!(pair_labels.contains("left"));
        assert!(pair_labels.contains("confirm"));
        assert!(source_labels.contains("key:left"));
    }

    #[test]
    fn computed_control_names_use_the_generic_input_surface() {
        let source = r#"local action = "confirm"
local controls = input({
  controls = {
    [action] = { "key:return" },
  },
})"#;
        let parsed = parse_tua_source("main.tua", source).expect("input should parse");
        let call = parsed
            .ast
            .nodes()
            .stmts()
            .filter_map(|stmt| match stmt {
                full_moon::ast::Stmt::LocalAssignment(local) => local
                    .expressions()
                    .iter()
                    .find_map(|expression| match expression {
                        Expression::FunctionCall(call) => Some(call),
                        _ => None,
                    }),
                _ => None,
            })
            .next()
            .expect("input call");

        assert_eq!(input_type_from_call(call), Some(Type::alias(TYPE_NAME)));
    }
}

use std::collections::{BTreeMap, BTreeSet};

use full_moon::ast::{
    Call, Expression, Field, FunctionArgs, FunctionCall, Suffix, TableConstructor,
};
use full_moon::node::Node;
use full_moon::visitors::Visitor;

use crate::ast_utils::{field_chain_before_first_call, token_text};
use crate::inspector::{
    StateMachineInspection, StateMachineStateInspection, StateMachineTransitionInspection,
};
use crate::types::{FunctionParameter, FunctionType, TableField, Type, TypeEnvironment};
use crate::{ParsedSource, SourcePosition, SourceSpan, TypedFunction, TypedParameter};

pub(crate) const TYPE_NAME: &str = "StateMachine";
pub(crate) const OPTIONS_TYPE: &str = "StateMachineOptions";
pub(crate) const EVENT_TYPE: &str = "StateMachineEvent";
pub(crate) const CALLBACK_TYPE: &str = "StateMachineCallback";
pub(crate) const CONSTRUCTOR_NAME: &str = "stateMachine";
pub(crate) const RUNTIME_MODULE: &str = "tuastatemachine";
pub(crate) const EMITTED_CONSTRUCTOR_LOCAL: &str = "__tua_state_machine";

const BUILTIN_TYPE_NAMES: &[&str] = &[TYPE_NAME, OPTIONS_TYPE, EVENT_TYPE, CALLBACK_TYPE];
const BASE_METHOD_NAMES: &[&str] = &[
    "is",
    "can",
    "cannot",
    "todot",
    "transition",
    "cancelTransition",
];
const BASE_FIELD_NAMES: &[&str] = &[
    "current",
    "currentTransitioningEvent",
    "asyncState",
    "events",
    "options",
    "NONE",
    "ASYNC",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StateMachineTransition {
    pub(crate) name: String,
    pub(crate) from: Vec<String>,
    pub(crate) wildcard_from: bool,
    pub(crate) to: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct StateMachineSpec {
    pub(crate) states: Vec<String>,
    pub(crate) events: Vec<String>,
    pub(crate) transitions: Vec<StateMachineTransition>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConfigContext {
    Options,
    Events,
    Event,
    Callbacks,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StringValueContext {
    State,
    Event,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConfigCompletionItem {
    pub(crate) label: String,
    pub(crate) detail: String,
    pub(crate) documentation: String,
    pub(crate) insert_text: String,
}

#[derive(Clone, Debug)]
struct StateMachineDeclaration {
    spec: StateMachineSpec,
    config_span: SourceSpan,
    events_span: Option<SourceSpan>,
    event_spans: Vec<SourceSpan>,
    callbacks_span: Option<SourceSpan>,
    string_values: Vec<(SourceSpan, StringValueContext, String)>,
}

pub(crate) fn builtin_type_names() -> &'static [&'static str] {
    BUILTIN_TYPE_NAMES
}

pub(crate) fn builtin_aliases() -> Vec<(String, Type)> {
    vec![
        (TYPE_NAME.to_owned(), base_machine_type()),
        (OPTIONS_TYPE.to_owned(), options_type()),
        (EVENT_TYPE.to_owned(), event_type()),
        (CALLBACK_TYPE.to_owned(), callback_type()),
    ]
}

pub(crate) fn is_constructor(name: &str) -> bool {
    name == CONSTRUCTOR_NAME
}

pub(crate) fn constructor_type() -> FunctionType {
    FunctionType::new([Type::alias(OPTIONS_TYPE)], Type::alias(TYPE_NAME))
}

pub(crate) fn constructor_typed_function() -> TypedFunction {
    typed_function_from_type(
        CONSTRUCTOR_NAME,
        constructor_type(),
        documentation().to_owned(),
    )
}

pub(crate) const fn detail() -> &'static str {
    "function stateMachine(options: StateMachineOptions): StateMachine"
}

pub(crate) const fn documentation() -> &'static str {
    "Create a typed finite state machine whose configured states, events, callbacks, and event methods are available to editor tooling."
}

pub(crate) const fn workflow_snippet() -> &'static str {
    "local ${1:machine} = stateMachine({\n  initial = \"${2:idle}\",\n  events = {\n    { name = \"${3:start}\", from = \"${2:idle}\", to = \"${4:running}\" },\n    { name = \"${5:stop}\", from = \"${4:running}\", to = \"${2:idle}\" },\n  },\n  callbacks = {\n    onenter${4:running} = function(self, event, from, to)\n      ${0}\n    end,\n  },\n})"
}

pub(crate) fn runtime_source() -> &'static str {
    include_str!("../../../vendor/runtime/tuastatemachine.lua")
}

pub(crate) fn inspections(parsed: &ParsedSource) -> Vec<StateMachineInspection> {
    let mut visitor = InspectionVisitor::default();
    visitor.visit_ast(&parsed.ast);
    visitor.inspections
}

pub(crate) fn machine_type_from_call(call: &FunctionCall) -> Option<Type> {
    let spec = spec_from_call(call)?;

    Some(machine_type(&spec))
}

pub(crate) fn derived_member_definition_span(
    call: &FunctionCall,
    member: &str,
) -> Option<SourceSpan> {
    let declaration = declaration_from_call(call)?;
    let (context, name) = derived_member_source(&declaration.spec, member)?;

    declaration
        .string_values
        .into_iter()
        .find_map(|(span, candidate_context, candidate)| {
            (candidate_context == context && candidate == name).then_some(span)
        })
}

pub(crate) fn derived_member_type(call: &FunctionCall, member: &str) -> Option<Type> {
    let spec = spec_from_call(call)?;
    let Type::Table(table) = machine_type(&spec) else {
        return None;
    };

    table.semantic_field(member).map(|field| field.ty.clone())
}

fn derived_member_source(
    spec: &StateMachineSpec,
    member: &str,
) -> Option<(StringValueContext, String)> {
    if spec.events.iter().any(|event| event == member) {
        return Some((StringValueContext::Event, member.to_owned()));
    }

    for prefix in ["onbefore", "onafter"] {
        if let Some(event) = member.strip_prefix(prefix) {
            if spec.events.iter().any(|candidate| candidate == event) {
                return Some((StringValueContext::Event, event.to_owned()));
            }
        }
    }
    for prefix in ["onleave", "onenter"] {
        if let Some(state) = member.strip_prefix(prefix) {
            if spec.states.iter().any(|candidate| candidate == state) {
                return Some((StringValueContext::State, state.to_owned()));
            }
        }
    }
    let name = member.strip_prefix("on")?;
    if spec.events.iter().any(|candidate| candidate == name) {
        return Some((StringValueContext::Event, name.to_owned()));
    }
    if spec.states.iter().any(|candidate| candidate == name) {
        return Some((StringValueContext::State, name.to_owned()));
    }

    None
}

pub(crate) fn type_is_state_machine(environment: &TypeEnvironment, candidate: &Type) -> bool {
    machine_table(environment, candidate, 0).is_some()
}

pub(crate) fn type_summary(
    environment: &TypeEnvironment,
    candidate: &Type,
) -> Option<(Type, Type)> {
    let table = machine_table(environment, candidate, 0)?;
    let states = table.field("current")?.ty.clone();
    let events = table
        .field("currentTransitioningEvent")
        .map(|field| without_nil(field.ty.clone()))
        .unwrap_or_else(Type::string);

    Some((states, events))
}

pub(crate) fn typed_member(
    environment: &TypeEnvironment,
    receiver_type: &Type,
    member: &str,
) -> Option<TypedFunction> {
    let table = machine_table(environment, receiver_type, 0)?;
    let Type::Function(function) = &table.semantic_field(member)?.ty else {
        return None;
    };
    let documentation = member_documentation(member, table);

    Some(typed_function_from_type(
        &format!("{TYPE_NAME}:{member}"),
        function.clone(),
        documentation,
    ))
}

pub(crate) fn config_completion_items(
    parsed: &ParsedSource,
    position: SourcePosition,
) -> Option<Vec<ConfigCompletionItem>> {
    let declaration = declaration_at(parsed, position)?;
    let context = declaration.context_at(position);
    let items = match context {
        ConfigContext::Options => options_completion_items(),
        ConfigContext::Events => events_completion_items(),
        ConfigContext::Event => event_completion_items(),
        ConfigContext::Callbacks => callback_completion_items(&declaration.spec),
    };

    Some(items)
}

pub(crate) fn string_value_completion_items(
    parsed: &ParsedSource,
    position: SourcePosition,
) -> Option<Vec<ConfigCompletionItem>> {
    let declaration = declaration_at(parsed, position)?;
    let (active_span, context, active_value) =
        declaration
            .string_values
            .iter()
            .find_map(|(span, context, value)| {
                span_contains_position(*span, position).then_some((*span, *context, value.as_str()))
            })?;
    let values = match context {
        StringValueContext::State => &declaration.spec.states,
        StringValueContext::Event => &declaration.spec.events,
    };
    let active_value_is_declared_elsewhere =
        declaration
            .string_values
            .iter()
            .any(|(span, candidate_context, value)| {
                *span != active_span && *candidate_context == context && value == active_value
            });
    let detail = match context {
        StringValueContext::State => "state machine state",
        StringValueContext::Event => "state machine event",
    };

    Some(
        values
            .iter()
            .filter(|value| **value != active_value || active_value_is_declared_elsewhere)
            .map(|value| ConfigCompletionItem {
                label: value.clone(),
                detail: detail.to_owned(),
                documentation: format!("Configured {detail} `{value}`."),
                insert_text: value.clone(),
            })
            .collect(),
    )
}

pub(crate) fn method_string_completion_items(
    environment: &TypeEnvironment,
    receiver_type: &Type,
    method: &str,
    active_parameter: usize,
) -> Option<Vec<ConfigCompletionItem>> {
    if active_parameter != 0 {
        return None;
    }
    let (states, events) = type_summary(environment, receiver_type)?;
    let (ty, detail) = match method {
        "is" => (states, "state machine state"),
        "can" | "cannot" | "transition" | "cancelTransition" => (events, "state machine event"),
        _ => return None,
    };
    let values = string_literal_values(&ty);
    if values.is_empty() {
        return None;
    }

    Some(
        values
            .into_iter()
            .map(|value| ConfigCompletionItem {
                label: value.clone(),
                detail: detail.to_owned(),
                documentation: format!("Configured {detail} `{value}`."),
                insert_text: value,
            })
            .collect(),
    )
}

fn options_type() -> Type {
    Type::table([
        ("initial", TableField::optional(Type::string())),
        (
            "events",
            TableField::required(Type::array(Type::alias(EVENT_TYPE))),
        ),
        ("callbacks", TableField::optional(Type::Any)),
    ])
}

fn event_type() -> Type {
    Type::table([
        ("name", TableField::required(Type::string())),
        (
            "from",
            TableField::required(Type::union([Type::string(), Type::array(Type::string())])),
        ),
        ("to", TableField::required(Type::string())),
    ])
}

fn callback_type() -> Type {
    Type::Function(FunctionType::new_signature(
        [
            FunctionParameter::named("self", Type::alias(TYPE_NAME)),
            FunctionParameter::named("event", Type::string()),
            FunctionParameter::named("from", Type::string()),
            FunctionParameter::named("to", Type::string()),
        ],
        4,
        Some(Type::Any),
        [callback_return_type()],
    ))
}

fn base_machine_type() -> Type {
    machine_type(&StateMachineSpec::default())
}

fn machine_type(spec: &StateMachineSpec) -> Type {
    let state_type = literal_union_or_string(&spec.states);
    let event_type = literal_union_or_string(&spec.events);
    // The receiver is hidden at colon call sites. Keeping it permissive avoids
    // recursively comparing a specialized machine against its generic alias.
    let self_type = Type::Any;
    let mut fields = BTreeMap::from([
        (
            "current".to_owned(),
            TableField::required(state_type.clone()),
        ),
        (
            "currentTransitioningEvent".to_owned(),
            TableField::optional(event_type.clone()),
        ),
        (
            "asyncState".to_owned(),
            TableField::required(Type::string()),
        ),
        (
            "events".to_owned(),
            TableField::required(Type::dictionary(event_type.clone(), Type::Any)),
        ),
        (
            "options".to_owned(),
            TableField::required(Type::alias(OPTIONS_TYPE)),
        ),
        (
            "NONE".to_owned(),
            TableField::required(Type::string_literal("none")),
        ),
        (
            "ASYNC".to_owned(),
            TableField::required(Type::string_literal("async")),
        ),
        (
            "is".to_owned(),
            function_field(
                [
                    FunctionParameter::named("self", self_type.clone()),
                    FunctionParameter::named("state", state_type.clone()),
                ],
                2,
                None,
                [Type::boolean()],
            ),
        ),
        (
            "can".to_owned(),
            function_field(
                [
                    FunctionParameter::named("self", self_type.clone()),
                    FunctionParameter::named("event", event_type.clone()),
                ],
                2,
                None,
                [
                    Type::boolean(),
                    Type::union([state_type.clone(), Type::nil()]),
                ],
            ),
        ),
        (
            "cannot".to_owned(),
            function_field(
                [
                    FunctionParameter::named("self", self_type.clone()),
                    FunctionParameter::named("event", event_type.clone()),
                ],
                2,
                None,
                [Type::boolean()],
            ),
        ),
        (
            "todot".to_owned(),
            function_field(
                [
                    FunctionParameter::named("self", self_type.clone()),
                    FunctionParameter::named("filename", Type::string()),
                ],
                2,
                None,
                [Type::nil()],
            ),
        ),
        (
            "transition".to_owned(),
            function_field(
                [
                    FunctionParameter::named("self", self_type.clone()),
                    FunctionParameter::named("event", event_type.clone()),
                ],
                2,
                None,
                [Type::union([Type::boolean(), Type::nil()])],
            ),
        ),
        (
            "cancelTransition".to_owned(),
            function_field(
                [
                    FunctionParameter::named("self", self_type.clone()),
                    FunctionParameter::named("event", event_type.clone()),
                ],
                2,
                None,
                [Type::nil()],
            ),
        ),
    ]);

    for event in &spec.events {
        fields.insert(
            event.clone(),
            function_field(
                [FunctionParameter::named("self", self_type.clone())],
                1,
                Some(Type::Any),
                [Type::boolean()],
            ),
        );
    }

    for callback in callback_names(spec) {
        fields.insert(
            callback,
            TableField::optional(Type::Function(callback_function_type(
                state_type.clone(),
                event_type.clone(),
            ))),
        );
    }

    Type::table(fields)
}

fn callback_function_type(state_type: Type, event_type: Type) -> FunctionType {
    FunctionType::new_signature(
        [
            FunctionParameter::named("self", Type::alias(TYPE_NAME)),
            FunctionParameter::named("event", event_type),
            FunctionParameter::named("from", state_type.clone()),
            FunctionParameter::named("to", state_type),
        ],
        4,
        Some(Type::Any),
        [callback_return_type()],
    )
}

fn callback_return_type() -> Type {
    Type::union([Type::boolean(), Type::string_literal("async"), Type::nil()])
}

fn function_field(
    parameters: impl IntoIterator<Item = FunctionParameter>,
    required_parameter_count: usize,
    vararg_type: Option<Type>,
    returns: impl IntoIterator<Item = Type>,
) -> TableField {
    TableField::required(Type::Function(FunctionType::new_signature(
        parameters,
        required_parameter_count,
        vararg_type,
        returns,
    )))
}

fn literal_union_or_string(values: &[String]) -> Type {
    match values {
        [] => Type::string(),
        [value] => Type::string_literal(value),
        _ => Type::union(values.iter().cloned().map(Type::string_literal)),
    }
}

fn without_nil(ty: Type) -> Type {
    let Type::Union(types) = ty else {
        return ty;
    };
    let types = types
        .into_iter()
        .filter(|ty| *ty != Type::nil())
        .collect::<Vec<_>>();

    match types.as_slice() {
        [] => Type::string(),
        [ty] => ty.clone(),
        _ => Type::union(types),
    }
}

fn string_literal_values(ty: &Type) -> Vec<String> {
    match ty {
        Type::StringLiteral(value) => vec![value.clone()],
        Type::Union(types) => types
            .iter()
            .flat_map(string_literal_values)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

fn machine_table<'a>(
    environment: &'a TypeEnvironment,
    candidate: &'a Type,
    depth: usize,
) -> Option<&'a crate::TableShape> {
    if depth >= 16 {
        return None;
    }

    match candidate {
        Type::Alias(name) if name == TYPE_NAME => environment.alias(name).and_then(|resolved| {
            if resolved == candidate {
                None
            } else {
                machine_table(environment, resolved, depth + 1)
            }
        }),
        Type::Alias(name) => environment
            .alias(name)
            .filter(|resolved| *resolved != candidate)
            .and_then(|resolved| machine_table(environment, resolved, depth + 1)),
        Type::Table(table) if table_is_state_machine(table) => Some(table),
        Type::Union(types) => types
            .iter()
            .find_map(|ty| machine_table(environment, ty, depth + 1)),
        _ => None,
    }
}

fn table_is_state_machine(table: &crate::TableShape) -> bool {
    BASE_METHOD_NAMES
        .iter()
        .chain(BASE_FIELD_NAMES)
        .all(|field| table.semantic_field(field).is_some())
}

fn typed_function_from_type(
    name: &str,
    function: FunctionType,
    documentation: String,
) -> TypedFunction {
    let parameters = function
        .parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TypedParameter {
            name: parameter
                .name
                .clone()
                .unwrap_or_else(|| format!("arg{}", index + 1)),
            ty: parameter.ty.clone(),
            documentation: None,
        })
        .collect::<Vec<_>>();

    TypedFunction {
        name: Some(name.to_owned()),
        parameter_count: parameters.len(),
        required_parameter_count: function.required_parameter_count,
        parameters,
        has_varargs: function.vararg_type.is_some(),
        vararg_type: function.vararg_type.as_deref().cloned(),
        return_type: function.return_types_or_any().first().cloned(),
        return_types: function.return_types_or_any().to_vec(),
        documentation: Some(documentation),
        overloads: Vec::new(),
    }
}

fn member_documentation(member: &str, table: &crate::TableShape) -> String {
    match member {
        "is" => "Return whether the machine is currently in the selected state.".to_owned(),
        "can" => "Return whether an event can fire from the current state and its destination state.".to_owned(),
        "cannot" => "Return whether an event cannot fire from the current state.".to_owned(),
        "todot" => "Write the configured transition graph as a Graphviz DOT file.".to_owned(),
        "transition" => "Resume a pending asynchronous transition for the selected event.".to_owned(),
        "cancelTransition" => "Cancel a pending asynchronous transition for the selected event.".to_owned(),
        _ if member.starts_with("on") => {
            "State machine callback receiving self, event, source state, destination state, and event payload arguments.".to_owned()
        }
        _ if table
            .field(member)
            .is_some_and(|field| matches!(&field.ty, Type::Function(function) if function.vararg_type.is_some())) =>
        {
            format!("Fire the `{member}` state machine event with optional payload arguments.")
        }
        _ => "State machine method.".to_owned(),
    }
}

fn options_completion_items() -> Vec<ConfigCompletionItem> {
    vec![
        completion_item(
            "initial",
            "state",
            "Initial state. Defaults to `none` when omitted.",
            "initial = \"${1:idle}\",",
        ),
        completion_item(
            "events",
            "StateMachineEvent[]",
            "Static transition declarations used to derive event methods and state types.",
            "events = {\n  { name = \"${1:start}\", from = \"${2:idle}\", to = \"${3:running}\" },\n},",
        ),
        completion_item(
            "callbacks",
            "{ [string]: StateMachineCallback }",
            "Callbacks derived from configured event and state names.",
            "callbacks = {\n  ${0}\n},",
        ),
    ]
}

fn events_completion_items() -> Vec<ConfigCompletionItem> {
    vec![completion_item(
        "event",
        "StateMachineEvent",
        "Named transition with a source state, source state list, or wildcard.",
        "{ name = \"${1:event}\", from = \"${2:from}\", to = \"${3:to}\" },",
    )]
}

fn event_completion_items() -> Vec<ConfigCompletionItem> {
    vec![
        completion_item(
            "name",
            "string",
            "Event name. The machine exposes a method with this exact name.",
            "name = \"${1:event}\",",
        ),
        completion_item(
            "from",
            "string | string[]",
            "State, state list, or `*` wildcard from which the event can fire.",
            "from = \"${1:state}\",",
        ),
        completion_item(
            "to",
            "string",
            "Destination state entered after the event fires.",
            "to = \"${1:state}\",",
        ),
    ]
}

fn callback_completion_items(spec: &StateMachineSpec) -> Vec<ConfigCompletionItem> {
    let state_type = literal_union_or_string(&spec.states).to_string();
    let event_type = literal_union_or_string(&spec.events).to_string();

    callback_names(spec)
        .into_iter()
        .map(|name| ConfigCompletionItem {
            label: name.clone(),
            detail: format!(
                "function(self: {TYPE_NAME}, event: {event_type}, from: {state_type}, to: {state_type}, ...: any)"
            ),
            documentation: callback_documentation(&name).to_owned(),
            insert_text: format!(
                "{name} = function(self: {TYPE_NAME}, event: {event_type}, from: {state_type}, to: {state_type}, ...: any)\n  ${{0}}\nend,"
            ),
        })
        .collect()
}

fn callback_names(spec: &StateMachineSpec) -> Vec<String> {
    let mut names = BTreeSet::from(["onstatechange".to_owned()]);

    for event in &spec.events {
        names.insert(format!("onbefore{event}"));
        names.insert(format!("onafter{event}"));
        names.insert(format!("on{event}"));
    }
    for state in &spec.states {
        names.insert(format!("onleave{state}"));
        names.insert(format!("onenter{state}"));
        names.insert(format!("on{state}"));
    }

    names.into_iter().collect()
}

fn callback_documentation(name: &str) -> &'static str {
    if name == "onstatechange" {
        "Called after every completed state change."
    } else if name.starts_with("onbefore") {
        "Called before the named event; return false to cancel it."
    } else if name.starts_with("onafter") {
        "Called after the named event completes."
    } else if name.starts_with("onleave") {
        "Called before leaving the named state; return false to cancel or self.ASYNC to defer."
    } else if name.starts_with("onenter") {
        "Called when entering the named state; return self.ASYNC to defer completion."
    } else {
        "Shorthand callback for entering a state or completing an event with this name."
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

fn declaration_at(
    parsed: &ParsedSource,
    position: SourcePosition,
) -> Option<StateMachineDeclaration> {
    let mut visitor = DeclarationVisitor::default();
    visitor.visit_ast(&parsed.ast);

    visitor
        .declarations
        .into_iter()
        .filter(|declaration| span_contains_position(declaration.config_span, position))
        .min_by_key(|declaration| span_size(declaration.config_span))
}

impl StateMachineDeclaration {
    fn context_at(&self, position: SourcePosition) -> ConfigContext {
        if self
            .callbacks_span
            .is_some_and(|span| span_contains_position(span, position))
        {
            return ConfigContext::Callbacks;
        }
        if self
            .event_spans
            .iter()
            .any(|span| span_contains_position(*span, position))
        {
            return ConfigContext::Event;
        }
        if self
            .events_span
            .is_some_and(|span| span_contains_position(span, position))
        {
            return ConfigContext::Events;
        }

        ConfigContext::Options
    }
}

#[derive(Default)]
struct DeclarationVisitor {
    declarations: Vec<StateMachineDeclaration>,
}

impl Visitor for DeclarationVisitor {
    fn visit_function_call(&mut self, call: &FunctionCall) {
        if let Some(declaration) = declaration_from_call(call) {
            self.declarations.push(declaration);
        }
    }
}

#[derive(Default)]
struct InspectionVisitor {
    inspections: Vec<StateMachineInspection>,
}

impl Visitor for InspectionVisitor {
    fn visit_local_assignment(&mut self, local: &full_moon::ast::LocalAssignment) {
        if let Some(inspection) = inspection_from_local_assignment(local) {
            self.inspections.push(inspection);
        }
    }
}

fn inspection_from_local_assignment(
    local: &full_moon::ast::LocalAssignment,
) -> Option<StateMachineInspection> {
    let names = local.names().iter().collect::<Vec<_>>();
    let expressions = local.expressions().iter().collect::<Vec<_>>();

    if names.len() != 1 || expressions.len() != 1 {
        return None;
    }

    let name = names[0];
    let call = state_machine_call(&expressions[0])?;
    let declaration = declaration_from_call(call)?;
    let Expression::TableConstructor(config) = first_parenthesized_argument(call)? else {
        return None;
    };
    let span = node_span(local)?;
    let name_span = node_span(name)?;
    let initial_expression = named_field(config, "initial");
    let initial = initial_expression
        .and_then(string_expression)
        .unwrap_or_else(|| "none".to_owned());
    let mut state_spans = BTreeMap::new();

    state_spans.insert(
        initial.clone(),
        initial_expression
            .and_then(node_span)
            .unwrap_or(declaration.config_span),
    );

    let mut transitions = Vec::new();
    if let Some(event_list) = named_field(config, "events").and_then(table_expression) {
        for event in table_values(event_list).filter_map(table_expression) {
            let Some(name_expression) = named_field(event, "name") else {
                continue;
            };
            let Some(event_name) = string_expression(name_expression) else {
                continue;
            };
            let Some(to_expression) = named_field(event, "to") else {
                continue;
            };
            let Some(to) = string_expression(to_expression) else {
                continue;
            };
            let from_entries = named_field(event, "from")
                .map(string_value_entries)
                .unwrap_or_default();
            let wildcard_from = from_entries.iter().any(|(state, _)| state == "*");
            let from = from_entries
                .iter()
                .filter(|(state, _)| state != "*")
                .map(|(state, _)| state.clone())
                .collect::<Vec<_>>();
            let event_span = node_span(name_expression)?;
            let transition_span = node_span(event)?;

            for (state, state_span) in from_entries {
                if state != "*" {
                    state_spans.entry(state).or_insert(state_span);
                }
            }
            state_spans
                .entry(to.clone())
                .or_insert(node_span(to_expression)?);
            transitions.push(StateMachineTransitionInspection {
                event: event_name,
                from,
                wildcard_from,
                to,
                span: transition_span,
                event_span,
            });
        }
    }

    let states = declaration
        .spec
        .states
        .into_iter()
        .map(|state| StateMachineStateInspection {
            initial: state == initial,
            span: state_spans
                .get(&state)
                .copied()
                .unwrap_or(declaration.config_span),
            name: state,
        })
        .collect();

    Some(StateMachineInspection {
        name: token_text(name),
        initial,
        states,
        transitions,
        span,
        name_span,
    })
}

fn state_machine_call(expression: &Expression) -> Option<&FunctionCall> {
    match expression {
        Expression::FunctionCall(call) => {
            (field_chain_before_first_call(call)?.as_slice() == [CONSTRUCTOR_NAME]).then_some(call)
        }
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => state_machine_call(expression),
        _ => None,
    }
}

fn string_value_entries(expression: &Expression) -> Vec<(String, SourceSpan)> {
    if let (Some(value), Some(span)) = (string_expression(expression), node_span(expression)) {
        return vec![(value, span)];
    }
    let Some(table) = table_expression(expression) else {
        return Vec::new();
    };

    table_values(table)
        .filter_map(|value| Some((string_expression(value)?, node_span(value)?)))
        .collect()
}

fn declaration_from_call(call: &FunctionCall) -> Option<StateMachineDeclaration> {
    let parts = field_chain_before_first_call(call)?;
    if parts.as_slice() != [CONSTRUCTOR_NAME] {
        return None;
    }
    let Expression::TableConstructor(config) = first_parenthesized_argument(call)? else {
        return None;
    };
    let config_span = node_span(config)?;
    let spec = spec_from_config(config);
    let mut string_values = Vec::new();

    if let Some(initial) = named_field(config, "initial") {
        push_string_value_spans(initial, StringValueContext::State, &mut string_values);
    }

    let events = named_field(config, "events").and_then(table_expression);
    let callbacks = named_field(config, "callbacks").and_then(table_expression);
    let mut event_spans = Vec::new();

    if let Some(events) = events {
        for event in table_values(events).filter_map(table_expression) {
            if let Some(span) = node_span(event) {
                event_spans.push(span);
            }
            if let Some(name) = named_field(event, "name") {
                push_string_value_spans(name, StringValueContext::Event, &mut string_values);
            }
            if let Some(from) = named_field(event, "from") {
                push_string_value_spans(from, StringValueContext::State, &mut string_values);
            }
            if let Some(to) = named_field(event, "to") {
                push_string_value_spans(to, StringValueContext::State, &mut string_values);
            }
        }
    }

    Some(StateMachineDeclaration {
        spec,
        config_span,
        events_span: events.and_then(node_span),
        event_spans,
        callbacks_span: callbacks.and_then(node_span),
        string_values,
    })
}

fn spec_from_call(call: &FunctionCall) -> Option<StateMachineSpec> {
    let parts = field_chain_before_first_call(call)?;
    if parts.as_slice() != [CONSTRUCTOR_NAME] {
        return None;
    }
    let Expression::TableConstructor(config) = first_parenthesized_argument(call)? else {
        return None;
    };

    Some(spec_from_config(config))
}

fn spec_from_config(config: &TableConstructor) -> StateMachineSpec {
    let mut states = BTreeSet::new();
    let mut events = BTreeSet::new();
    let mut transitions = Vec::new();

    if let Some(initial) = named_field(config, "initial").and_then(string_expression) {
        states.insert(initial);
    } else {
        states.insert("none".to_owned());
    }

    if let Some(event_list) = named_field(config, "events").and_then(table_expression) {
        for event in table_values(event_list).filter_map(table_expression) {
            let Some(name) = named_field(event, "name").and_then(string_expression) else {
                continue;
            };
            let Some(to) = named_field(event, "to").and_then(string_expression) else {
                continue;
            };
            let from = named_field(event, "from")
                .map(string_values)
                .unwrap_or_default();
            let wildcard_from = from.iter().any(|state| state == "*");
            let from = from
                .into_iter()
                .filter(|state| state != "*")
                .collect::<Vec<_>>();

            events.insert(name.clone());
            states.extend(from.iter().cloned());
            states.insert(to.clone());
            transitions.push(StateMachineTransition {
                name,
                from,
                wildcard_from,
                to,
            });
        }
    }

    StateMachineSpec {
        states: states.into_iter().collect(),
        events: events.into_iter().collect(),
        transitions,
    }
}

fn first_parenthesized_argument(call: &FunctionCall) -> Option<&Expression> {
    call.suffixes().find_map(|suffix| match suffix {
        Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. })) => {
            arguments.iter().next()
        }
        _ => None,
    })
}

fn named_field<'a>(table: &'a TableConstructor, name: &str) -> Option<&'a Expression> {
    table.fields().iter().find_map(|field| match field {
        Field::NameKey { key, value, .. } if token_text(key) == name => Some(value),
        Field::ExpressionKey { key, value, .. }
            if string_expression(key).as_deref() == Some(name) =>
        {
            Some(value)
        }
        _ => None,
    })
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

fn string_values(expression: &Expression) -> Vec<String> {
    if let Some(value) = string_expression(expression) {
        return vec![value];
    }
    let Some(table) = table_expression(expression) else {
        return Vec::new();
    };

    table_values(table).filter_map(string_expression).collect()
}

fn string_expression(expression: &Expression) -> Option<String> {
    match expression {
        Expression::String(value) => Some(unquote_string(&value.to_string())),
        _ => None,
    }
}

fn push_string_value_spans(
    expression: &Expression,
    context: StringValueContext,
    spans: &mut Vec<(SourceSpan, StringValueContext, String)>,
) {
    if let Some(value) = string_expression(expression) {
        if let Some(span) = node_span(expression) {
            spans.push((span, context, value));
        }
        return;
    }
    let Some(table) = table_expression(expression) else {
        return;
    };

    for value in table_values(table).filter(|value| matches!(value, Expression::String(_))) {
        if let (Some(span), Some(text)) = (node_span(value), string_expression(value)) {
            spans.push((span, context, text));
        }
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

    const SOURCE: &str = r#"local light = stateMachine({
  initial = "green",
  events = {
    { name = "warn", from = "green", to = "yellow" },
    { name = "panic", from = "yellow", to = "red" },
    { name = "reset", from = { "red", "yellow" }, to = "green" },
  },
  callbacks = {
    onpanic = function(self, event, from, to) end,
  },
})"#;

    #[test]
    fn derives_states_events_and_machine_fields() {
        let parsed = parse_tua_source("main.tua", SOURCE).expect("state machine should parse");
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
            .expect("state machine call");
        let spec = spec_from_call(call).expect("state machine spec");

        assert_eq!(spec.states, ["green", "red", "yellow"]);
        assert_eq!(spec.events, ["panic", "reset", "warn"]);

        let Type::Table(machine) = machine_type(&spec) else {
            panic!("machine should be a table");
        };
        assert!(machine.field("warn").is_some());
        assert!(machine.field("onbeforewarn").is_some());
        assert!(machine.field("onenterred").is_some());
        assert_eq!(
            machine.field("current").map(|field| field.ty.to_string()),
            Some("\"green\" | \"red\" | \"yellow\"".to_owned())
        );
    }

    #[test]
    fn completes_all_derived_callbacks() {
        let parsed = parse_tua_source("main.tua", SOURCE).expect("state machine should parse");
        let position = SourcePosition { line: 9, column: 5 };
        let labels = config_completion_items(&parsed, position)
            .expect("callback completion")
            .into_iter()
            .map(|item| item.label)
            .collect::<BTreeSet<_>>();

        assert!(labels.contains("onbeforepanic"));
        assert!(labels.contains("onafterwarn"));
        assert!(labels.contains("onleaveyellow"));
        assert!(labels.contains("onenterred"));
        assert!(labels.contains("onstatechange"));
    }

    #[test]
    fn locates_callback_context_while_a_key_is_incomplete() {
        let source = r#"local light = stateMachine({
  initial = "green",
  events = {
    { name = "warn", from = "green", to = "yellow" },
  },
  callbacks = {
    on
  },
})"#;
        let parsed = parse_tua_source("main.tua", source).expect("state machine should parse");
        let position = SourcePosition { line: 7, column: 7 };
        let labels = config_completion_items(&parsed, position)
            .expect("callback completion")
            .into_iter()
            .map(|item| item.label)
            .collect::<BTreeSet<_>>();

        assert!(labels.contains("onbeforewarn"));
    }
}

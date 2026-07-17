use full_moon::ast::luau::TypeInfo;
use full_moon::ast::{self, Call, Expression, FunctionArgs, FunctionCall, Suffix};
use full_moon::node::Node;
use full_moon::tokenizer::{TokenReference, TokenType};
use full_moon::visitors::Visitor;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::ast_utils::{
    field_chain_before_first_call, plain_field_chain_from_expression, token_text,
};
use crate::parser::{ParsedSource, SourcePosition, SourceSpan};
use crate::scope::ScopeMap;
use crate::semantic::SemanticModel;
use crate::types::{widen_inferred_type, FunctionType, TableField, Type, TypeEnvironment};
use crate::{Diagnostic, DiagnosticCode, TypedFunction, TypedParameter};

pub(crate) const TYPE_NAME: &str = "SignalBus";
pub(crate) const CONSTRUCTOR_NAME: &str = "signal";
pub(crate) const RUNTIME_MODULE: &str = "tuasignal";
pub(crate) const EMITTED_CONSTRUCTOR_LOCAL: &str = "__tua_signal";

const BUILTIN_TYPE_NAMES: &[&str] = &[TYPE_NAME];
const EVENT_NAME_METHODS: &[&str] = &[
    "on",
    "once",
    "register",
    "declare",
    "emit",
    "off",
    "remove",
    "clear",
    "has",
    "listeners",
];
const EVENT_SCHEMA_METHODS: &[&str] = &["on", "once", "register", "declare"];

pub(crate) fn builtin_type_names() -> &'static [&'static str] {
    BUILTIN_TYPE_NAMES
}

pub(crate) fn signal_bus_type() -> Type {
    Type::table(method_names().into_iter().map(|name| {
        (
            name,
            TableField::required(Type::Function(method_function_type(name))),
        )
    }))
}

pub(crate) fn constructor_type() -> FunctionType {
    FunctionType::new([], Type::alias(TYPE_NAME))
}

pub(crate) fn constructor_typed_function() -> TypedFunction {
    TypedFunction {
        name: Some(CONSTRUCTOR_NAME.to_owned()),
        parameters: Vec::new(),
        parameter_count: 0,
        required_parameter_count: 0,
        has_varargs: false,
        vararg_type: None,
        return_type: Some(Type::alias(TYPE_NAME)),
        return_types: vec![Type::alias(TYPE_NAME)],
        documentation: Some(documentation().to_owned()),
        overloads: Vec::new(),
    }
}

pub(crate) fn typed_method(method: &str) -> Option<TypedFunction> {
    let signal_bus = Type::alias(TYPE_NAME);
    let string = Type::string();
    let any = Type::Any;
    let nil = Type::nil();
    let boolean = Type::boolean();
    let table = Type::empty_table();

    match method {
        "on" | "once" | "register" | "declare" | "onPattern" | "registerPattern" => {
            let name_parameter = if matches!(method, "onPattern" | "registerPattern") {
                "pattern"
            } else {
                "name"
            };
            let required_parameter_count = if method == "declare" { 2 } else { 3 };
            Some(TypedFunction {
                name: Some(method.to_owned()),
                parameters: vec![
                    typed_parameter("self", signal_bus),
                    typed_parameter(name_parameter, string),
                    typed_parameter("handler", any.clone()),
                ],
                parameter_count: 3,
                required_parameter_count,
                has_varargs: false,
                vararg_type: None,
                return_type: Some(any.clone()),
                return_types: vec![any],
                documentation: Some(method_documentation(method).to_owned()),
                overloads: Vec::new(),
            })
        }
        "off" | "remove" | "offPattern" | "removePattern" => {
            let name_parameter = if matches!(method, "offPattern" | "removePattern") {
                "pattern"
            } else {
                "name"
            };
            Some(TypedFunction {
                name: Some(method.to_owned()),
                parameters: vec![
                    typed_parameter("self", signal_bus),
                    typed_parameter(name_parameter, string),
                    typed_parameter("handler", any.clone()),
                ],
                parameter_count: 3,
                required_parameter_count: 2,
                has_varargs: false,
                vararg_type: None,
                return_type: Some(nil.clone()),
                return_types: vec![nil],
                documentation: Some(method_documentation(method).to_owned()),
                overloads: Vec::new(),
            })
        }
        "emit" | "emitPattern" => Some(TypedFunction {
            name: Some(method.to_owned()),
            parameters: vec![
                typed_parameter("self", signal_bus),
                typed_parameter(if method == "emit" { "name" } else { "pattern" }, string),
            ],
            parameter_count: 2,
            required_parameter_count: 2,
            has_varargs: true,
            vararg_type: Some(any),
            return_type: Some(nil.clone()),
            return_types: vec![nil],
            documentation: Some(method_documentation(method).to_owned()),
            overloads: Vec::new(),
        }),
        "clear" => Some(TypedFunction {
            name: Some(method.to_owned()),
            parameters: vec![
                typed_parameter("self", signal_bus),
                typed_parameter("name", string),
            ],
            parameter_count: 2,
            required_parameter_count: 1,
            has_varargs: false,
            vararg_type: None,
            return_type: Some(nil.clone()),
            return_types: vec![nil],
            documentation: Some(method_documentation(method).to_owned()),
            overloads: Vec::new(),
        }),
        "clearPattern" => Some(TypedFunction {
            name: Some(method.to_owned()),
            parameters: vec![
                typed_parameter("self", signal_bus),
                typed_parameter("pattern", string),
            ],
            parameter_count: 2,
            required_parameter_count: 2,
            has_varargs: false,
            vararg_type: None,
            return_type: Some(nil.clone()),
            return_types: vec![nil],
            documentation: Some(method_documentation(method).to_owned()),
            overloads: Vec::new(),
        }),
        "has" => Some(TypedFunction {
            name: Some(method.to_owned()),
            parameters: vec![
                typed_parameter("self", signal_bus),
                typed_parameter("name", string),
            ],
            parameter_count: 2,
            required_parameter_count: 2,
            has_varargs: false,
            vararg_type: None,
            return_type: Some(boolean.clone()),
            return_types: vec![boolean],
            documentation: Some(method_documentation(method).to_owned()),
            overloads: Vec::new(),
        }),
        "listeners" => Some(TypedFunction {
            name: Some(method.to_owned()),
            parameters: vec![
                typed_parameter("self", signal_bus),
                typed_parameter("name", string),
            ],
            parameter_count: 2,
            required_parameter_count: 2,
            has_varargs: false,
            vararg_type: None,
            return_type: Some(table.clone()),
            return_types: vec![table],
            documentation: Some(method_documentation(method).to_owned()),
            overloads: Vec::new(),
        }),
        _ => None,
    }
}

pub(crate) fn is_constructor(name: &str) -> bool {
    name == CONSTRUCTOR_NAME
}

pub(crate) fn is_event_name_method(name: &str) -> bool {
    EVENT_NAME_METHODS.contains(&name)
}

pub(crate) fn is_event_schema_method(name: &str) -> bool {
    EVENT_SCHEMA_METHODS.contains(&name)
}

pub(crate) fn source_may_contain_event_call(source: &str) -> bool {
    (source.contains("signal") || source.contains("SignalBus"))
        && EVENT_NAME_METHODS.iter().any(|method| {
            source.contains(&format!(":{method}")) || source.contains(&format!(".{method}"))
        })
}

pub(crate) fn type_is_signal_bus(environment: &TypeEnvironment, ty: &Type) -> bool {
    match ty {
        Type::Alias(name) if name == TYPE_NAME => true,
        Type::Union(types) => types.iter().any(|ty| type_is_signal_bus(environment, ty)),
        _ => match resolve_aliases(environment, ty, 0) {
            Type::Table(table) => environment
                .alias(TYPE_NAME)
                .is_some_and(|signal_type| signal_type == &Type::Table(table)),
            _ => false,
        },
    }
}

pub(crate) const fn detail() -> &'static str {
    "function signal(): SignalBus"
}

pub(crate) const fn documentation() -> &'static str {
    "Creates a lightweight event bus for LOVE game events and gameplay messages."
}

pub(crate) const fn signal_snippet() -> &'static str {
    "local ${1:signals} = signal()\n${1:signals}:on(\"${2:event}\", function(${3})\n  ${0}\nend)"
}

pub(crate) fn runtime_source() -> &'static str {
    include_str!("../../../vendor/runtime/tuasignal.lua")
}

pub(crate) fn method_names() -> Vec<&'static str> {
    vec![
        "on",
        "once",
        "declare",
        "emit",
        "off",
        "clear",
        "has",
        "listeners",
        "register",
        "remove",
        "onPattern",
        "registerPattern",
        "emitPattern",
        "offPattern",
        "removePattern",
        "clearPattern",
    ]
}

pub(crate) fn method_documentation(method: &str) -> &'static str {
    match method {
        "on" => "Register a handler for a named signal and return the handler for later removal.",
        "once" => "Register a handler that removes itself after the next matching emit.",
        "declare" => "Declare a typed event payload schema without registering runtime behavior.",
        "emit" => "Emit a named signal and pass any payload values to each handler.",
        "off" => {
            "Remove one handler from a signal, or clear the signal when no handler is provided."
        }
        "clear" => "Clear one named signal, or every signal when no name is provided.",
        "has" => "Return true when a named signal has at least one handler.",
        "listeners" => "Return a copy of handlers currently registered for a signal.",
        "register" => "Alias for on, kept for Hump-style signal familiarity.",
        "remove" => "Alias for off, kept for Hump-style signal familiarity.",
        "onPattern" => "Register a handler for existing signal names that match a Lua pattern.",
        "registerPattern" => "Alias for onPattern.",
        "emitPattern" => "Emit every existing signal whose name matches a Lua pattern.",
        "offPattern" => "Remove a handler from every signal whose name matches a Lua pattern.",
        "removePattern" => "Alias for offPattern.",
        "clearPattern" => "Clear every signal whose name matches a Lua pattern.",
        _ => "SignalBus method.",
    }
}

fn method_function_type(method: &str) -> FunctionType {
    let function = typed_method(method).expect("method name should be known");

    FunctionType::new_returns_with_required(
        function
            .parameters
            .iter()
            .map(|parameter| parameter.ty.clone()),
        function.required_parameter_count,
        function.return_types_or_any(),
    )
}

fn typed_parameter(name: &str, ty: Type) -> TypedParameter {
    TypedParameter {
        name: name.to_owned(),
        ty,
        documentation: None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SignalEventMention {
    pub name: String,
    pub receiver_parts: Vec<String>,
    pub root_definition: Option<SourceSpan>,
    pub method: String,
    pub position: SourcePosition,
    pub span: SourceSpan,
    pub name_span: SourceSpan,
    pub call_span: SourceSpan,
    pub source_order: usize,
    pub payload: Option<Vec<TypedParameter>>,
    pub inferred_emit_payload: Option<Vec<TypedParameter>>,
    pub confirmed_bus: bool,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct SignalBusIdentity {
    pub origin: PathBuf,
    pub root_definition: Option<(usize, usize)>,
    pub receiver_parts: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct IndexedSignalEvent {
    pub path: PathBuf,
    pub mention: SignalEventMention,
    pub identity: SignalBusIdentity,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SignalEventKey {
    identity: SignalBusIdentity,
    name: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SignalEventIndex {
    events: Vec<IndexedSignalEvent>,
    groups: BTreeMap<SignalEventKey, Vec<usize>>,
    name_usage: BTreeMap<String, usize>,
}

impl SignalEventIndex {
    pub fn new(events: Vec<IndexedSignalEvent>) -> Self {
        let mut index = Self {
            events,
            ..Self::default()
        };

        for (event_index, event) in index.events.iter().enumerate() {
            *index
                .name_usage
                .entry(event.mention.name.clone())
                .or_default() += 1;
            index
                .groups
                .entry(SignalEventKey {
                    identity: event.identity.clone(),
                    name: event.mention.name.clone(),
                })
                .or_default()
                .push(event_index);
        }

        index
    }

    pub fn events(&self) -> &[IndexedSignalEvent] {
        &self.events
    }

    pub fn usage_count(&self, name: &str) -> usize {
        self.name_usage.get(name).copied().unwrap_or_default()
    }

    pub fn event_at(&self, path: &Path, position: SourcePosition) -> Option<&IndexedSignalEvent> {
        self.events.iter().find(|event| {
            event.path == path && span_contains_position(event.mention.span, position)
        })
    }

    pub fn event_for_receiver(
        &self,
        path: &Path,
        receiver_parts: &[String],
        name: &str,
        position: SourcePosition,
    ) -> Option<&IndexedSignalEvent> {
        self.events
            .iter()
            .filter(|event| {
                event.path == path
                    && event.mention.name == name
                    && event.mention.receiver_parts == receiver_parts
            })
            .min_by_key(|event| {
                (
                    line_distance(event.mention.position, position),
                    event.mention.source_order,
                )
            })
    }

    pub fn same_bus(
        &self,
        path: &Path,
        receiver_parts: &[String],
        candidate: &IndexedSignalEvent,
        position: SourcePosition,
    ) -> bool {
        self.events
            .iter()
            .filter(|event| event.path == path && event.mention.receiver_parts == receiver_parts)
            .min_by_key(|event| {
                (
                    line_distance(event.mention.position, position),
                    event.mention.source_order,
                )
            })
            .is_some_and(|event| event.identity == candidate.identity)
    }

    pub fn related_events(&self, event: &IndexedSignalEvent) -> Vec<&IndexedSignalEvent> {
        self.group_events(&SignalEventKey {
            identity: event.identity.clone(),
            name: event.mention.name.clone(),
        })
    }

    pub fn definition(&self, event: &IndexedSignalEvent) -> Option<&IndexedSignalEvent> {
        self.related_events(event)
            .into_iter()
            .min_by_key(|candidate| {
                (
                    definition_rank(&candidate.mention),
                    candidate.path.clone(),
                    candidate.mention.position.line,
                    candidate.mention.position.column,
                    candidate.mention.source_order,
                )
            })
    }

    pub fn schema_for_event(&self, event: &IndexedSignalEvent) -> Option<SignalEventSchema> {
        self.schema_for_key(&SignalEventKey {
            identity: event.identity.clone(),
            name: event.mention.name.clone(),
        })
    }

    pub fn schema_for_receiver(
        &self,
        path: &Path,
        receiver_parts: &[String],
        name: &str,
        position: SourcePosition,
    ) -> Option<SignalEventSchema> {
        let event = self.event_for_receiver(path, receiver_parts, name, position)?;

        self.schema_for_event(event)
    }

    pub fn diagnostics_for_path(&self, path: &Path) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for key in self.conflicting_keys() {
            let events = self.group_events(key);
            let schemas = distinct_payload_labels(&events);
            let message = format!(
                "conflicting payload schemas for SignalBus event `{}` on `{}`: {}",
                key.name,
                bus_label(&key.identity),
                schemas.join(" vs ")
            );

            diagnostics.extend(
                events
                    .into_iter()
                    .filter(|event| event.path == path && event.mention.payload.is_some())
                    .map(|event| {
                        Diagnostic::error(
                            event.path.clone(),
                            event.mention.name_span.start.line,
                            event.mention.name_span.start.column,
                            message.clone(),
                        )
                        .with_code(DiagnosticCode::DOMAIN_SIGNAL_SCHEMA)
                    }),
            );
        }

        diagnostics
    }

    pub fn grouped_events(&self) -> Vec<Vec<&IndexedSignalEvent>> {
        self.groups
            .keys()
            .map(|key| self.group_events(key))
            .collect()
    }

    fn schema_for_key(&self, key: &SignalEventKey) -> Option<SignalEventSchema> {
        let mut schemas = self
            .group_events(key)
            .into_iter()
            .filter(|event| event.mention.payload.is_some())
            .collect::<Vec<_>>();

        schemas.sort_by_key(|event| {
            (
                definition_rank(&event.mention),
                event.path.clone(),
                event.mention.position.line,
                event.mention.position.column,
                event.mention.source_order,
            )
        });

        let selected = schemas.first()?;
        let selected_payload = selected.mention.payload.clone()?;

        if schemas.iter().any(|event| {
            event
                .mention
                .payload
                .as_ref()
                .is_some_and(|payload| !payload_types_match(payload, &selected_payload))
        }) {
            return None;
        }

        Some(SignalEventSchema {
            name: key.name.clone(),
            receiver_parts: selected.mention.receiver_parts.clone(),
            payload: selected_payload,
        })
    }

    fn conflicting_keys(&self) -> impl Iterator<Item = &SignalEventKey> {
        self.groups.keys().filter(|key| {
            let payloads = self
                .group_events(key)
                .into_iter()
                .filter_map(|event| event.mention.payload.as_deref())
                .collect::<Vec<_>>();
            let Some(first) = payloads.first() else {
                return false;
            };

            payloads
                .iter()
                .skip(1)
                .any(|payload| !payload_types_match(first, payload))
        })
    }

    fn group_events(&self, key: &SignalEventKey) -> Vec<&IndexedSignalEvent> {
        self.groups
            .get(key)
            .into_iter()
            .flatten()
            .filter_map(|index| self.events.get(*index))
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SignalGraphCallRole {
    Declaration,
    Listener,
    Emit,
    Cleanup,
    Query,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignalGraphParameter {
    pub name: String,
    pub type_label: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignalGraphCall {
    pub path: PathBuf,
    pub event: String,
    pub receiver: String,
    pub method: String,
    pub role: SignalGraphCallRole,
    pub span: SourceSpan,
    pub call_span: SourceSpan,
    pub insertion_position: SourcePosition,
    pub indentation: String,
    pub payload: Option<Vec<SignalGraphParameter>>,
    pub inferred_emit_payload: Option<Vec<SignalGraphParameter>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignalGraphEvent {
    pub id: String,
    pub name: String,
    pub bus: String,
    pub bus_path: PathBuf,
    pub resolved_payload: Option<Vec<SignalGraphParameter>>,
    pub schema_conflict: bool,
    pub calls: Vec<SignalGraphCall>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SignalGraph {
    pub events: Vec<SignalGraphEvent>,
}

pub(crate) fn graph_call_from_mention(
    path: PathBuf,
    mention: SignalEventMention,
    source: Option<&str>,
) -> SignalGraphCall {
    let insertion_position = SourcePosition {
        line: mention.call_span.start.line,
        column: 1,
    };
    let indentation = source
        .and_then(|source| {
            source
                .lines()
                .nth(insertion_position.line.saturating_sub(1))
        })
        .map(|line| {
            line.chars()
                .take_while(|character| character.is_whitespace())
                .collect()
        })
        .unwrap_or_default();

    SignalGraphCall {
        path,
        event: mention.name,
        receiver: mention.receiver_parts.join("."),
        method: mention.method.clone(),
        role: graph_call_role(&mention.method),
        span: mention.span,
        call_span: mention.call_span,
        insertion_position,
        indentation,
        payload: mention.payload.map(graph_parameters),
        inferred_emit_payload: mention.inferred_emit_payload.map(graph_parameters),
    }
}

pub(crate) fn graph_schema_summary(
    events: &[&IndexedSignalEvent],
) -> (Option<Vec<SignalGraphParameter>>, bool) {
    let mut explicit = events
        .iter()
        .filter(|event| event.mention.payload.is_some())
        .copied()
        .collect::<Vec<_>>();
    explicit.sort_by_key(|event| {
        (
            definition_rank(&event.mention),
            event.path.clone(),
            event.mention.position.line,
            event.mention.position.column,
            event.mention.source_order,
        )
    });

    if let Some(selected) = explicit.first() {
        let selected_payload = selected
            .mention
            .payload
            .as_deref()
            .expect("explicit schema should have a payload");
        let conflict = explicit.iter().skip(1).any(|event| {
            event
                .mention
                .payload
                .as_deref()
                .is_some_and(|payload| !payload_types_match(payload, selected_payload))
        });

        return (
            (!conflict).then(|| graph_parameters(selected_payload.to_vec())),
            conflict,
        );
    }

    let inferred = events
        .iter()
        .filter_map(|event| event.mention.inferred_emit_payload.as_deref())
        .collect::<Vec<_>>();
    let Some(selected) = inferred.first() else {
        return (None, false);
    };
    let compatible = inferred
        .iter()
        .skip(1)
        .all(|payload| payload_types_match(selected, payload));

    (
        compatible.then(|| graph_parameters(selected.to_vec())),
        false,
    )
}

fn graph_parameters(parameters: Vec<TypedParameter>) -> Vec<SignalGraphParameter> {
    parameters
        .into_iter()
        .map(|parameter| SignalGraphParameter {
            name: parameter.name,
            type_label: parameter.ty.to_string(),
        })
        .collect()
}

fn graph_call_role(method: &str) -> SignalGraphCallRole {
    match method {
        "declare" => SignalGraphCallRole::Declaration,
        "on" | "once" | "register" => SignalGraphCallRole::Listener,
        "emit" => SignalGraphCallRole::Emit,
        "off" | "remove" | "clear" => SignalGraphCallRole::Cleanup,
        "has" | "listeners" => SignalGraphCallRole::Query,
        _ => SignalGraphCallRole::Query,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SignalEventSchema {
    pub name: String,
    pub receiver_parts: Vec<String>,
    pub payload: Vec<TypedParameter>,
}

pub(crate) fn collect_event_mentions(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    scope: Option<&ScopeMap>,
    skip_position: Option<SourcePosition>,
) -> Vec<SignalEventMention> {
    collect_event_mentions_with_mode(parsed, environment, scope, skip_position, false)
}

pub(crate) fn collect_potential_event_mentions(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    scope: Option<&ScopeMap>,
) -> Vec<SignalEventMention> {
    collect_event_mentions_with_mode(parsed, environment, scope, None, true)
}

fn collect_event_mentions_with_mode(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    scope: Option<&ScopeMap>,
    skip_position: Option<SourcePosition>,
    include_unconfirmed: bool,
) -> Vec<SignalEventMention> {
    let mut visitor = SignalEventVisitor {
        environment,
        semantic: SemanticModel::new(parsed, environment),
        scope,
        skip_position,
        include_unconfirmed,
        mentions: Vec::new(),
    };

    visitor.visit_ast(&parsed.ast);
    visitor.mentions
}

pub(crate) fn event_schema_for_receiver(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    receiver_parts: &[String],
    name: &str,
    position: SourcePosition,
) -> Option<SignalEventSchema> {
    let scope = crate::scope_map_parsed_source(parsed);
    let index = local_event_index(parsed, environment, &scope);

    index.schema_for_receiver(&parsed.path, receiver_parts, name, position)
}

pub(crate) fn event_schema_diagnostics(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
) -> Vec<Diagnostic> {
    let scope = crate::scope_map_parsed_source(parsed);
    let index = local_event_index(parsed, environment, &scope);

    index.diagnostics_for_path(&parsed.path)
}

fn payload_types_match(left: &[TypedParameter], right: &[TypedParameter]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.ty == right.ty)
}

pub(crate) fn event_emit_typed_function(name: &str, payload: &[TypedParameter]) -> TypedFunction {
    let nil = Type::nil();
    let mut parameters = vec![
        typed_parameter("self", Type::alias(TYPE_NAME)),
        typed_parameter("", Type::string_literal(name)),
    ];

    parameters.extend(payload.iter().cloned());

    TypedFunction {
        name: Some("emit".to_owned()),
        parameter_count: parameters.len(),
        required_parameter_count: parameters.len(),
        parameters,
        has_varargs: false,
        vararg_type: None,
        return_type: Some(nil.clone()),
        return_types: vec![nil],
        documentation: Some(format!(
            "Emit the `{name}` SignalBus event with its inferred payload schema."
        )),
        overloads: Vec::new(),
    }
}

struct SignalEventVisitor<'a> {
    environment: &'a TypeEnvironment,
    semantic: SemanticModel<'a>,
    scope: Option<&'a ScopeMap>,
    skip_position: Option<SourcePosition>,
    include_unconfirmed: bool,
    mentions: Vec<SignalEventMention>,
}

impl Visitor for SignalEventVisitor<'_> {
    fn visit_function_call(&mut self, function_call: &FunctionCall) {
        let Some(callee_parts) = field_chain_before_first_call(function_call) else {
            return;
        };
        let Some(method) = callee_parts.last() else {
            return;
        };

        if callee_parts.len() < 2 || !is_event_name_method(method) {
            return;
        }

        let receiver_parts = &callee_parts[..callee_parts.len() - 1];
        let position = node_start(function_call);
        let confirmed_bus = self
            .semantic
            .type_of_field_chain(receiver_parts, position)
            .is_some_and(|receiver_type| type_is_signal_bus(self.environment, &receiver_type));

        if !confirmed_bus && !self.include_unconfirmed {
            return;
        }

        let Some((name, span, name_span)) = first_string_argument(function_call) else {
            return;
        };

        if (name.is_empty() && !self.include_unconfirmed)
            || self
                .skip_position
                .is_some_and(|position| span_contains_position(span, position))
        {
            return;
        }

        let payload = if is_event_schema_method(method) {
            schema_payload_parameters(function_call)
        } else {
            None
        };
        let inferred_emit_payload = (method == "emit")
            .then(|| emit_payload_parameters(function_call, &self.semantic, position))
            .flatten();
        let call_span = node_span(function_call).unwrap_or(span);

        self.mentions.push(SignalEventMention {
            name,
            receiver_parts: receiver_parts.to_vec(),
            root_definition: root_definition_span(function_call, self.scope),
            method: method.clone(),
            position: span.start,
            span,
            name_span,
            call_span,
            source_order: self.mentions.len(),
            payload,
            inferred_emit_payload,
            confirmed_bus,
        });
    }
}

fn emit_payload_parameters(
    function_call: &FunctionCall,
    semantic: &SemanticModel<'_>,
    position: SourcePosition,
) -> Option<Vec<TypedParameter>> {
    let arguments = call_argument_expressions(function_call)?;
    let mut names = BTreeSet::new();

    arguments
        .into_iter()
        .skip(1)
        .enumerate()
        .map(|(index, expression)| {
            let preferred = plain_field_chain_from_expression(expression)
                .and_then(|parts| parts.last().cloned())
                .unwrap_or_else(|| format!("value{}", index + 1));
            let name = unique_parameter_name(preferred, index, &mut names);
            let ty = semantic
                .type_of_expression_return(expression, position, 0)
                .map(|ty| widen_inferred_type(&ty))
                .unwrap_or(Type::Any);

            Some(TypedParameter {
                name,
                ty,
                documentation: None,
            })
        })
        .collect()
}

fn unique_parameter_name(preferred: String, index: usize, names: &mut BTreeSet<String>) -> String {
    if names.insert(preferred.clone()) {
        return preferred;
    }

    let fallback = format!("{preferred}{}", index + 1);
    names.insert(fallback.clone());
    fallback
}

fn call_argument_expressions(function_call: &FunctionCall) -> Option<Vec<&Expression>> {
    let call = function_call.suffixes().find_map(|suffix| match suffix {
        Suffix::Call(call) => Some(call),
        _ => None,
    })?;

    match call {
        Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }) => {
            Some(arguments.iter().collect())
        }
        Call::MethodCall(method) => match method.args() {
            FunctionArgs::Parentheses { arguments, .. } => Some(arguments.iter().collect()),
            FunctionArgs::String(_) | FunctionArgs::TableConstructor(_) => Some(Vec::new()),
            _ => None,
        },
        Call::AnonymousCall(FunctionArgs::String(_))
        | Call::AnonymousCall(FunctionArgs::TableConstructor(_)) => Some(Vec::new()),
        _ => None,
    }
}

fn schema_payload_parameters(function_call: &FunctionCall) -> Option<Vec<TypedParameter>> {
    let Expression::Function(function) = argument_expression(function_call, 1)? else {
        return None;
    };

    function_body_payload_parameters(function.body())
}

fn function_body_payload_parameters(body: &ast::FunctionBody) -> Option<Vec<TypedParameter>> {
    let parameters = body
        .parameters()
        .iter()
        .map(|parameter| match parameter {
            ast::Parameter::Name(name) => Some((
                token_text(name),
                name.start_position()?,
                name.end_position()?,
            )),
            ast::Parameter::Ellipsis(_) => None,
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;

    if parameters.is_empty() {
        return Some(Vec::new());
    }

    let specifiers = body
        .type_specifiers()
        .filter_map(|specifier| {
            let specifier = specifier?;
            Some((
                specifier.start_position()?,
                type_from_type_info(specifier.type_info())?,
            ))
        })
        .collect::<Vec<_>>();

    parameters
        .iter()
        .enumerate()
        .map(|(index, (name, _, parameter_end))| {
            let next_parameter_start = parameters.get(index + 1).map(|(_, start, _)| *start);
            let ty = specifiers.iter().find_map(|(specifier_start, ty)| {
                if specifier_start >= parameter_end
                    && next_parameter_start
                        .map(|next_start| specifier_start < &next_start)
                        .unwrap_or(true)
                {
                    Some(ty.clone())
                } else {
                    None
                }
            })?;

            Some(TypedParameter {
                name: name.clone(),
                ty,
                documentation: None,
            })
        })
        .collect()
}

fn argument_expression(function_call: &FunctionCall, index: usize) -> Option<&Expression> {
    let call = function_call.suffixes().find_map(|suffix| match suffix {
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

fn first_string_argument(function_call: &FunctionCall) -> Option<(String, SourceSpan, SourceSpan)> {
    let call = function_call.suffixes().find_map(|suffix| match suffix {
        Suffix::Call(call) => Some(call),
        _ => None,
    })?;

    match call {
        Call::AnonymousCall(FunctionArgs::String(value)) => string_argument(value),
        Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }) => {
            let Expression::String(value) = arguments.iter().next()? else {
                return None;
            };

            string_argument(value)
        }
        Call::MethodCall(method) => match method.args() {
            FunctionArgs::String(value) => string_argument(value),
            FunctionArgs::Parentheses { arguments, .. } => {
                let Expression::String(value) = arguments.iter().next()? else {
                    return None;
                };

                string_argument(value)
            }
            _ => None,
        },
        _ => None,
    }
}

fn string_argument(value: &TokenReference) -> Option<(String, SourceSpan, SourceSpan)> {
    let name = string_literal_value(value)?;
    let span = node_span(value)?;
    let name_span = string_literal_name_span(value, span);

    Some((name, span, name_span))
}

fn string_literal_name_span(value: &TokenReference, span: SourceSpan) -> SourceSpan {
    let text = token_text(value);

    if span.start.line == span.end.line
        && ((text.starts_with('"') && text.ends_with('"'))
            || (text.starts_with('\'') && text.ends_with('\'')))
        && span.end.column > span.start.column + 1
    {
        return SourceSpan {
            start: SourcePosition {
                line: span.start.line,
                column: span.start.column + 1,
            },
            end: SourcePosition {
                line: span.end.line,
                column: span.end.column - 1,
            },
        };
    }

    span
}

fn root_definition_span(
    function_call: &FunctionCall,
    scope: Option<&ScopeMap>,
) -> Option<SourceSpan> {
    let ast::Prefix::Name(name) = function_call.prefix() else {
        return None;
    };
    let position = node_start(name);
    let scope = scope?;
    let symbol = scope.symbol_at_position(position)?;

    scope
        .declaration(symbol)
        .map(|declaration| declaration.name_span)
}

pub(crate) fn local_event_index(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    scope: &ScopeMap,
) -> SignalEventIndex {
    SignalEventIndex::new(
        collect_event_mentions(parsed, environment, Some(scope), None)
            .into_iter()
            .map(|mention| IndexedSignalEvent {
                identity: SignalBusIdentity {
                    origin: parsed.path.clone(),
                    root_definition: mention
                        .root_definition
                        .map(|span| (span.start.line, span.start.column)),
                    receiver_parts: mention.receiver_parts.clone(),
                },
                path: parsed.path.clone(),
                mention,
            })
            .collect(),
    )
}

fn definition_rank(mention: &SignalEventMention) -> usize {
    match mention.method.as_str() {
        "declare" => 0,
        "on" | "once" | "register" if mention.payload.is_some() => 1,
        "on" | "once" | "register" => 2,
        _ => 3,
    }
}

fn distinct_payload_labels(events: &[&IndexedSignalEvent]) -> Vec<String> {
    let mut labels = BTreeSet::new();

    for event in events {
        let Some(payload) = event.mention.payload.as_deref() else {
            continue;
        };

        labels.insert(payload_label(payload));
    }

    labels.into_iter().collect()
}

fn payload_label(payload: &[TypedParameter]) -> String {
    format!(
        "({})",
        payload
            .iter()
            .map(|parameter| format!("{}: {}", parameter.name, parameter.ty))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

pub(crate) fn bus_label(identity: &SignalBusIdentity) -> String {
    if identity.receiver_parts.is_empty() {
        "module SignalBus".to_owned()
    } else {
        identity.receiver_parts.join(".")
    }
}

pub(crate) fn graph_event_id(identity: &SignalBusIdentity, name: &str) -> String {
    let definition = identity
        .root_definition
        .map(|(line, column)| format!("{line}:{column}"))
        .unwrap_or_else(|| "module".to_owned());

    format!(
        "{}#{definition}#{}#{name}",
        identity.origin.display(),
        identity.receiver_parts.join(".")
    )
}

pub(crate) fn is_valid_event_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|character| !character.is_control() && !matches!(character, '"' | '\'' | '\\'))
}

fn type_from_type_info(type_info: &TypeInfo) -> Option<Type> {
    match type_info {
        TypeInfo::Basic(token) => Some(Type::named(token_text(token))),
        TypeInfo::String(token) => Some(Type::string_literal(unquote_string(&token_text(token)))),
        TypeInfo::Optional { base, .. } => {
            Some(Type::union([type_from_type_info(base)?, Type::nil()]))
        }
        TypeInfo::Union(union) => union
            .types()
            .iter()
            .map(type_from_type_info)
            .collect::<Option<Vec<_>>>()
            .map(Type::union),
        _ => Some(Type::named(type_info.to_string())),
    }
}

fn string_literal_value(token: &TokenReference) -> Option<String> {
    match token.token().token_type() {
        TokenType::StringLiteral { literal, .. } => Some(literal.to_string()),
        _ => None,
    }
}

fn line_distance(left: SourcePosition, right: SourcePosition) -> usize {
    left.line.abs_diff(right.line)
}

fn span_contains_position(span: SourceSpan, position: SourcePosition) -> bool {
    position_key(span.start) <= position_key(position)
        && position_key(position) <= position_key(span.end)
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

fn node_start(node: &impl Node) -> SourcePosition {
    node.range()
        .map(|(start, _)| SourcePosition::from_full_moon(start))
        .unwrap_or(SourcePosition { line: 1, column: 1 })
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn unquote_string(value: &str) -> String {
    value
        .trim()
        .strip_prefix(['"', '\''])
        .and_then(|value| value.strip_suffix(['"', '\'']))
        .unwrap_or_else(|| value.trim())
        .to_owned()
}

fn resolve_aliases(environment: &TypeEnvironment, ty: &Type, depth: usize) -> Type {
    if depth > 16 {
        return ty.clone();
    }

    match ty {
        Type::Alias(name) => environment
            .alias(name)
            .map(|resolved| resolve_aliases(environment, resolved, depth + 1))
            .unwrap_or_else(|| ty.clone()),
        Type::Union(types) => Type::union(
            types
                .iter()
                .map(|member| resolve_aliases(environment, member, depth + 1)),
        ),
        _ => ty.clone(),
    }
}

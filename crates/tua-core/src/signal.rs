use full_moon::ast::luau::TypeInfo;
use full_moon::ast::{self, Call, Expression, FunctionArgs, FunctionCall, Suffix};
use full_moon::node::Node;
use full_moon::tokenizer::{TokenReference, TokenType};
use full_moon::visitors::Visitor;

use crate::ast_utils::{field_chain_before_first_call, token_text};
use crate::parser::{ParsedSource, SourcePosition, SourceSpan};
use crate::semantic::SemanticModel;
use crate::types::{FunctionType, TableField, Type, TypeEnvironment};
use crate::{TypedFunction, TypedParameter};

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
    pub position: SourcePosition,
    pub source_order: usize,
    pub payload: Option<Vec<TypedParameter>>,
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
    skip_position: Option<SourcePosition>,
) -> Vec<SignalEventMention> {
    let mut visitor = SignalEventVisitor {
        environment,
        semantic: SemanticModel::new(parsed, environment),
        skip_position,
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
    let mut schemas = collect_event_mentions(parsed, environment, None)
        .into_iter()
        .filter(|mention| mention.name == name && mention.payload.is_some())
        .collect::<Vec<_>>();

    schemas.sort_by(|left, right| {
        let left_same_receiver = left.receiver_parts == receiver_parts;
        let right_same_receiver = right.receiver_parts == receiver_parts;

        right_same_receiver
            .cmp(&left_same_receiver)
            .then_with(|| {
                line_distance(left.position, position).cmp(&line_distance(right.position, position))
            })
            .then_with(|| left.source_order.cmp(&right.source_order))
            .then_with(|| left.receiver_parts.cmp(&right.receiver_parts))
    });

    let selected = schemas.first()?;
    let selected_receiver = selected.receiver_parts.clone();
    let selected_payload = selected.payload.clone()?;

    if schemas
        .iter()
        .filter(|schema| schema.receiver_parts == selected_receiver)
        .any(|schema| {
            schema
                .payload
                .as_ref()
                .is_some_and(|payload| !payload_types_match(payload, &selected_payload))
        })
    {
        return None;
    }

    Some(SignalEventSchema {
        name: name.to_owned(),
        receiver_parts: selected_receiver,
        payload: selected_payload,
    })
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
    skip_position: Option<SourcePosition>,
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
        let Some(receiver_type) = self.semantic.type_of_field_chain(receiver_parts, position)
        else {
            return;
        };

        if !type_is_signal_bus(self.environment, &receiver_type) {
            return;
        }

        let Some((name, span)) = first_string_argument(function_call) else {
            return;
        };

        if name.is_empty()
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

        self.mentions.push(SignalEventMention {
            name,
            receiver_parts: receiver_parts.to_vec(),
            position: span.start,
            source_order: self.mentions.len(),
            payload,
        });
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

fn first_string_argument(function_call: &FunctionCall) -> Option<(String, SourceSpan)> {
    let call = function_call.suffixes().find_map(|suffix| match suffix {
        Suffix::Call(call) => Some(call),
        _ => None,
    })?;

    match call {
        Call::AnonymousCall(FunctionArgs::String(value)) => {
            Some((string_literal_value(value)?, node_span(value)?))
        }
        Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }) => {
            let Expression::String(value) = arguments.iter().next()? else {
                return None;
            };

            Some((string_literal_value(value)?, node_span(value)?))
        }
        Call::MethodCall(method) => match method.args() {
            FunctionArgs::String(value) => Some((string_literal_value(value)?, node_span(value)?)),
            FunctionArgs::Parentheses { arguments, .. } => {
                let Expression::String(value) = arguments.iter().next()? else {
                    return None;
                };

                Some((string_literal_value(value)?, node_span(value)?))
            }
            _ => None,
        },
        _ => None,
    }
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

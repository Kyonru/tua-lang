use crate::semantic::resolve_aliases;
use crate::types::{
    FunctionParameter, FunctionType, TableField, Type, TypeEnvironment, TypedFunctionOverload,
};
use crate::{TypedFunction, TypedParameter};

pub(crate) const TYPE_NAME: &str = "Store";
pub(crate) const STATE_TYPE: &str = "StoreState";
pub(crate) const CONSTRUCTOR_NAME: &str = "store";
pub(crate) const RUNTIME_MODULE: &str = "tuastore";
pub(crate) const EMITTED_CONSTRUCTOR_LOCAL: &str = "__tua_store";

const METHOD_NAMES: &[&str] = &["get", "set", "update", "subscribe", "watch"];
const BUILTIN_TYPE_NAMES: &[&str] = &[TYPE_NAME, STATE_TYPE];

pub(crate) fn builtin_type_names() -> &'static [&'static str] {
    BUILTIN_TYPE_NAMES
}

pub(crate) fn builtin_aliases() -> Vec<(String, Type)> {
    vec![
        (TYPE_NAME.to_owned(), store_type(Type::Any)),
        (STATE_TYPE.to_owned(), Type::empty_table()),
    ]
}

pub(crate) fn is_constructor(name: &str) -> bool {
    name == CONSTRUCTOR_NAME
}

pub(crate) fn constructor_type() -> FunctionType {
    FunctionType::new([Type::alias(STATE_TYPE)], Type::alias(TYPE_NAME))
}

pub(crate) fn constructor_typed_function() -> TypedFunction {
    typed_function(
        CONSTRUCTOR_NAME,
        [parameter("initial", Type::alias(STATE_TYPE))],
        Type::alias(TYPE_NAME),
        documentation(),
    )
}

pub(crate) fn store_type(state_type: Type) -> Type {
    Type::table(METHOD_NAMES.iter().map(|method| {
        let function = typed_method_for_state(method, state_type.clone())
            .expect("known store method")
            .as_type()
            .expect("store method has a function type");

        (*method, TableField::required(function))
    }))
}

pub(crate) fn store_type_from_initial(initial: &Type) -> Type {
    store_type(initial.clone())
}

pub(crate) fn store_state_type(environment: &TypeEnvironment, candidate: &Type) -> Option<Type> {
    match candidate {
        Type::Alias(name) if name == TYPE_NAME => Some(Type::Any),
        Type::Alias(name) => environment
            .alias(name)
            .filter(|resolved| *resolved != candidate)
            .and_then(|resolved| store_state_type(environment, resolved)),
        Type::Union(types) => {
            let states = types
                .iter()
                .filter_map(|ty| store_state_type(environment, ty))
                .collect::<Vec<_>>();

            match states.len() {
                0 => None,
                1 => states.into_iter().next(),
                _ => Some(Type::union(states)),
            }
        }
        Type::Table(table)
            if METHOD_NAMES
                .iter()
                .all(|method| table.field(method).is_some()) =>
        {
            table.field("get").and_then(|field| match &field.ty {
                Type::Function(function) => function.return_types_or_any().first().cloned(),
                _ => None,
            })
        }
        _ => None,
    }
}

pub(crate) fn typed_member(
    environment: &TypeEnvironment,
    receiver_type: &Type,
    method: &str,
) -> Option<TypedFunction> {
    let state_type = store_state_type(environment, receiver_type)?;
    let mut function = typed_method_for_state(method, state_type.clone())?;

    if method == "watch" {
        function = typed_watch_method(environment, state_type);
    }

    Some(function)
}

fn typed_method_for_state(method: &str, state_type: Type) -> Option<TypedFunction> {
    let self_parameter = parameter("self", Type::alias(TYPE_NAME));
    let unsubscribe = Type::Function(FunctionType::new([], Type::nil()));
    let state_listener = callback_type([
        ("state", state_type.clone()),
        ("previous", state_type.clone()),
    ]);

    let mut function = match method {
        "get" => typed_function(
            method,
            [self_parameter],
            state_type,
            "Return the current state table.",
        ),
        "set" => typed_function(
            method,
            [self_parameter, parameter("next", state_type.clone())],
            state_type,
            "Replace the state table, notify subscribers, and return the new state.",
        ),
        "update" => typed_function(
            method,
            [
                self_parameter,
                parameter("mutator", callback_type([("state", state_type.clone())])),
            ],
            state_type,
            "Mutate the current state, notify subscribers, and return it.",
        ),
        "subscribe" => typed_function(
            method,
            [self_parameter, parameter("listener", state_listener)],
            unsubscribe,
            "Run a listener after every set or update and return an idempotent unsubscribe function.",
        ),
        "watch" => typed_function(
            method,
            [
                self_parameter,
                parameter("key", Type::string()),
                parameter("listener", Type::Any),
            ],
            unsubscribe,
            "Watch one top-level state field and return an idempotent unsubscribe function.",
        ),
        _ => return None,
    };

    function.name = Some(format!("{TYPE_NAME}:{method}"));
    Some(function)
}

fn typed_watch_method(environment: &TypeEnvironment, state_type: Type) -> TypedFunction {
    let resolved = resolve_aliases(environment, &state_type, 0);
    let Type::Table(table) = resolved else {
        return typed_method_for_state("watch", state_type).expect("watch method exists");
    };
    let unsubscribe = Type::Function(FunctionType::new([], Type::nil()));
    let overloads = table
        .semantic_fields()
        .into_iter()
        .map(|(name, field)| {
            let value_type = if field.optional {
                Type::union([field.ty.clone(), Type::nil()])
            } else {
                field.ty.clone()
            };

            TypedFunctionOverload {
                parameters: vec![
                    parameter("self", Type::alias(TYPE_NAME)),
                    parameter("key", Type::string_literal(name.clone())),
                    parameter(
                        "listener",
                        callback_type([
                            ("value", value_type.clone()),
                            ("previous", value_type),
                            ("state", state_type.clone()),
                        ]),
                    ),
                ],
                has_varargs: false,
                vararg_type: None,
                return_types: vec![unsubscribe.clone()],
                documentation: Some(format!("Watch changes to the `{name}` state field.")),
            }
        })
        .collect::<Vec<_>>();

    if overloads.is_empty() {
        return typed_method_for_state("watch", state_type).expect("watch method exists");
    }

    TypedFunction {
        name: Some(format!("{TYPE_NAME}:watch")),
        parameters: vec![
            parameter("self", Type::Any),
            parameter("key", Type::Any),
            parameter("listener", Type::Any),
        ],
        parameter_count: 3,
        required_parameter_count: 3,
        has_varargs: false,
        vararg_type: None,
        return_type: Some(unsubscribe.clone()),
        return_types: vec![unsubscribe],
        documentation: Some(
            "Watch one top-level state field and return an idempotent unsubscribe function."
                .to_owned(),
        ),
        overloads,
    }
}

fn callback_type(parameters: impl IntoIterator<Item = (&'static str, Type)>) -> Type {
    let parameters = parameters
        .into_iter()
        .map(|(name, ty)| FunctionParameter::named(name, ty))
        .collect::<Vec<_>>();

    Type::Function(FunctionType::new_signature(
        parameters.clone(),
        parameters.len(),
        None,
        [Type::nil()],
    ))
}

pub(crate) const fn detail() -> &'static str {
    "function store(initial: StoreState): Store"
}

pub(crate) const fn documentation() -> &'static str {
    "Create an explicit typed runtime-state store with whole-store and keyed subscriptions."
}

pub(crate) const fn workflow_snippet() -> &'static str {
    "local ${1:game} = store({\n  ${2:score} = ${3:0},\n  ${4:paused} = false,\n})\n\n${1:game}:update(function(state)\n  state.${2:score} += ${5:1}\nend)\n${0}"
}

pub(crate) fn runtime_source() -> &'static str {
    include_str!("../../../vendor/runtime/tuastore.lua")
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

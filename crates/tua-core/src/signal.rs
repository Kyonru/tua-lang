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
    "emit",
    "off",
    "remove",
    "clear",
    "has",
    "listeners",
];

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
        "on" | "once" | "register" | "onPattern" | "registerPattern" => {
            let name_parameter = if matches!(method, "onPattern" | "registerPattern") {
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
                required_parameter_count: 3,
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

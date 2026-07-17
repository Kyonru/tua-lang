use crate::types::{FunctionType, TableField, Type, TypeEnvironment};
use crate::{TypedFunction, TypedParameter};

pub(crate) const TYPE_NAME: &str = "ObjectPool";
pub(crate) const OPTIONS_TYPE: &str = "ObjectPoolOptions";
pub(crate) const CONSTRUCTOR_NAME: &str = "objectPool";
pub(crate) const RUNTIME_MODULE: &str = "tuaobjectpool";
pub(crate) const EMITTED_CONSTRUCTOR_LOCAL: &str = "__tua_object_pool";

const METHOD_NAMES: &[&str] = &[
    "acquire",
    "release",
    "prewarm",
    "clear",
    "owns",
    "activeCount",
    "availableCount",
    "capacity",
];

const BUILTIN_TYPE_NAMES: &[&str] = &[TYPE_NAME, OPTIONS_TYPE];

pub(crate) fn builtin_type_names() -> &'static [&'static str] {
    BUILTIN_TYPE_NAMES
}

pub(crate) fn builtin_aliases() -> Vec<(String, Type)> {
    vec![
        (TYPE_NAME.to_owned(), pool_type(Type::Any)),
        (OPTIONS_TYPE.to_owned(), options_type()),
    ]
}

pub(crate) fn options_type() -> Type {
    Type::table([
        (
            "create",
            TableField::required(Type::Function(FunctionType::new([], Type::Any))),
        ),
        ("reset", TableField::optional(Type::Any)),
        ("dispose", TableField::optional(Type::Any)),
        ("initial", TableField::optional(Type::number())),
    ])
}

pub(crate) fn pool_type(item_type: Type) -> Type {
    Type::table(METHOD_NAMES.iter().map(|method| {
        (
            *method,
            TableField::required(
                typed_method_for_item(method, item_type.clone())
                    .expect("known object pool method")
                    .as_type()
                    .expect("object pool method has a function type"),
            ),
        )
    }))
}

pub(crate) fn pool_type_from_options(options: &Type) -> Type {
    let item_type = match options {
        Type::Table(table) => table
            .field("create")
            .and_then(|field| match &field.ty {
                Type::Function(function) => function.return_types_or_any().first().cloned(),
                _ => None,
            })
            .unwrap_or(Type::Any),
        _ => Type::Any,
    };

    pool_type(item_type)
}

pub(crate) fn pool_item_type(environment: &TypeEnvironment, candidate: &Type) -> Option<Type> {
    match candidate {
        Type::Alias(name) if name == TYPE_NAME => Some(Type::Any),
        Type::Alias(name) => environment
            .alias(name)
            .filter(|resolved| *resolved != candidate)
            .and_then(|resolved| pool_item_type(environment, resolved)),
        Type::Union(types) => {
            let items = types
                .iter()
                .filter_map(|ty| pool_item_type(environment, ty))
                .collect::<Vec<_>>();

            match items.len() {
                0 => None,
                1 => items.into_iter().next(),
                _ => Some(Type::union(items)),
            }
        }
        Type::Table(table)
            if METHOD_NAMES
                .iter()
                .all(|method| table.field(method).is_some()) =>
        {
            table.field("acquire").and_then(|field| match &field.ty {
                Type::Function(function) => function.return_types_or_any().first().cloned(),
                _ => None,
            })
        }
        _ => None,
    }
}

pub(crate) fn is_constructor(name: &str) -> bool {
    name == CONSTRUCTOR_NAME
}

pub(crate) fn constructor_type() -> FunctionType {
    FunctionType::new([Type::alias(OPTIONS_TYPE)], Type::alias(TYPE_NAME))
}

pub(crate) fn constructor_typed_function() -> TypedFunction {
    typed_function(
        CONSTRUCTOR_NAME,
        [parameter("options", Type::alias(OPTIONS_TYPE))],
        Type::alias(TYPE_NAME),
        documentation(),
    )
}

pub(crate) fn typed_member(
    environment: &TypeEnvironment,
    receiver_type: &Type,
    method: &str,
) -> Option<TypedFunction> {
    let item_type = pool_item_type(environment, receiver_type)?;

    typed_method_for_item(method, item_type)
}

fn typed_method_for_item(method: &str, item_type: Type) -> Option<TypedFunction> {
    let self_parameter = parameter("self", Type::alias(TYPE_NAME));

    let mut function = match method {
        "acquire" => Some(typed_function(
            method,
            [self_parameter],
            item_type,
            "Take an available object or create one when the pool is empty.",
        )),
        "release" => Some(typed_function(
            method,
            [self_parameter, parameter("object", item_type)],
            Type::boolean(),
            "Reset and return an active object. Returns false for foreign or duplicate releases.",
        )),
        "prewarm" => Some(typed_function(
            method,
            [self_parameter, parameter("count", Type::number())],
            Type::number(),
            "Ensure at least count objects are available and return the number created.",
        )),
        "clear" => Some(typed_function(
            method,
            [self_parameter],
            Type::nil(),
            "Dispose every owned object and reset the pool to empty.",
        )),
        "owns" => Some(typed_function(
            method,
            [self_parameter, parameter("object", item_type)],
            Type::boolean(),
            "Return whether this pool created the object.",
        )),
        "activeCount" => Some(typed_function(
            method,
            [self_parameter],
            Type::number(),
            "Return the number of objects currently checked out.",
        )),
        "availableCount" => Some(typed_function(
            method,
            [self_parameter],
            Type::number(),
            "Return the number of objects ready for immediate reuse.",
        )),
        "capacity" => Some(typed_function(
            method,
            [self_parameter],
            Type::number(),
            "Return the total number of objects owned by the pool.",
        )),
        _ => None,
    }?;

    function.name = Some(format!("{TYPE_NAME}:{method}"));
    Some(function)
}

pub(crate) const fn detail() -> &'static str {
    "function objectPool(options: ObjectPoolOptions): ObjectPool"
}

pub(crate) const fn documentation() -> &'static str {
    "Create a reusable object pool for allocation-heavy gameplay objects such as bullets, particles, and temporary effects."
}

pub(crate) const fn workflow_snippet() -> &'static str {
    "type ${1:Bullet} = { x: number, y: number, active: boolean }\n\nlocal ${2:bulletPool} = objectPool({\n  create = function(): ${1:Bullet}\n    return { x = ${3:0}, y = ${4:0}, active = false }\n  end,\n  reset = function(object: ${1:Bullet})\n    object.active = false\n  end,\n  initial = ${5:64},\n})\n\nlocal ${6:bullet} = ${2:bulletPool}:acquire()\n${0}"
}

pub(crate) fn runtime_source() -> &'static str {
    include_str!("../../../vendor/runtime/tuaobjectpool.lua")
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

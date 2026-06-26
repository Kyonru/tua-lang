use crate::types::{FunctionType, TableField, Type};

pub(crate) const TYPE_NAME: &str = "Vec2";
pub(crate) const CONSTRUCTOR_NAME: &str = "vec2";

const BUILTIN_TYPE_NAMES: &[&str] = &[TYPE_NAME];

pub(crate) fn builtin_type_names() -> &'static [&'static str] {
    BUILTIN_TYPE_NAMES
}

pub(crate) fn vec2_type() -> Type {
    Type::table([
        ("x", TableField::required(Type::number())),
        ("y", TableField::required(Type::number())),
    ])
}

pub(crate) fn constructor_type() -> FunctionType {
    FunctionType::new([Type::number(), Type::number()], Type::alias(TYPE_NAME))
}

pub(crate) fn is_constructor(name: &str) -> bool {
    name == CONSTRUCTOR_NAME
}

pub(crate) const fn detail() -> &'static str {
    "function vec2(x: number, y: number): Vec2"
}

pub(crate) const fn documentation() -> &'static str {
    "Creates a LOVE-friendly 2D vector. Emits a plain Lua table: { x = x, y = y }."
}

use crate::types::{FunctionType, IndexFallback, MetatableFacts, TableField, TableShape, Type};

pub(crate) const TYPE_NAME: &str = "Vec2";
pub(crate) const CONSTRUCTOR_NAME: &str = "vec2";
pub(crate) const RUNTIME_MODULE: &str = "brinevector";
pub(crate) const EMITTED_CONSTRUCTOR_LOCAL: &str = "__tua_vec2";

const BUILTIN_TYPE_NAMES: &[&str] = &[TYPE_NAME];

pub(crate) fn builtin_type_names() -> &'static [&'static str] {
    BUILTIN_TYPE_NAMES
}

pub(crate) fn vec2_type() -> Type {
    let mut shape = TableShape::from_fields([
        ("x", TableField::required(Type::number())),
        ("y", TableField::required(Type::number())),
    ]);
    shape.set_metatable_facts(brinevector_metatable_facts());

    Type::Table(shape)
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
    "Creates a LOVE-friendly 2D vector backed by BrineVector in emitted Lua."
}

pub(crate) fn runtime_source() -> &'static str {
    include_str!("../../../vendor/runtime/brinevector.lua")
}

fn brinevector_metatable_facts() -> MetatableFacts {
    let vec2 = Type::alias(TYPE_NAME);
    let number = Type::number();
    let boolean = Type::boolean();
    let string = Type::string();
    let mut facts = MetatableFacts {
        index: IndexFallback::Table(brinevector_index_shape()),
        add: Some(FunctionType::new(
            [vec2.clone(), vec2.clone()],
            vec2.clone(),
        )),
        sub: Some(FunctionType::new(
            [vec2.clone(), vec2.clone()],
            vec2.clone(),
        )),
        mul: Some(FunctionType::new([vec2.clone(), Type::Any], vec2.clone())),
        div: Some(FunctionType::new([vec2.clone(), Type::Any], vec2.clone())),
        eq: Some(FunctionType::new(
            [vec2.clone(), vec2.clone()],
            boolean.clone(),
        )),
        len: Some(FunctionType::new([vec2.clone()], number.clone())),
        tostring: Some(FunctionType::new([vec2], string)),
        ..MetatableFacts::default()
    };

    facts.fields = brinevector_index_shape();
    facts
}

fn brinevector_index_shape() -> TableShape {
    let vec2 = Type::alias(TYPE_NAME);
    let number = Type::number();

    TableShape::from_fields([
        ("length", TableField::required(number.clone())),
        ("length2", TableField::required(number.clone())),
        ("angle", TableField::required(number.clone())),
        ("normalized", TableField::required(vec2.clone())),
        ("copy", TableField::required(vec2.clone())),
        ("inverse", TableField::required(vec2.clone())),
        ("floor", TableField::required(vec2.clone())),
        ("ceil", TableField::required(vec2.clone())),
        (
            "getLength",
            TableField::required(Type::function([vec2.clone()], number.clone())),
        ),
        (
            "getLengthSquared",
            TableField::required(Type::function([vec2.clone()], number.clone())),
        ),
        (
            "getNormalized",
            TableField::required(Type::function([vec2.clone()], vec2.clone())),
        ),
        (
            "getAngle",
            TableField::required(Type::function([vec2.clone()], number.clone())),
        ),
        (
            "getCopy",
            TableField::required(Type::function([vec2.clone()], vec2.clone())),
        ),
        (
            "getInverse",
            TableField::required(Type::function([vec2.clone()], vec2.clone())),
        ),
        (
            "getFloor",
            TableField::required(Type::function([vec2.clone()], vec2.clone())),
        ),
        (
            "getCeil",
            TableField::required(Type::function([vec2.clone()], vec2.clone())),
        ),
        (
            "angled",
            TableField::required(Type::function([vec2.clone(), number.clone()], vec2.clone())),
        ),
        (
            "trim",
            TableField::required(Type::function([vec2.clone(), number.clone()], vec2.clone())),
        ),
        (
            "split",
            TableField::required(Type::function_returns(
                [vec2.clone()],
                [number.clone(), number.clone()],
            )),
        ),
        (
            "clamp",
            TableField::required(Type::function(
                [vec2.clone(), vec2.clone(), vec2.clone()],
                vec2.clone(),
            )),
        ),
        (
            "hadamard",
            TableField::required(Type::function([vec2.clone(), vec2.clone()], vec2.clone())),
        ),
        (
            "rotated",
            TableField::required(Type::function([vec2.clone(), number.clone()], vec2.clone())),
        ),
        (
            "dot",
            TableField::required(Type::function([vec2.clone(), vec2], number)),
        ),
    ])
}

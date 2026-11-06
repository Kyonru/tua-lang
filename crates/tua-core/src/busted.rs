//! Test-file-scoped declarations for the Busted and LuaAssert globals.
//!
//! These types are attached to analysis environments only for configured test
//! sources. Keeping them here lets checking and editor queries share contracts
//! without rewriting the user's source or leaking the names into production.

use crate::lua::{LuaItem, LuaItemKind};
use crate::types::{FunctionParameter, FunctionType, MetatableFacts, TableField, TableShape};
use crate::Type;
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

pub(crate) const GLOBAL_NAMES: &[&str] = &[
    "after_each",
    "assert",
    "before_each",
    "context",
    "describe",
    "expose",
    "finally",
    "insulate",
    "it",
    "lazy_setup",
    "lazy_teardown",
    "match",
    "mock",
    "pending",
    "setup",
    "spec",
    "spy",
    "strict_setup",
    "strict_teardown",
    "stub",
    "teardown",
    "test",
];

fn items() -> Vec<LuaItem> {
    vec![
    item("after_each", "function after_each(callback: function): nil", "Run a callback after each test in the current Busted scope."),
    item("assert", "table", "LuaAssert assertion functions. Common forms include assert.is_true(value) and assert.are.equal(expected, actual)."),
    item("before_each", "function before_each(callback: function): nil", "Run a callback before each test in the current Busted scope."),
    item("context", "function context(name: string, callback: function): nil", "Create a Busted test context."),
    item("describe", "function describe(name: string, callback: function): nil", "Create a Busted test suite."),
    item("expose", "function expose(name: string): any", "Expose a value from an insulated Busted context."),
    item("finally", "function finally(callback: function): nil", "Register cleanup for an insulated Busted context."),
    item("insulate", "function insulate(name: string, callback: function): nil", "Run a Busted context with isolated state."),
    item("it", "function it(name: string, callback: function): nil", "Register a Busted test case."),
    item("lazy_setup", "function lazy_setup(callback: function): nil", "Register setup that runs lazily for a Busted scope."),
    item("lazy_teardown", "function lazy_teardown(callback: function): nil", "Register teardown that runs lazily for a Busted scope."),
    item("match", "table", "LuaAssert matchers used with assertions such as assert.is_true."),
    item("mock", "table", "Busted mock helpers."),
    item("pending", "function pending(name: string, callback?: function): nil", "Register a pending Busted test."),
    item("setup", "function setup(callback: function): nil", "Register setup for a Busted scope."),
    item("spec", "function spec(name: string, callback: function): nil", "Register a Busted test case."),
    item("spy", "table", "Busted spy helpers."),
    item("strict_setup", "function strict_setup(callback: function): nil", "Register strict setup for a Busted scope."),
    item("strict_teardown", "function strict_teardown(callback: function): nil", "Register strict teardown for a Busted scope."),
    item("stub", "table", "Busted stub helpers."),
    item("teardown", "function teardown(callback: function): nil", "Register teardown for a Busted scope."),
    item("test", "function test(name: string, callback: function): nil", "Register a Busted test case."),
    item("assert.is_true", "function assert.is_true(value: any, message?: string): nil", "Assert that a value is true."),
    item("assert.is_false", "function assert.is_false(value: any, message?: string): nil", "Assert that a value is false."),
    item("assert.is_nil", "function assert.is_nil(value: any, message?: string): nil", "Assert that a value is nil."),
    item("assert.is_not_nil", "function assert.is_not_nil(value: any, message?: string): nil", "Assert that a value is not nil."),
    item("assert.are.equal", "function assert.are.equal(expected: any, actual: any, message?: string): nil", "Assert that two values are equal."),
    item("assert.are.same", "function assert.are.same(expected: any, actual: any, message?: string): nil", "Assert that two values are deeply equal."),
    item("assert.are_not.equal", "function assert.are_not.equal(expected: any, actual: any, message?: string): nil", "Assert that two values are not equal."),
    item("assert.has_error", "function assert.has_error(callback: function, message?: string): nil", "Assert that a callback raises an error."),
    item("spy.new", "function spy.new(callback?: function): table", "Create a Busted spy."),
    item("spy.on", "function spy.on(object: any, method: string): table", "Spy on a method."),
    item("stub.new", "function stub.new(object: any, method?: string, replacement?: any): table", "Create a Busted stub."),
    item("mock.new", "function mock.new(module: any): table", "Create a Busted mock."),
    item("match.is_type", "function match.is_type(value: any, type_name: string): function", "Create a LuaAssert type matcher."),
    ]
}

pub(crate) fn top_level_items(prefix: &str) -> impl Iterator<Item = LuaItem> {
    let prefix = prefix.to_owned();
    items()
        .into_iter()
        .filter(move |item| !item.label.contains('.') && item.label.starts_with(&prefix))
}

pub(crate) fn all_items() -> impl Iterator<Item = LuaItem> + 'static {
    items().into_iter()
}

pub(crate) fn hover_item(parts: &[String]) -> Option<LuaItem> {
    let name = parts.join(".");
    items().into_iter().find(|item| item.label == name)
}

pub(crate) fn global_types() -> Arc<BTreeMap<String, Type>> {
    static GLOBAL_TYPES: OnceLock<Arc<BTreeMap<String, Type>>> = OnceLock::new();
    GLOBAL_TYPES
        .get_or_init(|| {
            Arc::new(
                GLOBAL_NAMES
                    .iter()
                    .filter_map(|name| global_type(name).map(|ty| ((*name).to_owned(), ty)))
                    .collect(),
            )
        })
        .clone()
}

pub(crate) fn global_type(name: &str) -> Option<Type> {
    let string = Type::string();
    let callback = Type::Function(FunctionType::new_returns([], [Type::Any]));
    let unit_callback = named_function([("callback", callback.clone())], Type::nil(), 1);
    let named_callback = named_function(
        [("name", string.clone()), ("callback", callback.clone())],
        Type::nil(),
        2,
    );

    Some(match name {
        "describe" | "context" | "it" | "spec" | "test" => named_callback,
        "pending" => named_function([("name", string), ("callback", callback)], Type::nil(), 1),
        "before_each" | "after_each" | "setup" | "teardown" | "lazy_setup" | "lazy_teardown"
        | "strict_setup" | "strict_teardown" => unit_callback,
        "insulate" => named_callback,
        "finally" => unit_callback,
        "expose" => named_function([("name", Type::string())], Type::Any, 1),
        "assert" => assert_type(),
        "spy" => table([
            (
                "new",
                named_function([("callback", callback.clone())], Type::empty_table(), 0),
            ),
            (
                "on",
                named_function(
                    [("object", Type::Any), ("method", Type::string())],
                    Type::empty_table(),
                    2,
                ),
            ),
        ]),
        "stub" => table([(
            "new",
            named_function(
                [
                    ("object", Type::Any),
                    ("method", Type::string()),
                    ("replacement", Type::Any),
                ],
                Type::empty_table(),
                1,
            ),
        )]),
        "mock" => table([(
            "new",
            named_function([("module", Type::Any)], Type::empty_table(), 1),
        )]),
        "match" => table([(
            "is_type",
            named_function(
                [("value", Type::Any), ("type_name", Type::string())],
                Type::function([], Type::nil()),
                2,
            ),
        )]),
        _ => return None,
    })
}

fn assert_type() -> Type {
    let mut facts = MetatableFacts::default();
    // The first parameter represents the callable table receiver and is
    // removed by the shared call resolver.
    facts.call = Some(FunctionType::new_signature(
        [
            FunctionParameter::anonymous(Type::Any),
            FunctionParameter::named("value", Type::Any),
            FunctionParameter::named("message", Type::string()),
        ],
        1,
        None,
        [Type::Any],
    ));
    let message = Type::string();
    let assertion = table([
        (
            "is_true",
            named_function(
                [("value", Type::boolean()), ("message", message.clone())],
                Type::nil(),
                1,
            ),
        ),
        (
            "is_false",
            named_function(
                [("value", Type::boolean()), ("message", message.clone())],
                Type::nil(),
                1,
            ),
        ),
        (
            "is_nil",
            named_function(
                [("value", Type::Any), ("message", message.clone())],
                Type::nil(),
                1,
            ),
        ),
        (
            "is_not_nil",
            named_function(
                [("value", Type::Any), ("message", message.clone())],
                Type::nil(),
                1,
            ),
        ),
        (
            "has_error",
            named_function(
                [
                    ("callback", Type::function([], Type::Any)),
                    ("message", message.clone()),
                ],
                Type::nil(),
                1,
            ),
        ),
        (
            "are",
            table([
                (
                    "equal",
                    named_function(
                        [
                            ("expected", Type::Any),
                            ("actual", Type::Any),
                            ("message", message.clone()),
                        ],
                        Type::nil(),
                        2,
                    ),
                ),
                (
                    "same",
                    named_function(
                        [
                            ("expected", Type::Any),
                            ("actual", Type::Any),
                            ("message", message.clone()),
                        ],
                        Type::nil(),
                        2,
                    ),
                ),
            ]),
        ),
        (
            "are_not",
            table([(
                "equal",
                named_function(
                    [
                        ("expected", Type::Any),
                        ("actual", Type::Any),
                        ("message", message),
                    ],
                    Type::nil(),
                    2,
                ),
            )]),
        ),
    ]);
    let Type::Table(mut shape) = assertion else {
        unreachable!()
    };
    shape.set_metatable_facts(facts);
    Type::Table(shape)
}

fn table(fields: impl IntoIterator<Item = (&'static str, Type)>) -> Type {
    Type::Table(TableShape::from_fields(
        fields
            .into_iter()
            .map(|(name, ty)| (name, TableField::required(ty))),
    ))
}

fn named_function(
    parameters: impl IntoIterator<Item = (&'static str, Type)>,
    result: Type,
    required: usize,
) -> Type {
    let parameters = parameters
        .into_iter()
        .map(|(name, ty)| FunctionParameter::named(name, ty));
    Type::Function(FunctionType::new_signature(
        parameters,
        required,
        None,
        [result],
    ))
}

fn item(label: &'static str, detail: &'static str, documentation: &'static str) -> LuaItem {
    LuaItem {
        label,
        kind: if detail == "table" {
            LuaItemKind::Value
        } else {
            LuaItemKind::Function
        },
        detail,
        documentation,
    }
}

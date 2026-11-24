//! Compile-time contracts for the optional LÖVE save runtime. Schema builder
//! calls carry their value type through normal expression and module inference.
use crate::ast_utils::{field_chain_before_first_call, string_literal_runtime_bytes, token_text};
use crate::types::{FunctionParameter, FunctionType, TableField, TableShape, Type};
use crate::{Diagnostic, DiagnosticCode, ParsedSource, SourceSpan};
use full_moon::ast::{
    Call, Expression, Field, FunctionArgs, FunctionCall, Suffix, TableConstructor,
};
use full_moon::node::Node;
use full_moon::visitors::Visitor;
use std::collections::BTreeSet;

pub(crate) const MODULE_NAME: &str = "Save";
pub(crate) const RUNTIME_MODULE: &str = "tuasave";
pub(crate) const ISSUE_TYPE: &str = "SaveIssue";
const BUILTIN_MARKER: &str = "__tua_save_builtin";
const SCHEMA_VALUE: &str = "__tua_save_value";
const SCHEMA_OPTIONAL: &str = "__tua_save_optional";
const CONTRACT_VALUE: &str = "__tua_save_snapshot";

fn function(parameters: Vec<(&str, Type)>, required: usize, returns: Vec<Type>) -> Type {
    Type::Function(FunctionType::new_signature(
        parameters
            .into_iter()
            .map(|(name, ty)| FunctionParameter::named(name, ty)),
        required,
        None,
        returns,
    ))
}

fn method(parameters: Vec<(&str, Type)>, required: usize, returns: Vec<Type>) -> TableField {
    let mut with_self = vec![("self", Type::Any)];
    with_self.extend(parameters);
    TableField::required(function(with_self, required + 1, returns))
}

fn field(ty: Type) -> TableField {
    TableField::required(ty)
}

pub(crate) fn issue_type() -> Type {
    Type::table([
        ("code", field(Type::string())),
        ("message", field(Type::string())),
        ("slot", field(Type::string())),
        ("phase", field(Type::string())),
        ("recoverable", field(Type::boolean())),
    ])
}

pub(crate) fn builtin_aliases() -> Vec<(String, Type)> {
    vec![(ISSUE_TYPE.to_owned(), issue_type())]
}

pub(crate) fn schema_type(value: Type, optional: bool) -> Type {
    Type::table([
        (SCHEMA_VALUE, field(value)),
        (
            SCHEMA_OPTIONAL,
            field(Type::string_literal(if optional {
                "optional"
            } else {
                "required"
            })),
        ),
    ])
}

pub(crate) fn schema_value(ty: &Type) -> Option<(Type, bool)> {
    let Type::Table(table) = ty else { return None };
    let value = table.field(SCHEMA_VALUE)?.ty.clone();
    let optional = table
        .field(SCHEMA_OPTIONAL)
        .is_some_and(|field| field.ty == Type::string_literal("optional"));
    Some((value, optional))
}

pub(crate) fn shape_type(value: Type) -> Type {
    Type::table([
        ("defaults", field(value.clone())),
        (
            "copy",
            field(function(vec![("value", value.clone())], 1, vec![value])),
        ),
    ])
}

pub(crate) fn binding_type() -> Type {
    let issue = Type::union([Type::alias(ISSUE_TYPE), Type::nil()]);
    Type::table([
        (
            "update",
            method(
                vec![("dt", Type::number())],
                1,
                vec![Type::boolean(), issue.clone()],
            ),
        ),
        (
            "flush",
            method(vec![], 0, vec![Type::boolean(), issue.clone()]),
        ),
        ("isDirty", method(vec![], 0, vec![Type::boolean()])),
        ("lastIssue", method(vec![], 0, vec![issue])),
        ("markDirty", method(vec![], 0, vec![Type::nil()])),
        ("dispose", method(vec![], 0, vec![Type::nil()])),
    ])
}

pub(crate) fn contract_type(snapshot: Type) -> Type {
    let issue = Type::union([Type::alias(ISSUE_TYPE), Type::nil()]);
    let maybe_snapshot = Type::union([snapshot.clone(), Type::nil()]);
    let bind = bind_method(snapshot.clone(), Type::Any);
    Type::table([
        (CONTRACT_VALUE, field(snapshot.clone())),
        ("defaults", method(vec![], 0, vec![snapshot.clone()])),
        (
            "load",
            method(vec![], 0, vec![maybe_snapshot, issue.clone()]),
        ),
        (
            "save",
            method(
                vec![("snapshot", snapshot.clone())],
                1,
                vec![Type::boolean(), issue.clone()],
            ),
        ),
        ("bind", field(bind)),
        ("acceptRecovery", method(vec![], 0, vec![Type::boolean()])),
        (
            "reset",
            method(vec![], 0, vec![maybe_snapshot_type(snapshot), issue]),
        ),
    ])
}

fn bind_method(snapshot: Type, state: Type) -> Type {
    let selector = function(vec![("state", state)], 1, vec![snapshot]);
    let options = Type::table([
        ("select", field(selector)),
        ("debounce", TableField::optional(Type::number())),
        ("max_interval", TableField::optional(Type::number())),
    ]);
    method(
        vec![
            ("store", Type::alias(crate::store::TYPE_NAME)),
            ("options", options),
        ],
        2,
        vec![binding_type()],
    )
    .ty
}

pub(crate) fn specialized_bind_type(contract: &Type, state: Type) -> Option<Type> {
    let Type::Table(table) = contract else {
        return None;
    };
    let snapshot = table.field(CONTRACT_VALUE)?.ty.clone();
    Some(bind_method(snapshot, state))
}

fn maybe_snapshot_type(snapshot: Type) -> Type {
    Type::union([snapshot, Type::nil()])
}

pub(crate) fn builtin_locals() -> Vec<(String, Type)> {
    let broad_schema = schema_type(Type::Any, false);
    let builders = Type::table([
        (BUILTIN_MARKER, field(Type::string_literal("builtin"))),
        (
            "number",
            field(function(
                vec![("default", Type::number())],
                0,
                vec![schema_type(Type::number(), false)],
            )),
        ),
        (
            "string",
            field(function(
                vec![("default", Type::string())],
                0,
                vec![schema_type(Type::string(), false)],
            )),
        ),
        (
            "boolean",
            field(function(
                vec![("default", Type::boolean())],
                0,
                vec![schema_type(Type::boolean(), false)],
            )),
        ),
        (
            "optional",
            field(function(
                vec![("child", Type::Any)],
                1,
                vec![schema_type(Type::Any, true)],
            )),
        ),
        (
            "array",
            field(function(
                vec![("child", Type::Any), ("default", Type::Any)],
                1,
                vec![schema_type(Type::array(Type::Any), false)],
            )),
        ),
        (
            "record",
            field(function(
                vec![("fields", Type::Any)],
                1,
                vec![broad_schema.clone()],
            )),
        ),
        (
            "enum",
            field(function(
                vec![
                    ("values", Type::array(Type::string())),
                    ("default", Type::string()),
                ],
                1,
                vec![schema_type(Type::string(), false)],
            )),
        ),
        (
            "shape",
            field(function(
                vec![("schema", Type::Any)],
                1,
                vec![shape_type(Type::Any)],
            )),
        ),
        (
            "contract",
            field(function(
                vec![(
                    "options",
                    Type::table([
                        ("slot", field(Type::string())),
                        ("version", field(Type::number())),
                        ("schema", field(Type::Any)),
                        ("migrations", TableField::optional(Type::Any)),
                        ("migration_schemas", TableField::optional(Type::Any)),
                        ("max_bytes", TableField::optional(Type::number())),
                    ]),
                )],
                1,
                vec![contract_type(Type::Any)],
            )),
        ),
    ]);
    vec![(MODULE_NAME.to_owned(), builders)]
}

pub(crate) fn is_builtin(ty: &Type) -> bool {
    matches!(ty, Type::Table(table) if table.field(BUILTIN_MARKER).is_some())
}

pub(crate) fn is_contract(ty: &Type) -> bool {
    matches!(ty, Type::Table(table) if table.field(CONTRACT_VALUE).is_some())
}

pub(crate) fn snapshot_type(contract: &Type) -> Option<Type> {
    let Type::Table(table) = contract else {
        return None;
    };
    table.field(CONTRACT_VALUE).map(|field| field.ty.clone())
}

pub(crate) fn migration_function_type(input: Type, snapshot: Type) -> Type {
    function(vec![("old", input)], 1, vec![snapshot])
}

pub(crate) fn migration_functions(call: &FunctionCall) -> Vec<(&Expression, Option<&Expression>)> {
    let Some(Expression::TableConstructor(options)) = first_argument(call) else {
        return Vec::new();
    };
    let Some(Expression::TableConstructor(migrations)) = named_field(options, "migrations") else {
        return Vec::new();
    };
    let historical = match named_field(options, "migration_schemas") {
        Some(Expression::TableConstructor(table)) => Some(table),
        _ => None,
    };
    migrations
        .fields()
        .iter()
        .filter_map(|field| {
            let Field::ExpressionKey { key, value, .. } = field else {
                return None;
            };
            let schema = number_literal(key).and_then(|version| {
                historical?.fields().iter().find_map(|entry| match entry {
                    Field::ExpressionKey { key, value, .. }
                        if number_literal(key) == Some(version) =>
                    {
                        Some(value)
                    }
                    _ => None,
                })
            });
            Some((value, schema))
        })
        .collect()
}

pub(crate) fn contract_schema_argument(call: &FunctionCall) -> Option<&Expression> {
    let Expression::TableConstructor(options) = first_argument(call)? else {
        return None;
    };
    named_field(options, "schema")
}

pub(crate) fn is_internal_field(table: &TableShape, name: &str) -> bool {
    (name == BUILTIN_MARKER
        && table
            .field(BUILTIN_MARKER)
            .is_some_and(|field| field.ty == Type::string_literal("builtin")))
        || (matches!(name, SCHEMA_VALUE | SCHEMA_OPTIONAL)
            && table.field(SCHEMA_VALUE).is_some()
            && table.field(SCHEMA_OPTIONAL).is_some())
        || (name == CONTRACT_VALUE
            && table.field(CONTRACT_VALUE).is_some()
            && table.field("defaults").is_some())
}

pub(crate) fn module_documentation() -> &'static str {
    "Declare a bounded JSON save contract for LÖVE. A using Tua file imports the optional runtime automatically; disk access begins only when a contract loads or writes."
}

pub(crate) fn builder_documentation(name: &str) -> Option<&'static str> {
    Some(match name {
        "contract" => "Create a versioned save slot from a record schema. `load()` validates both slots; optional `migration_schemas` validate old payloads before migration. Writes use read-back verification. Set a stable LÖVE save identity first.",
        "record" => "Describe a JSON object with named fields. Every required child needs a default; optional children may be omitted.",
        "array" => "Describe a dense JSON array. Supply a child schema and a default array; optional elements are unsupported.",
        "number" => "Describe a finite number. A required record field needs a default.",
        "string" => "Describe a UTF-8 string. A required record field needs a default.",
        "boolean" => "Describe a boolean. A required record field needs a default.",
        "optional" => "Allow a record field to be absent. `nil` omits the field from JSON.",
        "enum" => "Describe one of the listed UTF-8 strings. Supply a listed default for a required field.",
        _ => return None,
    })
}

pub(crate) fn contract_method_documentation(name: &str) -> Option<&'static str> {
    Some(match name {
        "defaults" => "Return a fresh, deep-copied snapshot from schema defaults.",
        "load" => "Read and validate both save slots. Returns snapshot or nil, then a SaveIssue or nil; recovery and future-version errors block writes.",
        "save" => "Write an explicit checkpoint synchronously to the inactive slot, then read it back and verify before reporting success.",
        "bind" => "Bind a store without loading or mutating it. `select(state)` projects only persistent data; update(dt) polls the store revision and schedules synchronous writes.",
        "acceptRecovery" => "After informing the player that another save slot is damaged, accept the selected valid snapshot so writes may resume. The damaged slot's generation may be unknown. Returns false for other blocked states.",
        "reset" => "Explicitly remove both slot files and return fresh defaults. This discards saved progress and is intended for a player-approved recovery choice.",
        _ => return None,
    })
}

pub(crate) fn config_diagnostics(parsed: &ParsedSource) -> Vec<Diagnostic> {
    struct Check<'a> {
        parsed: &'a ParsedSource,
        scope: crate::scope::ScopeMap,
        diagnostics: Vec<Diagnostic>,
    }
    impl Check<'_> {
        fn report(&mut self, span: SourceSpan, message: impl Into<String>) {
            self.diagnostics.push(
                Diagnostic::error(
                    &self.parsed.path,
                    span.start.line,
                    span.start.column,
                    message,
                )
                .with_code(DiagnosticCode::DOMAIN_SAVE)
                .with_range(
                    span.start.line,
                    span.start.column,
                    span.end.line,
                    span.end.column,
                ),
            );
        }

        fn check_contract(&mut self, call: &FunctionCall) {
            let Some(Expression::TableConstructor(options)) = first_argument(call) else {
                return;
            };
            for key in ["slot", "version", "schema"] {
                if named_field(options, key).is_none() {
                    if let Some(span) = node_span(options) {
                        self.report(span, format!("Save.contract requires `{key}`"));
                    }
                }
            }
            if let Some(slot) = named_field(options, "slot") {
                if let Some(value) = string_value(slot) {
                    if value.is_empty()
                        || value.len() > 64
                        || !value.as_bytes()[0].is_ascii_alphanumeric()
                        || !value.bytes().all(|byte| {
                            byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
                        })
                    {
                        if let Some(span) = node_span(slot) {
                            self.report(span, "save slot must be a 1–64 character ASCII identifier without path separators");
                        }
                    }
                }
            }
            if let Some(version) = named_field(options, "version") {
                if let Expression::Number(token) = version {
                    let value = token_text(token).parse::<f64>().ok();
                    if !value.is_some_and(|value| {
                        (1.0..1_000_000.0).contains(&value) && value.fract() == 0.0
                    }) {
                        if let Some(span) = node_span(version) {
                            self.report(
                                span,
                                "save version must be a positive integer below 1,000,000",
                            );
                        }
                    }
                }
            }
            if let Some(schema) = named_field(options, "schema") {
                if let Expression::FunctionCall(schema_call) = schema {
                    if field_chain_before_first_call(schema_call).as_deref()
                        != Some(&["Save".to_owned(), "record".to_owned()])
                    {
                        if let Some(span) = node_span(schema) {
                            self.report(span, "save root schema must use Save.record");
                        }
                    } else {
                        self.check_defaults(schema, "$", 0);
                    }
                }
            }
            if let Some(max_bytes) = named_field(options, "max_bytes") {
                if let Expression::Number(token) = max_bytes {
                    let value = token_text(token).parse::<f64>().ok();
                    if !value.is_some_and(|value| {
                        (1024.0..=4_194_304.0).contains(&value) && value.fract() == 0.0
                    }) {
                        if let Some(span) = node_span(max_bytes) {
                            self.report(
                                span,
                                "save max_bytes must be an integer from 1 KiB to 4 MiB",
                            );
                        }
                    }
                }
            }
            if let Some(Expression::TableConstructor(schemas)) =
                named_field(options, "migration_schemas")
            {
                let current = named_field(options, "version").and_then(number_literal);
                let migrations = match named_field(options, "migrations") {
                    Some(Expression::TableConstructor(table)) => Some(table),
                    _ => None,
                };
                let mut versions = BTreeSet::new();
                for entry in schemas.fields().iter() {
                    let Field::ExpressionKey { key, value, .. } = entry else {
                        if let Some(span) = node_span(entry) {
                            self.report(
                                span,
                                "historical schemas must use numeric old-version keys",
                            );
                        }
                        continue;
                    };
                    if let Some(old) = number_literal(key) {
                        let invalid =
                            old < 1.0 || old.fract() != 0.0 || current.is_some_and(|v| old >= v);
                        if invalid || !versions.insert(old as i64) {
                            if let Some(span) = node_span(key) {
                                self.report(span, "historical schema key must be unique and below the current version");
                            }
                        }
                        if migrations.is_some() || named_field(options, "migrations").is_none() {
                            let found = migrations.is_some_and(|migrations| migrations.fields().iter().any(|entry| matches!(entry,
                                Field::ExpressionKey { key, .. } if number_literal(key) == Some(old))));
                            if !found {
                                if let Some(span) = node_span(key) {
                                    self.report(
                                        span,
                                        "historical schema needs a matching migration",
                                    );
                                }
                            }
                        }
                    }
                    if let Expression::FunctionCall(call) = value {
                        if let Some(parts) = field_chain_before_first_call(call) {
                            if parts.first().is_some_and(|root| root == MODULE_NAME)
                                && parts.get(1).is_some_and(|builder| builder != "record")
                            {
                                if let Some(span) = node_span(value) {
                                    self.report(
                                        span,
                                        "historical save root schema must use Save.record",
                                    );
                                }
                            }
                        }
                    }
                    self.check_defaults(value, "$", 0);
                }
            }
            if let Some(Expression::TableConstructor(migrations)) =
                named_field(options, "migrations")
            {
                let current = named_field(options, "version").and_then(number_literal);
                let mut versions = BTreeSet::new();
                for field in migrations.fields().iter() {
                    let Field::ExpressionKey { key, .. } = field else {
                        if let Some(span) = node_span(field) {
                            self.report(
                                span,
                                "migration entries must use numeric old-version keys",
                            );
                        }
                        continue;
                    };
                    if let Some(old) = number_literal(key) {
                        if old < 1.0
                            || old.fract() != 0.0
                            || current.is_some_and(|current| old >= current)
                        {
                            if let Some(span) = node_span(key) {
                                self.report(span, "migration key must be a positive integer below the current version");
                            }
                        }
                        if !versions.insert(old as i64) {
                            if let Some(span) = node_span(key) {
                                self.report(span, format!("duplicate migration for version {old}"));
                            }
                        }
                    }
                    // Migration callability, arity and result shape are checked by the
                    // shared expression checker. Named/imported functions are values too.
                }
            }
        }

        fn check_defaults(&mut self, schema: &Expression, path: &str, depth: usize) {
            if depth >= 32 {
                return;
            }
            let Expression::FunctionCall(call) = schema else {
                return;
            };
            let Some(parts) = field_chain_before_first_call(call) else {
                return;
            };
            let [root, builder] = parts.as_slice() else {
                return;
            };
            if root != MODULE_NAME {
                return;
            }
            if builder == "record" {
                if let Some(Expression::TableConstructor(fields)) = first_argument(call) {
                    for field in fields.fields().iter() {
                        if let Field::NameKey { key, value, .. } = field {
                            self.check_defaults(
                                value,
                                &format!("{path}.{}", token_text(key)),
                                depth + 1,
                            );
                        }
                    }
                }
                return;
            }
            if builder == "optional" {
                return;
            }
            let required_index = match builder.as_str() {
                "number" | "string" | "boolean" => 0,
                "array" | "enum" => 1,
                _ => return,
            };
            let default = call.suffixes().find_map(|suffix| match suffix {
                Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses {
                    arguments, ..
                })) => Some(arguments.iter().nth(required_index)),
                _ => None,
            });
            if default.flatten().is_none() {
                if let Some(span) = node_span(call) {
                    self.report(span, format!("save field `{path}` needs a default value"));
                }
            }
        }

        fn check_record(&mut self, call: &FunctionCall) {
            let Some(Expression::TableConstructor(fields)) = first_argument(call) else {
                return;
            };
            let mut names = BTreeSet::new();
            for field in fields.fields().iter() {
                let Field::NameKey { key, value, .. } = field else {
                    if let Some(span) = node_span(field) {
                        self.report(span, "Save.record fields must use named keys");
                    }
                    continue;
                };
                let name = token_text(key);
                if !names.insert(name.clone()) {
                    if let Some(span) = node_span(field) {
                        self.report(span, format!("duplicate save field `{name}`"));
                    }
                }
                if matches!(
                    value,
                    Expression::Number(_)
                        | Expression::String(_)
                        | Expression::Symbol(_)
                        | Expression::TableConstructor(_)
                        | Expression::Function(_)
                ) {
                    if let Some(span) = node_span(value) {
                        self.report(
                            span,
                            format!("save field `{name}` needs a Save schema descriptor"),
                        );
                    }
                }
            }
        }
    }
    impl Visitor for Check<'_> {
        fn visit_function_call(&mut self, call: &FunctionCall) {
            let Some(parts) = field_chain_before_first_call(call) else {
                return;
            };
            if parts.len() != 2 || parts[0] != MODULE_NAME {
                return;
            }
            let Some(span) = node_span(call) else { return };
            if self
                .scope
                .has_non_builtin_binding_at(MODULE_NAME, span.start)
            {
                return;
            }
            match parts[1].as_str() {
                "contract" => self.check_contract(call),
                "shape" => {
                    if let Some(schema) = first_argument(call) {
                        self.check_defaults(schema, "$", 0);
                        if let Expression::FunctionCall(schema_call) = schema {
                            if field_chain_before_first_call(schema_call).as_deref()
                                != Some(&["Save".to_owned(), "record".to_owned()])
                            {
                                self.report(span, "Save.shape root must use Save.record");
                            }
                        }
                    }
                }
                "record" => self.check_record(call),
                _ => {}
            }
        }
    }
    let mut check = Check {
        parsed,
        scope: crate::scope::scope_map_parsed_source(parsed),
        diagnostics: Vec::new(),
    };
    check.visit_ast(&parsed.ast);
    check.diagnostics
}

pub(crate) fn first_argument(call: &FunctionCall) -> Option<&Expression> {
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
    table.fields().iter().find_map(|field| match field {
        Field::NameKey { key, value, .. } if token_text(key) == name => Some(value),
        _ => None,
    })
}

fn string_value(expression: &Expression) -> Option<String> {
    let Expression::String(token) = expression else {
        return None;
    };
    String::from_utf8(string_literal_runtime_bytes(token)?).ok()
}

fn number_literal(expression: &Expression) -> Option<f64> {
    let Expression::Number(token) = expression else {
        return None;
    };
    token_text(token).parse().ok()
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

/// Preserve enum literals before ordinary mutable-table inference widens them.
/// Computed lists stay string-valued; their runtime contents are not statically known.
pub(crate) fn inferred_call_return(
    parts: &[String],
    argument: Option<&Type>,
    call: &FunctionCall,
) -> Option<Type> {
    if parts == ["Save", "enum"] {
        fn literal_values(expression: &Expression) -> Option<Vec<Type>> {
            match expression {
                Expression::Parentheses { expression, .. } => literal_values(expression),
                Expression::TableConstructor(table) => table
                    .fields()
                    .iter()
                    .map(|field| {
                        let Field::NoKey(value) = field else {
                            return None;
                        };
                        string_value(value).map(Type::string_literal)
                    })
                    .collect(),
                _ => None,
            }
        }
        if let Some(values) = first_argument(call).and_then(literal_values) {
            if !values.is_empty() {
                return Some(schema_type(Type::union(values), false));
            }
        }
    }
    inferred_return(parts, argument)
}

pub(crate) fn inferred_return(parts: &[String], argument: Option<&Type>) -> Option<Type> {
    if parts.first().map(String::as_str) != Some(MODULE_NAME) || parts.len() != 2 {
        return None;
    }
    let result = match parts[1].as_str() {
        "number" => schema_type(Type::number(), false),
        "string" => schema_type(Type::string(), false),
        "boolean" => schema_type(Type::boolean(), false),
        "enum" => schema_type(Type::string(), false),
        "optional" => schema_type(
            argument
                .and_then(schema_value)
                .map(|(value, _)| value)
                .unwrap_or(Type::Any),
            true,
        ),
        "array" => schema_type(
            Type::array(
                argument
                    .and_then(schema_value)
                    .map(|(value, _)| value)
                    .unwrap_or(Type::Any),
            ),
            false,
        ),
        "record" => {
            let value = match argument {
                Some(Type::Table(table)) => {
                    Type::table(table.fields().filter_map(|(name, field)| {
                        schema_value(&field.ty).map(|(value, optional)| {
                            (
                                name.to_owned(),
                                if optional {
                                    TableField::optional(value)
                                } else {
                                    TableField::required(value)
                                },
                            )
                        })
                    }))
                }
                _ => Type::Any,
            };
            schema_type(value, false)
        }
        "shape" => shape_type(
            argument
                .and_then(schema_value)
                .map(|(value, _)| value)
                .unwrap_or(Type::Any),
        ),
        "contract" => {
            let snapshot = match argument {
                Some(Type::Table(table)) => table
                    .field("schema")
                    .and_then(|field| schema_value(&field.ty))
                    .map(|(value, _)| value)
                    .unwrap_or(Type::Any),
                _ => Type::Any,
            };
            contract_type(snapshot)
        }
        _ => return None,
    };
    Some(result)
}

pub(crate) fn runtime_source() -> &'static str {
    include_str!("../../../vendor/runtime/tuasave.lua")
}

pub(crate) fn json_runtime_source() -> &'static str {
    include_str!("../../../vendor/runtime/tuasave_json.lua")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_literals_are_precise_but_computed_and_shadowed_lists_stay_dynamic() {
        let source = r#"
local shape = Save.shape(Save.record({ state = Save.enum({ "ready", "waiting" }, "ready") }))
local value = shape.defaults
local state: "ready" | "waiting" = value.state
value.state = "waiting"
local choices = { "ready", "waiting" }
choices[1] = "changed"
local dynamic = Save.shape(Save.record({ state = Save.enum(choices, "waiting") }))
local text: string = dynamic.defaults.state
print(state, text)
"#;
        let shadowed = crate::check::check_tua_source_with_diagnostics_config(
            "src/shadowed.tua",
            r#"
local Save = { enum = function(values: string[]): number return #values end }
local count: number = Save.enum({ "ready", "waiting" })
print(count)
"#
            .to_owned(),
            &Default::default(),
        )
        .err()
        .unwrap_or_default();
        assert!(
            shadowed
                .iter()
                .all(|d| d.severity != crate::DiagnosticSeverity::Error),
            "{shadowed:?}"
        );
        let valid = crate::check::check_tua_source_with_diagnostics_config(
            "src/main.tua",
            source.to_owned(),
            &crate::config::DiagnosticsConfig {
                globals: vec!["Save".to_owned()],
                ..Default::default()
            },
        )
        .err()
        .unwrap_or_default();
        assert!(
            valid
                .iter()
                .all(|d| d.severity != crate::DiagnosticSeverity::Error),
            "{valid:?}"
        );
        let diagnostics = crate::check::check_tua_source_with_diagnostics_config(
            "src/bad.tua",
            source.replace("value.state = \"waiting\"", "value.state = \"invalid\""),
            &crate::config::DiagnosticsConfig {
                globals: vec!["Save".to_owned()],
                ..Default::default()
            },
        )
        .expect_err("invalid enum member must fail");
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == Some(crate::DiagnosticCode::TYPE_MISMATCH)),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn literal_schema_infers_nested_snapshot_type() {
        let fields = Type::table([
            ("score", field(schema_type(Type::number(), false))),
            (
                "tags",
                field(schema_type(Type::array(Type::string()), false)),
            ),
            (
                "settings",
                field(schema_type(
                    Type::table([("enabled", field(Type::boolean()))]),
                    false,
                )),
            ),
        ]);
        let schema = inferred_return(&["Save".into(), "record".into()], Some(&fields)).unwrap();
        let Some((Type::Table(initial), _)) = schema_value(&schema) else {
            panic!("record schema should carry its snapshot type");
        };
        assert_eq!(initial.field("score").unwrap().ty, Type::number());
        assert_eq!(
            initial.field("tags").unwrap().ty,
            Type::array(Type::string())
        );
        let Type::Table(settings) = &initial.field("settings").unwrap().ty else {
            panic!("nested settings should remain a record");
        };
        assert_eq!(settings.field("enabled").unwrap().ty, Type::boolean());
    }
}

#[cfg(test)]
mod named_migration_tests {
    fn check(source: String) -> Result<(), Vec<crate::Diagnostic>> {
        crate::check::check_tua_source_with_diagnostics_config(
            "src/main.tua",
            source,
            &crate::config::DiagnosticsConfig {
                globals: vec!["Save".to_owned()],
                ..Default::default()
            },
        )
    }

    #[test]
    fn named_migrations_use_the_function_contract_not_literal_syntax() {
        let contract = r#"
local _profile = Save.contract({
  slot = "profile", version = 2,
  schema = Save.record({ count = Save.number(0) }),
  migrations = { [1] = migrate },
})
"#;
        let valid = format!("local function migrate(old: any): {{ count: number }} return {{ count = old.count }} end\n{contract}");
        check(valid).unwrap();
        for declaration in [
            "local migrate = 42",
            "local function migrate(old: any): { count: string } return { count = 'wrong' } end",
            "local function migrate(old: any, required: number): { count: number } return { count = required } end",
        ] {
            let diagnostics = check(format!("{declaration}\n{contract}")).expect_err("invalid migration must fail semantic checking");
            assert!(diagnostics.iter().any(|d| d.code == Some(crate::DiagnosticCode::TYPE_MISMATCH)), "{diagnostics:?}");
        }
    }
    #[test]
    fn imported_named_migrations_keep_contracts_and_editor_signatures() {
        let fixture = crate::Fixture::parse(
            r#"-- file: src/migrations.tua
export type Legacy = { points: number }
local function migrate(old: Legacy): { count: number } return { count = old.points } end
return { migrate = migrate, schema = Save.record({ points = Save.number(0) }) }
-- file: src/main.tua
local migrations = require("migrations")
local _profile = Save.contract({ slot = "profile", version = 2,
  schema = Save.record({ count = Save.number(0) }),
  migration_schemas = { [1] = migrations.schema },
  migrations = { [1] = migrations.<|hover|>migrate },
})
local result = migrations.migrate(<|signature|>{ points = 1 })
"#,
        );
        let mut host = crate::AnalysisHost::new();
        host.set_source_roots([std::path::PathBuf::from("src")]);
        for file in fixture.files() {
            host.set_file(&file.path, file.source.clone());
        }
        let snapshot = host.snapshot();
        let diagnostics = snapshot.pipeline_diagnostics("src/main.tua");
        assert!(
            !diagnostics
                .iter()
                .any(|d| d.severity == crate::DiagnosticSeverity::Error),
            "{diagnostics:?}"
        );
        let hover = snapshot
            .hover("src/main.tua", fixture.position("hover"))
            .unwrap();
        assert!(format!("{hover:?}").contains("count"), "{hover:?}");
        assert!(snapshot
            .signature_help("src/main.tua", fixture.position("signature"))
            .is_some());
        host.set_file(
            "src/migrations.tua",
            fixture
                .source("src/migrations.tua")
                .replace("count: number", "count: string"),
        );
        let diagnostics = host.snapshot().pipeline_diagnostics("src/main.tua");
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == Some(crate::DiagnosticCode::TYPE_MISMATCH)),
            "{diagnostics:?}"
        );
    }
}

#[cfg(test)]
mod historical_schema_tests {
    #[test]
    fn historical_schema_checks_migration_input_and_configuration() {
        let contract = r#"
local schema = Save.record({ points = Save.number(0) })
local _profile = Save.contract({ slot = "profile", version = 2,
  schema = Save.record({ score = Save.number(0) }),
  migration_schemas = { [1] = schema },
  migrations = { [1] = migrate },
})
"#;
        let check = |declaration: &str, contract: &str| {
            crate::check::check_tua_source_with_diagnostics_config(
                "src/main.tua",
                format!("{declaration}\n{contract}"),
                &crate::config::DiagnosticsConfig {
                    globals: vec!["Save".to_owned()],
                    ..Default::default()
                },
            )
        };
        let valid = "local function migrate(old: { points: number }): { score: number } return { score = old.points } end";
        check(valid, contract).unwrap();
        let wrong = valid.replace("points: number", "points: string");
        assert!(check(&wrong, contract).is_err());
        for bad in [
            contract.replace("[1] = schema", "[2] = schema"),
            contract.replace("[1] = schema", "[0] = schema"),
            contract.replace("[1] = schema", "[1.5] = schema"),
            contract.replace("[1] = schema", "[1] = Save.array(Save.string(), {})"),
            contract.replace("[1] = schema", "[1] = schema, [1] = schema"),
            contract.replace("[1] = migrate", "[0] = migrate"),
            contract.replace("  migrations = { [1] = migrate },", ""),
        ] {
            assert!(check(valid, &bad).is_err(), "{bad}");
        }
    }
}

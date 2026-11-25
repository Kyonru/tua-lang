//! Typed contract for the optional renderer-independent dialogue runtime.
use crate::types::{FunctionParameter, FunctionType, TableField};
use crate::{Type, TypeEnvironment};
use std::sync::OnceLock;
mod actions;

fn action_handle_constructor() -> Type {
    static CONTRACT: OnceLock<Type> = OnceLock::new();
    CONTRACT
        .get_or_init(|| {
            let parsed = crate::parse_tua_source(
                "tuadialogue.contract.tua",
                include_str!("../../../vendor/runtime/tuadialogue/contract.tua"),
            )
            .expect("bundled dialogue contract parses");
            let environment =
                TypeEnvironment::from_parsed(&parsed).expect("bundled dialogue contract resolves");
            let constructor = environment
                .function("actionHandle")
                .and_then(|function| function.as_type())
                .expect("action handle contract resolves");
            environment.resolve_nested_aliases(&constructor)
        })
        .clone()
}

fn action_result_type() -> Type {
    let Type::Function(constructor) = action_handle_constructor() else {
        unreachable!()
    };
    Type::union([Type::boolean(), *constructor.return_type])
}

pub(crate) fn module_type() -> Type {
    static CONTRACT: OnceLock<Type> = OnceLock::new();
    CONTRACT
        .get_or_init(|| {
            let parsed = crate::parse_tua_source(
                "tuadialogue.contract.tua",
                include_str!("../../../vendor/runtime/tuadialogue/contract.tua"),
            )
            .expect("bundled dialogue contract parses");
            let environment =
                TypeEnvironment::from_parsed(&parsed).expect("bundled dialogue contract resolves");
            let constructor = environment
                .function("new")
                .and_then(|function| function.as_type())
                .expect("dialogue constructor has an exported contract");
            let mut constructor = environment.resolve_nested_aliases(&constructor);
            let start = environment
                .function("start")
                .and_then(|function| function.as_type())
                .expect("dialogue start contract resolves");
            // Function declarations retain optional-argument arity; callback type
            // annotations currently describe required parameters only.
            if let Type::Function(function) = &mut constructor {
                let add_start = |ty: &mut Type| {
                    if let Type::Table(player) = ty {
                        player.insert("start", TableField::required(start.clone()));
                    }
                };
                add_start(&mut function.return_type);
                for ty in &mut function.return_types {
                    add_start(ty);
                }
            }
            let Type::Function(new) = &constructor else {
                unreachable!()
            };
            let factory = context_function(
                Type::Any,
                Type::Any,
                &new.parameters[0].ty,
                &new.parameters[1].ty,
                &new.return_type,
            );
            let event_factory = signature(
                vec![(
                    "schemas",
                    Type::dictionary(Type::string(), crate::save::shape_type(Type::Any)),
                )],
                1,
                events_definition(
                    Type::empty_table(),
                    &new.parameters[0].ty,
                    &new.parameters[1].ty,
                    &new.return_type,
                ),
            );
            let action_factory = signature(
                vec![(
                    "specs",
                    Type::dictionary(Type::string(), actions::spec_type()),
                )],
                1,
                actions::definition(
                    Type::empty_table(),
                    &new.parameters[0].ty,
                    &new.parameters[1].ty,
                    &new.return_type,
                ),
            );
            let speech = environment
                .function("speechProfile")
                .and_then(|function| function.as_type())
                .expect("speech profile contract");
            let speech = environment.resolve_nested_aliases(&speech);
            let reveal = environment
                .function("reveal")
                .and_then(|function| function.as_type())
                .expect("dialogue reveal contract resolves");
            let reveal = environment.resolve_nested_aliases(&reveal);
            Type::table([
                ("reveal", TableField::required(reveal)),
                ("speechProfile", TableField::required(speech)),
                (
                    "actionHandle",
                    TableField::required(action_handle_constructor()),
                ),
                ("actions", TableField::required(action_factory)),
                ("events", TableField::required(event_factory)),
                ("context", TableField::required(factory)),
                ("new", TableField::required(constructor)),
                ("schemaVersion", TableField::required(Type::number())),
            ])
        })
        .clone()
}

/// Asset exports use the exact constructor input contract, not widened Lua literals.
pub(crate) fn document_type() -> Type {
    let Type::Table(module) = module_type() else {
        unreachable!()
    };
    let Type::Function(new) = &module.field("new").expect("constructor").ty else {
        unreachable!()
    };
    new.parameters[0].ty.clone()
}

pub(crate) fn document_exports(path: &std::path::Path) -> crate::ModuleExports {
    let value_type = document_type();
    let Type::Table(table) = &value_type else {
        unreachable!()
    };
    let fields = table
        .fields()
        .map(|(name, field)| {
            (
                name.to_owned(),
                crate::module_index::ModuleExportField {
                    ty: field.ty.clone(),
                    documentation: None,
                    deprecation: None,
                    must_use: false,
                    parameter_names: Vec::new(),
                    parameter_documentation: Vec::new(),
                },
            )
        })
        .collect();
    crate::ModuleExports {
        path: path.to_owned(),
        value_type,
        fields,
    }
}

fn signature(parameters: Vec<(&str, Type)>, required: usize, result: Type) -> Type {
    Type::Function(FunctionType::new_signature(
        parameters
            .into_iter()
            .map(|(name, ty)| FunctionParameter::named(name, ty)),
        required,
        None,
        [result],
    ))
}

fn definition_type(
    facts: Type,
    locals: Type,
    document: &Type,
    options: &Type,
    base: &Type,
) -> Type {
    let Type::Table(mut player) = base.clone() else {
        unreachable!()
    };
    // Context checkpoints carry the concrete local shape through imports, just
    // like getLocals/setLocals. Plain players retain their version-1 contract.
    let Type::Function(export) = &player.field("checkpoint").expect("checkpoint contract").ty
    else {
        unreachable!()
    };
    let Type::Table(mut checkpoint) = (*export.return_type).clone() else {
        unreachable!()
    };
    checkpoint.insert("locals", TableField::required(locals.clone()));
    let checkpoint = Type::Table(checkpoint);
    player.insert(
        "checkpoint",
        TableField::required(signature(vec![("self", Type::Any)], 1, checkpoint.clone())),
    );
    player.insert(
        "restore",
        TableField::required(signature(
            vec![("self", Type::Any), ("checkpoint", checkpoint)],
            2,
            Type::nil(),
        )),
    );
    for (name, value) in [("getContext", facts.clone()), ("getLocals", locals.clone())] {
        player.insert(
            name,
            TableField::required(signature(
                vec![("self", Type::Any)],
                1,
                Type::Readonly(Box::new(value)),
            )),
        );
    }
    for (name, value) in [
        ("updateContext", facts.clone()),
        ("setLocals", locals.clone()),
    ] {
        player.insert(
            name,
            TableField::required(signature(
                vec![("self", Type::Any), ("value", value)],
                2,
                Type::nil(),
            )),
        );
    }
    Type::table([
        (
            "kind",
            TableField::required(Type::string_literal("context")),
        ),
        (
            "facts",
            TableField::required(Type::Readonly(Box::new(facts.clone()))),
        ),
        (
            "locals",
            TableField::required(Type::Readonly(Box::new(locals))),
        ),
        (
            "new",
            TableField::required(signature(
                vec![
                    ("source", document.clone()),
                    ("facts", facts),
                    ("options", options.clone()),
                ],
                2,
                Type::Table(player),
            )),
        ),
    ])
}

fn context_function(
    facts: Type,
    locals: Type,
    document: &Type,
    options: &Type,
    player: &Type,
) -> Type {
    signature(
        vec![(
            "schemas",
            Type::table([
                (
                    "actions",
                    TableField::optional(Type::table([
                        (
                            "kind",
                            TableField::required(Type::string_literal("actions")),
                        ),
                        ("payloads", TableField::required(Type::Any)),
                    ])),
                ),
                (
                    "events",
                    TableField::optional(Type::table([
                        ("kind", TableField::required(Type::string_literal("events"))),
                        ("payloads", TableField::required(Type::Any)),
                    ])),
                ),
                (
                    "facts",
                    TableField::required(crate::save::shape_type(facts.clone())),
                ),
                (
                    "locals",
                    TableField::required(crate::save::shape_type(locals.clone())),
                ),
            ]),
        )],
        1,
        definition_type(facts, locals, document, options, player),
    )
}

pub(crate) fn is_builtin(ty: &Type) -> bool {
    ty == &module_type()
}

pub(crate) fn inferred_context(options: Option<&Type>) -> Option<Type> {
    let Type::Table(options) = options? else {
        return None;
    };
    let shape = |name| {
        let Type::Table(schema) = &options.field(name)?.ty else {
            return None;
        };
        Some(schema.field("defaults")?.ty.clone())
    };
    let facts = shape("facts")?;
    let locals = shape("locals")?;
    let Type::Table(module) = module_type() else {
        unreachable!()
    };
    let Type::Function(new) = &module.field("new")?.ty else {
        unreachable!()
    };
    let payloads = options.field("events").and_then(|event| {
        let Type::Table(definition) = &event.ty else {
            return None;
        };
        definition.field("payloads").map(|field| &field.ty)
    });
    let document = payloads
        .map(|p| specialize_events(&new.parameters[0].ty, p))
        .unwrap_or_else(|| new.parameters[0].ty.clone());
    let player = payloads
        .map(|p| specialize_events(&new.return_type, p))
        .unwrap_or_else(|| (*new.return_type).clone());
    let action_payloads = options.field("actions").and_then(|action| {
        let Type::Table(definition) = &action.ty else {
            return None;
        };
        definition.field("payloads").map(|field| &field.ty)
    });
    let options = action_payloads
        .map(|p| actions::options(&new.parameters[1].ty, p, &player))
        .unwrap_or_else(|| new.parameters[1].ty.clone());
    Some(definition_type(facts, locals, &document, &options, &player))
}

/// Reuse one contract specialization for compiler inference, imports and editor queries.
pub(crate) fn inferred_factory(parts: &[String], argument: Option<&Type>) -> Option<Type> {
    match parts.get(1)?.as_str() {
        "context" => inferred_context(argument),
        "events" | "actions" => {
            let Type::Table(schemas) = argument? else {
                return None;
            };
            let mut fields = Vec::new();
            for (name, field) in schemas.fields() {
                let shape_type = if parts[1] == "actions" {
                    let Type::Table(spec) = &field.ty else {
                        return None;
                    };
                    &spec.field("payload")?.ty
                } else {
                    &field.ty
                };
                let Type::Table(shape) = shape_type else {
                    return None;
                };
                fields.push((
                    name.to_owned(),
                    TableField::required(shape.field("defaults")?.ty.clone()),
                ));
            }
            let Type::Table(module) = module_type() else {
                return None;
            };
            let Type::Function(new) = &module.field("new")?.ty else {
                return None;
            };
            if parts[1] == "actions" {
                return Some(actions::definition(
                    Type::table(fields),
                    &new.parameters[0].ty,
                    &new.parameters[1].ty,
                    &new.return_type,
                ));
            }
            Some(events_definition(
                Type::table(fields),
                &new.parameters[0].ty,
                &new.parameters[1].ty,
                &new.return_type,
            ))
        }
        _ => None,
    }
}

fn events_definition(payloads: Type, document: &Type, options: &Type, player: &Type) -> Type {
    let specialized = specialize_events(player, &payloads);
    let Type::Table(player_fields) = &specialized else {
        unreachable!()
    };
    let Type::Function(update) = &player_fields.field("update").expect("update contract").ty else {
        unreachable!()
    };
    let Type::Array(notification) = update.return_type.as_ref() else {
        unreachable!()
    };
    let Type::Table(fields) = &payloads else {
        unreachable!()
    };
    let readers = Type::table(fields.fields().map(|(name, field)| {
        (
            name.to_owned(),
            TableField::required(signature(
                vec![("event", (**notification).clone())],
                1,
                Type::union([field.ty.clone(), Type::nil()]),
            )),
        )
    }));
    Type::table([
        ("read", TableField::required(readers)),
        ("kind", TableField::required(Type::string_literal("events"))),
        (
            "payloads",
            TableField::required(Type::Readonly(Box::new(payloads.clone()))),
        ),
        (
            "new",
            TableField::required(signature(
                vec![
                    ("source", specialize_events(document, &payloads)),
                    ("options", options.clone()),
                ],
                1,
                specialize_events(player, &payloads),
            )),
        ),
    ])
}

fn specialize_events(ty: &Type, payloads: &Type) -> Type {
    let payloads = match payloads {
        Type::Readonly(inner) => inner,
        value => value,
    };
    let Type::Table(payloads) = payloads else {
        return ty.clone();
    };
    specialize_event_contract(ty, payloads)
}

fn specialize_event_contract(ty: &Type, payloads: &crate::types::TableShape) -> Type {
    match ty {
        Type::Table(table) => {
            if table.field("payload").is_some()
                && table.field("name").is_some()
                && (table.field("anchor").is_some() || table.field("generation").is_some())
            {
                let mut variants = vec![ty.clone()];
                for (name, payload) in payloads.fields() {
                    let mut event = table.clone();
                    event.insert("kind", TableField::required(Type::string_literal(name)));
                    event.insert("name", TableField::required(Type::string_literal(name)));
                    event.insert("payload", TableField::required(payload.ty.clone()));
                    variants.push(Type::Table(event));
                }
                Type::union(variants)
            } else {
                let mut result = table.clone();
                for (name, field) in table.fields() {
                    let mut field = field.clone();
                    field.ty = specialize_event_contract(&field.ty, payloads);
                    result.insert(name, field);
                }
                Type::Table(result)
            }
        }
        Type::Array(item) => Type::array(specialize_event_contract(item, payloads)),
        Type::Dictionary { key, value } => {
            Type::dictionary((**key).clone(), specialize_event_contract(value, payloads))
        }
        Type::Union(items) => Type::union(
            items
                .iter()
                .map(|item| specialize_event_contract(item, payloads)),
        ),
        Type::Function(function) => {
            let mut result = function.clone();
            for parameter in &mut result.parameters {
                parameter.ty = specialize_event_contract(&parameter.ty, payloads);
            }
            result.return_type = Box::new(specialize_event_contract(&result.return_type, payloads));
            result.return_types = result
                .return_types
                .iter()
                .map(|value| specialize_event_contract(value, payloads))
                .collect();
            Type::Function(result)
        }
        _ => ty.clone(),
    }
}

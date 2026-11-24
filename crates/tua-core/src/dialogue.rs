//! Typed contract for the optional renderer-independent dialogue runtime.
use crate::types::{FunctionParameter, FunctionType, TableField};
use crate::{Type, TypeEnvironment};
use std::sync::OnceLock;

pub(crate) fn module_type() -> Type {
    static CONTRACT: OnceLock<Type> = OnceLock::new();
    CONTRACT
        .get_or_init(|| {
            let parsed = crate::parse_tua_source(
                "tuadialogue.contract.tua",
                include_str!("../../../vendor/runtime/tuadialogue.contract.tua"),
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
            Type::table([
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
    Some(definition_type(
        facts,
        locals,
        &new.parameters[0].ty,
        &new.parameters[1].ty,
        &new.return_type,
    ))
}

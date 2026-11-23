//! Typed contract for the optional renderer-independent dialogue runtime.
use crate::types::TableField;
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
            Type::table([("new", TableField::required(constructor))])
        })
        .clone()
}

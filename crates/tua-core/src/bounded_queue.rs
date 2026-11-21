//! The ordinary imported module uses the shared generic function machinery.
use crate::types::TableField;
use crate::{Type, TypeEnvironment};
use std::sync::OnceLock;

pub(crate) fn module_type() -> Type {
    static CONTRACT: OnceLock<Type> = OnceLock::new();
    CONTRACT
        .get_or_init(|| {
            let parsed = crate::parse_tua_source(
                "tuaboundedqueue.contract.tua",
                include_str!("../../../vendor/runtime/tuaboundedqueue.contract.tua"),
            )
            .expect("bundled queue contract parses");
            let environment =
                TypeEnvironment::from_parsed(&parsed).expect("bundled queue contract resolves");
            let constructor = environment
                .function("new")
                .and_then(|function| function.as_type())
                .expect("queue constructor has an exported contract");
            Type::table([("new", TableField::required(constructor))])
        })
        .clone()
}

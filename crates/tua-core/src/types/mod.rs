mod assignability;
mod environment;
mod model;

pub use assignability::{is_assignable, TypeContext};
pub(crate) use environment::build_type_environment_with_diagnostics;
pub use environment::{build_type_environment, TypeEnvironment, TypedFunction, TypedParameter};
pub use model::{FunctionType, PrimitiveType, TableField, TableShape, Type, TypeAlias};

#[cfg(test)]
mod tests;

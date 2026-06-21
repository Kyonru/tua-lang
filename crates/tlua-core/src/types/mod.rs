mod assignability;
mod model;

pub use assignability::{is_assignable, TypeContext};
pub use model::{FunctionType, PrimitiveType, TableField, TableShape, Type, TypeAlias};

#[cfg(test)]
mod tests;

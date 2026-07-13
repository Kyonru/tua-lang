mod assignability;
mod environment;
mod model;

pub use assignability::{is_assignable, TypeContext};
pub use environment::{
    build_type_environment, TypeEnvironment, TypedFunction, TypedFunctionOverload, TypedParameter,
};
pub(crate) use environment::{
    build_type_environment_with_builtin_aliases, build_type_environment_with_diagnostics,
};
pub use model::{
    FunctionType, IndexFallback, MetatableFacts, NewIndexRule, PrimitiveType, TableField,
    TableShape, Type, TypeAlias,
};

use crate::parser::TuaType;

pub(crate) fn type_from_tua_type(ty: &TuaType) -> Type {
    match ty {
        TuaType::Named(name) => Type::named(name),
        TuaType::StringLiteral(value) => Type::string_literal(value),
        TuaType::Union(types) => Type::union(types.iter().map(type_from_tua_type)),
        TuaType::Table(fields) => Type::table(fields.iter().map(|field| {
            let ty = type_from_tua_type(&field.ty);
            let table_field = if field.optional {
                TableField::optional(ty)
            } else {
                TableField::required(ty)
            };

            (field.name.clone(), table_field)
        })),
        TuaType::Array(item) => Type::array(type_from_tua_type(item)),
        TuaType::Tuple(items) => Type::tuple(items.iter().map(type_from_tua_type)),
        TuaType::Dictionary { key, value } => {
            Type::dictionary(type_from_tua_type(key), type_from_tua_type(value))
        }
        TuaType::Function {
            parameters,
            returns,
        } => {
            let required_parameter_count = parameters
                .iter()
                .take_while(|parameter| !parameter.optional)
                .count();

            Type::function_returns_with_required(
                parameters.iter().map(|parameter| {
                    let ty = type_from_tua_type(&parameter.ty);

                    if parameter.optional {
                        Type::union([ty, Type::nil()])
                    } else {
                        ty
                    }
                }),
                required_parameter_count,
                returns.iter().map(type_from_tua_type),
            )
        }
    }
}

#[cfg(test)]
mod tests;

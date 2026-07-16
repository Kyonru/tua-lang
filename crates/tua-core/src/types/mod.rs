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

pub(crate) fn widen_inferred_type(ty: &Type) -> Type {
    match ty {
        Type::StringLiteral(_) => Type::string(),
        Type::Union(types) => Type::union(types.iter().map(widen_inferred_type)),
        Type::Function(function) => Type::function_returns_with_required(
            function.parameters.iter().map(widen_inferred_type),
            function.required_parameter_count,
            function
                .return_types_or_any()
                .iter()
                .map(widen_inferred_type),
        ),
        Type::Table(table) => {
            let mut widened = TableShape::from_fields(table.fields().map(|(name, field)| {
                let ty = if name.starts_with("__") {
                    field.ty.clone()
                } else {
                    widen_inferred_type(&field.ty)
                };

                (
                    name.to_owned(),
                    TableField {
                        ty,
                        optional: field.optional,
                    },
                )
            }));

            if let Some(facts) = table.metatable_facts() {
                widened.set_metatable_facts(facts.clone());
            }

            if table.has_self_index() {
                widened.mark_self_index();
            }

            for method in table.constructor_methods() {
                widened.mark_constructor_method(method);
            }

            Type::Table(widened)
        }
        Type::Array(item) => Type::array(widen_inferred_type(item)),
        Type::Tuple(items) => Type::tuple(items.iter().map(widen_inferred_type)),
        Type::Dictionary { key, value } => {
            Type::dictionary(widen_inferred_type(key), widen_inferred_type(value))
        }
        Type::Primitive(_) | Type::Any | Type::Alias(_) => ty.clone(),
    }
}

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

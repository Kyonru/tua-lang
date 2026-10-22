mod assignability;
mod environment;
mod format;
mod model;

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

pub use assignability::{
    is_assignable, AssignabilityFailure, AssignabilityFailureKind, AssignabilityPathSegment,
    TypeContext,
};
pub use environment::{
    build_type_environment, TypeEnvironment, TypedFunction, TypedFunctionOverload, TypedParameter,
};
pub(crate) use environment::{
    build_type_environment_with_diagnostics, build_type_environment_with_domain_registry,
    inferred_function_type_from_body,
};
pub(crate) use format::{format_named_function_pretty, format_type_with_prefix};
pub use model::{
    FunctionParameter, FunctionType, IndexFallback, MetatableFacts, NewIndexRule, PrimitiveType,
    TableField, TableShape, Type, TypeAlias,
};

/// Remove duplicate inferred types without repeatedly comparing every deeply
/// nested type with every type already seen. Generated table literals can
/// contain thousands of structurally large records; the previous Vec::contains
/// loop made array inference quadratic in both item count and type size.
pub(crate) fn deduplicate_inferred_types(types: Vec<Type>) -> Vec<Type> {
    let mut unique: Vec<Type> = Vec::new();
    let mut buckets: HashMap<u64, Vec<usize>> = HashMap::new();

    for ty in types {
        let mut hasher = DefaultHasher::new();
        ty.hash(&mut hasher);
        let fingerprint = hasher.finish();
        let bucket = buckets.entry(fingerprint).or_default();

        if bucket.iter().any(|index| unique[*index] == ty) {
            continue;
        }

        bucket.push(unique.len());
        unique.push(ty);
    }

    unique
}

use crate::parser::{FunctionTypeParameter as ParsedFunctionTypeParameter, TuaType};
use crate::parser::{LuaDocAnnotationKind, ParsedSource};
use full_moon::ast;

/// Whether a statement begins immediately after a LuaDoc `---@type` contract.
/// Blank lines and adjacent LuaDoc lines remain part of the same documentation
/// group, matching the environment's LuaDoc lowering behavior.
pub(crate) fn has_leading_lua_doc_type(parsed: &ParsedSource, line: usize) -> bool {
    let Some(annotation) = parsed.lua_doc_annotations.iter().rev().find(|annotation| {
        annotation.span.end.line < line && matches!(annotation.kind, LuaDocAnnotationKind::Type(_))
    }) else {
        return false;
    };

    parsed
        .source
        .lines()
        .enumerate()
        .skip(annotation.span.end.line)
        .take(line.saturating_sub(annotation.span.end.line + 1))
        .all(|(_, source)| {
            let source = source.trim_start();
            source.is_empty() || source.starts_with("---")
        })
}

pub(crate) fn widen_inferred_type(ty: &Type) -> Type {
    match ty {
        Type::StringLiteral(_) => Type::string(),
        Type::Union(types) => Type::union(types.iter().map(widen_inferred_type)),
        // Parameter types are never inferred from values: a literal there
        // is an annotated contract such as `---@param mode "fill" | "line"`,
        // and widening it would accept arguments the contract rejects.
        Type::Function(function) => Type::Function(FunctionType::new_signature(
            function.parameters.iter().cloned(),
            function.required_parameter_count,
            function.vararg_type.as_deref().cloned(),
            function
                .return_types_or_any()
                .iter()
                .map(widen_inferred_type),
        )),
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
            widened.copy_semantic_metadata_from(table);

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
        } => Type::Function(function_type_from_tua_signature(
            parameters,
            returns,
            |ty| Some(type_from_tua_type(ty)),
        )),
    }
}

pub(crate) fn function_type_from_tua_signature(
    parameters: &[ParsedFunctionTypeParameter],
    returns: &[TuaType],
    mut resolve: impl FnMut(&TuaType) -> Option<Type>,
) -> FunctionType {
    let fixed_parameters = parameters
        .iter()
        .filter(|parameter| !parameter.variadic)
        .collect::<Vec<_>>();
    let required_parameter_count = fixed_parameters
        .iter()
        .take_while(|parameter| !parameter.optional)
        .count();
    let vararg_type = parameters
        .iter()
        .find(|parameter| parameter.variadic)
        .and_then(|parameter| resolve(&parameter.ty));
    let parameters = fixed_parameters
        .into_iter()
        .filter_map(|parameter| {
            let ty = resolve(&parameter.ty)?;
            let ty = if parameter.optional {
                Type::union([ty, Type::nil()])
            } else {
                ty
            };

            Some(if let Some(name) = parameter.name.as_deref() {
                FunctionParameter::named(name, ty)
            } else {
                FunctionParameter::anonymous(ty)
            })
        })
        .collect::<Vec<_>>();
    let returns = returns.iter().filter_map(resolve).collect::<Vec<_>>();

    FunctionType::new_signature(parameters, required_parameter_count, vararg_type, returns)
}

pub(crate) fn function_type_from_body(body: &ast::FunctionBody) -> Type {
    let mut parameters = Vec::new();
    let mut vararg_type = None;

    for (parameter, specifier) in body.parameters().iter().zip(body.type_specifiers()) {
        let ty = specifier
            .map(|specifier| {
                type_from_tua_type(&crate::parser::tua_type_from_type_info(
                    specifier.type_info(),
                ))
            })
            .unwrap_or(Type::Any);

        match parameter {
            ast::Parameter::Name(name) => {
                parameters.push(FunctionParameter::named(name.to_string().trim(), ty))
            }
            ast::Parameter::Ellipsis(_) => vararg_type = Some(ty),
            _ => {}
        }
    }

    let return_types = body
        .return_type()
        .map(|specifier| crate::parser::tua_type_from_type_info(specifier.type_info()))
        .map(|ty| match ty {
            TuaType::Tuple(types) => types
                .into_iter()
                .map(|ty| type_from_tua_type(&ty))
                .collect(),
            ty => vec![type_from_tua_type(&ty)],
        })
        .unwrap_or_else(|| vec![Type::Any]);
    let required_parameter_count = parameters.len();

    Type::Function(FunctionType::new_signature(
        parameters,
        required_parameter_count,
        vararg_type,
        return_types,
    ))
}

#[cfg(test)]
mod tests;

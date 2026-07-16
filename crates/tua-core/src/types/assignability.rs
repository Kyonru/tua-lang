use std::collections::{BTreeMap, BTreeSet};

use super::model::{FunctionType, TableShape, Type, TypeAlias};

const MAX_ALIAS_DEPTH: usize = 64;

#[derive(Clone, Debug, Default)]
pub struct TypeContext {
    aliases: BTreeMap<String, Type>,
    nominal_supertypes: BTreeMap<String, BTreeSet<String>>,
}

impl TypeContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_alias(mut self, alias: TypeAlias) -> Self {
        self.insert_alias(alias);
        self
    }

    pub fn insert_alias(&mut self, alias: TypeAlias) {
        self.aliases.insert(alias.name, alias.ty);
    }

    pub fn alias(&self, name: &str) -> Option<&Type> {
        self.aliases.get(name)
    }

    pub fn with_nominal_supertype(
        mut self,
        type_name: impl Into<String>,
        supertype: impl Into<String>,
    ) -> Self {
        self.insert_nominal_supertype(type_name, supertype);
        self
    }

    pub fn insert_nominal_supertype(
        &mut self,
        type_name: impl Into<String>,
        supertype: impl Into<String>,
    ) {
        self.nominal_supertypes
            .entry(type_name.into())
            .or_default()
            .insert(supertype.into());
    }

    pub fn is_assignable(&self, source: &Type, target: &Type) -> bool {
        self.is_assignable_inner(source, target, 0)
    }

    fn is_assignable_inner(&self, source: &Type, target: &Type, depth: usize) -> bool {
        if source == target {
            return true;
        }

        if depth >= MAX_ALIAS_DEPTH {
            return false;
        }

        match (source, target) {
            (Type::Any, _) | (_, Type::Any) => true,
            (Type::Alias(source), Type::Alias(target))
                if self.alias(source).is_none()
                    && self.alias(target).is_none()
                    && self.is_nominal_subtype(source, target, depth + 1) =>
            {
                true
            }
            (Type::Alias(name), _) if self.alias(name).is_some() => self
                .alias(name)
                .is_some_and(|resolved| self.is_assignable_inner(resolved, target, depth + 1)),
            (_, Type::Alias(name)) if self.alias(name).is_some() => self
                .alias(name)
                .is_some_and(|resolved| self.is_assignable_inner(source, resolved, depth + 1)),
            (Type::Union(sources), _) => sources
                .iter()
                .all(|source| self.is_assignable_inner(source, target, depth + 1)),
            (_, Type::Union(targets)) => targets
                .iter()
                .any(|target| self.is_assignable_inner(source, target, depth + 1)),
            (Type::StringLiteral(_), Type::Primitive(super::PrimitiveType::String)) => true,
            (Type::Primitive(super::PrimitiveType::String), Type::StringLiteral(_)) => false,
            (Type::StringLiteral(source), Type::StringLiteral(target)) => source == target,
            (Type::Primitive(source), Type::Primitive(target)) => source == target,
            (Type::Function(source), Type::Function(target)) => {
                self.is_function_assignable(source, target, depth + 1)
            }
            (Type::Table(source), Type::Table(target)) => {
                self.is_table_assignable(source, target, depth + 1)
            }
            (Type::Array(source), Type::Array(target)) => {
                self.is_assignable_inner(source, target, depth + 1)
            }
            (Type::Tuple(source), Type::Tuple(target)) => {
                source.len() == target.len()
                    && source
                        .iter()
                        .zip(target)
                        .all(|(source, target)| self.is_assignable_inner(source, target, depth + 1))
            }
            (
                Type::Dictionary {
                    key: source_key,
                    value: source_value,
                },
                Type::Dictionary {
                    key: target_key,
                    value: target_value,
                },
            ) => {
                self.is_assignable_inner(source_key, target_key, depth + 1)
                    && self.is_assignable_inner(source_value, target_value, depth + 1)
            }
            _ => false,
        }
    }

    fn is_nominal_subtype(&self, source: &str, target: &str, depth: usize) -> bool {
        if source == target {
            return true;
        }

        if depth >= MAX_ALIAS_DEPTH {
            return false;
        }

        self.nominal_supertypes
            .get(source)
            .is_some_and(|supertypes| {
                supertypes.iter().any(|supertype| {
                    supertype == target || self.is_nominal_subtype(supertype, target, depth + 1)
                })
            })
    }

    fn is_function_assignable(
        &self,
        source: &FunctionType,
        target: &FunctionType,
        depth: usize,
    ) -> bool {
        source.required_parameter_count <= target.required_parameter_count
            && target.parameters.iter().enumerate().all(|(index, target)| {
                source
                    .parameters
                    .get(index)
                    .map(|parameter| &parameter.ty)
                    .or(source.vararg_type.as_deref())
                    .is_some_and(|source| self.is_assignable_inner(&target.ty, source, depth + 1))
            })
            && match (source.vararg_type.as_deref(), target.vararg_type.as_deref()) {
                (_, None) => true,
                (Some(source), Some(target)) => self.is_assignable_inner(target, source, depth + 1),
                (None, Some(_)) => false,
            }
            && source.return_types_or_any().len() == target.return_types_or_any().len()
            && source
                .return_types_or_any()
                .iter()
                .zip(target.return_types_or_any())
                .all(|(source, target)| self.is_assignable_inner(source, target, depth + 1))
    }

    fn is_table_assignable(&self, source: &TableShape, target: &TableShape, depth: usize) -> bool {
        target.fields().all(|(name, target_field)| {
            let Some(source_field) = source.semantic_field(name) else {
                return target_field.optional;
            };

            if !target_field.optional && source_field.optional {
                return false;
            }

            self.is_assignable_inner(&source_field.ty, &target_field.ty, depth + 1)
        })
    }
}

pub fn is_assignable(source: &Type, target: &Type) -> bool {
    TypeContext::new().is_assignable(source, target)
}

impl Type {
    pub fn is_assignable_to(&self, target: &Type) -> bool {
        is_assignable(self, target)
    }
}

use std::collections::BTreeMap;

use super::model::{FunctionType, TableShape, Type, TypeAlias};

const MAX_ALIAS_DEPTH: usize = 64;

#[derive(Clone, Debug, Default)]
pub struct TypeContext {
    aliases: BTreeMap<String, Type>,
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
            (Type::Alias(name), _) => self
                .alias(name)
                .is_some_and(|resolved| self.is_assignable_inner(resolved, target, depth + 1)),
            (_, Type::Alias(name)) => self
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

    fn is_function_assignable(
        &self,
        source: &FunctionType,
        target: &FunctionType,
        depth: usize,
    ) -> bool {
        source.parameters.len() == target.parameters.len()
            && source
                .parameters
                .iter()
                .zip(&target.parameters)
                .all(|(source, target)| self.is_assignable_inner(target, source, depth + 1))
            && source.return_types_or_any().len() == target.return_types_or_any().len()
            && source
                .return_types_or_any()
                .iter()
                .zip(target.return_types_or_any())
                .all(|(source, target)| self.is_assignable_inner(source, target, depth + 1))
    }

    fn is_table_assignable(&self, source: &TableShape, target: &TableShape, depth: usize) -> bool {
        target.fields().all(|(name, target_field)| {
            let Some(source_field) = source.field(name) else {
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

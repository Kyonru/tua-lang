use std::collections::{BTreeMap, BTreeSet};

use super::model::{is_index_signature_field, FunctionType, TableShape, Type, TypeAlias};

const MAX_ASSIGNABILITY_DEPTH: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssignabilityPathSegment {
    Field(String),
    ArrayElement,
    TupleElement(usize),
    DictionaryKey(Option<String>),
    DictionaryValue,
    FunctionParameter { index: usize, name: Option<String> },
    FunctionReturn(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssignabilityFailureKind {
    TypeMismatch,
    MissingField,
    OptionalField,
    FunctionParameterCount { expected: usize, found: usize },
    FunctionVararg,
    FunctionReturnCount { expected: usize, found: usize },
    TupleLength { expected: usize, found: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssignabilityFailure {
    pub path: Vec<AssignabilityPathSegment>,
    pub expected: Type,
    pub found: Type,
    pub kind: AssignabilityFailureKind,
}

impl AssignabilityFailure {
    fn type_mismatch(found: &Type, expected: &Type) -> Box<Self> {
        Box::new(Self {
            path: Vec::new(),
            expected: expected.clone(),
            found: found.clone(),
            kind: AssignabilityFailureKind::TypeMismatch,
        })
    }

    fn at(mut self: Box<Self>, segment: AssignabilityPathSegment) -> Box<Self> {
        self.path.insert(0, segment);
        self
    }
}

#[derive(Clone, Debug, Default)]
pub struct TypeContext {
    aliases: BTreeMap<String, Type>,
    nominal_supertypes: BTreeMap<String, BTreeSet<String>>,
    nominal_aliases: BTreeSet<String>,
    nominal_alias_supertypes: BTreeMap<String, BTreeSet<String>>,
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

    pub fn insert_nominal_alias_supertype(
        &mut self,
        type_name: impl Into<String>,
        supertype: impl Into<String>,
    ) {
        let type_name = type_name.into();
        let supertype = supertype.into();

        self.nominal_aliases.insert(type_name.clone());
        self.nominal_aliases.insert(supertype.clone());
        self.nominal_alias_supertypes
            .entry(type_name)
            .or_default()
            .insert(supertype);
    }

    pub fn insert_nominal_alias(&mut self, name: impl Into<String>) {
        self.nominal_aliases.insert(name.into());
    }

    pub fn is_assignable(&self, source: &Type, target: &Type) -> bool {
        self.check_assignable(source, target).is_ok()
    }

    pub fn check_assignable(
        &self,
        source: &Type,
        target: &Type,
    ) -> Result<(), Box<AssignabilityFailure>> {
        self.check_assignable_inner(source, target, 0, &mut BTreeSet::new())
    }

    pub fn assignability_failure(
        &self,
        source: &Type,
        target: &Type,
    ) -> Option<AssignabilityFailure> {
        self.check_assignable(source, target)
            .err()
            .map(|failure| *failure)
    }

    fn check_assignable_inner(
        &self,
        source: &Type,
        target: &Type,
        depth: usize,
        resolving: &mut BTreeSet<(bool, String)>,
    ) -> Result<(), Box<AssignabilityFailure>> {
        if source == target {
            return Ok(());
        }

        if depth >= MAX_ASSIGNABILITY_DEPTH {
            return Err(AssignabilityFailure::type_mismatch(source, target));
        }

        match (source, target) {
            (Type::Any, _) | (_, Type::Any) => Ok(()),
            (Type::Alias(source), Type::Alias(target))
                if self.is_nominal_alias_type(source) || self.is_nominal_alias_type(target) =>
            {
                self.is_nominal_alias_subtype(source, target, depth + 1)
                    .then_some(())
                    .ok_or_else(|| {
                        AssignabilityFailure::type_mismatch(
                            &Type::Alias(source.clone()),
                            &Type::Alias(target.clone()),
                        )
                    })
            }
            (Type::Union(sources), _) => {
                for source in sources {
                    self.check_assignable_inner(source, target, depth + 1, resolving)?;
                }
                Ok(())
            }
            (_, Type::Union(targets)) => {
                let mut failures = Vec::new();
                for target in targets {
                    match self.check_assignable_inner(source, target, depth + 1, resolving) {
                        Ok(()) => return Ok(()),
                        Err(failure) => failures.push(*failure),
                    }
                }

                Err(best_union_failure(source, target, failures))
            }
            (_, Type::Alias(target)) if self.is_nominal_alias_type(target) => Err(
                AssignabilityFailure::type_mismatch(source, &Type::Alias(target.clone())),
            ),
            (Type::Alias(source), Type::Alias(target))
                if self.alias(source).is_none()
                    && self.alias(target).is_none()
                    && self.is_nominal_subtype(source, target, depth + 1) =>
            {
                Ok(())
            }
            (Type::Alias(name), _) if self.alias(name).is_some() => {
                let key = (true, name.clone());
                if !resolving.insert(key.clone()) {
                    return Ok(());
                }
                let resolved = self.alias(name).expect("alias existence was checked");
                let result = self.check_assignable_inner(resolved, target, depth + 1, resolving);
                resolving.remove(&key);
                result
            }
            (_, Type::Alias(name)) if self.alias(name).is_some() => {
                let key = (false, name.clone());
                if !resolving.insert(key.clone()) {
                    return Ok(());
                }
                let resolved = self.alias(name).expect("alias existence was checked");
                let result = self.check_assignable_inner(source, resolved, depth + 1, resolving);
                resolving.remove(&key);
                result
            }
            (Type::StringLiteral(_), Type::Primitive(super::PrimitiveType::String)) => Ok(()),
            (Type::StringLiteral(source), Type::StringLiteral(target)) if source == target => {
                Ok(())
            }
            (Type::Primitive(source), Type::Primitive(target)) if source == target => Ok(()),
            (Type::Function(source), Type::Function(target)) => {
                self.check_function_assignable(source, target, depth + 1, resolving)
            }
            (Type::Table(source), Type::Table(target)) => {
                self.check_table_assignable(source, target, depth + 1, resolving)
            }
            (
                Type::Table(source),
                Type::Dictionary {
                    key: target_key,
                    value: target_value,
                },
            ) => self.check_table_assignable_to_dictionary(
                source,
                target_key,
                target_value,
                depth + 1,
                resolving,
            ),
            (Type::Table(source), Type::Array(_)) if source.is_plain_empty() => Ok(()),
            (Type::Array(source), Type::Array(target)) => self
                .check_assignable_inner(source, target, depth + 1, resolving)
                .map_err(|failure| failure.at(AssignabilityPathSegment::ArrayElement)),
            (Type::Tuple(source), Type::Tuple(target)) => {
                if source.len() != target.len() {
                    return Err(Box::new(AssignabilityFailure {
                        path: Vec::new(),
                        expected: Type::Tuple(target.clone()),
                        found: Type::Tuple(source.clone()),
                        kind: AssignabilityFailureKind::TupleLength {
                            expected: target.len(),
                            found: source.len(),
                        },
                    }));
                }

                for (index, (source, target)) in source.iter().zip(target).enumerate() {
                    self.check_assignable_inner(source, target, depth + 1, resolving)
                        .map_err(|failure| {
                            failure.at(AssignabilityPathSegment::TupleElement(index))
                        })?;
                }
                Ok(())
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
                self.check_assignable_inner(source_key, target_key, depth + 1, resolving)
                    .map_err(|failure| failure.at(AssignabilityPathSegment::DictionaryKey(None)))?;
                self.check_assignable_inner(source_value, target_value, depth + 1, resolving)
                    .map_err(|failure| failure.at(AssignabilityPathSegment::DictionaryValue))
            }
            _ => Err(AssignabilityFailure::type_mismatch(source, target)),
        }
    }

    fn is_nominal_subtype(&self, source: &str, target: &str, depth: usize) -> bool {
        if source == target {
            return true;
        }

        if depth >= MAX_ASSIGNABILITY_DEPTH {
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

    fn is_nominal_alias_subtype(&self, source: &str, target: &str, depth: usize) -> bool {
        if source == target {
            return true;
        }

        if depth >= MAX_ASSIGNABILITY_DEPTH {
            return false;
        }

        self.nominal_alias_supertypes
            .get(source)
            .is_some_and(|supertypes| {
                supertypes.iter().any(|supertype| {
                    supertype == target
                        || self.is_nominal_alias_subtype(supertype, target, depth + 1)
                })
            })
    }

    fn is_nominal_alias_type(&self, name: &str) -> bool {
        self.nominal_aliases.contains(name)
    }

    fn check_function_assignable(
        &self,
        source: &FunctionType,
        target: &FunctionType,
        depth: usize,
        resolving: &mut BTreeSet<(bool, String)>,
    ) -> Result<(), Box<AssignabilityFailure>> {
        if source.required_parameter_count > target.required_parameter_count {
            return Err(Box::new(AssignabilityFailure {
                path: Vec::new(),
                expected: Type::Function(target.clone()),
                found: Type::Function(source.clone()),
                kind: AssignabilityFailureKind::FunctionParameterCount {
                    expected: target.required_parameter_count,
                    found: source.required_parameter_count,
                },
            }));
        }

        for (index, target_parameter) in target.parameters.iter().enumerate() {
            let Some(source_parameter) = source
                .parameters
                .get(index)
                .map(|parameter| &parameter.ty)
                .or(source.vararg_type.as_deref())
            else {
                return Err(Box::new(AssignabilityFailure {
                    path: Vec::new(),
                    expected: Type::Function(target.clone()),
                    found: Type::Function(source.clone()),
                    kind: AssignabilityFailureKind::FunctionParameterCount {
                        expected: target.parameters.len(),
                        found: source.parameters.len(),
                    },
                }));
            };

            if self
                .check_assignable_inner(
                    &target_parameter.ty,
                    source_parameter,
                    depth + 1,
                    resolving,
                )
                .is_err()
            {
                return Err(Box::new(AssignabilityFailure {
                    path: vec![AssignabilityPathSegment::FunctionParameter {
                        index,
                        name: target_parameter.name.clone(),
                    }],
                    expected: target_parameter.ty.clone(),
                    found: source_parameter.clone(),
                    kind: AssignabilityFailureKind::TypeMismatch,
                }));
            }
        }

        match (source.vararg_type.as_deref(), target.vararg_type.as_deref()) {
            (_, None) => {}
            (Some(source_vararg), Some(target_vararg)) => {
                if self
                    .check_assignable_inner(target_vararg, source_vararg, depth + 1, resolving)
                    .is_err()
                {
                    return Err(Box::new(AssignabilityFailure {
                        path: vec![AssignabilityPathSegment::FunctionParameter {
                            index: target.parameters.len(),
                            name: Some("...".to_owned()),
                        }],
                        expected: target_vararg.clone(),
                        found: source_vararg.clone(),
                        kind: AssignabilityFailureKind::TypeMismatch,
                    }));
                }
            }
            (None, Some(_)) => {
                return Err(Box::new(AssignabilityFailure {
                    path: Vec::new(),
                    expected: Type::Function(target.clone()),
                    found: Type::Function(source.clone()),
                    kind: AssignabilityFailureKind::FunctionVararg,
                }));
            }
        }

        let source_returns = source.return_types_or_any();
        let target_returns = target.return_types_or_any();
        if source_returns.len() != target_returns.len() {
            return Err(Box::new(AssignabilityFailure {
                path: Vec::new(),
                expected: Type::Function(target.clone()),
                found: Type::Function(source.clone()),
                kind: AssignabilityFailureKind::FunctionReturnCount {
                    expected: target_returns.len(),
                    found: source_returns.len(),
                },
            }));
        }

        for (index, (source, target)) in source_returns.iter().zip(target_returns).enumerate() {
            self.check_assignable_inner(source, target, depth + 1, resolving)
                .map_err(|failure| failure.at(AssignabilityPathSegment::FunctionReturn(index)))?;
        }

        Ok(())
    }

    fn check_table_assignable(
        &self,
        source: &TableShape,
        target: &TableShape,
        depth: usize,
        resolving: &mut BTreeSet<(bool, String)>,
    ) -> Result<(), Box<AssignabilityFailure>> {
        if let Some(target_identity) = target.nominal_identity() {
            return source
                .is_nominal_subtype_of(target_identity)
                .then_some(())
                .ok_or_else(|| {
                    AssignabilityFailure::type_mismatch(
                        &Type::Table(source.clone()),
                        &Type::Table(target.clone()),
                    )
                });
        }

        for (name, target_field) in target.fields() {
            if is_index_signature_field(name) {
                continue;
            }

            let Some(source_field) = source.semantic_field(name) else {
                if target_field.optional {
                    continue;
                }

                return Err(Box::new(AssignabilityFailure {
                    path: vec![AssignabilityPathSegment::Field(name.to_owned())],
                    expected: target_field.ty.clone(),
                    found: Type::nil(),
                    kind: AssignabilityFailureKind::MissingField,
                }));
            };

            if !target_field.optional && source_field.optional {
                return Err(Box::new(AssignabilityFailure {
                    path: vec![AssignabilityPathSegment::Field(name.to_owned())],
                    expected: target_field.ty.clone(),
                    found: source_field.ty.clone(),
                    kind: AssignabilityFailureKind::OptionalField,
                }));
            }

            self.check_assignable_inner(&source_field.ty, &target_field.ty, depth + 1, resolving)
                .map_err(|failure| failure.at(AssignabilityPathSegment::Field(name.to_owned())))?;
        }

        Ok(())
    }

    fn check_table_assignable_to_dictionary(
        &self,
        source: &TableShape,
        target_key: &Type,
        target_value: &Type,
        depth: usize,
        resolving: &mut BTreeSet<(bool, String)>,
    ) -> Result<(), Box<AssignabilityFailure>> {
        for (name, field) in source.fields() {
            if is_index_signature_field(name) {
                return Err(AssignabilityFailure::type_mismatch(
                    &Type::Table(source.clone()),
                    &Type::dictionary(target_key.clone(), target_value.clone()),
                ));
            }

            self.check_assignable_inner(
                &Type::string_literal(name),
                target_key,
                depth + 1,
                resolving,
            )
            .map_err(|failure| {
                failure.at(AssignabilityPathSegment::DictionaryKey(Some(
                    name.to_owned(),
                )))
            })?;
            self.check_assignable_inner(&field.ty, target_value, depth + 1, resolving)
                .map_err(|failure| failure.at(AssignabilityPathSegment::Field(name.to_owned())))?;
        }

        Ok(())
    }
}

fn best_union_failure(
    source: &Type,
    target: &Type,
    failures: Vec<AssignabilityFailure>,
) -> Box<AssignabilityFailure> {
    let Some(first) = failures.first() else {
        return AssignabilityFailure::type_mismatch(source, target);
    };
    let first_path = first.path.clone();
    let first_found = first.found.clone();

    if failures.iter().all(|failure| {
        failure.path == first_path
            && failure.kind == AssignabilityFailureKind::TypeMismatch
            && failure.found == first_found
    }) {
        return Box::new(AssignabilityFailure {
            path: first_path,
            expected: Type::union(failures.into_iter().map(|failure| failure.expected)),
            found: first_found,
            kind: AssignabilityFailureKind::TypeMismatch,
        });
    }

    failures
        .into_iter()
        .max_by_key(|failure| failure.path.len())
        .map(Box::new)
        .unwrap_or_else(|| AssignabilityFailure::type_mismatch(source, target))
}

pub fn is_assignable(source: &Type, target: &Type) -> bool {
    TypeContext::new().is_assignable(source, target)
}

impl Type {
    pub fn is_assignable_to(&self, target: &Type) -> bool {
        is_assignable(self, target)
    }
}

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use super::model::{
    is_index_signature_field, FunctionType, GenericAliasDefinition, GenericAliasReference,
    GenericClassDefinition, TableShape, Type, TypeAlias,
};
use crate::parser::{GenericProtocolBound, TypeAliasId, TypeParameterId};

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
    GenericArgument(usize),
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
    generic_aliases: HashMap<TypeAliasId, GenericAliasDefinition>,
    generic_alias_bindings: HashMap<(std::path::PathBuf, String), TypeAliasId>,
    generic_classes: HashMap<crate::parser::GenericDeclarationId, GenericClassDefinition>,
    nominal_supertypes: BTreeMap<String, BTreeSet<String>>,
    nominal_aliases: BTreeSet<String>,
    nominal_alias_supertypes: BTreeMap<String, BTreeSet<String>>,
    nominal_protocols: BTreeSet<String>,
    protocol_identities: BTreeMap<String, (std::path::PathBuf, String)>,
    protocol_conformance_identities: BTreeSet<(String, std::path::PathBuf, String)>,
    generic_protocol_bounds: HashMap<TypeParameterId, GenericProtocolBound>,
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

    pub(crate) fn insert_generic_alias(&mut self, definition: GenericAliasDefinition) {
        self.generic_aliases
            .insert(definition.id.clone(), definition);
    }

    pub(crate) fn insert_generic_alias_binding(
        &mut self,
        source_module: std::path::PathBuf,
        local_name: String,
        definition: GenericAliasDefinition,
    ) {
        self.generic_alias_bindings.insert(
            (crate::vfs::normalize_path(source_module), local_name),
            definition.id.clone(),
        );
        self.insert_generic_alias(definition);
    }

    pub(crate) fn insert_generic_class_definition(&mut self, definition: GenericClassDefinition) {
        self.generic_classes
            .insert(definition.id.clone(), definition);
    }

    pub(crate) fn generic_class_projection(
        &self,
        ty: &Type,
        target: &crate::parser::GenericDeclarationId,
    ) -> Option<Type> {
        let mut current = ty.clone();
        let mut visited = HashSet::new();
        for _ in 0..MAX_ASSIGNABILITY_DEPTH {
            let Type::GenericClassApplication {
                identity,
                arguments,
                ..
            } = &current
            else {
                return None;
            };
            if identity == target {
                return Some(current);
            }
            if !visited.insert(identity.clone()) {
                return None;
            }
            let definition = self.generic_classes.get(identity)?;
            current = instantiate_class_parent(definition, arguments)?;
        }
        None
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

    pub fn insert_nominal_protocol(&mut self, name: impl Into<String>) {
        let name = name.into();
        self.nominal_aliases.insert(name.clone());
        self.nominal_protocols.insert(name);
    }

    pub(crate) fn insert_protocol_identity(
        &mut self,
        visible_name: impl Into<String>,
        module_path: impl Into<std::path::PathBuf>,
        protocol_name: impl Into<String>,
    ) {
        self.protocol_identities.insert(
            visible_name.into(),
            (module_path.into(), protocol_name.into()),
        );
    }

    pub(crate) fn insert_protocol_conformance_identity(
        &mut self,
        class_identity: impl Into<String>,
        protocol_module: impl Into<std::path::PathBuf>,
        protocol_name: impl Into<String>,
    ) {
        let protocol_module = protocol_module.into();
        self.protocol_conformance_identities.insert((
            class_identity.into(),
            crate::vfs::normalize_path(&protocol_module),
            protocol_name.into(),
        ));
    }

    pub(crate) fn explicitly_conforms_to_protocol(
        &self,
        actual: &Type,
        protocol: &(std::path::PathBuf, String),
    ) -> bool {
        self.explicitly_conforms_to_protocol_inner(actual, protocol, &mut HashSet::new())
    }

    fn explicitly_conforms_to_protocol_inner(
        &self,
        actual: &Type,
        protocol: &(std::path::PathBuf, String),
        resolving_aliases: &mut HashSet<String>,
    ) -> bool {
        match actual {
            Type::Any => true,
            Type::Union(types) => types.iter().all(|ty| {
                self.explicitly_conforms_to_protocol_inner(ty, protocol, resolving_aliases)
            }),
            Type::Table(table) => table.nominal_identity().is_some_and(|identity| {
                self.protocol_conformance_identities.contains(&(
                    identity.to_owned(),
                    crate::vfs::normalize_path(&protocol.0),
                    protocol.1.clone(),
                ))
            }),
            Type::GenericClassApplication { identity, .. } => {
                let class_identity = format!(
                    "{}::class::{}",
                    identity.module.to_string_lossy(),
                    identity.name
                );
                self.protocol_conformance_identities.contains(&(
                    class_identity,
                    crate::vfs::normalize_path(&protocol.0),
                    protocol.1.clone(),
                ))
            }
            Type::GenericParameter { id, .. } => self
                .generic_protocol_bounds
                .get(id)
                .and_then(|bound| bound.identity.as_ref())
                .is_some_and(|bound| {
                    crate::vfs::normalize_path(&bound.0) == crate::vfs::normalize_path(&protocol.0)
                        && bound.1 == protocol.1
                }),
            Type::Alias(name) => {
                if !resolving_aliases.insert(name.clone()) {
                    return false;
                }
                let result = self.aliases.get(name).is_some_and(|ty| {
                    self.explicitly_conforms_to_protocol_inner(ty, protocol, resolving_aliases)
                });
                resolving_aliases.remove(name);
                result
            }
            _ => false,
        }
    }

    pub(crate) fn insert_generic_protocol_bound(
        &mut self,
        parameter: TypeParameterId,
        bound: GenericProtocolBound,
    ) {
        self.generic_protocol_bounds.insert(parameter, bound);
    }

    pub(crate) fn satisfies_protocol_bound(
        &self,
        actual: &Type,
        bound: &GenericProtocolBound,
        bound_owner: &std::path::Path,
    ) -> bool {
        if let Type::GenericParameter { id, .. } = actual {
            return self
                .generic_protocol_bounds
                .get(id)
                .is_some_and(
                    |actual_bound| match (&actual_bound.identity, &bound.identity) {
                        (Some(actual), Some(expected)) => {
                            crate::vfs::normalize_path(&actual.0)
                                == crate::vfs::normalize_path(&expected.0)
                                && actual.1 == expected.1
                        }
                        (None, None) => {
                            actual_bound.name == bound.name && id.declaration.module == bound_owner
                        }
                        _ => false,
                    },
                );
        }
        bound.identity.as_ref().map_or_else(
            || self.is_assignable(actual, &Type::alias(bound.name.clone())),
            |identity| self.explicitly_conforms_to_protocol(actual, identity),
        )
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

        if let Type::GenericAliasApplication {
            reference,
            arguments,
            ..
        } = source
        {
            if let Some(expanded) = self.expand_generic_alias(reference, arguments) {
                return self.check_assignable_inner(&expanded, target, depth + 1, resolving);
            }
        }
        if let Type::GenericAliasApplication {
            reference,
            arguments,
            ..
        } = target
        {
            if let Some(expanded) = self.expand_generic_alias(reference, arguments) {
                return self.check_assignable_inner(source, &expanded, depth + 1, resolving);
            }
        }

        if let Some(failure) = self
            .generic_class_bound_failure(source)
            .or_else(|| self.generic_class_bound_failure(target))
        {
            return Err(failure);
        }

        if let Type::Alias(protocol_name) = target {
            if self.nominal_protocols.contains(protocol_name)
                && self
                    .protocol_identities
                    .get(protocol_name)
                    .is_some_and(|identity| self.explicitly_conforms_to_protocol(source, identity))
            {
                return Ok(());
            }
        }

        match (source, target) {
            (Type::Any, _) | (_, Type::Any) => Ok(()),
            (Type::GenericParameter { id, .. }, Type::Alias(target_name))
                if self
                    .generic_protocol_bounds
                    .get(id)
                    .is_some_and(|bound| bound.name == *target_name) =>
            {
                Ok(())
            }
            (Type::GenericParameter { id, .. }, target) => {
                let Some(bound) = self.generic_protocol_bounds.get(id) else {
                    return Err(AssignabilityFailure::type_mismatch(source, target));
                };
                self.check_assignable_inner(
                    &Type::Alias(bound.name.clone()),
                    target,
                    depth + 1,
                    resolving,
                )
            }
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
            (
                Type::GenericClassApplication {
                    identity: source_identity,
                    arguments: source_arguments,
                    ..
                },
                Type::GenericClassApplication {
                    identity: target_identity,
                    arguments: target_arguments,
                    ..
                },
            ) if source_identity == target_identity
                && source_arguments.len() == target_arguments.len() =>
            {
                for (index, (source_argument, target_argument)) in
                    source_arguments.iter().zip(target_arguments).enumerate()
                {
                    self.check_assignable_inner(
                        source_argument,
                        target_argument,
                        depth + 1,
                        resolving,
                    )
                    .and_then(|()| {
                        self.check_assignable_inner(
                            target_argument,
                            source_argument,
                            depth + 1,
                            resolving,
                        )
                    })
                    .map_err(|failure| {
                        failure.at(AssignabilityPathSegment::GenericArgument(index))
                    })?;
                }
                Ok(())
            }
            (
                source @ Type::GenericClassApplication {
                    identity,
                    arguments,
                    ..
                },
                target @ Type::GenericClassApplication { .. },
            ) => {
                let parent = self
                    .generic_classes
                    .get(identity)
                    .and_then(|definition| instantiate_class_parent(definition, arguments));
                parent
                    .as_ref()
                    .ok_or_else(|| AssignabilityFailure::type_mismatch(source, target))
                    .and_then(|parent| {
                        self.check_assignable_inner(parent, target, depth + 1, resolving)
                    })
            }
            (Type::GenericClassApplication { identity, name, .. }, Type::Alias(target_name))
                if self.generic_classes.contains_key(identity)
                    && self.is_nominal_alias_subtype(name, target_name, depth + 1) =>
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
                // Resolving a nominal alias to its structural shape must not erase
                // the requirement that the source was constructed with that identity.
                if self.is_nominal_alias_type(name) {
                    return match source {
                        Type::Union(sources) => sources.iter().try_for_each(|source| {
                            self.check_assignable_inner(source, target, depth + 1, resolving)
                        }),
                        Type::Table(source_table) => match self.alias(name) {
                            Some(Type::Table(target_table))
                                if target_table.nominal_identity().is_some() =>
                            {
                                self.check_table_assignable(
                                    source_table,
                                    target_table,
                                    depth + 1,
                                    resolving,
                                )
                            }
                            _ => Err(AssignabilityFailure::type_mismatch(source, target)),
                        },
                        _ => Err(AssignabilityFailure::type_mismatch(source, target)),
                    };
                }
                let key = (false, name.clone());
                if !resolving.insert(key.clone()) {
                    return Ok(());
                }
                let resolved = self.alias(name).expect("alias existence was checked");
                let result = self.check_assignable_inner(source, resolved, depth + 1, resolving);
                resolving.remove(&key);
                result
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
            // A plain `{}` requires nothing of the table, which is how LOVE's
            // `table` parameters (vectors, colors, matrices) are modeled.
            (Type::Array(_) | Type::Dictionary { .. } | Type::Tuple(_), Type::Table(target))
                if target.is_plain_empty() =>
            {
                Ok(())
            }
            (Type::Array(source), Type::Array(target)) => {
                if !matches!(source.as_ref(), Type::Any)
                    && source != target
                    && self.contains_protocol_view(target, &mut BTreeSet::new())
                {
                    return Err(AssignabilityFailure::type_mismatch(
                        &Type::Array(source.clone()),
                        &Type::Array(target.clone()),
                    ));
                }

                self.check_assignable_inner(source, target, depth + 1, resolving)
                    .map_err(|failure| failure.at(AssignabilityPathSegment::ArrayElement))
            }
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

    fn generic_class_bound_failure(&self, ty: &Type) -> Option<Box<AssignabilityFailure>> {
        let Type::GenericClassApplication {
            identity,
            arguments,
            ..
        } = ty
        else {
            return None;
        };
        let definition = self.generic_classes.get(identity)?;
        for (index, (parameter, actual)) in definition
            .generic_parameters
            .iter()
            .zip(arguments)
            .enumerate()
        {
            let Some(bound) = &parameter.protocol_bound else {
                continue;
            };
            let conforms =
                self.satisfies_protocol_bound(actual, bound, &parameter.id.declaration.module);
            if !conforms {
                return Some(
                    AssignabilityFailure::type_mismatch(actual, &Type::alias(bound.name.clone()))
                        .at(AssignabilityPathSegment::GenericArgument(index)),
                );
            }
        }
        None
    }

    fn contains_protocol_view(&self, ty: &Type, resolving: &mut BTreeSet<String>) -> bool {
        match ty {
            Type::Alias(name) if self.nominal_protocols.contains(name) => true,
            Type::Alias(name) if resolving.insert(name.clone()) => {
                let result = self
                    .aliases
                    .get(name)
                    .is_some_and(|aliased| self.contains_protocol_view(aliased, resolving));
                resolving.remove(name);
                result
            }
            Type::GenericAliasApplication {
                reference,
                arguments,
                ..
            } => {
                // Mutable arrays cannot safely widen class elements to a
                // protocol view, even when that view is hidden behind a type
                // alias. Bound recursive expansion and fail closed if the
                // application cannot be resolved.
                if resolving.len() >= 64 {
                    return true;
                }
                let key = format!("{reference:?}<{arguments:?}>");
                if !resolving.insert(key.clone()) {
                    return false;
                }
                let contains = self
                    .expand_generic_alias(reference, arguments)
                    .is_none_or(|expanded| self.contains_protocol_view(&expanded, resolving));
                resolving.remove(&key);
                contains
            }
            Type::Union(types) => types
                .iter()
                .any(|ty| self.contains_protocol_view(ty, resolving)),
            _ => false,
        }
    }

    fn expand_generic_alias(
        &self,
        reference: &GenericAliasReference,
        arguments: &[Type],
    ) -> Option<Type> {
        let definition = match reference {
            GenericAliasReference::Declaration(id) => self.generic_aliases.get(id),
            GenericAliasReference::Imported {
                source_module,
                name,
                ..
            } => self
                .generic_alias_bindings
                .get(&(crate::vfs::normalize_path(source_module), name.clone()))
                .and_then(|id| self.generic_aliases.get(id))
                .or_else(|| {
                    self.generic_aliases.values().find(|definition| {
                        definition.id.module == *source_module && definition.id.name == *name
                    })
                }),
        }?;
        if definition.parameters.len() != arguments.len() {
            return None;
        }
        let substitutions = definition
            .parameters
            .iter()
            .cloned()
            .zip(arguments.iter().cloned())
            .collect::<HashMap<_, _>>();
        Some(crate::types::substitute_type_parameters(
            &definition.body,
            &substitutions,
        ))
    }

    pub(crate) fn expand_generic_alias_application(&self, ty: &Type) -> Option<Type> {
        let Type::GenericAliasApplication {
            reference,
            arguments,
            ..
        } = ty
        else {
            return None;
        };
        self.expand_generic_alias(reference, arguments)
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
                // Lua callbacks may ignore trailing arguments. Required source
                // arity was checked above; only consumed arguments need variance checks.
                continue;
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

            self.check_assignable_inner(
                &source_field.ty,
                &target_field.value_type(),
                depth + 1,
                resolving,
            )
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

fn instantiate_class_parent(
    definition: &GenericClassDefinition,
    arguments: &[Type],
) -> Option<Type> {
    if definition.parameters.len() != arguments.len() {
        return None;
    }
    let parent = definition.parent.as_ref()?;
    let substitutions = definition
        .parameters
        .iter()
        .cloned()
        .zip(arguments.iter().cloned())
        .collect::<HashMap<_, _>>();
    Some(crate::types::substitute_type_parameters(
        parent,
        &substitutions,
    ))
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

use std::collections::{HashMap, HashSet};

use crate::parser::TypeParameterId;
use crate::types::substitute_type_parameters;

use super::{FunctionParameter, FunctionType, Type, TypeContext};

const MAX_GENERIC_CONSTRAINT_DEPTH: usize = 64;
pub(crate) const MAX_GENERIC_CALL_INSTANTIATIONS_PER_FILE: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GenericInferenceFailure {
    pub parameter: String,
    pub kind: GenericInferenceFailureKind,
    pub argument_index: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum GenericInferenceFailureKind {
    ReturnOnly,
    Callback,
    Unresolved,
    WorkLimit,
    Conflict {
        previous: Box<Type>,
        next: Box<Type>,
    },
    ProtocolBound {
        actual: Box<Type>,
        protocol: String,
    },
}

impl GenericInferenceFailure {
    pub(crate) fn message(&self) -> String {
        match &self.kind {
            GenericInferenceFailureKind::ReturnOnly => format!(
                "cannot infer generic type parameter `{}` from the return type alone",
                self.parameter
            ),
            GenericInferenceFailureKind::Callback => format!(
                "cannot infer generic type parameter `{}` from a callback argument; pass a typed value argument or add an explicit callback type",
                self.parameter
            ),
            GenericInferenceFailureKind::Unresolved => format!(
                "cannot infer generic type parameter `{}` from the supplied arguments",
                self.parameter
            ),
            GenericInferenceFailureKind::WorkLimit => format!(
                "generic inference stopped after {} distinct call instantiations in this source file; split generated call-heavy code across files or simplify the generic signatures",
                MAX_GENERIC_CALL_INSTANTIATIONS_PER_FILE
            ),
            GenericInferenceFailureKind::Conflict { previous, next } => format!(
                "conflicting arguments for generic type parameter `{}`: inferred both `{previous}` and `{next}`",
                self.parameter
            ),
            GenericInferenceFailureKind::ProtocolBound { actual, protocol } => format!(
                "generic type parameter `{}` requires explicit conformance to protocol `{protocol}`, but inferred `{actual}` does not conform",
                self.parameter
            ),
        }
    }
}

/// Instantiate a generic function scheme for one call. The source scheme is
/// immutable; each invocation receives a fresh substitution map and result.
/// Callback inference is deliberately not included in this first-order solver.
pub(crate) fn instantiate_generic_function(
    function: &FunctionType,
    arguments: &[Option<Type>],
    context: &TypeContext,
) -> Result<FunctionType, GenericInferenceFailure> {
    if function.generic_parameters.is_empty() {
        return Ok(function.clone());
    }

    let binders = function
        .generic_parameters
        .iter()
        .map(|parameter| parameter.id.clone())
        .collect::<HashSet<_>>();
    let mut invariant_binders = HashSet::new();
    for parameter in function
        .parameters
        .iter()
        .map(|parameter| &parameter.ty)
        .chain(function.vararg_type.as_deref())
    {
        collect_invariant_parameters(context, parameter, &binders, &mut invariant_binders, 0);
    }
    let mut substitutions = HashMap::new();
    let mut substitution_arguments = HashMap::new();
    let mut callback_binders = HashSet::new();
    let mut callback_argument_indices = HashMap::new();
    let mut incompatible_callback_binders = HashSet::new();

    for (index, actual) in arguments.iter().enumerate() {
        let Some(actual) = actual.as_ref() else {
            continue;
        };
        let Some(parameter_type) = function
            .parameters
            .get(index)
            .map(|parameter| &parameter.ty)
            .or(function.vararg_type.as_deref())
        else {
            continue;
        };
        let bound_before = substitutions.keys().cloned().collect::<HashSet<_>>();
        if matches!(parameter_type, Type::Function(_)) {
            let found = if matches!(actual, Type::Function(_)) {
                &mut callback_binders
            } else {
                &mut incompatible_callback_binders
            };
            collect_generic_parameters(parameter_type, &binders, found, 0);
            if matches!(actual, Type::Function(_)) {
                for id in found.iter() {
                    callback_argument_indices.entry(id.clone()).or_insert(index);
                }
            }
            continue;
        }
        if let Err(mut failure) = collect_constraints(
            context,
            parameter_type,
            actual,
            &binders,
            &invariant_binders,
            &mut substitutions,
            0,
        ) {
            failure.argument_index = Some(index);
            return Err(failure);
        }
        for id in substitutions.keys() {
            if !bound_before.contains(id) {
                substitution_arguments.entry(id.clone()).or_insert(index);
            }
        }
    }

    for parameter in &function.generic_parameters {
        let used_in_parameters = function
            .parameters
            .iter()
            .any(|parameter_ty| contains_parameter(&parameter_ty.ty, &parameter.id, 0))
            || function
                .vararg_type
                .as_deref()
                .is_some_and(|ty| contains_parameter(ty, &parameter.id, 0));
        let used_in_returns = function
            .return_types_or_any()
            .iter()
            .any(|ty| contains_parameter(ty, &parameter.id, 0));
        if !(used_in_parameters || used_in_returns)
            || substitutions.contains_key(&parameter.id)
            || incompatible_callback_binders.contains(&parameter.id)
        {
            continue;
        }
        let kind = if callback_binders.contains(&parameter.id) {
            GenericInferenceFailureKind::Callback
        } else if !used_in_parameters && used_in_returns {
            GenericInferenceFailureKind::ReturnOnly
        } else {
            GenericInferenceFailureKind::Unresolved
        };
        return Err(GenericInferenceFailure {
            parameter: parameter.name.clone(),
            kind,
            argument_index: callback_argument_indices.get(&parameter.id).copied(),
        });
    }

    for parameter in &function.generic_parameters {
        let Some(bound) = &parameter.protocol_bound else {
            continue;
        };
        let Some(actual) = substitutions.get(&parameter.id) else {
            continue;
        };
        let conforms =
            context.satisfies_protocol_bound(actual, bound, &parameter.id.declaration.module);
        if !conforms {
            return Err(GenericInferenceFailure {
                parameter: parameter.name.clone(),
                kind: GenericInferenceFailureKind::ProtocolBound {
                    actual: Box::new(actual.clone()),
                    protocol: bound.name.clone(),
                },
                argument_index: substitution_arguments.get(&parameter.id).copied(),
            });
        }
    }

    let substitute = |ty: &Type| substitute_type_parameters(ty, &substitutions);
    let mut instantiated = FunctionType::new_signature(
        function
            .parameters
            .iter()
            .map(|parameter| FunctionParameter {
                name: parameter.name.clone(),
                ty: substitute(&parameter.ty),
            }),
        function.required_parameter_count,
        function.vararg_type.as_deref().map(substitute),
        function.return_types_or_any().iter().map(substitute),
    )
    .with_generic_parameters(
        function
            .generic_parameters
            .iter()
            .filter(|parameter| incompatible_callback_binders.contains(&parameter.id))
            .cloned(),
    )
    .with_guarded_return_types(
        function
            .guarded_return_types
            .as_ref()
            .map(|types| types.iter().map(substitute).collect()),
    );
    instantiated.overloads = function
        .overloads
        .iter()
        .map(|overload| {
            instantiate_generic_function(overload, arguments, context)
                .unwrap_or_else(|_| overload.clone())
        })
        .collect();
    Ok(instantiated)
}

fn collect_constraints(
    context: &TypeContext,
    pattern: &Type,
    actual: &Type,
    binders: &HashSet<TypeParameterId>,
    invariant_binders: &HashSet<TypeParameterId>,
    substitutions: &mut HashMap<TypeParameterId, Type>,
    depth: usize,
) -> Result<(), GenericInferenceFailure> {
    if depth >= MAX_GENERIC_CONSTRAINT_DEPTH {
        return Ok(());
    }
    if let Some(expanded) = context.expand_generic_alias_application(pattern) {
        return collect_constraints(
            context,
            &expanded,
            actual,
            binders,
            invariant_binders,
            substitutions,
            depth + 1,
        );
    }
    if let Some(expanded) = context.expand_generic_alias_application(actual) {
        return collect_constraints(
            context,
            pattern,
            &expanded,
            binders,
            invariant_binders,
            substitutions,
            depth + 1,
        );
    }
    match (pattern, actual) {
        (Type::GenericParameter { id, name }, actual) if binders.contains(id) => {
            if let Some(previous) = substitutions.get(id) {
                if context.is_assignable(actual, previous)
                    && context.is_assignable(previous, actual)
                {
                    return Ok(());
                }
                if !invariant_binders.contains(id) && context.is_assignable(previous, actual) {
                    substitutions.insert(id.clone(), actual.clone());
                    return Ok(());
                }
                if !invariant_binders.contains(id) && context.is_assignable(actual, previous) {
                    return Ok(());
                }
                return Err(GenericInferenceFailure {
                    parameter: name.clone(),
                    kind: GenericInferenceFailureKind::Conflict {
                        previous: Box::new(previous.clone()),
                        next: Box::new(actual.clone()),
                    },
                    argument_index: None,
                });
            }
            substitutions.insert(id.clone(), actual.clone());
        }
        (Type::Array(pattern), Type::Array(actual)) => collect_constraints(
            context,
            pattern,
            actual,
            binders,
            invariant_binders,
            substitutions,
            depth + 1,
        )?,
        (Type::Tuple(patterns), Type::Tuple(actuals)) if patterns.len() == actuals.len() => {
            for (pattern, actual) in patterns.iter().zip(actuals) {
                collect_constraints(
                    context,
                    pattern,
                    actual,
                    binders,
                    invariant_binders,
                    substitutions,
                    depth + 1,
                )?;
            }
        }
        (
            Type::Dictionary {
                key: pattern_key,
                value: pattern_value,
            },
            Type::Dictionary {
                key: actual_key,
                value: actual_value,
            },
        ) => {
            collect_constraints(
                context,
                pattern_key,
                actual_key,
                binders,
                invariant_binders,
                substitutions,
                depth + 1,
            )?;
            collect_constraints(
                context,
                pattern_value,
                actual_value,
                binders,
                invariant_binders,
                substitutions,
                depth + 1,
            )?;
        }
        (Type::Table(pattern), Type::Table(actual)) => {
            for (name, field) in pattern.fields() {
                if let Some(actual_field) = actual.field(name) {
                    collect_constraints(
                        context,
                        &field.ty,
                        &actual_field.ty,
                        binders,
                        invariant_binders,
                        substitutions,
                        depth + 1,
                    )?;
                }
            }
        }
        (Type::Union(patterns), actual) => {
            let actuals = match actual {
                Type::Union(actuals) => actuals.as_slice(),
                other => std::slice::from_ref(other),
            };
            for actual in actuals {
                // Prefer an exact concrete arm such as `nil` before a generic
                // arm such as `T`; otherwise `T | nil` can infer T = nil from
                // the nil member of `number | nil`.
                let concrete = (!matches!(actual, Type::Any))
                    .then(|| {
                        patterns.iter().find(|pattern| {
                            !contains_any_parameter(pattern, binders, depth + 1)
                                && context.is_assignable(actual, pattern)
                        })
                    })
                    .flatten();
                let pattern = concrete.or_else(|| {
                    patterns
                        .iter()
                        .find(|pattern| contains_any_parameter(pattern, binders, depth + 1))
                });
                if let Some(pattern) = pattern {
                    collect_constraints(
                        context,
                        pattern,
                        actual,
                        binders,
                        invariant_binders,
                        substitutions,
                        depth + 1,
                    )?;
                }
            }
        }
        (
            Type::GenericAliasApplication {
                reference: pattern_ref,
                arguments: patterns,
                ..
            },
            Type::GenericAliasApplication {
                reference: actual_ref,
                arguments: actuals,
                ..
            },
        ) if pattern_ref == actual_ref && patterns.len() == actuals.len() => {
            for (pattern, actual) in patterns.iter().zip(actuals) {
                collect_constraints(
                    context,
                    pattern,
                    actual,
                    binders,
                    invariant_binders,
                    substitutions,
                    depth + 1,
                )?;
            }
        }
        (
            Type::GenericClassApplication {
                identity: pattern_identity,
                arguments: patterns,
                ..
            },
            Type::GenericClassApplication {
                identity: actual_identity,
                arguments: actuals,
                ..
            },
        ) if pattern_identity == actual_identity && patterns.len() == actuals.len() => {
            for (pattern, actual) in patterns.iter().zip(actuals) {
                collect_constraints(
                    context,
                    pattern,
                    actual,
                    binders,
                    invariant_binders,
                    substitutions,
                    depth + 1,
                )?;
            }
        }
        (
            pattern @ Type::GenericClassApplication {
                identity: pattern_identity,
                ..
            },
            actual @ Type::GenericClassApplication { .. },
        ) => {
            if let Some(projected) = context.generic_class_projection(actual, pattern_identity) {
                collect_constraints(
                    context,
                    pattern,
                    &projected,
                    binders,
                    invariant_binders,
                    substitutions,
                    depth + 1,
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Find binders whose values flow through a mutable container in the formal
/// signature. A value used anywhere in an array, dictionary, or record field
/// must not be widened to a common supertype at another argument position:
/// the generic body can write that widened value back into the caller's
/// narrower container.
fn collect_invariant_parameters(
    context: &TypeContext,
    ty: &Type,
    binders: &HashSet<TypeParameterId>,
    found: &mut HashSet<TypeParameterId>,
    depth: usize,
) {
    if depth >= MAX_GENERIC_CONSTRAINT_DEPTH {
        return;
    }
    if let Some(expanded) = context.expand_generic_alias_application(ty) {
        collect_invariant_parameters(context, &expanded, binders, found, depth + 1);
        return;
    }
    match ty {
        Type::Array(item) => collect_generic_parameters(item, binders, found, depth + 1),
        Type::Dictionary { key, value } => {
            collect_generic_parameters(key, binders, found, depth + 1);
            collect_generic_parameters(value, binders, found, depth + 1);
        }
        Type::Table(table) => {
            for (_, field) in table.fields() {
                collect_generic_parameters(&field.ty, binders, found, depth + 1);
            }
        }
        Type::Union(types) | Type::Tuple(types) => {
            for ty in types {
                collect_invariant_parameters(context, ty, binders, found, depth + 1);
            }
        }
        Type::Function(function) => {
            for parameter in &function.parameters {
                collect_invariant_parameters(context, &parameter.ty, binders, found, depth + 1);
            }
            for ty in function.return_types_or_any() {
                collect_invariant_parameters(context, ty, binders, found, depth + 1);
            }
        }
        Type::GenericAliasApplication { arguments, .. } => {
            // A recursive or otherwise non-expandable alias may still hide a
            // mutable occurrence. Be conservative rather than permit
            // supertype widening through that alias.
            for argument in arguments {
                collect_invariant_parameters(context, argument, binders, found, depth + 1);
            }
        }
        Type::GenericClassApplication { arguments, .. } => {
            collect_generic_parameters_from_class_arguments(arguments, binders, found, depth + 1);
        }
        Type::GenericParameter { .. }
        | Type::Alias(_)
        | Type::Primitive(_)
        | Type::Any
        | Type::StringLiteral(_) => {}
    }
}

fn contains_any_parameter(ty: &Type, binders: &HashSet<TypeParameterId>, depth: usize) -> bool {
    if depth >= MAX_GENERIC_CONSTRAINT_DEPTH {
        return false;
    }
    match ty {
        Type::GenericParameter { id, .. } => binders.contains(id),
        Type::Union(types) | Type::Tuple(types) => types
            .iter()
            .any(|ty| contains_any_parameter(ty, binders, depth + 1)),
        Type::Function(function) => {
            function
                .parameters
                .iter()
                .any(|parameter| contains_any_parameter(&parameter.ty, binders, depth + 1))
                || function
                    .return_types_or_any()
                    .iter()
                    .any(|ty| contains_any_parameter(ty, binders, depth + 1))
        }
        Type::Table(table) => table
            .fields()
            .any(|(_, field)| contains_any_parameter(&field.ty, binders, depth + 1)),
        Type::Array(item) => contains_any_parameter(item, binders, depth + 1),
        Type::Dictionary { key, value } => {
            contains_any_parameter(key, binders, depth + 1)
                || contains_any_parameter(value, binders, depth + 1)
        }
        Type::GenericAliasApplication { arguments, .. } => arguments
            .iter()
            .any(|ty| contains_any_parameter(ty, binders, depth + 1)),
        Type::GenericClassApplication { arguments, .. } => arguments
            .iter()
            .any(|ty| contains_any_parameter(ty, binders, depth + 1)),
        Type::Alias(_) | Type::Primitive(_) | Type::Any | Type::StringLiteral(_) => false,
    }
}

fn contains_parameter(ty: &Type, parameter: &TypeParameterId, depth: usize) -> bool {
    if depth >= MAX_GENERIC_CONSTRAINT_DEPTH {
        // Constraint collection stops at this depth, but deciding that the
        // binder is absent would mislabel a deeply nested input use as a
        // return-only parameter. Finish this presence check iteratively so the
        // Rust call stack stays bounded while the caller reports an unresolved
        // inference instead of a misleading diagnostic.
        return contains_parameter_iteratively(ty, parameter);
    }
    match ty {
        Type::GenericParameter { id, .. } => id == parameter,
        Type::Union(types) | Type::Tuple(types) => types
            .iter()
            .any(|ty| contains_parameter(ty, parameter, depth + 1)),
        Type::Function(function) => {
            function.parameters.iter().any(|function_parameter| {
                contains_parameter(&function_parameter.ty, parameter, depth + 1)
            }) || function
                .return_types_or_any()
                .iter()
                .any(|ty| contains_parameter(ty, parameter, depth + 1))
        }
        Type::Table(table) => table
            .fields()
            .any(|(_, field)| contains_parameter(&field.ty, parameter, depth + 1)),
        Type::Array(item) => contains_parameter(item, parameter, depth + 1),
        Type::Dictionary { key, value } => {
            contains_parameter(key, parameter, depth + 1)
                || contains_parameter(value, parameter, depth + 1)
        }
        Type::GenericAliasApplication { arguments, .. } => arguments
            .iter()
            .any(|ty| contains_parameter(ty, parameter, depth + 1)),
        Type::GenericClassApplication { arguments, .. } => arguments
            .iter()
            .any(|ty| contains_parameter(ty, parameter, depth + 1)),
        Type::Alias(_) | Type::Primitive(_) | Type::Any | Type::StringLiteral(_) => false,
    }
}

fn contains_parameter_iteratively(root: &Type, parameter: &TypeParameterId) -> bool {
    let mut pending = vec![root];
    while let Some(ty) = pending.pop() {
        match ty {
            Type::GenericParameter { id, .. } if id == parameter => return true,
            Type::Union(types) | Type::Tuple(types) => pending.extend(types),
            Type::Function(function) => {
                pending.extend(function.parameters.iter().map(|parameter| &parameter.ty));
                pending.extend(function.vararg_type.iter().map(Box::as_ref));
                pending.extend(function.return_types_or_any());
            }
            Type::Table(table) => pending.extend(table.fields().map(|(_, field)| &field.ty)),
            Type::Array(item) => pending.push(item),
            Type::Dictionary { key, value } => {
                pending.push(key);
                pending.push(value);
            }
            Type::GenericAliasApplication { arguments, .. }
            | Type::GenericClassApplication { arguments, .. } => pending.extend(arguments),
            Type::GenericParameter { .. }
            | Type::Alias(_)
            | Type::Primitive(_)
            | Type::Any
            | Type::StringLiteral(_) => {}
        }
    }
    false
}

fn collect_generic_parameters(
    ty: &Type,
    binders: &HashSet<TypeParameterId>,
    found: &mut HashSet<TypeParameterId>,
    depth: usize,
) {
    if depth >= MAX_GENERIC_CONSTRAINT_DEPTH {
        return;
    }
    match ty {
        Type::GenericParameter { id, .. } if binders.contains(id) => {
            found.insert(id.clone());
        }
        Type::Union(types) | Type::Tuple(types) => {
            for ty in types {
                collect_generic_parameters(ty, binders, found, depth + 1);
            }
        }
        Type::Function(function) => {
            for parameter in &function.parameters {
                collect_generic_parameters(&parameter.ty, binders, found, depth + 1);
            }
            for ty in function.return_types_or_any() {
                collect_generic_parameters(ty, binders, found, depth + 1);
            }
        }
        Type::Table(table) => {
            for (_, field) in table.fields() {
                collect_generic_parameters(&field.ty, binders, found, depth + 1);
            }
        }
        Type::Array(item) => collect_generic_parameters(item, binders, found, depth + 1),
        Type::Dictionary { key, value } => {
            collect_generic_parameters(key, binders, found, depth + 1);
            collect_generic_parameters(value, binders, found, depth + 1);
        }
        Type::GenericAliasApplication { arguments, .. } => {
            for ty in arguments {
                collect_generic_parameters(ty, binders, found, depth + 1);
            }
        }
        Type::GenericClassApplication { arguments, .. } => {
            collect_generic_parameters_from_class_arguments(arguments, binders, found, depth + 1);
        }
        Type::GenericParameter { .. }
        | Type::Alias(_)
        | Type::Primitive(_)
        | Type::Any
        | Type::StringLiteral(_) => {}
    }
}

fn collect_generic_parameters_from_class_arguments(
    arguments: &[Type],
    binders: &HashSet<TypeParameterId>,
    found: &mut HashSet<TypeParameterId>,
    depth: usize,
) {
    for argument in arguments {
        collect_generic_parameters(argument, binders, found, depth + 1);
    }
}

/// Specialize a generic function value when its complete function type is
/// known from a callback parameter. Unlike call inference, both parameter and
/// return positions can constrain the declaration's binders here.
pub(crate) fn instantiate_generic_function_from_context(
    function: &FunctionType,
    expected: &FunctionType,
    context: &TypeContext,
) -> Result<FunctionType, GenericInferenceFailure> {
    if function.generic_parameters.is_empty() {
        return Ok(function.clone());
    }

    let binders = function
        .generic_parameters
        .iter()
        .map(|parameter| parameter.id.clone())
        .collect::<HashSet<_>>();
    let mut invariant_binders = HashSet::new();
    for parameter in function
        .parameters
        .iter()
        .map(|parameter| &parameter.ty)
        .chain(function.vararg_type.as_deref())
    {
        collect_invariant_parameters(context, parameter, &binders, &mut invariant_binders, 0);
    }

    let mut substitutions = HashMap::new();
    for (parameter, actual) in function.parameters.iter().zip(&expected.parameters) {
        collect_constraints(
            context,
            &parameter.ty,
            &actual.ty,
            &binders,
            &invariant_binders,
            &mut substitutions,
            0,
        )?;
    }
    if let (Some(pattern), Some(actual)) = (&function.vararg_type, &expected.vararg_type) {
        collect_constraints(
            context,
            pattern,
            actual,
            &binders,
            &invariant_binders,
            &mut substitutions,
            0,
        )?;
    }
    for (pattern, actual) in function
        .return_types_or_any()
        .iter()
        .zip(expected.return_types_or_any())
    {
        collect_constraints(
            context,
            pattern,
            actual,
            &binders,
            &invariant_binders,
            &mut substitutions,
            0,
        )?;
    }

    for parameter in &function.generic_parameters {
        if !substitutions.contains_key(&parameter.id) {
            let used_in_parameters = function
                .parameters
                .iter()
                .any(|candidate| contains_parameter(&candidate.ty, &parameter.id, 0))
                || function
                    .vararg_type
                    .as_deref()
                    .is_some_and(|ty| contains_parameter(ty, &parameter.id, 0));
            let kind = if !used_in_parameters
                && function
                    .return_types_or_any()
                    .iter()
                    .any(|ty| contains_parameter(ty, &parameter.id, 0))
            {
                GenericInferenceFailureKind::ReturnOnly
            } else {
                GenericInferenceFailureKind::Unresolved
            };
            return Err(GenericInferenceFailure {
                parameter: parameter.name.clone(),
                kind,
                argument_index: None,
            });
        }
    }

    for parameter in &function.generic_parameters {
        let Some(bound) = &parameter.protocol_bound else {
            continue;
        };
        let Some(actual) = substitutions.get(&parameter.id) else {
            continue;
        };
        if !context.satisfies_protocol_bound(actual, bound, &parameter.id.declaration.module) {
            return Err(GenericInferenceFailure {
                parameter: parameter.name.clone(),
                kind: GenericInferenceFailureKind::ProtocolBound {
                    actual: Box::new(actual.clone()),
                    protocol: bound.name.clone(),
                },
                argument_index: None,
            });
        }
    }

    let substitute = |ty: &Type| substitute_type_parameters(ty, &substitutions);
    let mut instantiated = FunctionType::new_signature(
        function
            .parameters
            .iter()
            .map(|parameter| FunctionParameter {
                name: parameter.name.clone(),
                ty: substitute(&parameter.ty),
            }),
        function.required_parameter_count,
        function.vararg_type.as_deref().map(substitute),
        function.return_types_or_any().iter().map(substitute),
    )
    .with_guarded_return_types(
        function
            .guarded_return_types
            .as_ref()
            .map(|types| types.iter().map(substitute).collect()),
    );
    instantiated.overloads = function
        .overloads
        .iter()
        .map(|overload| {
            instantiate_generic_function_from_context(overload, expected, context)
                .unwrap_or_else(|_| overload.clone())
        })
        .collect();
    Ok(instantiated)
}

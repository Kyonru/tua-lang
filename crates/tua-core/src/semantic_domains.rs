use full_moon::ast::FunctionCall;

use crate::types::{FunctionType, Type, TypeEnvironment, TypedFunction};
use crate::{
    ecs, input, object_pool, signal, sprite, state_machine, store, vector, Diagnostic, ParsedSource,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BuiltinDomain {
    Vector,
    Sprite,
    Signal,
    ObjectPool,
    StateMachine,
    Store,
    Input,
    Ecs,
}

const DEFAULT_DOMAINS: &[BuiltinDomain] = &[
    BuiltinDomain::Vector,
    BuiltinDomain::Sprite,
    BuiltinDomain::Signal,
    BuiltinDomain::ObjectPool,
    BuiltinDomain::StateMachine,
    BuiltinDomain::Store,
    BuiltinDomain::Input,
];

const MEMBER_DOMAINS: &[BuiltinDomain] = &[
    BuiltinDomain::Ecs,
    BuiltinDomain::Signal,
    BuiltinDomain::ObjectPool,
    BuiltinDomain::StateMachine,
    BuiltinDomain::Store,
    BuiltinDomain::Input,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BuiltinDomainRegistry {
    ecs_enabled: bool,
}

impl BuiltinDomainRegistry {
    pub(crate) const fn standard() -> Self {
        Self { ecs_enabled: false }
    }

    pub(crate) const fn with_ecs(ecs_enabled: bool) -> Self {
        Self { ecs_enabled }
    }

    pub(crate) fn domains(self) -> impl Iterator<Item = BuiltinDomain> {
        DEFAULT_DOMAINS
            .iter()
            .copied()
            .chain(self.ecs_enabled.then_some(BuiltinDomain::Ecs))
    }

    pub(crate) fn builtin_type_names(self) -> Vec<&'static str> {
        self.domains()
            .flat_map(BuiltinDomain::builtin_type_names)
            .collect()
    }

    pub(crate) fn builtin_aliases(self) -> Vec<(String, Type)> {
        self.domains()
            .flat_map(BuiltinDomain::builtin_aliases)
            .collect()
    }

    pub(crate) fn builtin_locals(self) -> Vec<(String, Type)> {
        if self.ecs_enabled {
            ecs::builtin_locals()
        } else {
            Vec::new()
        }
    }

    pub(crate) fn constructor_domain(self, name: &str) -> Option<BuiltinDomain> {
        self.domains().find(|domain| domain.is_constructor(name))
    }

    pub(crate) fn constructor(self, name: &str) -> Option<BuiltinConstructor> {
        let domain = self.constructor_domain(name)?;
        domain.constructor()
    }

    pub(crate) fn typed_member(
        self,
        environment: &TypeEnvironment,
        receiver_type: &Type,
        member: &str,
    ) -> Option<TypedFunction> {
        MEMBER_DOMAINS
            .iter()
            .find_map(|domain| domain.typed_member(environment, receiver_type, member))
    }

    pub(crate) fn method_return_types_from_direct_type(
        self,
        receiver_type: &Type,
        call: &FunctionCall,
    ) -> Option<Vec<Type>> {
        self.domains()
            .find_map(|domain| domain.method_return_types_from_direct_type(receiver_type, call))
    }

    pub(crate) fn diagnostics(
        self,
        parsed: &ParsedSource,
        environment: &TypeEnvironment,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = signal::event_schema_diagnostics(parsed, environment);

        if environment.local(input::CONSTRUCTOR_NAME).is_none()
            && environment.function(input::CONSTRUCTOR_NAME).is_none()
        {
            diagnostics.extend(input::config_diagnostics(parsed));
        }

        diagnostics
    }
}

#[derive(Clone, Debug)]
pub(crate) enum BuiltinConstructor {
    Function(FunctionType),
    Typed(Box<TypedFunction>),
}

impl BuiltinConstructor {
    pub(crate) fn function_type(&self) -> FunctionType {
        match self {
            Self::Function(function) => function.clone(),
            Self::Typed(function) => function
                .as_type()
                .and_then(|ty| match ty {
                    Type::Function(function) => Some(function),
                    _ => None,
                })
                .expect("built-in constructor signatures must be fully typed"),
        }
    }
}

impl BuiltinDomain {
    pub(crate) fn builtin_type_names(self) -> impl Iterator<Item = &'static str> {
        let names = match self {
            Self::Vector => vector::builtin_type_names(),
            Self::Sprite => sprite::builtin_type_names(),
            Self::Signal => signal::builtin_type_names(),
            Self::ObjectPool => object_pool::builtin_type_names(),
            Self::StateMachine => state_machine::builtin_type_names(),
            Self::Store => store::builtin_type_names(),
            Self::Input => input::builtin_type_names(),
            Self::Ecs => ecs::builtin_type_names(),
        };

        names.iter().copied()
    }

    pub(crate) fn builtin_aliases(self) -> Vec<(String, Type)> {
        match self {
            Self::Vector => vec![(vector::TYPE_NAME.to_owned(), vector::vec2_type())],
            Self::Sprite => sprite::builtin_aliases(),
            Self::Signal => vec![(signal::TYPE_NAME.to_owned(), signal::signal_bus_type())],
            Self::ObjectPool => object_pool::builtin_aliases(),
            Self::StateMachine => state_machine::builtin_aliases(),
            Self::Store => store::builtin_aliases(),
            Self::Input => input::builtin_aliases(),
            Self::Ecs => ecs::builtin_aliases(),
        }
    }

    pub(crate) fn is_constructor(self, name: &str) -> bool {
        match self {
            Self::Vector => vector::is_constructor(name),
            Self::Sprite => sprite::is_helper(name),
            Self::Signal => signal::is_constructor(name),
            Self::ObjectPool => object_pool::is_constructor(name),
            Self::StateMachine => state_machine::is_constructor(name),
            Self::Store => store::is_constructor(name),
            Self::Input => input::is_constructor(name),
            Self::Ecs => false,
        }
    }

    pub(crate) const fn preserves_literal_unions(self) -> bool {
        matches!(self, Self::StateMachine | Self::Input)
    }

    pub(crate) fn inferred_constructor_return_type(
        self,
        call: &FunctionCall,
        first_argument_type: Option<&Type>,
    ) -> Option<Type> {
        match self {
            Self::StateMachine => state_machine::machine_type_from_call(call)
                .or_else(|| Some(Type::alias(state_machine::TYPE_NAME))),
            Self::Input => {
                input::input_type_from_call(call).or_else(|| Some(Type::alias(input::TYPE_NAME)))
            }
            Self::ObjectPool => Some(
                first_argument_type
                    .map(object_pool::pool_type_from_options)
                    .unwrap_or_else(|| Type::alias(object_pool::TYPE_NAME)),
            ),
            Self::Store => Some(
                first_argument_type
                    .map(store::store_type_from_initial)
                    .unwrap_or_else(|| Type::alias(store::TYPE_NAME)),
            ),
            Self::Vector | Self::Sprite | Self::Signal | Self::Ecs => None,
        }
    }

    fn constructor(self) -> Option<BuiltinConstructor> {
        match self {
            Self::Vector => Some(BuiltinConstructor::Function(vector::constructor_type())),
            Self::Sprite => Some(BuiltinConstructor::Function(sprite::constructor_type())),
            Self::Signal => Some(BuiltinConstructor::Function(signal::constructor_type())),
            Self::ObjectPool => Some(BuiltinConstructor::Typed(Box::new(
                object_pool::constructor_typed_function(),
            ))),
            Self::StateMachine => Some(BuiltinConstructor::Typed(Box::new(
                state_machine::constructor_typed_function(),
            ))),
            Self::Store => Some(BuiltinConstructor::Typed(Box::new(
                store::constructor_typed_function(),
            ))),
            Self::Input => Some(BuiltinConstructor::Typed(Box::new(
                input::constructor_typed_function(),
            ))),
            Self::Ecs => None,
        }
    }

    fn typed_member(
        self,
        environment: &TypeEnvironment,
        receiver_type: &Type,
        member: &str,
    ) -> Option<TypedFunction> {
        match self {
            Self::Ecs => ecs::typed_member(environment, receiver_type, member),
            Self::Signal if signal::type_is_signal_bus(environment, receiver_type) => {
                signal::typed_method(member)
            }
            Self::ObjectPool => object_pool::typed_member(environment, receiver_type, member),
            Self::StateMachine => state_machine::typed_member(environment, receiver_type, member),
            Self::Store => store::typed_member(environment, receiver_type, member),
            Self::Input => input::typed_member(environment, receiver_type, member),
            Self::Vector | Self::Sprite | Self::Signal => None,
        }
    }

    fn method_return_types_from_direct_type(
        self,
        receiver_type: &Type,
        call: &FunctionCall,
    ) -> Option<Vec<Type>> {
        match self {
            Self::Input => input::method_return_types_from_direct_type(receiver_type, call),
            Self::Vector
            | Self::Sprite
            | Self::Signal
            | Self::ObjectPool
            | Self::StateMachine
            | Self::Store
            | Self::Ecs => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_keeps_ecs_explicit_and_constructor_order_stable() {
        let standard = BuiltinDomainRegistry::standard();
        let with_ecs = BuiltinDomainRegistry::with_ecs(true);

        assert!(!standard.builtin_type_names().contains(&ecs::WORLD_TYPE));
        assert!(with_ecs.builtin_type_names().contains(&ecs::WORLD_TYPE));
        assert_eq!(
            standard.constructor_domain(state_machine::CONSTRUCTOR_NAME),
            Some(BuiltinDomain::StateMachine)
        );
    }
}

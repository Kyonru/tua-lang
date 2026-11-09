use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use full_moon::ast::{
    self, Call, Expression, Field, FunctionArgs, FunctionCall, Index, LastStmt, Prefix, Suffix, Var,
};
use full_moon::node::Node;

use crate::ast_utils::{
    assignment_expression_for_local, field_chain_before_first_call, plain_field_chain_from_var,
    plain_field_chain_from_var_expression,
};
use crate::classes::nominal_class_identity;
use crate::deprecation::Deprecation;
use crate::module_index::required_module_export_type_for_fields;
use crate::narrowing::{
    directly_assigned_name, extend_with_return_guards, forget_return_guards,
    narrowed_locals_after_false_condition, narrowed_locals_for_condition, return_guards_for_names,
    ReturnGuard,
};
use crate::parser::{
    FunctionAnnotation as ParsedFunctionAnnotation, FunctionTypeParameter, GenericDeclarationId,
    GenericDeclarationKind, GenericParameter, LuaDocAnnotation, LuaDocAnnotationKind, LuaDocParam,
    LuaDocReturnType, ParsedLanguage, ParsedSource, SourceSpan,
    TableFieldType as ParsedTableFieldType, TuaType, TypeParameterId,
};
use crate::semantic_domains::{BuiltinDomain, BuiltinDomainRegistry};
use crate::source_text::function_name_table_field;
use crate::{asset, lua, Diagnostic, DiagnosticCode};

use super::model::{
    FunctionType, GenericAliasDefinition, GenericAliasReference, GenericClassDefinition,
    MetatableFacts, TableField, TableShape, Type, TypeAlias,
};
use super::{
    deduplicate_inferred_types, function_type_from_tua_signature, has_leading_lua_doc_type,
    instantiate_generic_function, widen_inferred_type, FunctionParameter, TypeContext,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeEnvironment {
    path: PathBuf,
    aliases: Arc<BTreeMap<String, Type>>,
    generic_aliases: Arc<HashMap<crate::parser::TypeAliasId, GenericAliasDefinition>>,
    generic_alias_bindings: Arc<HashMap<(PathBuf, String), crate::parser::TypeAliasId>>,
    generic_classes: Arc<HashMap<GenericDeclarationId, GenericClassDefinition>>,
    generic_protocol_views: Arc<HashMap<crate::parser::TypeParameterId, Type>>,
    alias_docs: BTreeMap<String, String>,
    class_names: BTreeSet<String>,
    class_parents: BTreeMap<String, String>,
    class_protocols: BTreeMap<String, Vec<String>>,
    protocol_names: BTreeSet<String>,
    protocol_identities: BTreeMap<String, (PathBuf, String)>,
    protocol_conformance_identities: BTreeSet<(String, PathBuf, String)>,
    locals: Arc<BTreeMap<String, Type>>,
    local_docs: BTreeMap<String, String>,
    local_contracts: LocalContracts,
    functions: Vec<TypedFunction>,
    function_declarations: BTreeMap<(usize, usize), usize>,
    deprecated_accesses: BTreeMap<Vec<String>, Deprecation>,
    must_use_accesses: BTreeSet<Vec<String>>,
    exact_module_bindings: BTreeMap<String, String>,
    project_module_signatures: BTreeMap<String, (Type, TypedFunction)>,
    required_module_types: Option<BTreeMap<String, Type>>,
    table_remove_overridden: bool,
    assert_overridden: bool,
    table_insert_overridden: bool,
    table_sort_overridden: bool,
    project_module_globals: BTreeSet<String>,
    busted_globals: Option<Arc<BTreeMap<String, Type>>>,
}

/// Annotation contracts belong to declarations, not names shared across scopes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct LocalContracts {
    types: BTreeMap<(usize, usize), Type>,
    names: BTreeSet<String>,
    scope: Option<Arc<crate::ScopeMap>>,
}

impl LocalContracts {
    fn at_declaration(&self, position: crate::SourcePosition) -> Option<&Type> {
        self.types.get(&(position.line, position.column))
    }

    fn binding(
        &self,
        name: &str,
        position: crate::SourcePosition,
    ) -> Option<&crate::ScopeDeclaration> {
        crate::semantic::binding_at(self.scope.as_deref()?, name, position)
    }
}

impl TypeEnvironment {
    pub(crate) fn with_busted_globals(mut self) -> Self {
        self.busted_globals = Some(crate::busted::global_types());
        self
    }

    pub(crate) fn has_busted_globals(&self) -> bool {
        self.busted_globals.is_some()
    }

    pub(crate) fn busted_global(&self, name: &str) -> Option<&Type> {
        self.busted_globals.as_ref()?.get(name)
    }

    pub fn from_parsed(parsed: &ParsedSource) -> Result<Self, Vec<Diagnostic>> {
        build_type_environment(parsed)
    }

    pub(crate) fn assert_is_overridden(&self) -> bool {
        self.assert_overridden
    }

    pub(crate) fn table_remove_is_overridden(&self) -> bool {
        self.table_remove_overridden
    }

    pub(crate) fn table_sort_is_overridden(&self) -> bool {
        self.table_sort_overridden
    }

    pub(crate) fn table_insert_is_overridden(&self) -> bool {
        self.table_insert_overridden
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn annotation_parameter_type(
        &self,
        annotation: &ParsedFunctionAnnotation,
        name: &str,
    ) -> Option<Type> {
        let parameter = annotation
            .parameters
            .iter()
            .find(|parameter| parameter.name == name);
        if parameter.is_none() {
            let function_name = annotation.name.as_deref()?;
            if function_name.starts_with("love.") {
                return self
                    .function(function_name)?
                    .parameters
                    .iter()
                    .find(|parameter| parameter.name == name)
                    .map(|parameter| parameter.ty.clone());
            }
        }
        let parameter = parameter?;
        // Native methods already have resolved scoped Self contracts. Reuse
        // those facts rather than rebuilding an alias environment per lookup.
        if let Some(function_name) = annotation.name.as_deref() {
            if generated_class_function_owner(function_name, &self.class_names).is_some() {
                if let Some(parameter) = self.function(function_name).and_then(|function| {
                    function
                        .parameters
                        .iter()
                        .find(|parameter| parameter.name == name)
                }) {
                    return Some(parameter.ty.clone());
                }
            }
        }
        Some(super::type_from_tua_type(&parameter.ty))
    }

    pub fn alias(&self, name: &str) -> Option<&Type> {
        self.aliases.get(name)
    }

    pub fn alias_documentation(&self, name: &str) -> Option<&str> {
        self.alias_docs.get(name).map(String::as_str)
    }

    /// Borrow the immutable alias catalog for shared semantic queries.
    pub(crate) fn alias_map(&self) -> &BTreeMap<String, Type> {
        &self.aliases
    }

    pub fn aliases(&self) -> impl Iterator<Item = (&str, &Type)> {
        self.aliases.iter().map(|(name, ty)| (name.as_str(), ty))
    }

    pub(crate) fn generic_alias_definitions(
        &self,
    ) -> impl Iterator<Item = &GenericAliasDefinition> {
        self.generic_aliases.values()
    }

    pub(crate) fn generic_alias_definition_for_local_name(
        &self,
        name: &str,
    ) -> Option<&GenericAliasDefinition> {
        let id = self
            .generic_alias_bindings
            .get(&(crate::vfs::normalize_path(&self.path), name.to_owned()))?;
        self.generic_aliases.get(id)
    }

    pub(crate) fn generic_class_definitions(
        &self,
    ) -> impl Iterator<Item = &GenericClassDefinition> {
        self.generic_classes.values()
    }

    pub(crate) fn with_generic_class_definitions(
        mut self,
        definitions: impl IntoIterator<Item = GenericClassDefinition>,
    ) -> Self {
        let classes = Arc::make_mut(&mut self.generic_classes);
        for definition in definitions {
            classes.insert(definition.id.clone(), definition);
        }
        self
    }

    pub(crate) fn generic_alias_diagnostics(&self, parsed: &ParsedSource) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut emitted = BTreeSet::new();

        for annotation in &parsed.local_annotations {
            let Some(ty) = self.local(&annotation.name) else {
                continue;
            };
            validate_generic_alias_type(
                self,
                ty,
                annotation.type_span.or(annotation.span),
                parsed,
                &mut diagnostics,
                &mut emitted,
            );
        }
        for alias in &parsed.type_aliases {
            if alias.generic_parameters.is_empty() {
                if let Some(ty) = self.alias(&alias.name) {
                    validate_generic_alias_type(
                        self,
                        ty,
                        alias.span,
                        parsed,
                        &mut diagnostics,
                        &mut emitted,
                    );
                }
            }
            if let Some(definition) = self.generic_aliases.get(&alias.id) {
                validate_generic_alias_type(
                    self,
                    &definition.body,
                    alias.span,
                    parsed,
                    &mut diagnostics,
                    &mut emitted,
                );
            }
        }
        for class in &parsed.class_declarations {
            if let Some(parent_application) = &class.parent_application {
                let parent_type = self
                    .generic_classes
                    .values()
                    .find(|definition| {
                        definition.id.module == crate::vfs::normalize_path(&parsed.path)
                            && definition.id.name == class.name
                    })
                    .and_then(|definition| definition.parent.as_ref());
                if let Some(parent_type) = parent_type {
                    validate_generic_alias_type(
                        self,
                        parent_type,
                        Some(parent_application.span),
                        parsed,
                        &mut diagnostics,
                        &mut emitted,
                    );
                }
            }
            for field in &class.fields {
                let ty = super::type_from_tua_type(&field.parsed_ty);
                validate_generic_alias_type(
                    self,
                    &ty,
                    Some(field.span),
                    parsed,
                    &mut diagnostics,
                    &mut emitted,
                );
            }
            for method in class.init.iter().chain(class.methods.iter()) {
                let signature =
                    super::type_from_tua_type(&crate::parser::parse_tua_type_text(&format!(
                        "({}) -> {}",
                        method.parameters,
                        method.return_type.as_deref().unwrap_or("any")
                    )));
                validate_generic_alias_type(
                    self,
                    &signature,
                    Some(method.span),
                    parsed,
                    &mut diagnostics,
                    &mut emitted,
                );
            }
        }
        for protocol in &parsed.protocol_declarations {
            for method in &protocol.methods {
                let signature = super::type_from_tua_type(&crate::parser::parse_tua_type_text(
                    &format!("({}) -> {}", method.parameters, method.return_type),
                ));
                validate_generic_alias_type(
                    self,
                    &signature,
                    Some(method.span),
                    parsed,
                    &mut diagnostics,
                    &mut emitted,
                );
            }
        }
        for annotation in &parsed.function_annotations {
            let Some(name) = annotation.name.as_deref() else {
                continue;
            };
            let Some(function) = self.function(name) else {
                continue;
            };
            for parameter in &function.parameters {
                validate_generic_alias_type(
                    self,
                    &parameter.ty,
                    annotation.span,
                    parsed,
                    &mut diagnostics,
                    &mut emitted,
                );
            }
            for ty in &function.return_types {
                validate_generic_alias_type(
                    self,
                    ty,
                    annotation.span,
                    parsed,
                    &mut diagnostics,
                    &mut emitted,
                );
            }
        }
        for annotation in &parsed.lua_doc_annotations {
            let mut check = |ty: &TuaType, span: SourceSpan| {
                let ty = super::type_from_tua_type(ty);
                validate_generic_alias_type(
                    self,
                    &ty,
                    Some(span),
                    parsed,
                    &mut diagnostics,
                    &mut emitted,
                );
            };
            match &annotation.kind {
                LuaDocAnnotationKind::Alias(alias) => check(&alias.ty, alias.type_span),
                LuaDocAnnotationKind::Field(field) => check(&field.ty, field.type_span),
                LuaDocAnnotationKind::Type(ty) => check(&ty.ty, ty.type_span),
                LuaDocAnnotationKind::Param(parameter) => check(&parameter.ty, parameter.type_span),
                LuaDocAnnotationKind::Return(returns) => {
                    for result in &returns.returns {
                        check(&result.ty, result.type_span);
                    }
                }
                LuaDocAnnotationKind::As(annotation) => check(&annotation.ty, annotation.type_span),
                LuaDocAnnotationKind::Overload(overload) => check(&overload.ty, overload.type_span),
                LuaDocAnnotationKind::Vararg(vararg) => check(&vararg.ty, vararg.type_span),
                _ => {}
            }
        }

        diagnostics
    }

    pub(crate) fn with_generic_alias_bindings(
        mut self,
        bindings: impl IntoIterator<Item = (PathBuf, String, GenericAliasDefinition)>,
    ) -> Self {
        let aliases = Arc::make_mut(&mut self.generic_aliases);
        let names = Arc::make_mut(&mut self.generic_alias_bindings);
        for (source_module, local_name, definition) in bindings {
            names.insert(
                (crate::vfs::normalize_path(source_module), local_name),
                definition.id.clone(),
            );
            aliases.insert(definition.id.clone(), definition);
        }
        self
    }

    /// Resolves aliases throughout a type while keeping recursive LuaCATS
    /// declarations finite. A repeated alias on the current expansion path is
    /// the recursive back-edge: native classes retain a nominal reference,
    /// while structural LuaCATS back-edges become `any`. Outer structure remains
    /// available to semantic and editor features.
    pub(crate) fn resolve_aliases(&self, ty: &Type) -> Type {
        if type_contains_generic_alias_application(ty, 0) {
            let mut context = GenericAliasExpansionContext::default();
            return context.resolve(self, ty);
        }
        if let Some(resolved) = resolve_nominal_reference(&self.aliases, ty) {
            return resolved;
        }
        if !type_contains_alias_cycle(self, ty, 0, &mut BTreeSet::new()) {
            return resolve_shallow_alias_chain(self, ty, 0, &mut BTreeSet::new());
        }

        let mut context = AliasResolutionContext::default();
        context.resolve(self, ty)
    }

    /// Projects the substituted instance members for a nominal generic class
    /// application. This is intentionally separate from `resolve_aliases`:
    /// expression and assignment types must retain their class identity.
    pub(crate) fn generic_class_members(&self, ty: &Type) -> Option<Type> {
        self.generic_class_members_inner(ty, &mut HashSet::new(), 0)
    }

    fn generic_class_members_inner(
        &self,
        ty: &Type,
        visited: &mut HashSet<GenericDeclarationId>,
        depth: usize,
    ) -> Option<Type> {
        if depth >= 64 {
            return None;
        }
        let Type::GenericClassApplication {
            identity,
            arguments,
            ..
        } = ty
        else {
            return None;
        };
        let definition = self.generic_classes.get(identity)?;
        if !visited.insert(identity.clone()) {
            return None;
        }
        if definition.parameters.len() != arguments.len() {
            return None;
        }
        let substitutions = definition
            .parameters
            .iter()
            .cloned()
            .zip(arguments.iter().cloned())
            .collect::<HashMap<_, _>>();
        let mut members =
            match definition.parent.as_ref().map(|parent| {
                self.resolve_aliases(&substitute_type_parameters(parent, &substitutions))
            }) {
                Some(parent @ Type::GenericClassApplication { .. }) => {
                    match self.generic_class_members_inner(&parent, visited, depth + 1) {
                        Some(Type::Table(table)) => table,
                        _ => TableShape::new(),
                    }
                }
                Some(Type::Alias(parent_name)) => match self.alias(&parent_name) {
                    Some(Type::Table(table)) => table.clone(),
                    _ => TableShape::new(),
                },
                Some(Type::Table(table)) => table,
                _ => TableShape::new(),
            };
        let Type::Table(body) = self.resolve_aliases(&substitute_type_parameters(
            &definition.body,
            &substitutions,
        )) else {
            return Some(Type::Table(members));
        };
        for (name, field) in body.fields() {
            members.insert(name.to_owned(), field.clone());
        }
        visited.remove(identity);
        Some(Type::Table(members))
    }

    /// Returns the value produced by a checked raw lookup. Ordinary `T[]`
    /// indexing stays optimistic for ergonomic Lua code, while `rawget`
    /// exposes the runtime possibility that a key is absent.
    pub(crate) fn optional_index_value_type(&self, collection: &Type) -> Option<Type> {
        optional_index_value_type(self.resolve_aliases(collection))
    }

    pub(crate) fn is_class(&self, name: &str) -> bool {
        self.class_names.contains(name)
    }

    pub(crate) fn class_constructor(&self, name: &str) -> Option<&TypedFunction> {
        self.is_class(name)
            .then(|| format!("{name}_new"))
            .and_then(|constructor| self.function(&constructor))
    }

    pub(crate) fn class_value_type(&self, name: &str) -> Option<Type> {
        if !self.is_class(name) {
            return None;
        }

        let mut fields = BTreeMap::new();
        self.collect_class_value_fields(name, &mut fields, &mut BTreeSet::new());

        Some(Type::table(fields))
    }

    fn collect_class_value_fields(
        &self,
        name: &str,
        fields: &mut BTreeMap<String, TableField>,
        visited: &mut BTreeSet<String>,
    ) {
        if !visited.insert(name.to_owned()) {
            return;
        }

        if let Some(parent) = self.class_parents.get(name) {
            self.collect_class_value_fields(parent, fields, visited);
        }

        if let Some(constructor) = self
            .class_constructor(name)
            .and_then(TypedFunction::as_type)
        {
            fields.insert("new".to_owned(), TableField::required(constructor));
        }

        let prefix = format!("{name}_");
        for function in &self.functions {
            let Some(function_name) = function.name.as_deref() else {
                continue;
            };
            if generated_class_function_owner(function_name, &self.class_names).as_deref()
                != Some(name)
            {
                continue;
            }
            let Some(method_name) = function_name.strip_prefix(&prefix) else {
                continue;
            };

            if matches!(method_name, "init" | "new") {
                continue;
            }

            if let Some(ty) = function.as_type() {
                fields.insert(method_name.to_owned(), TableField::required(ty));
            }
        }
    }

    pub fn local(&self, name: &str) -> Option<&Type> {
        self.locals.get(name)
    }

    pub(crate) fn has_local_contract_at(&self, position: crate::SourcePosition) -> bool {
        self.local_contracts.at_declaration(position).is_some()
    }

    pub(crate) fn local_at_declaration(
        &self,
        name: &str,
        position: crate::SourcePosition,
    ) -> Option<&Type> {
        if self.local_contracts.names.contains(name) {
            self.local_contracts.at_declaration(position)
        } else {
            self.local(name)
        }
    }

    /// Position is in parser coordinates, like the shared scope map.
    pub(crate) fn local_at_position(
        &self,
        name: &str,
        position: crate::SourcePosition,
    ) -> Option<&Type> {
        if self.local_contracts.names.contains(name) {
            let binding = self.local_contracts.binding(name, position)?;
            self.local_contracts.at_declaration(binding.name_span.start)
        } else {
            self.local(name)
        }
    }

    pub(crate) fn annotated_name_binding(
        &self,
        name: &str,
        position: crate::SourcePosition,
    ) -> Option<&crate::ScopeDeclaration> {
        self.local_contracts
            .names
            .contains(name)
            .then(|| self.local_contracts.binding(name, position))
            .flatten()
    }

    pub fn local_documentation(&self, name: &str) -> Option<&str> {
        self.local_docs.get(name).map(String::as_str)
    }

    pub fn locals(&self) -> impl Iterator<Item = (&str, &Type)> {
        self.locals.iter().map(|(name, ty)| (name.as_str(), ty))
    }

    pub fn functions(&self) -> &[TypedFunction] {
        &self.functions
    }

    pub fn function(&self, name: &str) -> Option<&TypedFunction> {
        self.functions
            .iter()
            .find(|function| function.name.as_deref() == Some(name))
    }

    /// Resolve a function at a parser-coordinate use site, respecting lexical
    /// declarations instead of selecting the first equal spelling in the file.
    pub fn function_at(
        &self,
        name: &str,
        position: crate::SourcePosition,
    ) -> Option<&TypedFunction> {
        if let Some(binding) = self.local_contracts.binding(name, position) {
            let key = (binding.name_span.start.line, binding.name_span.start.column);
            return self
                .function_declarations
                .get(&key)
                .and_then(|index| self.functions.get(*index));
        }
        self.function(name)
    }

    pub(crate) fn deprecation_for_access(&self, path: &[String]) -> Option<&Deprecation> {
        self.deprecated_accesses.get(path)
    }

    pub(crate) fn deprecated_accesses(&self) -> impl Iterator<Item = (&[String], &Deprecation)> {
        self.deprecated_accesses
            .iter()
            .map(|(path, deprecation)| (path.as_slice(), deprecation))
    }

    pub(crate) fn must_use_accesses(&self) -> impl Iterator<Item = &[String]> {
        self.must_use_accesses.iter().map(Vec::as_slice)
    }

    pub(crate) fn exact_module_binding(&self, name: &str) -> Option<&str> {
        self.exact_module_bindings.get(name).map(String::as_str)
    }

    pub fn to_type_context(&self) -> TypeContext {
        let mut context = self
            .aliases
            .iter()
            .fold(TypeContext::new(), |context, (name, ty)| {
                context.with_alias(TypeAlias::new(name.clone(), ty.clone()))
            });

        for definition in self.generic_aliases.values() {
            context.insert_generic_alias(definition.clone());
        }
        for ((source_module, local_name), id) in self.generic_alias_bindings.iter() {
            if let Some(definition) = self.generic_aliases.get(id) {
                context.insert_generic_alias_binding(
                    source_module.clone(),
                    local_name.clone(),
                    definition.clone(),
                );
            }
        }

        for &(type_name, supertype) in lua::love_type_supertypes() {
            context.insert_nominal_supertype(type_name, supertype);
        }

        for class in &self.class_names {
            context.insert_nominal_alias(class);
        }
        for protocols in self.class_protocols.values() {
            for protocol in protocols {
                context.insert_nominal_protocol(protocol);
            }
        }
        for protocol in &self.protocol_names {
            context.insert_nominal_protocol(protocol);
        }
        for (name, (module_path, protocol_name)) in &self.protocol_identities {
            context.insert_protocol_identity(
                name.clone(),
                module_path.clone(),
                protocol_name.clone(),
            );
        }

        // `Vec2` is a runtime BrineVector value, not merely any table with x
        // and y fields. Keeping the built-in alias nominal prevents a plain
        // structural table from acquiring operator semantics that it will not
        // have in emitted Lua. A file that declares its own `type Vec2` keeps
        // that alias structural.
        if self.aliases.get(crate::vector::TYPE_NAME) == Some(&crate::vector::vec2_type()) {
            context.insert_nominal_alias(crate::vector::TYPE_NAME);
        }

        for (class, parent) in &self.class_parents {
            context.insert_nominal_alias_supertype(class, parent);
        }
        for definition in self.generic_classes.values() {
            context.insert_generic_class_definition(definition.clone());
            for parameter in &definition.generic_parameters {
                if let Some(bound) = &parameter.protocol_bound {
                    context.insert_generic_protocol_bound(parameter.id.clone(), bound.clone());
                }
            }
        }
        for function in &self.functions {
            for parameter in &function.generic_parameters {
                if let Some(bound) = &parameter.protocol_bound {
                    context.insert_generic_protocol_bound(parameter.id.clone(), bound.clone());
                }
            }
        }
        for (class, protocols) in &self.class_protocols {
            for protocol in protocols {
                context.insert_nominal_alias_supertype(class, protocol);
            }
        }
        for (class, protocol_module, protocol_name) in &self.protocol_conformance_identities {
            context.insert_protocol_conformance_identity(
                class.clone(),
                protocol_module.clone(),
                protocol_name.clone(),
            );
        }

        context
    }

    pub(crate) fn generic_protocol_view(&self, ty: &Type) -> Option<&Type> {
        let Type::GenericParameter { id, .. } = ty else {
            return None;
        };
        self.generic_protocol_views.get(id)
    }

    /// Project assignments own these module names; the compiler must not inject
    /// a different runtime. Unknown cross-file values remain dynamically typed.
    pub(crate) fn with_project_module_globals(
        mut self,
        parsed: &ParsedSource,
        names: &BTreeSet<String>,
    ) -> Self {
        self.project_module_globals = names.clone();
        for name in names {
            if crate::builtin_module::is_visible(parsed, name) {
                Arc::make_mut(&mut self.locals).insert(name.clone(), Type::Any);
            }
        }
        self
    }

    pub(crate) fn project_owns_module(&self, name: &str) -> bool {
        self.project_module_globals.contains(name)
    }

    pub(crate) fn with_local_types(
        mut self,
        locals: impl IntoIterator<Item = (String, Type)>,
    ) -> Self {
        for (name, ty) in locals {
            if self.locals.get(&name) != Some(&ty) {
                Arc::make_mut(&mut self.locals).insert(name, ty);
            }
        }

        self
    }

    pub(crate) fn with_local_contract_types(
        mut self,
        contracts: impl IntoIterator<Item = (String, crate::SourcePosition, Type)>,
    ) -> Self {
        for (name, position, ty) in contracts {
            self.local_contracts
                .types
                .insert((position.line, position.column), ty.clone());
            Arc::make_mut(&mut self.locals).insert(name, ty);
        }
        self
    }

    pub(crate) fn with_alias_types(
        mut self,
        aliases: impl IntoIterator<Item = (String, Type)>,
    ) -> Self {
        for (name, ty) in aliases {
            if !self.aliases.contains_key(&name) {
                Arc::make_mut(&mut self.aliases).insert(name, ty);
            }
        }

        self
    }

    pub(crate) fn with_protocol_names(
        mut self,
        protocols: impl IntoIterator<Item = String>,
    ) -> Self {
        self.protocol_names.extend(protocols);
        self
    }

    pub(crate) fn with_protocol_identities(
        mut self,
        identities: impl IntoIterator<Item = (String, PathBuf, String)>,
    ) -> Self {
        self.protocol_identities.extend(
            identities
                .into_iter()
                .map(|(name, module_path, protocol_name)| (name, (module_path, protocol_name))),
        );
        self
    }

    pub(crate) fn with_protocol_conformance_identities(
        mut self,
        conformances: impl IntoIterator<Item = (String, PathBuf, String)>,
    ) -> Self {
        self.protocol_conformance_identities.extend(conformances);
        self
    }

    pub(crate) fn with_class_protocol_conformances(
        mut self,
        conformances: impl IntoIterator<Item = (String, String)>,
    ) -> Self {
        for (class, protocol) in conformances {
            let protocols = self.class_protocols.entry(class).or_default();
            if !protocols.contains(&protocol) {
                protocols.push(protocol);
            }
        }
        self
    }

    pub(crate) fn with_deprecated_accesses(
        mut self,
        accesses: impl IntoIterator<Item = (Vec<String>, Deprecation)>,
    ) -> Self {
        self.deprecated_accesses.extend(accesses);
        self
    }

    pub(crate) fn with_must_use_accesses(
        mut self,
        accesses: impl IntoIterator<Item = Vec<String>>,
    ) -> Self {
        self.must_use_accesses.extend(accesses);
        self
    }

    pub(crate) fn with_required_module_types(
        mut self,
        modules: impl IntoIterator<Item = (String, Type)>,
    ) -> Self {
        self.required_module_types = Some(modules.into_iter().collect());
        self
    }

    pub(crate) fn required_module_type(&self, module: &str) -> Option<&Type> {
        self.required_module_types.as_ref()?.get(module)
    }

    pub(crate) fn has_project_module_types(&self) -> bool {
        self.required_module_types.is_some()
    }

    pub(crate) fn with_project_module_signatures(
        mut self,
        signatures: impl IntoIterator<Item = (String, (Type, TypedFunction))>,
    ) -> Self {
        self.project_module_signatures.extend(signatures);
        self
    }

    pub(crate) fn project_module_signature(&self, name: &str) -> Option<&(Type, TypedFunction)> {
        self.project_module_signatures.get(name)
    }

    pub(crate) fn with_exact_module_bindings(
        mut self,
        bindings: impl IntoIterator<Item = (String, String)>,
    ) -> Self {
        self.exact_module_bindings.extend(bindings);
        self
    }
}

const MAX_ALIAS_RESOLUTION_DEPTH: usize = 64;
// Expansion is recursive; keep this well below the parser's structural
// limits so pathological changing-argument recursion cannot exhaust the Rust
// thread stack before the work budget stops it.
const MAX_GENERIC_ALIAS_EXPANSION_WORK: usize = 4096;
const MAX_GENERIC_ALIAS_INSTANTIATION_DEPTH: usize = 128;

#[derive(Default)]
struct GenericAliasExpansionContext {
    work: usize,
    active: Vec<(crate::parser::TypeAliasId, Vec<Type>)>,
    aliases: AliasResolutionContext,
}

impl GenericAliasExpansionContext {
    fn resolve(&mut self, environment: &TypeEnvironment, ty: &Type) -> Type {
        if self.work >= MAX_GENERIC_ALIAS_EXPANSION_WORK {
            return ty.clone();
        }
        self.work += 1;
        match ty {
            Type::GenericClassApplication {
                identity,
                name,
                arguments,
                span,
            } => Type::GenericClassApplication {
                identity: identity.clone(),
                name: name.clone(),
                arguments: arguments
                    .iter()
                    .map(|argument| self.resolve(environment, argument))
                    .collect(),
                span: *span,
            },
            Type::GenericAliasApplication {
                reference,
                arguments,
                span,
            } => {
                let arguments = arguments
                    .iter()
                    .map(|argument| self.resolve(environment, argument))
                    .collect::<Vec<_>>();
                let definition = match reference {
                    GenericAliasReference::Declaration(id) => environment.generic_aliases.get(id),
                    GenericAliasReference::Imported {
                        source_module,
                        name,
                        ..
                    } => environment
                        .generic_alias_bindings
                        .get(&(crate::vfs::normalize_path(source_module), name.clone()))
                        .and_then(|id| environment.generic_aliases.get(id))
                        .or_else(|| {
                            environment.generic_aliases.values().find(|definition| {
                                definition.id.module == *source_module
                                    && definition.id.name == *name
                            })
                        }),
                };
                let Some(definition) = definition else {
                    return Type::GenericAliasApplication {
                        reference: reference.clone(),
                        arguments,
                        span: *span,
                    };
                };
                if definition.parameters.len() != arguments.len() {
                    return Type::GenericAliasApplication {
                        reference: reference.clone(),
                        arguments,
                        span: *span,
                    };
                }

                if self.active.len() >= MAX_GENERIC_ALIAS_INSTANTIATION_DEPTH {
                    return Type::GenericAliasApplication {
                        reference: GenericAliasReference::Declaration(definition.id.clone()),
                        arguments,
                        span: *span,
                    };
                }

                let key = (definition.id.clone(), arguments.clone());
                if self.active.contains(&key) {
                    // A recursive alias at the same instantiation has reached
                    // its fixed point. Retain its named edge within the outer
                    // structure so hover/completion stay finite and useful.
                    return Type::GenericAliasApplication {
                        reference: GenericAliasReference::Declaration(definition.id.clone()),
                        arguments,
                        span: *span,
                    };
                }

                let substitutions = definition
                    .parameters
                    .iter()
                    .cloned()
                    .zip(arguments)
                    .collect::<HashMap<_, _>>();
                let substituted = substitute_type_parameters(&definition.body, &substitutions);
                self.active.push(key);
                let resolved = self.resolve(environment, &substituted);
                self.active.pop();
                resolved
            }
            Type::Alias(name) => {
                let Some(alias) = environment.alias(name) else {
                    return ty.clone();
                };
                self.aliases.resolve(environment, alias)
            }
            Type::Union(types) => Type::union(types.iter().map(|ty| self.resolve(environment, ty))),
            Type::Function(function) => {
                let parameters = function
                    .parameters
                    .iter()
                    .map(|parameter| FunctionParameter {
                        name: parameter.name.clone(),
                        ty: self.resolve(environment, &parameter.ty),
                    })
                    .collect::<Vec<_>>();
                let vararg_type = function
                    .vararg_type
                    .as_deref()
                    .map(|ty| self.resolve(environment, ty));
                let returns = function
                    .return_types_or_any()
                    .iter()
                    .map(|ty| self.resolve(environment, ty))
                    .collect::<Vec<_>>();
                let guarded = function.guarded_return_types.as_ref().map(|types| {
                    types
                        .iter()
                        .map(|ty| self.resolve(environment, ty))
                        .collect()
                });
                let overloads: Vec<FunctionType> = function
                    .overloads
                    .iter()
                    .map(|overload| {
                        let Type::Function(overload) =
                            self.resolve(environment, &Type::Function(overload.clone()))
                        else {
                            unreachable!()
                        };
                        overload
                    })
                    .collect();
                Type::Function(
                    FunctionType::new_signature(
                        parameters,
                        function.required_parameter_count,
                        vararg_type,
                        returns,
                    )
                    .with_generic_parameters(function.generic_parameters.clone())
                    .with_guarded_return_types(guarded)
                    .with_overloads(overloads),
                )
            }
            Type::Table(table) => {
                let mut resolved =
                    super::TableShape::from_fields(table.fields().map(|(name, field)| {
                        (
                            name.to_owned(),
                            TableField {
                                ty: self.resolve(environment, &field.ty),
                                optional: field.optional,
                            },
                        )
                    }));
                resolved.copy_semantic_metadata_from(table);
                Type::Table(resolved)
            }
            Type::Array(item) => Type::array(self.resolve(environment, item)),
            Type::Tuple(items) => Type::tuple(items.iter().map(|ty| self.resolve(environment, ty))),
            Type::Dictionary { key, value } => Type::dictionary(
                self.resolve(environment, key),
                self.resolve(environment, value),
            ),
            Type::GenericParameter { .. }
            | Type::Primitive(_)
            | Type::Any
            | Type::StringLiteral(_) => ty.clone(),
        }
    }
}

fn type_contains_generic_alias_application(ty: &Type, depth: usize) -> bool {
    if depth >= MAX_ALIAS_RESOLUTION_DEPTH {
        return false;
    }
    match ty {
        Type::GenericAliasApplication { .. } => true,
        Type::GenericClassApplication { arguments, .. } => arguments
            .iter()
            .any(|ty| type_contains_generic_alias_application(ty, depth + 1)),
        Type::Union(types) | Type::Tuple(types) => types
            .iter()
            .any(|ty| type_contains_generic_alias_application(ty, depth + 1)),
        Type::Function(function) => {
            function
                .parameters
                .iter()
                .any(|parameter| type_contains_generic_alias_application(&parameter.ty, depth + 1))
                || function
                    .vararg_type
                    .as_deref()
                    .is_some_and(|ty| type_contains_generic_alias_application(ty, depth + 1))
                || function
                    .return_types_or_any()
                    .iter()
                    .any(|ty| type_contains_generic_alias_application(ty, depth + 1))
        }
        Type::Table(table) => table
            .fields()
            .any(|(_, field)| type_contains_generic_alias_application(&field.ty, depth + 1)),
        Type::Array(item) => type_contains_generic_alias_application(item, depth + 1),
        Type::Dictionary { key, value } => {
            type_contains_generic_alias_application(key, depth + 1)
                || type_contains_generic_alias_application(value, depth + 1)
        }
        Type::Alias(_)
        | Type::GenericParameter { .. }
        | Type::Primitive(_)
        | Type::Any
        | Type::StringLiteral(_) => false,
    }
}

pub(crate) fn substitute_type_parameters(
    ty: &Type,
    substitutions: &HashMap<crate::parser::TypeParameterId, Type>,
) -> Type {
    match ty {
        Type::GenericParameter { id, .. } => {
            substitutions.get(id).cloned().unwrap_or_else(|| ty.clone())
        }
        Type::Union(types) => Type::union(
            types
                .iter()
                .map(|ty| substitute_type_parameters(ty, substitutions)),
        ),
        Type::Function(function) => {
            // A nested scheme owns its binders. Keep them bound even when an
            // enclosing generic alias or call is being instantiated.
            let nested_substitutions = substitutions
                .iter()
                .filter(|(id, _)| {
                    !function
                        .generic_parameters
                        .iter()
                        .any(|parameter| &parameter.id == *id)
                })
                .map(|(id, ty)| (id.clone(), ty.clone()))
                .collect::<HashMap<_, _>>();
            let substitutions = &nested_substitutions;
            let parameters = function
                .parameters
                .iter()
                .map(|parameter| FunctionParameter {
                    name: parameter.name.clone(),
                    ty: substitute_type_parameters(&parameter.ty, substitutions),
                });
            let vararg = function
                .vararg_type
                .as_deref()
                .map(|ty| substitute_type_parameters(ty, substitutions));
            let returns = function
                .return_types_or_any()
                .iter()
                .map(|ty| substitute_type_parameters(ty, substitutions));
            let mut result = FunctionType::new_signature(
                parameters,
                function.required_parameter_count,
                vararg,
                returns,
            )
            .with_generic_parameters(function.generic_parameters.clone());
            result.guarded_return_types = function.guarded_return_types.as_ref().map(|types| {
                types
                    .iter()
                    .map(|ty| substitute_type_parameters(ty, substitutions))
                    .collect()
            });
            result.overloads = function
                .overloads
                .iter()
                .map(|overload| {
                    let Type::Function(overload) = substitute_type_parameters(
                        &Type::Function(overload.clone()),
                        substitutions,
                    ) else {
                        unreachable!()
                    };
                    overload
                })
                .collect();
            Type::Function(result)
        }
        Type::Table(table) => {
            let mut result = super::TableShape::from_fields(table.fields().map(|(name, field)| {
                (
                    name.to_owned(),
                    TableField {
                        ty: substitute_type_parameters(&field.ty, substitutions),
                        optional: field.optional,
                    },
                )
            }));
            result.copy_semantic_metadata_from(table);
            Type::Table(result)
        }
        Type::Array(item) => Type::array(substitute_type_parameters(item, substitutions)),
        Type::Tuple(items) => Type::tuple(
            items
                .iter()
                .map(|ty| substitute_type_parameters(ty, substitutions)),
        ),
        Type::Dictionary { key, value } => Type::dictionary(
            substitute_type_parameters(key, substitutions),
            substitute_type_parameters(value, substitutions),
        ),
        Type::GenericAliasApplication {
            reference,
            arguments,
            span,
        } => Type::GenericAliasApplication {
            reference: reference.clone(),
            arguments: arguments
                .iter()
                .map(|ty| substitute_type_parameters(ty, substitutions))
                .collect(),
            span: *span,
        },
        Type::GenericClassApplication {
            identity,
            name,
            arguments,
            span,
        } => Type::GenericClassApplication {
            identity: identity.clone(),
            name: name.clone(),
            arguments: arguments
                .iter()
                .map(|ty| substitute_type_parameters(ty, substitutions))
                .collect(),
            span: *span,
        },
        Type::Alias(_) | Type::Primitive(_) | Type::Any | Type::StringLiteral(_) => ty.clone(),
    }
}

fn resolve_shallow_alias_chain(
    environment: &TypeEnvironment,
    ty: &Type,
    depth: usize,
    visiting: &mut BTreeSet<String>,
) -> Type {
    if depth >= MAX_ALIAS_RESOLUTION_DEPTH {
        return ty.clone();
    }
    let Type::Alias(name) = ty else {
        return ty.clone();
    };
    let Some(alias) = environment.alias(name) else {
        return ty.clone();
    };
    if !visiting.insert(name.clone()) {
        return Type::Any;
    }
    let resolved = resolve_shallow_alias_chain(environment, alias, depth + 1, visiting);
    visiting.remove(name);
    resolved
}

fn type_contains_alias_cycle(
    environment: &TypeEnvironment,
    ty: &Type,
    depth: usize,
    visiting: &mut BTreeSet<String>,
) -> bool {
    if depth >= MAX_ALIAS_RESOLUTION_DEPTH {
        return false;
    }

    match ty {
        Type::Alias(name) => {
            let Some(alias) = environment.alias(name) else {
                return false;
            };
            if !visiting.insert(name.clone()) {
                return true;
            }
            let recursive = type_contains_alias_cycle(environment, alias, depth + 1, visiting);
            visiting.remove(name);
            recursive
        }
        Type::Union(types) | Type::Tuple(types) => types
            .iter()
            .any(|ty| type_contains_alias_cycle(environment, ty, depth + 1, visiting)),
        Type::Function(function) => {
            function
                .parameters
                .iter()
                .filter(|parameter| parameter.name.as_deref() != Some("self"))
                .any(|parameter| {
                    type_contains_alias_cycle(environment, &parameter.ty, depth + 1, visiting)
                })
                || function.vararg_type.as_deref().is_some_and(|ty| {
                    type_contains_alias_cycle(environment, ty, depth + 1, visiting)
                })
                || function
                    .return_types_or_any()
                    .iter()
                    .any(|ty| type_contains_alias_cycle(environment, ty, depth + 1, visiting))
        }
        Type::Table(table) => table.fields().any(|(_, field)| {
            type_contains_alias_cycle(environment, &field.ty, depth + 1, visiting)
        }),
        Type::Array(item) => type_contains_alias_cycle(environment, item, depth + 1, visiting),
        Type::Dictionary { key, value } => {
            type_contains_alias_cycle(environment, key, depth + 1, visiting)
                || type_contains_alias_cycle(environment, value, depth + 1, visiting)
        }
        Type::Primitive(_)
        | Type::Any
        | Type::StringLiteral(_)
        | Type::GenericParameter { .. }
        | Type::GenericAliasApplication { .. }
        | Type::GenericClassApplication { .. } => false,
    }
}

#[derive(Default)]
struct AliasResolutionContext {
    depth: usize,
    visiting: BTreeSet<String>,
}

impl AliasResolutionContext {
    fn resolve(&mut self, environment: &TypeEnvironment, ty: &Type) -> Type {
        if self.depth >= MAX_ALIAS_RESOLUTION_DEPTH {
            return ty.clone();
        }

        self.depth += 1;
        let resolved = match ty {
            Type::Alias(name) => {
                let Some(alias) = environment.alias(name) else {
                    self.depth -= 1;
                    return ty.clone();
                };
                if !self.visiting.insert(name.clone()) {
                    self.depth -= 1;
                    return match alias {
                        Type::Table(table) => table.nominal_reference().map(Type::Table),
                        _ => None,
                    }
                    .unwrap_or(Type::Any);
                }

                let resolved = self.resolve(environment, alias);
                self.visiting.remove(name);
                resolved
            }
            Type::Union(types) => Type::union(types.iter().map(|ty| self.resolve(environment, ty))),
            Type::Function(function) => {
                let parameters = function
                    .parameters
                    .iter()
                    .map(|parameter| FunctionParameter {
                        name: parameter.name.clone(),
                        ty: self.resolve(environment, &parameter.ty),
                    })
                    .collect::<Vec<_>>();
                let vararg_type = function
                    .vararg_type
                    .as_deref()
                    .map(|ty| self.resolve(environment, ty));
                let return_types = function
                    .return_types_or_any()
                    .iter()
                    .map(|ty| self.resolve(environment, ty))
                    .collect::<Vec<_>>();
                let guarded = function.guarded_return_types.as_ref().map(|types| {
                    types
                        .iter()
                        .map(|ty| self.resolve(environment, ty))
                        .collect()
                });
                let overloads = function
                    .overloads
                    .iter()
                    .map(|overload| {
                        let Type::Function(overload) =
                            self.resolve(environment, &Type::Function(overload.clone()))
                        else {
                            unreachable!()
                        };
                        overload
                    })
                    .collect::<Vec<_>>();
                Type::Function(
                    FunctionType::new_signature(
                        parameters,
                        function.required_parameter_count,
                        vararg_type,
                        return_types,
                    )
                    .with_guarded_return_types(guarded)
                    .with_overloads(overloads),
                )
            }
            Type::Table(table) => {
                let mut resolved =
                    super::TableShape::from_fields(table.fields().map(|(name, field)| {
                        (
                            name.to_owned(),
                            TableField {
                                ty: self.resolve(environment, &field.ty),
                                optional: field.optional,
                            },
                        )
                    }));
                resolved.copy_semantic_metadata_from(table);
                Type::Table(resolved)
            }
            Type::Array(item) => Type::array(self.resolve(environment, item)),
            Type::Tuple(items) => Type::tuple(items.iter().map(|ty| self.resolve(environment, ty))),
            Type::Dictionary { key, value } => Type::dictionary(
                self.resolve(environment, key),
                self.resolve(environment, value),
            ),
            Type::Primitive(_)
            | Type::Any
            | Type::StringLiteral(_)
            | Type::GenericParameter { .. }
            | Type::GenericAliasApplication { .. }
            | Type::GenericClassApplication { .. } => ty.clone(),
        };
        self.depth -= 1;
        resolved
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedFunction {
    pub name: Option<String>,
    pub generic_parameters: Vec<crate::parser::GenericParameter>,
    pub parameters: Vec<TypedParameter>,
    pub parameter_count: usize,
    pub required_parameter_count: usize,
    pub has_varargs: bool,
    pub vararg_type: Option<Type>,
    pub return_type: Option<Type>,
    pub return_types: Vec<Type>,
    /// See [`FunctionType::guarded_return_types`].
    pub guarded_return_types: Option<Vec<Type>>,
    pub documentation: Option<String>,
    pub overloads: Vec<TypedFunctionOverload>,
}

impl TypedFunction {
    pub fn has_fully_annotated_parameters(&self) -> bool {
        self.parameters.len() == self.parameter_count
            && (!self.has_varargs || self.vararg_type.is_some())
    }

    pub fn return_type_or_any(&self) -> Type {
        self.return_type.clone().unwrap_or(Type::Any)
    }

    pub fn return_types_or_any(&self) -> Vec<Type> {
        if !self.return_types.is_empty() {
            return self.return_types.clone();
        }

        vec![self.return_type_or_any()]
    }

    pub fn as_type(&self) -> Option<Type> {
        self.has_fully_annotated_parameters().then(|| {
            Type::Function(
                FunctionType::new_signature(
                    self.parameters.iter().map(|parameter| {
                        FunctionParameter::named(&parameter.name, parameter.ty.clone())
                    }),
                    self.required_parameter_count,
                    self.vararg_type.clone(),
                    self.return_types_or_any(),
                )
                .with_generic_parameters(self.generic_parameters.clone())
                .with_guarded_return_types(self.guarded_return_types.clone())
                .with_overloads(
                    self.overloads
                        .iter()
                        .map(TypedFunctionOverload::as_function_type),
                ),
            )
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedFunctionOverload {
    pub parameters: Vec<TypedParameter>,
    pub has_varargs: bool,
    pub vararg_type: Option<Type>,
    pub return_types: Vec<Type>,
    pub documentation: Option<String>,
}

impl TypedFunctionOverload {
    pub(crate) fn as_function_type(&self) -> FunctionType {
        FunctionType::new_signature(
            self.parameters
                .iter()
                .map(|p| FunctionParameter::named(&p.name, p.ty.clone())),
            self.parameters.len(),
            self.vararg_type.clone(),
            self.return_types_or_any(),
        )
    }

    pub fn return_types_or_any(&self) -> Vec<Type> {
        if self.return_types.is_empty() {
            return vec![Type::Any];
        }

        self.return_types.clone()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedParameter {
    pub name: String,
    pub ty: Type,
    pub documentation: Option<String>,
}

pub fn build_type_environment(parsed: &ParsedSource) -> Result<TypeEnvironment, Vec<Diagnostic>> {
    let (environment, diagnostics) = build_type_environment_with_diagnostics(parsed);

    if diagnostics.is_empty() {
        Ok(environment)
    } else {
        Err(diagnostics)
    }
}

pub(crate) fn build_type_environment_with_diagnostics(
    parsed: &ParsedSource,
) -> (TypeEnvironment, Vec<Diagnostic>) {
    build_type_environment_with_domain_registry(parsed, BuiltinDomainRegistry::standard())
}

pub(crate) fn build_type_environment_with_domain_registry(
    parsed: &ParsedSource,
    domains: BuiltinDomainRegistry,
) -> (TypeEnvironment, Vec<Diagnostic>) {
    let builtin_aliases = domains.builtin_aliases();
    let mut diagnostics = Vec::new();
    let mut alias_names = collect_alias_names(parsed, &mut diagnostics);
    collect_class_names(parsed, &mut alias_names);
    for protocol in &parsed.protocol_declarations {
        if !alias_names.insert(protocol.name.clone()) {
            diagnostics.push(type_diagnostic(
                &parsed.path,
                Some(protocol.name_span),
                format!(
                    "protocol `{}` conflicts with an existing type name",
                    protocol.name
                ),
                DiagnosticCode::TYPE_DUPLICATE_ALIAS,
            ));
        }
    }
    let doc_aliases = lua_doc_alias_declarations(parsed, &alias_names, &mut diagnostics);
    let alias_names = alias_names
        .into_iter()
        .chain(doc_aliases.iter().map(|alias| alias.name.clone()))
        .chain(
            asset::builtin_type_names()
                .iter()
                .map(|name| (*name).to_owned()),
        )
        .chain(lua::love_type_names().iter().map(|name| (*name).to_owned()))
        .chain(domains.builtin_type_names().into_iter().map(str::to_owned))
        .chain(builtin_aliases.iter().map(|(name, _)| name.clone()))
        .collect::<BTreeSet<_>>();
    let mut aliases = BTreeMap::new();
    let mut generic_aliases = HashMap::new();
    let mut generic_alias_bindings = HashMap::new();
    let mut alias_docs = BTreeMap::new();
    let mut locals = BTreeMap::new();
    let mut local_docs = BTreeMap::new();
    let mut functions = Vec::new();
    let class_names = parsed
        .class_declarations
        .iter()
        .map(|class| class.name.clone())
        .collect::<BTreeSet<_>>();

    for alias in &parsed.type_aliases {
        if alias.generic_parameters.is_empty() {
            continue;
        }
        let mut context =
            TypeResolutionContext::with_classes(&parsed.path, &parsed.class_declarations);
        context.self_type = Some(Type::alias(alias.name.clone()));
        let body = resolve_tua_type_with_context(
            &parsed.path,
            &alias.ty,
            alias.span,
            &alias_names,
            &mut diagnostics,
            &context,
        );
        generic_aliases.insert(
            alias.id.clone(),
            GenericAliasDefinition {
                id: alias.id.clone(),
                parameters: alias
                    .generic_parameters
                    .iter()
                    .map(|parameter| parameter.id.clone())
                    .collect(),
                parameter_names: alias
                    .generic_parameters
                    .iter()
                    .map(|parameter| parameter.name.clone())
                    .collect(),
                body: Box::new(body),
            },
        );
        generic_alias_bindings.insert(
            (crate::vfs::normalize_path(&parsed.path), alias.name.clone()),
            alias.id.clone(),
        );
    }

    for alias in &parsed.type_aliases {
        if aliases.contains_key(&alias.name) {
            continue;
        }
        if !alias.generic_parameters.is_empty() {
            // Generic alias declarations are stored as bound syntax and are
            // instantiated from type positions; resolving their body as a
            // monomorphic alias would make its parameters appear unbound.
            continue;
        }

        let mut context =
            TypeResolutionContext::with_classes(&parsed.path, &parsed.class_declarations);
        context.self_type = Some(Type::alias(alias.name.clone()));
        let ty = resolve_tua_type_with_context(
            &parsed.path,
            &alias.ty,
            alias.span,
            &alias_names,
            &mut diagnostics,
            &context,
        );
        aliases.insert(alias.name.clone(), ty);
    }

    for alias in &doc_aliases {
        if aliases.contains_key(&alias.name) {
            continue;
        }

        let context = TypeResolutionContext::with_self_type(Type::alias(alias.name.clone()));
        let ty = if let TuaType::Table(fields) = &alias.ty {
            let mut resolved = BTreeMap::<String, TableField>::new();
            for field in fields {
                let ty = resolve_table_field_type(
                    &parsed.path,
                    field,
                    &alias_names,
                    &mut diagnostics,
                    &context,
                );
                if let (Some(existing), Type::Function(overload)) =
                    (resolved.get_mut(&field.name), &ty)
                {
                    if let Type::Function(function) = &mut existing.ty {
                        function.overloads.push(overload.clone());
                        existing.optional &= field.optional;
                        continue;
                    }
                }
                resolved.insert(
                    field.name.clone(),
                    TableField {
                        ty,
                        optional: field.optional,
                    },
                );
            }
            Type::table(resolved)
        } else {
            resolve_tua_type_with_context(
                &parsed.path,
                &alias.ty,
                Some(alias.span),
                &alias_names,
                &mut diagnostics,
                &context,
            )
        };

        aliases.insert(alias.name.clone(), ty);

        if let Some(documentation) = &alias.documentation {
            alias_docs.insert(alias.name.clone(), documentation.clone());
        }
    }

    for protocol in &parsed.protocol_declarations {
        let fields = protocol.methods.iter().map(|method| {
            let signature = super::type_from_tua_type(&crate::parser::parse_tua_type_text(
                &format!("({}) -> {}", method.parameters, method.return_type),
            ));
            let signature = match signature {
                Type::Function(mut function) => {
                    function.parameters.insert(
                        0,
                        super::FunctionParameter::named("self", Type::alias(protocol.name.clone())),
                    );
                    function.required_parameter_count += 1;
                    Type::Function(function)
                }
                other => other,
            };
            (method.name.clone(), TableField::required(signature))
        });
        aliases.insert(protocol.name.clone(), Type::table(fields));
    }

    for (name, ty) in builtin_aliases {
        aliases.entry(name).or_insert(ty);
    }

    register_class_aliases(parsed, &alias_names, &mut aliases, &mut diagnostics);

    let mut local_contracts = LocalContracts::default();
    for local in &parsed.local_annotations {
        let local_type = local.type_span.map_or_else(
            || local.ty.clone(),
            |span| {
                crate::parser::bind_generic_parameter_references(
                    &local.ty,
                    &generic_parameters_visible_at(parsed, span.start),
                )
            },
        );
        let ty = resolve_tua_type(
            &parsed.path,
            &local_type,
            local.span,
            &alias_names,
            &mut diagnostics,
            &parsed.class_declarations,
        );
        if let Some(span) = local.name_span {
            local_contracts
                .types
                .insert((span.start.line, span.start.column), ty.clone());
            local_contracts.names.insert(local.name.clone());
        }
        locals.insert(local.name.clone(), ty);
    }

    if !parsed.function_annotations.is_empty()
        || !local_contracts.types.is_empty()
        || parsed
            .lua_doc_annotations
            .iter()
            .any(|doc| matches!(doc.kind, LuaDocAnnotationKind::Type(_)))
    {
        local_contracts.scope = Some(Arc::new(crate::scope_map_parsed_source(parsed)));
    }

    for function in &parsed.function_annotations {
        functions.push(resolve_function_annotation(
            &parsed.path,
            function,
            &alias_names,
            &class_names,
            &parsed.class_declarations,
            &mut diagnostics,
        ));
    }

    let mut lua_doc_state = LuaDocLoweringState {
        local_contracts: &mut local_contracts,
        locals: &mut locals,
        local_docs: &mut local_docs,
        functions: &mut functions,
        diagnostics: &mut diagnostics,
    };
    lower_lua_doc_annotations_into_environment(parsed, &alias_names, &mut lua_doc_state);

    // Contextual LOVE declarations reuse the generated catalog, while explicit
    // native/LuaDoc contracts and locally shadowed love tables remain authoritative.
    for (annotation, function) in parsed.function_annotations.iter().zip(functions.iter_mut()) {
        let Some(label) = annotation
            .name
            .as_deref()
            .and_then(|name| name.strip_prefix("love."))
        else {
            continue;
        };
        if annotation.name_span.is_some_and(|span| {
            local_contracts
                .scope
                .as_ref()
                .is_some_and(|scope| scope.has_non_builtin_binding_at("love", span.start))
        }) {
            continue;
        }
        let Some(expected) =
            lua::love_callback_item(label).and_then(crate::semantic::typed_function_from_lua_item)
        else {
            continue;
        };
        for (index, parameter) in annotation.signature_parameters.iter().enumerate() {
            if parameter.ty.is_some()
                || function
                    .parameters
                    .iter()
                    .any(|typed| typed.name == parameter.name)
            {
                continue;
            }
            if let Some(context) = expected.parameters.get(index) {
                function.parameters.push(TypedParameter {
                    name: parameter.name.clone(),
                    ty: context.ty.clone(),
                    documentation: context.documentation.clone(),
                });
            }
        }
        function.parameters.sort_by_key(|typed| {
            annotation
                .signature_parameters
                .iter()
                .position(|parameter| parameter.name == typed.name)
        });
    }

    let inference_locals = locals
        .iter()
        .filter(|(name, _)| !local_contracts.names.contains(*name))
        .map(|(name, ty)| (name.clone(), ty.clone()))
        .collect();
    let function_origins = function_origins(parsed, &functions);
    let inferred_returns = infer_function_return_types(
        parsed,
        &aliases,
        &inference_locals,
        &functions,
        &local_contracts,
    );

    for (index, function) in functions.iter_mut().enumerate() {
        if !function.return_types.is_empty() || function.return_type.is_some() {
            continue;
        }

        let Some(name) = &function.name else {
            continue;
        };

        if let Some(returns) = inferred_returns.get(&function_inference_key(
            name,
            function_origins
                .get(index)
                .and_then(|origin| origin.body_span),
        )) {
            function.return_types = returns.types.clone();
            function.return_type = function.return_types.first().cloned();
            function.guarded_return_types = returns.guarded.clone();
        }
    }

    register_class_constructor_types(parsed, &mut functions);
    register_class_method_types(parsed, &mut aliases, &functions);
    let generic_classes = generic_class_definitions(parsed, &aliases);
    let generic_protocol_views = parsed
        .function_annotations
        .iter()
        .flat_map(|function| function.generic_parameters.iter())
        .chain(
            parsed
                .class_declarations
                .iter()
                .flat_map(|class| class.generic_parameters.iter()),
        )
        .filter_map(|parameter| {
            parameter.protocol_bound.as_ref().map(|bound| {
                (
                    parameter.id.clone(),
                    Type::alias(bound.name.trim().to_owned()),
                )
            })
        })
        .collect();

    let shared_aliases = domains.shared_builtin_aliases();
    let aliases = if aliases == *shared_aliases {
        shared_aliases
    } else {
        Arc::new(aliases)
    };
    let builtin_locals = crate::builtin_module::builtin_locals(parsed);
    // During builtin inference this list is intentionally empty. Do not seed
    // the catalog until all builtin modules have finished their own inference.
    let can_share_locals =
        locals.is_empty() && builtin_locals.len() == crate::builtin_module::MODULES.len();
    let locals = builtin_locals
        .into_iter()
        .chain(locals)
        .chain(domains.builtin_locals())
        .collect::<BTreeMap<_, _>>();
    let locals = if can_share_locals {
        domains.shared_builtin_locals(locals)
    } else {
        Arc::new(locals)
    };

    let environment = TypeEnvironment {
        path: parsed.path.clone(),
        aliases,
        generic_aliases: Arc::new(generic_aliases),
        generic_alias_bindings: Arc::new(generic_alias_bindings),
        generic_classes: Arc::new(generic_classes),
        generic_protocol_views: Arc::new(generic_protocol_views),
        alias_docs,
        class_names,
        class_parents: parsed
            .class_declarations
            .iter()
            .filter_map(|class| {
                class
                    .parent
                    .as_ref()
                    .map(|parent| (class.name.clone(), parent.clone()))
            })
            .collect(),
        class_protocols: parsed
            .class_declarations
            .iter()
            .map(|class| {
                (
                    class.name.clone(),
                    class
                        .implemented_protocols
                        .iter()
                        .map(|(name, _)| name.clone())
                        .collect(),
                )
            })
            .collect(),
        protocol_names: parsed
            .protocol_declarations
            .iter()
            .map(|protocol| protocol.name.clone())
            .collect(),
        protocol_identities: BTreeMap::new(),
        protocol_conformance_identities: BTreeSet::new(),
        locals,
        local_docs,
        local_contracts,
        function_declarations: function_origins
            .iter()
            .enumerate()
            .filter_map(|(index, origin)| {
                origin
                    .name_span
                    .map(|span| ((span.start.line, span.start.column), index))
            })
            .collect(),
        functions,
        deprecated_accesses: BTreeMap::new(),
        must_use_accesses: BTreeSet::new(),
        exact_module_bindings: BTreeMap::new(),
        project_module_signatures: BTreeMap::new(),
        required_module_types: None,
        table_remove_overridden: lua::table_remove_is_overridden(parsed),
        assert_overridden: lua::global_name_is_assigned(parsed, "assert"),
        table_insert_overridden: lua::table_member_is_overridden(parsed, "insert"),
        table_sort_overridden: lua::table_member_is_overridden(parsed, "sort"),
        project_module_globals: BTreeSet::new(),
        busted_globals: None,
    };
    diagnostics.extend(environment.generic_alias_diagnostics(parsed));
    (environment, diagnostics)
}

fn generic_class_definitions(
    parsed: &ParsedSource,
    aliases: &BTreeMap<String, Type>,
) -> HashMap<GenericDeclarationId, GenericClassDefinition> {
    let alias_names = aliases.keys().cloned().collect::<BTreeSet<_>>();
    let class_context =
        TypeResolutionContext::with_classes(&parsed.path, &parsed.class_declarations);
    parsed
        .class_declarations
        .iter()
        .filter_map(|class| {
            let Type::Table(full_body) = aliases.get(&class.name)?.clone() else {
                return None;
            };
            let declared_fields = class
                .fields
                .iter()
                .map(|field| field.name.as_str())
                .chain(class.methods.iter().map(|method| method.name.as_str()))
                .collect::<BTreeSet<_>>();
            let body = TableShape::from_fields(full_body.fields().filter_map(|(name, field)| {
                declared_fields
                    .contains(name)
                    .then(|| (name.to_owned(), field.clone()))
            }));
            let id = generic_class_identity(&parsed.path, class);
            Some((
                id.clone(),
                GenericClassDefinition {
                    id,
                    parameters: class
                        .generic_parameters
                        .iter()
                        .map(|parameter| parameter.id.clone())
                        .collect(),
                    generic_parameters: class.generic_parameters.clone(),
                    parent: class.parent_application.as_ref().map(|application| {
                        let context = TypeResolutionContext::for_class(
                            &parsed.path,
                            &parsed.class_declarations,
                            class,
                        );
                        if let Some(parent_identity) =
                            class_context.class_bindings.get(&application.name)
                        {
                            Type::GenericClassApplication {
                                identity: parent_identity.clone(),
                                name: application.name.clone(),
                                arguments: application
                                    .arguments
                                    .iter()
                                    .map(|argument| {
                                        resolve_tua_type_with_context(
                                            &parsed.path,
                                            &argument.ty,
                                            Some(argument.span),
                                            &alias_names,
                                            &mut Vec::new(),
                                            &context,
                                        )
                                    })
                                    .collect(),
                                span: Some(application.span),
                            }
                        } else {
                            Type::Alias(application.name.clone())
                        }
                    }),
                    body: Box::new(Type::Table(body)),
                },
            ))
        })
        .collect()
}

fn register_class_constructor_types(parsed: &ParsedSource, functions: &mut [TypedFunction]) {
    for class in &parsed.class_declarations {
        let constructor_name = format!("{}_new", class.name);
        let Some(constructor) = functions
            .iter_mut()
            .find(|function| function.name.as_deref() == Some(constructor_name.as_str()))
        else {
            continue;
        };
        let instance_type = if class.generic_parameters.is_empty() {
            Type::alias(class.name.clone())
        } else {
            let identity = generic_class_identity(&parsed.path, class);
            constructor.generic_parameters = class.generic_parameters.clone();
            Type::GenericClassApplication {
                identity,
                name: class.name.clone(),
                arguments: class
                    .generic_parameters
                    .iter()
                    .map(|parameter| Type::GenericParameter {
                        id: parameter.id.clone(),
                        name: parameter.name.clone(),
                    })
                    .collect(),
                span: Some(class.name_span),
            }
        };

        constructor.return_type = Some(instance_type.clone());
        constructor.return_types = vec![instance_type];
    }
}

fn collect_alias_names(
    parsed: &ParsedSource,
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    for alias in &parsed.type_aliases {
        if !names.insert(alias.name.clone()) {
            diagnostics.push(type_diagnostic(
                &parsed.path,
                alias.span,
                format!("duplicate type alias `{}`", alias.name),
                DiagnosticCode::TYPE_DUPLICATE_ALIAS,
            ));
        }
    }

    for import in &parsed.type_imports {
        if import.reexport {
            continue;
        }
        if !import.namespace.is_empty() && !names.insert(import.namespace.clone()) {
            diagnostics.push(type_diagnostic(
                &parsed.path,
                Some(import.namespace_span),
                format!(
                    "type import name `{}` conflicts with a type alias",
                    import.namespace
                ),
                DiagnosticCode::TYPE_IMPORT_COLLISION,
            ));
        }
        for binding in &import.bindings {
            if !names.insert(binding.local.clone()) {
                diagnostics.push(type_diagnostic(
                    &parsed.path,
                    Some(binding.local_span),
                    format!(
                        "type import name `{}` conflicts with a type alias",
                        binding.local
                    ),
                    DiagnosticCode::TYPE_IMPORT_COLLISION,
                ));
            }
        }
    }

    names
}

fn collect_class_names(parsed: &ParsedSource, names: &mut BTreeSet<String>) {
    for class in &parsed.class_declarations {
        // Class normalization emits a synthetic empty alias with this name.
        // The parsed class declaration replaces that placeholder below.
        names.insert(class.name.clone());
    }
}

fn register_class_aliases(
    parsed: &ParsedSource,
    alias_names: &BTreeSet<String>,
    aliases: &mut BTreeMap<String, Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for class in &parsed.class_declarations {
        aliases.insert(
            class.name.clone(),
            class_alias_type(parsed, &class.name, alias_names, diagnostics),
        );
    }
}

fn class_alias_type(
    parsed: &ParsedSource,
    class_name: &str,
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Type {
    let Some(class) = parsed
        .class_declarations
        .iter()
        .find(|class| class.name == class_name)
    else {
        return Type::Any;
    };
    let context = TypeResolutionContext::for_class(&parsed.path, &parsed.class_declarations, class);
    let mut fields = BTreeMap::new();
    collect_class_alias_fields(
        parsed,
        class_name,
        alias_names,
        diagnostics,
        &context,
        &mut fields,
    );
    let mut table = super::TableShape::from_fields(fields);
    table.mark_nominal(
        nominal_class_identity(&parsed.path, class_name),
        class_name,
        class_nominal_supertypes(parsed, class_name),
    );

    Type::Table(table)
}

fn class_nominal_supertypes(parsed: &ParsedSource, class_name: &str) -> Vec<String> {
    let mut supertypes = Vec::new();
    let mut current = class_name;
    let mut visited = BTreeSet::new();

    while visited.insert(current.to_owned()) {
        let Some(parent) = parsed
            .class_declarations
            .iter()
            .find(|class| class.name == current)
            .and_then(|class| class.parent.as_deref())
        else {
            break;
        };

        supertypes.push(nominal_class_identity(&parsed.path, parent));
        current = parent;
    }

    supertypes
}

fn collect_class_alias_fields(
    parsed: &ParsedSource,
    class_name: &str,
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    context: &TypeResolutionContext,
    fields: &mut BTreeMap<String, TableField>,
) {
    let Some(class) = parsed
        .class_declarations
        .iter()
        .find(|candidate| candidate.name == class_name)
    else {
        return;
    };

    if let Some(parent) = &class.parent {
        collect_class_alias_fields(parsed, parent, alias_names, diagnostics, context, fields);
    }

    for field in &class.fields {
        fields.insert(
            field.name.clone(),
            TableField::required(resolve_tua_type_with_context(
                &parsed.path,
                &field.parsed_ty,
                Some(field.span),
                alias_names,
                diagnostics,
                context,
            )),
        );
    }

    for method in &class.methods {
        fields.insert(method.name.clone(), TableField::required(Type::Any));
    }
}

fn register_class_method_types(
    parsed: &ParsedSource,
    aliases: &mut BTreeMap<String, Type>,
    functions: &[TypedFunction],
) {
    for class in &parsed.class_declarations {
        let Some(Type::Table(mut table)) = aliases.get(&class.name).cloned() else {
            continue;
        };

        collect_class_method_types(parsed, &class.name, functions, &mut table);
        aliases.insert(class.name.clone(), Type::Table(table));
    }
}

fn collect_class_method_types(
    parsed: &ParsedSource,
    class_name: &str,
    functions: &[TypedFunction],
    table: &mut super::TableShape,
) {
    let Some(class) = parsed
        .class_declarations
        .iter()
        .find(|candidate| candidate.name == class_name)
    else {
        return;
    };

    if let Some(parent) = &class.parent {
        collect_class_method_types(parsed, parent, functions, table);
    }

    for method in &class.methods {
        let generated_name = format!("{}_{}", class.name, method.name);
        let ty = functions
            .iter()
            .find(|function| function.name.as_deref() == Some(generated_name.as_str()))
            .and_then(TypedFunction::as_type)
            .unwrap_or(Type::Any);

        table.insert(method.name.clone(), TableField::required(ty));
    }
}

fn resolve_function_annotation(
    path: &Path,
    function: &ParsedFunctionAnnotation,
    alias_names: &BTreeSet<String>,
    class_names: &BTreeSet<String>,
    class_declarations: &[crate::classes::ClassDeclaration],
    diagnostics: &mut Vec<Diagnostic>,
) -> TypedFunction {
    let implicit_self_type = function
        .name
        .as_deref()
        .and_then(|name| function_self_type(name, alias_names));
    let class_owner = function
        .name
        .as_deref()
        .and_then(|name| generated_class_function_owner(name, class_names));
    let context = class_owner
        .as_deref()
        .and_then(|owner| class_declarations.iter().find(|class| class.name == owner))
        .map(|class| TypeResolutionContext::for_class(path, class_declarations, class))
        .unwrap_or_else(|| {
            TypeResolutionContext::from_self_type(
                implicit_self_type
                    .clone()
                    .or_else(|| class_owner.clone().map(Type::alias)),
            )
        });
    let mut parameters = function
        .parameters
        .iter()
        .map(|parameter| TypedParameter {
            name: parameter.name.clone(),
            ty: resolve_tua_type_with_context(
                path,
                &parameter.ty,
                parameter.span,
                alias_names,
                diagnostics,
                &context,
            ),
            documentation: None,
        })
        .collect::<Vec<_>>();
    if class_owner.is_some()
        && parameters
            .first()
            .is_some_and(|parameter| parameter.name == "self")
    {
        if let Some(self_type) = &context.self_type {
            parameters[0].ty = self_type.clone();
        }
    }
    let mut parameter_count = function.parameter_count;
    let mut required_parameter_count = function.required_parameter_count;

    if let Some(receiver_type) = implicit_self_type {
        parameters.insert(
            0,
            TypedParameter {
                name: "self".to_owned(),
                ty: receiver_type,
                documentation: None,
            },
        );
        parameter_count += 1;
        required_parameter_count += 1;
    }

    let return_types = match function.return_type.as_ref() {
        Some(TuaType::Tuple(items)) => items
            .iter()
            .map(|ty| {
                resolve_tua_type_with_context(
                    path,
                    ty,
                    function.span,
                    alias_names,
                    diagnostics,
                    &context,
                )
            })
            .collect(),
        Some(ty) => vec![resolve_tua_type_with_context(
            path,
            ty,
            function.span,
            alias_names,
            diagnostics,
            &context,
        )],
        None => Vec::new(),
    };
    let return_type = return_types.first().cloned();
    let mut generic_parameters = function.generic_parameters.clone();
    if let (Some(owner), Some(name)) = (class_owner.as_deref(), function.name.as_deref()) {
        if name == format!("{owner}_init") {
            if let Some(class) = class_declarations.iter().find(|class| class.name == owner) {
                generic_parameters.extend(class.generic_parameters.iter().cloned());
            }
        }
    }

    TypedFunction {
        name: function.name.clone(),
        generic_parameters,
        parameters,
        parameter_count,
        required_parameter_count,
        has_varargs: function.has_varargs,
        vararg_type: function.vararg_type.as_ref().map(|ty| {
            resolve_tua_type_with_context(
                path,
                ty,
                function.span,
                alias_names,
                diagnostics,
                &context,
            )
        }),
        return_type,
        guarded_return_types: None,
        return_types,
        documentation: None,
        overloads: Vec::new(),
    }
}

fn generated_class_function_owner(name: &str, class_names: &BTreeSet<String>) -> Option<String> {
    class_names
        .iter()
        .filter(|class| name.starts_with(&format!("{class}_")))
        .max_by_key(|class| class.len())
        .cloned()
}

fn function_self_type(name: &str, alias_names: &BTreeSet<String>) -> Option<Type> {
    let (owner, _, true) = function_name_table_field(name)? else {
        return None;
    };

    Some(receiver_type_for_owner(&owner, alias_names))
}

fn receiver_type_for_owner(owner: &str, alias_names: &BTreeSet<String>) -> Type {
    if alias_names.contains(owner) {
        Type::alias(owner)
    } else {
        Type::Any
    }
}

#[derive(Clone, Debug)]
struct LuaDocAliasDeclaration {
    name: String,
    ty: TuaType,
    span: SourceSpan,
    documentation: Option<String>,
}

fn lua_doc_alias_declarations(
    parsed: &ParsedSource,
    tua_alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<LuaDocAliasDeclaration> {
    let mut declarations = Vec::new();
    let mut names = tua_alias_names.clone();
    let mut current_class: Option<LuaDocClassBuilder> = None;

    for annotation in &parsed.lua_doc_annotations {
        match &annotation.kind {
            LuaDocAnnotationKind::Class(class) => {
                flush_lua_doc_class(
                    parsed,
                    &mut names,
                    &mut current_class,
                    &mut declarations,
                    diagnostics,
                );
                current_class = Some(LuaDocClassBuilder {
                    name: class.name.clone(),
                    span: annotation.span,
                    documentation: annotation.documentation.clone(),
                    fields: Vec::new(),
                });
            }
            LuaDocAnnotationKind::Field(field) => {
                if let Some(class) = &mut current_class {
                    class.fields.push(field.clone());
                }
            }
            LuaDocAnnotationKind::Alias(alias) => {
                flush_lua_doc_class(
                    parsed,
                    &mut names,
                    &mut current_class,
                    &mut declarations,
                    diagnostics,
                );

                if !names.insert(alias.name.clone()) {
                    diagnostics.push(type_diagnostic(
                        &parsed.path,
                        Some(annotation.span),
                        format!("duplicate type alias `{}`", alias.name),
                        DiagnosticCode::TYPE_DUPLICATE_ALIAS,
                    ));
                    continue;
                }

                declarations.push(LuaDocAliasDeclaration {
                    name: alias.name.clone(),
                    ty: alias.ty.clone(),
                    span: annotation.span,
                    documentation: annotation.documentation.clone(),
                });
            }
            _ => {
                flush_lua_doc_class(
                    parsed,
                    &mut names,
                    &mut current_class,
                    &mut declarations,
                    diagnostics,
                );
            }
        }
    }

    flush_lua_doc_class(
        parsed,
        &mut names,
        &mut current_class,
        &mut declarations,
        diagnostics,
    );

    declarations
}

#[derive(Clone, Debug)]
struct LuaDocClassBuilder {
    name: String,
    span: SourceSpan,
    documentation: Option<String>,
    fields: Vec<crate::LuaDocField>,
}

fn flush_lua_doc_class(
    parsed: &ParsedSource,
    names: &mut BTreeSet<String>,
    current: &mut Option<LuaDocClassBuilder>,
    declarations: &mut Vec<LuaDocAliasDeclaration>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(class) = current.take() else {
        return;
    };

    if !names.insert(class.name.clone()) {
        diagnostics.push(type_diagnostic(
            &parsed.path,
            Some(class.span),
            format!("duplicate type alias `{}`", class.name),
            DiagnosticCode::TYPE_DUPLICATE_ALIAS,
        ));
        return;
    }

    declarations.push(LuaDocAliasDeclaration {
        name: class.name,
        ty: TuaType::Table(
            class
                .fields
                .into_iter()
                .map(|field| ParsedTableFieldType {
                    name: field.name,
                    optional: field.optional,
                    ty: field.ty,
                    name_span: Some(field.name_span),
                    span: Some(field.type_span),
                })
                .collect(),
        ),
        span: class.span,
        documentation: class.documentation,
    });
}

struct LuaDocLoweringState<'a> {
    local_contracts: &'a mut LocalContracts,
    locals: &'a mut BTreeMap<String, Type>,
    local_docs: &'a mut BTreeMap<String, String>,
    functions: &'a mut Vec<TypedFunction>,
    diagnostics: &'a mut Vec<Diagnostic>,
}

fn lower_lua_doc_annotations_into_environment(
    parsed: &ParsedSource,
    alias_names: &BTreeSet<String>,
    state: &mut LuaDocLoweringState<'_>,
) {
    for stmt in parsed.ast.nodes().stmts() {
        let Some(stmt_span) = stmt_code_span(parsed, stmt).or_else(|| node_span(stmt)) else {
            continue;
        };
        let docs = leading_lua_doc_annotations(parsed, stmt_span.start.line);

        if docs.is_empty() {
            collect_nested_lua_doc_annotations(parsed, stmt, alias_names, state);
            continue;
        }

        lower_lua_doc_annotations_for_stmt(parsed, stmt, stmt_span, &docs, alias_names, state);
        collect_nested_lua_doc_annotations(parsed, stmt, alias_names, state);
    }

    lower_line_based_lua_doc_annotations_into_environment(parsed, alias_names, state);
}

fn collect_nested_lua_doc_annotations(
    parsed: &ParsedSource,
    stmt: &ast::Stmt,
    alias_names: &BTreeSet<String>,
    state: &mut LuaDocLoweringState<'_>,
) {
    match stmt {
        ast::Stmt::LocalFunction(function) => {
            lower_lua_doc_annotations_in_block(parsed, function.body().block(), alias_names, state);
        }
        ast::Stmt::FunctionDeclaration(function) => {
            lower_lua_doc_annotations_in_block(parsed, function.body().block(), alias_names, state);
        }
        ast::Stmt::Do(block) => {
            lower_lua_doc_annotations_in_block(parsed, block.block(), alias_names, state);
        }
        ast::Stmt::If(if_stmt) => {
            lower_lua_doc_annotations_in_block(parsed, if_stmt.block(), alias_names, state);

            if let Some(else_ifs) = if_stmt.else_if() {
                for else_if in else_ifs {
                    lower_lua_doc_annotations_in_block(parsed, else_if.block(), alias_names, state);
                }
            }

            if let Some(else_block) = if_stmt.else_block() {
                lower_lua_doc_annotations_in_block(parsed, else_block, alias_names, state);
            }
        }
        ast::Stmt::GenericFor(for_stmt) => {
            lower_lua_doc_annotations_in_block(parsed, for_stmt.block(), alias_names, state);
        }
        ast::Stmt::NumericFor(for_stmt) => {
            lower_lua_doc_annotations_in_block(parsed, for_stmt.block(), alias_names, state);
        }
        ast::Stmt::Repeat(repeat) => {
            lower_lua_doc_annotations_in_block(parsed, repeat.block(), alias_names, state);
        }
        ast::Stmt::While(while_stmt) => {
            lower_lua_doc_annotations_in_block(parsed, while_stmt.block(), alias_names, state);
        }
        _ => {}
    }
}

fn lower_lua_doc_annotations_in_block(
    parsed: &ParsedSource,
    block: &ast::Block,
    alias_names: &BTreeSet<String>,
    state: &mut LuaDocLoweringState<'_>,
) {
    for stmt in block.stmts() {
        let Some(stmt_span) = stmt_code_span(parsed, stmt).or_else(|| node_span(stmt)) else {
            continue;
        };
        let docs = leading_lua_doc_annotations(parsed, stmt_span.start.line);

        if !docs.is_empty() {
            lower_lua_doc_annotations_for_stmt(parsed, stmt, stmt_span, &docs, alias_names, state);
        }

        collect_nested_lua_doc_annotations(parsed, stmt, alias_names, state);
    }
}

fn lower_lua_doc_annotations_for_stmt(
    parsed: &ParsedSource,
    stmt: &ast::Stmt,
    stmt_span: SourceSpan,
    docs: &[&LuaDocAnnotation],
    alias_names: &BTreeSet<String>,
    state: &mut LuaDocLoweringState<'_>,
) {
    if let Some((name, ty, documentation)) =
        lua_doc_local_for_stmt(parsed, stmt, docs, alias_names, state.diagnostics)
    {
        let declaration = match stmt {
            ast::Stmt::LocalAssignment(local) => local
                .names()
                .iter()
                .find(|token| token_text(token) == name)
                .and_then(node_span)
                .map(|span| span.start),
            _ => None,
        };
        merge_doc_local(
            parsed,
            &name,
            ty,
            documentation,
            stmt_span,
            declaration,
            state,
        );
    }

    if let Some(function) =
        lua_doc_function_for_stmt(parsed, stmt, docs, alias_names, state.diagnostics)
    {
        merge_doc_function(parsed, function, stmt_span, state);
    }
}

fn lower_line_based_lua_doc_annotations_into_environment(
    parsed: &ParsedSource,
    alias_names: &BTreeSet<String>,
    state: &mut LuaDocLoweringState<'_>,
) {
    for group in lua_doc_groups(parsed) {
        if let Some((name, ty, documentation)) =
            lua_doc_local_for_code_line(parsed, &group, alias_names, state.diagnostics)
        {
            let parser_position = parsed
                .raw
                .parser_position_for_source(group.code_position)
                .unwrap_or(group.code_position);
            let declaration = state
                .local_contracts
                .scope
                .as_deref()
                .and_then(|scope| {
                    scope.declarations().iter().find(|binding| {
                        binding.name == name
                            && binding.kind == crate::ScopeDeclarationKind::Local
                            && binding.name_span.start.line == parser_position.line
                    })
                })
                .map(|binding| binding.name_span.start);
            merge_doc_local(
                parsed,
                &name,
                ty,
                documentation,
                group.span,
                declaration,
                state,
            );
        }

        if let Some(function) =
            lua_doc_function_for_code_line(parsed, &group, alias_names, state.diagnostics)
        {
            merge_doc_function(parsed, function, group.span, state);
        }
    }
}

struct LuaDocGroup<'a> {
    code_position: crate::SourcePosition,
    annotations: Vec<&'a LuaDocAnnotation>,
    code: &'a str,
    span: SourceSpan,
}

fn lua_doc_generic_parameters(
    parsed: &ParsedSource,
    name: &str,
    annotations: &[&LuaDocAnnotation],
    declaration: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<GenericParameter> {
    // Both AST-backed and line-based LuaDoc lowering see the same source
    // annotations. Anchor binder identity to the generic tag so merging those
    // independently built views does not report `T` as conflicting with itself.
    let declaration = annotations
        .iter()
        .find(|annotation| matches!(annotation.kind, LuaDocAnnotationKind::Generic(_)))
        .map(|annotation| annotation.span)
        .unwrap_or(declaration);
    let declaration_id = GenericDeclarationId {
        module: crate::vfs::normalize_path(&parsed.path),
        kind: GenericDeclarationKind::Function,
        name: name.to_owned(),
        declaration,
    };
    let mut parameters = Vec::new();
    let mut names = BTreeSet::new();
    let has_generic_tag = annotations
        .iter()
        .any(|annotation| matches!(annotation.kind, LuaDocAnnotationKind::Generic(_)));
    if has_generic_tag && name.contains(':') {
        let span = annotations
            .iter()
            .find(|annotation| matches!(annotation.kind, LuaDocAnnotationKind::Generic(_)))
            .map(|annotation| annotation.span)
            .unwrap_or(declaration);
        let diagnostic = Diagnostic::error(
            &parsed.path,
            span.start.line,
            span.start.column,
            "LuaDoc generic methods are not supported; use a generic free function",
        )
        .with_code(DiagnosticCode::SYNTAX_DOC_ANNOTATION);
        if !diagnostics.contains(&diagnostic) {
            diagnostics.push(diagnostic);
        }
    }
    for generic in annotations
        .iter()
        .filter_map(|annotation| match &annotation.kind {
            LuaDocAnnotationKind::Generic(generic) => Some(generic),
            _ => None,
        })
    {
        for parameter in &generic.parameters {
            if let Some(bound_span) = parameter.bound_span {
                let diagnostic = Diagnostic::error(
                        &parsed.path,
                        bound_span.start.line,
                        bound_span.start.column,
                        "LuaDoc generic bounds are not supported yet; use an unbounded `---@generic T` parameter",
                    )
                    .with_code(DiagnosticCode::SYNTAX_DOC_ANNOTATION);
                if !diagnostics.contains(&diagnostic) {
                    diagnostics.push(diagnostic);
                }
            }
            if !names.insert(parameter.name.clone()) {
                let diagnostic = Diagnostic::error(
                    &parsed.path,
                    parameter.name_span.start.line,
                    parameter.name_span.start.column,
                    format!("duplicate LuaDoc generic parameter `{}`", parameter.name),
                )
                .with_code(DiagnosticCode::SYNTAX_DOC_ANNOTATION);
                if !diagnostics.contains(&diagnostic) {
                    diagnostics.push(diagnostic);
                }
                continue;
            }
            let ordinal = parameters.len();
            parameters.push(GenericParameter {
                id: TypeParameterId {
                    declaration: declaration_id.clone(),
                    ordinal,
                },
                name: parameter.name.clone(),
                name_span: parameter.name_span,
                protocol_bound: None,
                protocol_bound_error: None,
            });
        }
    }
    parameters
}

fn lua_doc_groups(parsed: &ParsedSource) -> Vec<LuaDocGroup<'_>> {
    let mut by_line = BTreeMap::new();

    for annotation in &parsed.lua_doc_annotations {
        by_line.insert(annotation.span.start.line, annotation);
    }

    let mut groups = Vec::new();
    let mut pending = Vec::new();
    let mut pending_span = None;

    for (index, line) in parsed.source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim_start();

        if let Some(annotation) = by_line.get(&line_number) {
            pending_span.get_or_insert(annotation.span);
            pending.push(*annotation);
            continue;
        }

        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with("--") {
            pending.clear();
            pending_span = None;
            continue;
        }

        if !pending.is_empty() {
            let leading = line.len() - trimmed.len();
            groups.push(LuaDocGroup {
                code_position: crate::SourcePosition {
                    line: line_number,
                    column: leading + 1,
                },
                annotations: std::mem::take(&mut pending),
                code: trimmed,
                span: pending_span.unwrap_or(SourceSpan {
                    start: crate::SourcePosition {
                        line: line_number,
                        column: leading + 1,
                    },
                    end: crate::SourcePosition {
                        line: line_number,
                        column: line.len() + 1,
                    },
                }),
            });
            pending_span = None;
        }
    }

    groups
}

fn lua_doc_local_for_code_line(
    parsed: &ParsedSource,
    group: &LuaDocGroup<'_>,
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(String, Type, Option<String>)> {
    let type_annotation =
        group
            .annotations
            .iter()
            .find_map(|annotation| match &annotation.kind {
                LuaDocAnnotationKind::Type(type_annotation) => Some((*annotation, type_annotation)),
                _ => None,
            })?;
    let name = annotated_value_name(group.code)?;
    let ty = resolve_tua_type(
        &parsed.path,
        &type_annotation.1.ty,
        Some(type_annotation.1.type_span),
        alias_names,
        diagnostics,
        &parsed.class_declarations,
    );

    Some((name, ty, type_annotation.0.documentation.clone()))
}

fn lua_doc_function_for_code_line(
    parsed: &ParsedSource,
    group: &LuaDocGroup<'_>,
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<TypedFunction> {
    if !group.annotations.iter().any(|annotation| {
        matches!(
            annotation.kind,
            LuaDocAnnotationKind::Param(_)
                | LuaDocAnnotationKind::Return(_)
                | LuaDocAnnotationKind::Overload(_)
                | LuaDocAnnotationKind::Vararg(_)
        )
    }) {
        return None;
    }

    let name = annotated_function_name(group.code)?;
    let context = TypeResolutionContext::from_self_type(function_self_type(&name, alias_names));
    let declaration = SourceSpan {
        start: group.code_position,
        end: crate::SourcePosition {
            line: group.code_position.line,
            column: group.code_position.column + group.code.len(),
        },
    };
    let generic_parameters =
        lua_doc_generic_parameters(parsed, &name, &group.annotations, declaration, diagnostics);
    let parameter_names = group
        .annotations
        .iter()
        .filter_map(|annotation| match &annotation.kind {
            LuaDocAnnotationKind::Param(param) => Some(param.name.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let param_annotations = lua_doc_param_annotations(&group.annotations);
    let parameters = parameter_names
        .into_iter()
        .map(|name| {
            let doc = param_annotations.get(&name);
            let ty = doc
                .map(|annotation| {
                    let ty = crate::parser::bind_generic_parameter_references(
                        &annotation.0.ty,
                        &generic_parameters,
                    );
                    resolve_tua_type_with_context(
                        &parsed.path,
                        &ty,
                        Some(annotation.0.type_span),
                        alias_names,
                        diagnostics,
                        &context,
                    )
                })
                .unwrap_or(Type::Any);

            TypedParameter {
                name,
                ty,
                documentation: doc.and_then(|annotation| annotation.1.clone()),
            }
        })
        .collect::<Vec<_>>();
    let return_types = group
        .annotations
        .iter()
        .find_map(|annotation| match &annotation.kind {
            LuaDocAnnotationKind::Return(return_annotation) => Some(
                return_annotation
                    .returns
                    .iter()
                    .map(|return_type| {
                        let bound_type = crate::parser::bind_generic_parameter_references(
                            &return_type.ty,
                            &generic_parameters,
                        );
                        resolve_lua_doc_return_type(
                            &parsed.path,
                            &LuaDocReturnType {
                                ty: bound_type,
                                type_span: return_type.type_span,
                            },
                            alias_names,
                            diagnostics,
                            &context,
                        )
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .unwrap_or_default();
    let return_type = return_types.first().cloned();
    let vararg_type = group
        .annotations
        .iter()
        .find_map(|annotation| match &annotation.kind {
            LuaDocAnnotationKind::Vararg(vararg) => {
                let ty = crate::parser::bind_generic_parameter_references(
                    &vararg.ty,
                    &generic_parameters,
                );
                Some(resolve_tua_type_with_context(
                    &parsed.path,
                    &ty,
                    Some(vararg.type_span),
                    alias_names,
                    diagnostics,
                    &context,
                ))
            }
            _ => None,
        });

    let required_parameter_count = lua_doc_required_parameter_count(
        parameters
            .iter()
            .map(|parameter| param_annotations.get(&parameter.name).map(|doc| doc.0)),
    );

    Some(TypedFunction {
        name: Some(name),
        generic_parameters,
        parameter_count: parameters.len(),
        required_parameter_count,
        parameters,
        has_varargs: vararg_type.is_some(),
        vararg_type,
        return_type,
        guarded_return_types: None,
        return_types,
        documentation: lua_doc_documentation(&group.annotations),
        overloads: lua_doc_overloads(
            parsed,
            &group.annotations,
            alias_names,
            diagnostics,
            &context,
        ),
    })
}

fn annotated_value_name(code: &str) -> Option<String> {
    let code = code.strip_prefix("local ").unwrap_or(code).trim_start();
    let end = code
        .char_indices()
        .find_map(|(index, ch)| (!is_name_char(ch) && ch != '.').then_some(index))
        .unwrap_or(code.len());

    (end > 0).then(|| code[..end].trim().to_owned())
}

fn annotated_function_name(code: &str) -> Option<String> {
    let code = code.trim_start();

    if let Some(rest) = code.strip_prefix("local function ") {
        return take_name(rest);
    }

    if let Some(rest) = code.strip_prefix("function ") {
        return take_name(rest);
    }

    if let Some(rest) = code.strip_prefix("local ") {
        let (name, value) = rest.split_once('=')?;

        if value.trim_start().starts_with("function") {
            return Some(name.trim().to_owned());
        }
    }

    let (name, value) = code.split_once('=')?;

    value
        .trim_start()
        .starts_with("function")
        .then(|| name.trim().to_owned())
}

fn take_name(rest: &str) -> Option<String> {
    let end = rest
        .char_indices()
        .find_map(|(index, ch)| (!is_name_char(ch) && ch != '.' && ch != ':').then_some(index))
        .unwrap_or(rest.len());

    (end > 0).then(|| rest[..end].trim().to_owned())
}

fn is_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn lua_doc_local_for_stmt(
    parsed: &ParsedSource,
    stmt: &ast::Stmt,
    docs: &[&LuaDocAnnotation],
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(String, Type, Option<String>)> {
    let type_annotation = docs.iter().find_map(|annotation| match &annotation.kind {
        LuaDocAnnotationKind::Type(type_annotation) => Some((annotation, type_annotation)),
        _ => None,
    })?;
    let name = match stmt {
        ast::Stmt::LocalAssignment(local) => local.names().iter().next().map(token_text),
        ast::Stmt::Assignment(assignment) => assignment
            .variables()
            .iter()
            .next()
            .and_then(plain_field_chain_from_var)
            .map(|parts| parts.join(".")),
        _ => None,
    }?;
    let ty = resolve_tua_type(
        &parsed.path,
        &type_annotation.1.ty,
        Some(type_annotation.1.type_span),
        alias_names,
        diagnostics,
        &parsed.class_declarations,
    );

    Some((name, ty, type_annotation.0.documentation.clone()))
}

fn lua_doc_function_for_stmt(
    parsed: &ParsedSource,
    stmt: &ast::Stmt,
    docs: &[&LuaDocAnnotation],
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<TypedFunction> {
    match stmt {
        ast::Stmt::LocalFunction(function) => typed_function_from_lua_doc_body(
            parsed,
            Some(token_text(function.name())),
            function.body(),
            docs,
            alias_names,
            diagnostics,
        ),
        ast::Stmt::FunctionDeclaration(function) => typed_function_from_lua_doc_body(
            parsed,
            Some(function.name().to_string()),
            function.body(),
            docs,
            alias_names,
            diagnostics,
        ),
        ast::Stmt::LocalAssignment(local) => {
            let name = local.names().iter().next().map(token_text)?;
            let body = local
                .expressions()
                .iter()
                .next()
                .and_then(function_body_from_expression)?;

            typed_function_from_lua_doc_body(
                parsed,
                Some(name),
                body,
                docs,
                alias_names,
                diagnostics,
            )
        }
        ast::Stmt::Assignment(assignment) => {
            let name = assignment
                .variables()
                .iter()
                .next()
                .and_then(plain_field_chain_from_var)
                .map(|parts| parts.join("."))?;
            let body = assignment
                .expressions()
                .iter()
                .next()
                .and_then(function_body_from_expression)?;

            typed_function_from_lua_doc_body(
                parsed,
                Some(name),
                body,
                docs,
                alias_names,
                diagnostics,
            )
        }
        _ => None,
    }
}

fn function_body_from_expression(expression: &Expression) -> Option<&ast::FunctionBody> {
    match expression {
        Expression::Function(function) => Some(function.body()),
        Expression::Parentheses { expression, .. } => function_body_from_expression(expression),
        _ => None,
    }
}

pub(crate) fn typed_function_from_lua_doc_body(
    parsed: &ParsedSource,
    name: Option<String>,
    body: &ast::FunctionBody,
    docs: &[&LuaDocAnnotation],
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<TypedFunction> {
    let param_annotations = lua_doc_param_annotations(docs);
    let return_annotation = docs.iter().find_map(|annotation| match &annotation.kind {
        LuaDocAnnotationKind::Return(return_annotation) => Some(return_annotation),
        _ => None,
    });
    let vararg_annotation = docs.iter().find_map(|annotation| match &annotation.kind {
        LuaDocAnnotationKind::Vararg(vararg) => Some((*annotation, vararg)),
        _ => None,
    });

    if param_annotations.is_empty()
        && return_annotation.is_none()
        && vararg_annotation.is_none()
        && !docs
            .iter()
            .any(|annotation| matches!(annotation.kind, LuaDocAnnotationKind::Overload(_)))
    {
        return None;
    }

    let context = TypeResolutionContext::from_self_type(
        name.as_deref()
            .and_then(|name| function_self_type(name, alias_names)),
    );
    let generic_name = name.as_deref().unwrap_or("<anonymous-function>");
    let declaration = node_span(body).unwrap_or_else(|| {
        docs.iter()
            .find_map(|annotation| match annotation.kind {
                LuaDocAnnotationKind::Generic(_) => Some(annotation.span),
                _ => None,
            })
            .unwrap_or(SourceSpan {
                start: crate::SourcePosition { line: 1, column: 1 },
                end: crate::SourcePosition { line: 1, column: 1 },
            })
    });
    let generic_parameters =
        lua_doc_generic_parameters(parsed, generic_name, docs, declaration, diagnostics);
    let parameter_count = body
        .parameters()
        .iter()
        .filter(|parameter| matches!(parameter, ast::Parameter::Name(_)))
        .count();
    let has_varargs = body
        .parameters()
        .iter()
        .any(|parameter| matches!(parameter, ast::Parameter::Ellipsis(_)));
    let vararg_type = vararg_annotation.map(|(_, vararg)| {
        let ty = crate::parser::bind_generic_parameter_references(&vararg.ty, &generic_parameters);
        resolve_tua_type_with_context(
            &parsed.path,
            &ty,
            Some(vararg.type_span),
            alias_names,
            diagnostics,
            &context,
        )
    });
    let parameters = body
        .parameters()
        .iter()
        .filter_map(|parameter| {
            let ast::Parameter::Name(name) = parameter else {
                return None;
            };
            let name = token_text(name);
            let doc = param_annotations.get(&name);
            let ty = doc
                .map(|annotation| {
                    let ty = crate::parser::bind_generic_parameter_references(
                        &annotation.0.ty,
                        &generic_parameters,
                    );
                    resolve_tua_type_with_context(
                        &parsed.path,
                        &ty,
                        Some(annotation.0.type_span),
                        alias_names,
                        diagnostics,
                        &context,
                    )
                })
                .unwrap_or(Type::Any);

            Some(TypedParameter {
                name,
                ty,
                documentation: doc.and_then(|annotation| annotation.1.clone()),
            })
        })
        .collect::<Vec<_>>();
    let return_types = return_annotation
        .map(|annotation| {
            annotation
                .returns
                .iter()
                .map(|return_type| {
                    let ty = crate::parser::bind_generic_parameter_references(
                        &return_type.ty,
                        &generic_parameters,
                    );
                    resolve_lua_doc_return_type(
                        &parsed.path,
                        &LuaDocReturnType {
                            ty,
                            type_span: return_type.type_span,
                        },
                        alias_names,
                        diagnostics,
                        &context,
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let return_type = return_types.first().cloned();
    let documentation = lua_doc_documentation(docs);
    let required_parameter_count = lua_doc_required_parameter_count(
        parameters
            .iter()
            .map(|parameter| param_annotations.get(&parameter.name).map(|doc| doc.0)),
    );

    Some(TypedFunction {
        name,
        generic_parameters,
        parameters,
        parameter_count,
        required_parameter_count,
        has_varargs,
        vararg_type,
        return_type,
        guarded_return_types: None,
        return_types,
        documentation,
        overloads: lua_doc_overloads(parsed, docs, alias_names, diagnostics, &context),
    })
}

/// The parameters a call must supply: through the last one that is not
/// optional. A LuaDoc parameter is optional when written `name?` or typed with
/// `nil`, as in `T?`; an unannotated parameter stays required.
pub(crate) fn lua_doc_required_parameter_count<'a>(
    parameters: impl IntoIterator<Item = Option<&'a LuaDocParam>>,
) -> usize {
    parameters
        .into_iter()
        .enumerate()
        .filter(|(_, doc)| !doc.is_some_and(lua_doc_param_is_optional))
        .map(|(index, _)| index + 1)
        .last()
        .unwrap_or(0)
}

fn lua_doc_param_is_optional(param: &LuaDocParam) -> bool {
    fn allows_nil(ty: &TuaType) -> bool {
        match ty {
            TuaType::Named(name) => name == "nil",
            TuaType::Union(types) => types.iter().any(allows_nil),
            _ => false,
        }
    }

    param.optional || allows_nil(&param.ty)
}

fn lua_doc_param_annotations<'a>(
    docs: &'a [&'a LuaDocAnnotation],
) -> BTreeMap<String, (&'a LuaDocParam, Option<String>)> {
    docs.iter()
        .filter_map(|annotation| match &annotation.kind {
            LuaDocAnnotationKind::Param(param) => Some((
                param.name.clone(),
                (param, annotation.documentation.clone()),
            )),
            _ => None,
        })
        .collect()
}

fn lua_doc_overloads(
    parsed: &ParsedSource,
    docs: &[&LuaDocAnnotation],
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    context: &TypeResolutionContext,
) -> Vec<TypedFunctionOverload> {
    docs.iter()
        .filter_map(|annotation| match &annotation.kind {
            LuaDocAnnotationKind::Overload(overload) => overload_from_tua_type(
                parsed,
                &overload.ty,
                Some(overload.type_span),
                annotation.documentation.clone(),
                alias_names,
                diagnostics,
                context,
            ),
            _ => None,
        })
        .collect()
}

fn overload_from_tua_type(
    parsed: &ParsedSource,
    ty: &TuaType,
    span: Option<SourceSpan>,
    documentation: Option<String>,
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    context: &TypeResolutionContext,
) -> Option<TypedFunctionOverload> {
    let TuaType::Function {
        parameters,
        returns,
    } = ty
    else {
        return None;
    };
    let mut has_varargs = false;
    let mut vararg_type = None;
    let mut typed_parameters = Vec::new();

    for parameter in parameters {
        if function_type_parameter_is_vararg(parameter) {
            has_varargs = true;
            vararg_type = Some(resolve_tua_type_with_context(
                &parsed.path,
                &parameter.ty,
                span,
                alias_names,
                diagnostics,
                context,
            ));
            continue;
        }

        let ty = resolve_tua_type_with_context(
            &parsed.path,
            &parameter.ty,
            span,
            alias_names,
            diagnostics,
            context,
        );
        typed_parameters.push(TypedParameter {
            name: parameter.name.clone().unwrap_or_default(),
            ty: if parameter.optional {
                Type::union([ty, Type::nil()])
            } else {
                ty
            },
            documentation: None,
        });
    }

    let return_types = returns
        .iter()
        .map(|ty| {
            resolve_tua_type_with_context(&parsed.path, ty, span, alias_names, diagnostics, context)
        })
        .collect();

    Some(TypedFunctionOverload {
        parameters: typed_parameters,
        has_varargs,
        vararg_type,
        return_types,
        documentation,
    })
}

fn generic_parameters_visible_at(
    parsed: &ParsedSource,
    position: crate::SourcePosition,
) -> Vec<GenericParameter> {
    let mut functions = parsed
        .function_annotations
        .iter()
        .filter(|function| {
            function.span.is_some_and(|span| {
                (position.line, position.column) >= (span.start.line, span.start.column)
                    && (position.line, position.column) <= (span.end.line, span.end.column)
            })
        })
        .collect::<Vec<_>>();
    functions.sort_by_key(|function| {
        function.span.map_or((usize::MAX, usize::MAX), |span| {
            (
                span.end.line.saturating_sub(span.start.line),
                span.end.column.saturating_sub(span.start.column),
            )
        })
    });

    functions
        .into_iter()
        .flat_map(|function| function.generic_parameters.iter().cloned())
        .collect()
}

fn function_type_parameter_is_vararg(parameter: &FunctionTypeParameter) -> bool {
    parameter.variadic
        || parameter.name.as_deref() == Some("...")
        || matches!(&parameter.ty, TuaType::Named(name) if name == "...")
}

fn resolve_lua_doc_return_type(
    path: &Path,
    ty: &LuaDocReturnType,
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    context: &TypeResolutionContext,
) -> Type {
    resolve_tua_type_with_context(
        path,
        &ty.ty,
        Some(ty.type_span),
        alias_names,
        diagnostics,
        context,
    )
}

fn lua_doc_documentation(docs: &[&LuaDocAnnotation]) -> Option<String> {
    let documentation = docs
        .iter()
        .filter(|annotation| !matches!(annotation.kind, LuaDocAnnotationKind::Deprecated(_)))
        .filter_map(|annotation| annotation.documentation.as_deref())
        .collect::<Vec<_>>();

    (!documentation.is_empty()).then(|| documentation.join("\n"))
}

fn merge_doc_local(
    parsed: &ParsedSource,
    name: &str,
    ty: Type,
    documentation: Option<String>,
    span: SourceSpan,
    declaration: Option<crate::SourcePosition>,
    state: &mut LuaDocLoweringState<'_>,
) {
    let existing = match declaration {
        Some(position) => state.local_contracts.at_declaration(position),
        None => state.locals.get(name),
    };
    if let Some(existing) = existing {
        if existing != &ty {
            push_doc_conflict(
                state.diagnostics,
                doc_conflict_diagnostic(
                    &parsed.path,
                    span,
                    format!("conflicting annotations for `{name}`: `{existing}` vs `{ty}`"),
                ),
            );
        }
    } else {
        if let Some(position) = declaration {
            state.local_contracts.names.insert(name.to_owned());
            state
                .local_contracts
                .types
                .insert((position.line, position.column), ty.clone());
        }
        state.locals.insert(name.to_owned(), ty);
    }

    if let Some(documentation) = documentation {
        state
            .local_docs
            .entry(name.to_owned())
            .or_insert(documentation);
    }
}

fn merge_doc_function(
    parsed: &ParsedSource,
    function: TypedFunction,
    span: SourceSpan,
    state: &mut LuaDocLoweringState<'_>,
) {
    let Some(name) = function.name.as_deref() else {
        state.functions.push(function);
        return;
    };
    let Some(existing) = state
        .functions
        .iter_mut()
        .find(|existing| existing.name.as_deref() == Some(name))
    else {
        state.functions.push(function);
        return;
    };

    let has_primary_parameters = !function.parameters.is_empty() || function.has_varargs;

    if has_primary_parameters && existing.parameter_count != function.parameter_count {
        push_doc_conflict(
            state.diagnostics,
            doc_conflict_diagnostic(
                &parsed.path,
                span,
                format!("conflicting annotations for `{name}`: parameter count differs"),
            ),
        );
    }

    if existing.generic_parameters.is_empty() {
        existing.generic_parameters = function.generic_parameters.clone();
    } else if !function.generic_parameters.is_empty()
        && existing.generic_parameters != function.generic_parameters
    {
        push_doc_conflict(
            state.diagnostics,
            doc_conflict_diagnostic(
                &parsed.path,
                span,
                format!("conflicting generic binders for `{name}`"),
            ),
        );
    }

    for (index, parameter) in function.parameters.into_iter().enumerate() {
        let Some(existing_parameter) = existing.parameters.get_mut(index) else {
            existing.parameters.push(parameter);
            continue;
        };

        if existing_parameter.ty != Type::Any
            && parameter.ty != Type::Any
            && existing_parameter.ty != parameter.ty
        {
            push_doc_conflict(
                state.diagnostics,
                doc_conflict_diagnostic(
                    &parsed.path,
                    span,
                    format!(
                        "conflicting annotations for `{name}` parameter `{}`: `{}` vs `{}`",
                        existing_parameter.name, existing_parameter.ty, parameter.ty
                    ),
                ),
            );
            continue;
        }

        if existing_parameter.ty == Type::Any {
            existing_parameter.ty = parameter.ty;
        }

        if existing_parameter.documentation.is_none() {
            existing_parameter.documentation = parameter.documentation;
        }
    }

    if function.has_varargs {
        existing.has_varargs = true;
    }

    if existing.vararg_type.is_none() {
        existing.vararg_type = function.vararg_type;
    } else if let Some(vararg_type) = function.vararg_type {
        if existing.vararg_type.as_ref() != Some(&vararg_type) {
            push_doc_conflict(
                state.diagnostics,
                doc_conflict_diagnostic(
                    &parsed.path,
                    span,
                    format!("conflicting annotations for `{name}` vararg type"),
                ),
            );
        }
    }

    if !function.return_types.is_empty() {
        if !existing.return_types.is_empty() && existing.return_types != function.return_types {
            push_doc_conflict(
                state.diagnostics,
                doc_conflict_diagnostic(
                    &parsed.path,
                    span,
                    format!(
                        "conflicting annotations for `{name}` return type: `{}` vs `{}`",
                        format_type_list(&existing.return_types),
                        format_type_list(&function.return_types)
                    ),
                ),
            );
        } else if existing.return_types.is_empty() {
            existing.return_types = function.return_types;
            existing.return_type = existing.return_types.first().cloned();
        }
    } else if let Some(return_type) = function.return_type {
        match &existing.return_type {
            Some(existing_return) if existing_return != &return_type => {
                push_doc_conflict(
                    state.diagnostics,
                    doc_conflict_diagnostic(
                        &parsed.path,
                        span,
                        format!(
                            "conflicting annotations for `{name}` return type: `{existing_return}` vs `{return_type}`"
                        ),
                    ),
                );
            }
            Some(_) => {}
            None => {
                existing.return_type = Some(return_type.clone());
                existing.return_types = vec![return_type];
            }
        }
    }

    if existing.documentation.is_none() {
        existing.documentation = function.documentation;
    }

    for overload in function.overloads {
        if !existing.overloads.contains(&overload) {
            existing.overloads.push(overload);
        }
    }
}

fn leading_lua_doc_annotations(parsed: &ParsedSource, line: usize) -> Vec<&LuaDocAnnotation> {
    if line <= 1 {
        return Vec::new();
    }

    let lines = parsed.source.lines().collect::<Vec<_>>();
    let mut current = line - 1;
    let mut first_doc_line = None;

    while current > 0 {
        let text = lines.get(current - 1).copied().unwrap_or("").trim_start();

        if text.is_empty() {
            current -= 1;
            continue;
        }

        if text.starts_with("---") {
            first_doc_line = Some(current);
            current -= 1;
            continue;
        }

        break;
    }

    let first = first_doc_line.unwrap_or(line);

    parsed
        .lua_doc_annotations
        .iter()
        .filter(|annotation| annotation.span.start.line >= first && annotation.span.end.line < line)
        .collect()
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn stmt_code_span(parsed: &ParsedSource, stmt: &ast::Stmt) -> Option<SourceSpan> {
    if let Some(span) = node_span(stmt).and_then(|span| first_code_line_span(parsed, span)) {
        return Some(span);
    }

    let span = match stmt {
        ast::Stmt::LocalAssignment(local) => local.names().iter().next().and_then(node_span),
        ast::Stmt::Assignment(assignment) => {
            assignment.variables().iter().next().and_then(node_span)
        }
        ast::Stmt::LocalFunction(function) => node_span(function.name()),
        ast::Stmt::FunctionDeclaration(function) => node_span(function.name()),
        ast::Stmt::CompoundAssignment(assignment) => node_span(assignment.lhs()),
        _ => node_span(stmt),
    }?;

    first_code_line_span(parsed, span).or(Some(span))
}

fn first_code_line_span(parsed: &ParsedSource, span: SourceSpan) -> Option<SourceSpan> {
    for line in span.start.line..=span.end.line.max(span.start.line) {
        let text = parsed.source.lines().nth(line.saturating_sub(1))?;
        let trimmed = text.trim_start();

        if trimmed.is_empty() || trimmed.starts_with("--") {
            continue;
        }

        let leading = text.len() - trimmed.len();

        return Some(SourceSpan {
            start: crate::SourcePosition {
                line,
                column: leading + 1,
            },
            end: crate::SourcePosition {
                line,
                column: text.len() + 1,
            },
        });
    }

    None
}

fn expression_is_name(expression: &Expression, expected: &str) -> bool {
    matches!(expression, Expression::Var(Var::Name(name)) if token_text(name) == expected)
}

fn function_declaration_table_field(
    function: &ast::FunctionDeclaration,
) -> Option<(String, String, bool)> {
    let name = function.name().to_string();
    let colon = name.rfind(':');
    let dot = name.rfind('.');
    let separator = match (colon, dot) {
        (Some(colon), Some(dot)) => Some((colon.max(dot), colon > dot)),
        (Some(colon), None) => Some((colon, true)),
        (None, Some(dot)) => Some((dot, false)),
        (None, None) => None,
    }?;
    let owner = name[..separator.0].trim();
    let field = name[separator.0 + 1..].trim();

    (!owner.is_empty() && !field.is_empty())
        .then(|| (owner.to_owned(), field.to_owned(), separator.1))
}

fn doc_conflict_diagnostic(
    path: &Path,
    span: SourceSpan,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic::error(path, span.start.line, span.start.column, message)
        .with_code(DiagnosticCode::TYPE_DOC_CONFLICT)
}

fn push_doc_conflict(diagnostics: &mut Vec<Diagnostic>, diagnostic: Diagnostic) {
    if diagnostics.iter().any(|existing| {
        existing.file == diagnostic.file
            && existing.code == diagnostic.code
            && existing.message == diagnostic.message
    }) {
        return;
    }

    diagnostics.push(diagnostic);
}

#[derive(Clone, Copy)]
struct FunctionReturnQuery<'a> {
    local_contracts: &'a LocalContracts,
    parsed: &'a ParsedSource,
    aliases: &'a BTreeMap<String, Type>,
    function_types: &'a FunctionInferenceTypes,
    parameters: &'a BTreeMap<FunctionInferenceKey, Vec<TypedParameter>>,
    runtime_imports: &'a BTreeMap<String, String>,
    table_remove_overridden: bool,
    assert_overridden: bool,
}

#[derive(Clone, Copy)]
struct InferenceQueryContext<'a> {
    local_contracts: &'a LocalContracts,
    parsed: &'a ParsedSource,
    aliases: &'a BTreeMap<String, Type>,
    function_types: &'a FunctionInferenceTypes,
    current_function: &'a str,
    domains: BuiltinDomainRegistry,
    runtime_imports: &'a BTreeMap<String, String>,
    table_remove_overridden: bool,
    assert_overridden: bool,
}

impl<'a> InferenceQueryContext<'a> {
    fn new(
        parsed: &'a ParsedSource,
        local_contracts: &'a LocalContracts,
        aliases: &'a BTreeMap<String, Type>,
        function_types: &'a FunctionInferenceTypes,
        current_function: &'a str,
        runtime_imports: &'a BTreeMap<String, String>,
        table_remove_overridden: bool,
    ) -> Self {
        Self {
            parsed,
            local_contracts,
            aliases,
            function_types,
            current_function,
            domains: BuiltinDomainRegistry::standard(),
            runtime_imports,
            table_remove_overridden,
            assert_overridden: false,
        }
    }

    fn with_assert_overridden(mut self, overridden: bool) -> Self {
        self.assert_overridden = overridden;
        self
    }

    fn expression<'locals>(
        self,
        locals: &'locals BTreeMap<String, Type>,
    ) -> ExpressionInferenceQuery<'a, 'locals> {
        ExpressionInferenceQuery {
            context: self,
            locals,
        }
    }
}

#[derive(Clone, Copy)]
struct ExpressionInferenceQuery<'a, 'locals> {
    context: InferenceQueryContext<'a>,
    locals: &'locals BTreeMap<String, Type>,
}

impl ExpressionInferenceQuery<'_, '_> {
    fn type_of(self, expression: &Expression) -> Option<Type> {
        infer_expression_return_type(self, expression, 0)
    }

    fn return_type(self, expression: &Expression, index: usize) -> Option<Type> {
        infer_expression_return_type(self, expression, index)
    }

    fn return_types(self, expression: &Expression) -> Vec<Type> {
        infer_expression_return_types(self, expression)
    }
}

fn infer_function_return_types(
    parsed: &ParsedSource,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    functions: &[TypedFunction],
    local_contracts: &LocalContracts,
) -> BTreeMap<FunctionInferenceKey, InferredReturns> {
    let mut returns = BTreeMap::new();
    let parameters = function_parameters_by_declaration(parsed, functions);
    let runtime_imports = runtime_import_bindings(parsed);
    let table_remove_overridden = lua::table_remove_is_overridden(parsed);
    let assert_overridden = lua::global_name_is_assigned(parsed, "assert");

    for _ in 0..8 {
        let previous_returns = returns.clone();
        let function_types = function_types_by_name(parsed, functions, &returns);

        let query = FunctionReturnQuery {
            parsed,
            local_contracts,
            aliases,
            function_types: &function_types,
            parameters: &parameters,
            runtime_imports: &runtime_imports,
            table_remove_overridden,
            assert_overridden,
        };
        collect_function_return_types(parsed.ast.nodes(), query, locals, &mut returns);

        if returns == previous_returns {
            break;
        }
    }

    returns
}

pub(crate) fn inferred_function_type_from_body(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    body: &ast::FunctionBody,
) -> Type {
    let Type::Function(mut function) = super::function_type_from_body_with_metadata(parsed, body)
    else {
        return Type::Any;
    };

    if body.return_type().is_some() {
        return Type::Function(function);
    }

    let aliases = environment
        .aliases()
        .map(|(name, ty)| (name.to_owned(), ty.clone()))
        .collect::<BTreeMap<_, _>>();
    let function_types = function_types_by_name(parsed, environment.functions(), &BTreeMap::new());
    let mut locals = environment
        .locals()
        .filter_map(|(name, _)| {
            let position = node_span(body)?.start;
            environment
                .local_at_position(name, position)
                .map(|ty| (name.to_owned(), ty.clone()))
        })
        .collect::<BTreeMap<_, _>>();

    for parameter in &function.parameters {
        if let Some(name) = &parameter.name {
            locals.insert(name.clone(), parameter.ty.clone());
        }
    }

    let runtime_imports = runtime_import_bindings(parsed);
    let context = InferenceQueryContext::new(
        parsed,
        &environment.local_contracts,
        &aliases,
        &function_types,
        "",
        &runtime_imports,
        environment.table_remove_is_overridden(),
    )
    .with_assert_overridden(environment.assert_overridden);
    if let Some(returns) = infer_block_return_types(body.block(), context, &locals) {
        *function.return_type = returns.types.first().cloned().unwrap_or(Type::Any);
        function.return_types = returns.types;
        function.guarded_return_types = returns.guarded;
    }

    Type::Function(function)
}

/// Native annotations carry declaration identity directly. LuaDoc-only functions
/// are appended during lowering, so recover their origins from the Lua AST
/// rather than indexing them into the native annotation vector.
#[derive(Clone, Copy, Default)]
struct FunctionOrigin {
    name_span: Option<SourceSpan>,
    body_span: Option<SourceSpan>,
}

fn function_origins(parsed: &ParsedSource, functions: &[TypedFunction]) -> Vec<FunctionOrigin> {
    use full_moon::visitors::Visitor;
    #[derive(Default)]
    struct Origins(BTreeMap<String, FunctionOrigin>);
    impl Visitor for Origins {
        fn visit_local_function(&mut self, function: &ast::LocalFunction) {
            self.0
                .entry(token_text(function.name()))
                .or_insert(FunctionOrigin {
                    name_span: node_span(function.name()),
                    body_span: node_span(function.body()),
                });
        }
        fn visit_function_declaration(&mut self, function: &ast::FunctionDeclaration) {
            self.0
                .entry(function.name().to_string())
                .or_insert(FunctionOrigin {
                    name_span: node_span(function.name()),
                    body_span: node_span(function.body()),
                });
        }
    }
    let mut lua = Origins::default();
    if functions.len() > parsed.function_annotations.len() {
        lua.visit_ast(&parsed.ast);
    }
    functions
        .iter()
        .enumerate()
        .map(|(index, function)| {
            parsed
                .function_annotations
                .get(index)
                .map(|annotation| FunctionOrigin {
                    name_span: annotation.name_span,
                    body_span: annotation.span,
                })
                .or_else(|| {
                    function
                        .name
                        .as_ref()
                        .and_then(|name| lua.0.get(name).copied())
                })
                .unwrap_or_default()
        })
        .collect()
}

type FunctionInferenceKey = (String, usize, usize);

fn function_inference_key(name: &str, span: Option<SourceSpan>) -> FunctionInferenceKey {
    let position = span
        .map(|span| span.start)
        .unwrap_or(crate::SourcePosition { line: 0, column: 0 });
    (name.to_owned(), position.line, position.column)
}

fn function_parameters_by_declaration(
    parsed: &ParsedSource,
    functions: &[TypedFunction],
) -> BTreeMap<FunctionInferenceKey, Vec<TypedParameter>> {
    let origins = function_origins(parsed, functions);
    functions
        .iter()
        .enumerate()
        .filter_map(|(index, function)| {
            let name = function.name.as_ref()?;

            Some((
                function_inference_key(
                    name,
                    origins.get(index).and_then(|origin| origin.body_span),
                ),
                function.parameters.clone(),
            ))
        })
        .collect()
}

fn runtime_import_bindings(parsed: &ParsedSource) -> BTreeMap<String, String> {
    crate::module_index::require_bindings_for_parsed_source(parsed)
        .into_iter()
        .filter(|binding| crate::is_bundled_runtime_module(&binding.module))
        .map(|binding| (binding.alias, binding.module))
        .collect()
}

#[derive(Default)]
struct FunctionInferenceTypes {
    by_name: BTreeMap<String, Type>,
    by_declaration: BTreeMap<(usize, usize), Type>,
}

impl FunctionInferenceTypes {
    fn get(&self, name: &str) -> Option<&Type> {
        self.by_name.get(name)
    }
    fn contains_key(&self, name: &str) -> bool {
        self.by_name.contains_key(name)
    }
    fn at(
        &self,
        contracts: &LocalContracts,
        name: &str,
        position: crate::SourcePosition,
    ) -> Option<&Type> {
        if let Some(binding) = contracts.binding(name, position) {
            return self
                .by_declaration
                .get(&(binding.name_span.start.line, binding.name_span.start.column));
        }
        self.get(name)
    }
}

fn function_types_by_name(
    parsed: &ParsedSource,
    functions: &[TypedFunction],
    inferred_returns: &BTreeMap<FunctionInferenceKey, InferredReturns>,
) -> FunctionInferenceTypes {
    let origins = function_origins(parsed, functions);
    let entries = functions
        .iter()
        .enumerate()
        .filter_map(|(index, function)| {
            let name = function.name.as_ref()?;

            if !function.has_fully_annotated_parameters() {
                return None;
            }

            let inferred = inferred_returns.get(&function_inference_key(
                name,
                origins.get(index).and_then(|origin| origin.body_span),
            ));
            let (return_types, guarded) = if !function.return_types.is_empty() {
                (
                    function.return_types.clone(),
                    function.guarded_return_types.clone(),
                )
            } else if let Some(inferred) = inferred {
                (inferred.types.clone(), inferred.guarded.clone())
            } else {
                (
                    function
                        .return_type
                        .clone()
                        .map_or_else(|| vec![Type::Any], |ty| vec![ty]),
                    None,
                )
            };

            Some((
                name.clone(),
                origins.get(index).and_then(|origin| origin.name_span),
                Type::Function(
                    FunctionType::new_signature(
                        function.parameters.iter().map(|parameter| {
                            FunctionParameter::named(&parameter.name, parameter.ty.clone())
                        }),
                        function.required_parameter_count,
                        function.vararg_type.clone(),
                        return_types,
                    )
                    .with_generic_parameters(function.generic_parameters.clone())
                    .with_guarded_return_types(guarded)
                    .with_overloads(
                        function
                            .overloads
                            .iter()
                            .map(TypedFunctionOverload::as_function_type),
                    ),
                ),
            ))
        })
        .collect::<Vec<_>>();
    let mut result = FunctionInferenceTypes::default();
    for (name, span, ty) in entries {
        result.by_name.insert(name, ty.clone());
        if let Some(span) = span {
            result
                .by_declaration
                .insert((span.start.line, span.start.column), ty);
        }
    }
    result
}

fn collect_function_return_types(
    block: &ast::Block,
    query: FunctionReturnQuery<'_>,
    locals: &BTreeMap<String, Type>,
    returns: &mut BTreeMap<FunctionInferenceKey, InferredReturns>,
) {
    let mut scoped_locals = locals.clone();
    let root_inference = InferenceQueryContext::new(
        query.parsed,
        query.local_contracts,
        query.aliases,
        query.function_types,
        "",
        query.runtime_imports,
        query.table_remove_overridden,
    )
    .with_assert_overridden(query.assert_overridden);

    for stmt in block.stmts() {
        match stmt {
            ast::Stmt::Assignment(assignment) => {
                infer_assignment_types(assignment, root_inference, &mut scoped_locals);
            }
            ast::Stmt::LocalAssignment(local) => {
                infer_local_assignment_types(local, root_inference, &mut scoped_locals);
            }
            ast::Stmt::LocalFunction(function) => {
                let name = token_text(function.name());
                let function_locals = locals_with_function_parameters(
                    &scoped_locals,
                    query.parameters,
                    &function_inference_key(&name, node_span(function.body())),
                );
                let inference = InferenceQueryContext::new(
                    query.parsed,
                    query.local_contracts,
                    query.aliases,
                    query.function_types,
                    &name,
                    query.runtime_imports,
                    query.table_remove_overridden,
                )
                .with_assert_overridden(query.assert_overridden);

                if let Some(return_types) =
                    infer_block_return_types(function.body().block(), inference, &function_locals)
                {
                    returns.insert(
                        function_inference_key(&name, node_span(function.body())),
                        return_types,
                    );
                }
                collect_function_return_types(
                    function.body().block(),
                    query,
                    &function_locals,
                    returns,
                );
            }
            ast::Stmt::FunctionDeclaration(function) => {
                infer_function_declaration_type(function, query.function_types, &mut scoped_locals);

                let name = function.name().to_string();
                let function_locals = locals_with_function_parameters(
                    &scoped_locals,
                    query.parameters,
                    &function_inference_key(&name, node_span(function.body())),
                );
                let inference = InferenceQueryContext::new(
                    query.parsed,
                    query.local_contracts,
                    query.aliases,
                    query.function_types,
                    &name,
                    query.runtime_imports,
                    query.table_remove_overridden,
                )
                .with_assert_overridden(query.assert_overridden);

                if let Some(return_types) =
                    infer_block_return_types(function.body().block(), inference, &function_locals)
                {
                    returns.insert(
                        function_inference_key(&name, node_span(function.body())),
                        return_types,
                    );
                }
                collect_function_return_types(
                    function.body().block(),
                    query,
                    &function_locals,
                    returns,
                );
            }
            ast::Stmt::Do(block) => {
                collect_function_return_types(block.block(), query, &scoped_locals, returns);
            }
            ast::Stmt::GenericFor(for_stmt) => {
                collect_function_return_types(for_stmt.block(), query, &scoped_locals, returns);
            }
            ast::Stmt::If(if_stmt) => {
                collect_function_return_types(if_stmt.block(), query, &scoped_locals, returns);

                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        collect_function_return_types(
                            else_if.block(),
                            query,
                            &scoped_locals,
                            returns,
                        );
                    }
                }

                if let Some(else_block) = if_stmt.else_block() {
                    collect_function_return_types(else_block, query, &scoped_locals, returns);
                }
            }
            ast::Stmt::NumericFor(for_stmt) => {
                collect_function_return_types(for_stmt.block(), query, &scoped_locals, returns);
            }
            ast::Stmt::Repeat(repeat) => {
                collect_function_return_types(repeat.block(), query, &scoped_locals, returns);
            }
            ast::Stmt::While(while_stmt) => {
                collect_function_return_types(while_stmt.block(), query, &scoped_locals, returns);
            }
            _ => {}
        }
    }
}

fn locals_with_function_parameters(
    locals: &BTreeMap<String, Type>,
    parameters: &BTreeMap<FunctionInferenceKey, Vec<TypedParameter>>,
    function_name: &FunctionInferenceKey,
) -> BTreeMap<String, Type> {
    let mut scoped = locals.clone();

    if let Some(parameters) = parameters.get(function_name) {
        for parameter in parameters {
            scoped.insert(parameter.name.clone(), parameter.ty.clone());
        }
    }

    scoped
}

#[derive(Clone, Debug, PartialEq)]
struct InferredReturns {
    types: Vec<Type>,
    guarded: Option<Vec<Type>>,
}

fn infer_block_return_types(
    block: &ast::Block,
    context: InferenceQueryContext<'_>,
    locals: &BTreeMap<String, Type>,
) -> Option<InferredReturns> {
    let mut returns = Vec::new();

    collect_return_types_in_block(block, context, locals, &[], &mut returns);

    if returns.is_empty() {
        return None;
    }

    if block_may_fall_through(block) {
        returns.push(vec![Type::nil()]);
    }

    let guarded = guarded_return_types(&returns, context.aliases);
    Some(InferredReturns {
        types: type_from_return_type_lists(returns)?,
        guarded,
    })
}

/// When every return either yields `nil` first or a first value that is never
/// `nil` or `false`, a caller that checks the first value learns which
/// returns produced the others. This collects the types those returns give.
fn guarded_return_types(
    return_lists: &[Vec<Type>],
    aliases: &BTreeMap<String, Type>,
) -> Option<Vec<Type>> {
    let mut guarded = Vec::new();
    let mut has_nil_first = false;

    for list in return_lists {
        match list.first() {
            None => has_nil_first = true,
            Some(first) if *first == Type::nil() => has_nil_first = true,
            Some(first) if type_is_always_truthy(first, aliases, 0) => {
                guarded.push(list.clone());
            }
            Some(_) => return None,
        }
    }

    let width = guarded.iter().map(Vec::len).max()?;
    if !has_nil_first || width < 2 {
        return None;
    }
    type_from_return_type_lists(guarded)
}

fn type_is_always_truthy(ty: &Type, aliases: &BTreeMap<String, Type>, depth: usize) -> bool {
    const MAX_ALIAS_DEPTH: usize = 16;
    match ty {
        Type::Any => false,
        // Aliases outside the map name LOVE and runtime objects.
        Type::Alias(name) => match aliases.get(name) {
            Some(resolved) if depth < MAX_ALIAS_DEPTH => {
                type_is_always_truthy(resolved, aliases, depth + 1)
            }
            Some(_) => false,
            None => true,
        },
        Type::Primitive(primitive) => !matches!(
            primitive,
            crate::PrimitiveType::Nil | crate::PrimitiveType::Boolean
        ),
        Type::Union(types) => types
            .iter()
            .all(|ty| type_is_always_truthy(ty, aliases, depth)),
        Type::StringLiteral(_)
        | Type::Function(_)
        | Type::Table(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. }
        | Type::GenericParameter { .. }
        | Type::GenericAliasApplication { .. }
        | Type::GenericClassApplication { .. } => true,
    }
}

fn collect_return_types_in_block(
    block: &ast::Block,
    context: InferenceQueryContext<'_>,
    locals: &BTreeMap<String, Type>,
    guards: &[ReturnGuard],
    returns: &mut Vec<Vec<Type>>,
) {
    let mut scoped_locals = locals.clone();
    let mut guards = guards.to_vec();

    for stmt in block.stmts() {
        match stmt {
            ast::Stmt::Assignment(assignment) => {
                infer_assignment_types(assignment, context, &mut scoped_locals);
                for variable in assignment.variables() {
                    if let Some(name) = directly_assigned_name(variable) {
                        forget_return_guards(&mut guards, &name);
                    }
                }
            }
            ast::Stmt::LocalAssignment(local) => {
                let local_guards = local_return_guards(local, context.expression(&scoped_locals));
                infer_local_assignment_types(local, context, &mut scoped_locals);
                for name in local.names() {
                    forget_return_guards(&mut guards, &token_text(name));
                }
                guards.extend(local_guards);
            }
            ast::Stmt::FunctionDeclaration(function) => {
                infer_function_declaration_type(
                    function,
                    context.function_types,
                    &mut scoped_locals,
                );
            }
            ast::Stmt::FunctionCall(function_call) => {
                record_setmetatable_call(function_call, context, &mut scoped_locals);
                if let Some(condition) = crate::narrowing::assertion_condition(stmt, || {
                    !context.assert_overridden
                        && !scoped_locals.contains_key("assert")
                        && !context.function_types.contains_key("assert")
                }) {
                    let mut narrowed =
                        narrowed_locals_for_condition(condition, context.aliases, |name| {
                            scoped_locals.get(name).cloned()
                        });
                    extend_with_return_guards(&mut narrowed, &guards);
                    scoped_locals.extend(narrowed);
                }
            }
            ast::Stmt::Do(block) => collect_return_types_in_block(
                block.block(),
                context,
                &scoped_locals,
                &guards,
                returns,
            ),
            ast::Stmt::GenericFor(for_stmt) => collect_return_types_in_block(
                for_stmt.block(),
                context,
                &scoped_locals,
                &guards,
                returns,
            ),
            ast::Stmt::If(if_stmt) => {
                let branch_locals = locals_with_condition_narrowing(
                    if_stmt.condition(),
                    context.aliases,
                    &scoped_locals,
                    &guards,
                );
                collect_return_types_in_block(
                    if_stmt.block(),
                    context,
                    &branch_locals,
                    &guards,
                    returns,
                );

                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        let branch_locals = locals_with_condition_narrowing(
                            else_if.condition(),
                            context.aliases,
                            &scoped_locals,
                            &guards,
                        );
                        collect_return_types_in_block(
                            else_if.block(),
                            context,
                            &branch_locals,
                            &guards,
                            returns,
                        );
                    }
                }

                if let Some(else_block) = if_stmt.else_block() {
                    collect_return_types_in_block(
                        else_block,
                        context,
                        &scoped_locals,
                        &guards,
                        returns,
                    );
                }
                if if_stmt.else_if().is_none()
                    && if_stmt.else_block().is_none()
                    && !block_may_fall_through(if_stmt.block())
                {
                    let mut narrowed = narrowed_locals_after_false_condition(
                        if_stmt.condition(),
                        context.aliases,
                        |name| scoped_locals.get(name).cloned(),
                    );
                    extend_with_return_guards(&mut narrowed, &guards);
                    scoped_locals.extend(narrowed);
                }
            }
            ast::Stmt::NumericFor(for_stmt) => collect_return_types_in_block(
                for_stmt.block(),
                context,
                &scoped_locals,
                &guards,
                returns,
            ),
            ast::Stmt::Repeat(repeat) => collect_return_types_in_block(
                repeat.block(),
                context,
                &scoped_locals,
                &guards,
                returns,
            ),
            ast::Stmt::While(while_stmt) => collect_return_types_in_block(
                while_stmt.block(),
                context,
                &scoped_locals,
                &guards,
                returns,
            ),
            ast::Stmt::LocalFunction(_) => {}
            _ => {}
        }
    }

    if let Some(LastStmt::Return(return_stmt)) = block.last_stmt() {
        if let Some(return_types) =
            infer_return_statement_types(return_stmt, context.expression(&scoped_locals))
        {
            returns.push(return_types);
        }
    }
}

fn block_may_fall_through(block: &ast::Block) -> bool {
    if matches!(block.last_stmt(), Some(LastStmt::Return(_))) {
        return false;
    }

    match block.stmts().last() {
        Some(ast::Stmt::If(if_stmt)) => !if_guarantees_return(if_stmt),
        _ => true,
    }
}

fn if_guarantees_return(if_stmt: &ast::If) -> bool {
    if block_may_fall_through(if_stmt.block()) {
        return false;
    }

    if let Some(else_ifs) = if_stmt.else_if() {
        if else_ifs
            .iter()
            .any(|else_if| block_may_fall_through(else_if.block()))
        {
            return false;
        }
    }

    if_stmt
        .else_block()
        .is_some_and(|else_block| !block_may_fall_through(else_block))
}

fn infer_return_statement_types(
    return_stmt: &ast::Return,
    query: ExpressionInferenceQuery<'_, '_>,
) -> Option<Vec<Type>> {
    let expressions = return_stmt.returns().iter().collect::<Vec<_>>();

    if expressions.is_empty() {
        return Some(vec![Type::nil()]);
    }

    let fixed_expression_count = expressions.len().saturating_sub(1);
    let mut return_types = Vec::new();

    for (index, expression) in expressions.iter().enumerate() {
        // An unresolved expression still occupies a return slot. Keep its type
        // dynamic rather than discarding the entire return list and its arity.
        if index < fixed_expression_count {
            return_types.push(query.return_type(expression, 0).unwrap_or(Type::Any));
            continue;
        }

        let expression_returns = query.return_types(expression);

        if expression_returns.is_empty() {
            return_types.push(query.return_type(expression, 0).or_else(|| {
                // A lone unresolved call may have unknown arity; preserve the
                // existing deferred inference for that case.
                (expressions.len() > 1).then_some(Type::Any)
            })?);
        } else {
            return_types.extend(expression_returns);
        }
    }

    Some(return_types)
}

fn type_from_return_type_lists(return_lists: Vec<Vec<Type>>) -> Option<Vec<Type>> {
    let max_len = return_lists.iter().map(Vec::len).max()?;
    let mut merged = Vec::new();

    for index in 0..max_len {
        let mut slot_types = Vec::new();

        for return_list in &return_lists {
            let ty = return_list.get(index).cloned().unwrap_or_else(Type::nil);
            push_unique_type(&mut slot_types, ty);
        }

        merged.push(type_from_return_types(slot_types)?);
    }

    Some(merged)
}

fn type_from_return_types(types: Vec<Type>) -> Option<Type> {
    match types.len() {
        0 => None,
        1 => types.into_iter().next(),
        _ => Some(Type::union(types)),
    }
}

fn format_type_list(types: &[Type]) -> String {
    types
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn push_unique_type(types: &mut Vec<Type>, ty: Type) {
    if !types.contains(&ty) {
        types.push(ty);
    }
}

fn infer_local_assignment_types(
    local: &ast::LocalAssignment,
    context: InferenceQueryContext<'_>,
    locals: &mut BTreeMap<String, Type>,
) {
    let expressions = local.expressions().iter().collect::<Vec<_>>();
    let specifiers = local.type_specifiers().collect::<Vec<_>>();

    for (index, name) in local.names().iter().enumerate() {
        // Native annotations have already been resolved into `locals`. Keep
        // that compact contract instead of replacing it with the complete
        // inferred shape of a potentially enormous generated initializer.
        let has_native_annotation = specifiers
            .get(index)
            .and_then(|specifier| *specifier)
            .is_some();
        let has_lua_doc_annotation = index == 0
            && local
                .start_position()
                .is_some_and(|position| has_leading_lua_doc_type(context.parsed, position.line()));
        if has_native_annotation || has_lua_doc_annotation {
            if let Some(ty) =
                node_span(name).and_then(|span| context.local_contracts.at_declaration(span.start))
            {
                locals.insert(token_text(name), ty.clone());
            }
            continue;
        }

        let ty = match assignment_expression_for_local(&expressions, index) {
            Some((expression, return_index)) => {
                let Some(ty) = context
                    .expression(locals)
                    .return_type(expression, return_index)
                else {
                    continue;
                };
                ty
            }
            None if context.parsed.raw.language == ParsedLanguage::Tua => Type::nil(),
            None => continue,
        };

        let preserve_literal_unions = match assignment_expression_for_local(&expressions, index) {
            Some((Expression::FunctionCall(call), _)) => field_chain_before_first_call(call)
                .and_then(|parts| {
                    let imported = context
                        .domains
                        .imported_constructor_domain_from_bindings(context.runtime_imports, &parts);
                    let domain = imported.or_else(|| {
                        let [name] = parts.as_slice() else {
                            return None;
                        };
                        context.domains.constructor_domain(name)
                    })?;

                    (domain.preserves_literal_unions()
                        && (imported.is_some()
                            || parts.first().is_some_and(|name| {
                                !locals.contains_key(name)
                                    && !context.function_types.contains_key(name)
                            })))
                    .then_some(())
                })
                .is_some(),
            _ => false,
        } || crate::sprite::direct_type_is_sprite_player(&ty);
        let ty = if context.parsed.raw.language == ParsedLanguage::Tua && !preserve_literal_unions {
            widen_inferred_type(&ty)
        } else {
            ty
        };
        locals.insert(token_text(name), ty);
    }
}

fn infer_assignment_types(
    assignment: &ast::Assignment,
    context: InferenceQueryContext<'_>,
    locals: &mut BTreeMap<String, Type>,
) {
    for (variable, expression) in assignment.variables().iter().zip(assignment.expressions()) {
        if infer_table_field_assignment_type(variable, expression, context, locals) {
            continue;
        }

        let Some(name) = directly_assigned_name(variable) else {
            continue;
        };
        let Some(ty) = context.expression(locals).type_of(expression) else {
            locals.remove(&name);
            continue;
        };

        if context.parsed.raw.language == ParsedLanguage::Tua {
            locals
                .entry(name)
                .or_insert_with(|| widen_inferred_type(&ty));
        } else {
            locals.insert(name, ty);
        }
    }
}

fn infer_function_declaration_type(
    function: &ast::FunctionDeclaration,
    function_types: &FunctionInferenceTypes,
    locals: &mut BTreeMap<String, Type>,
) {
    let Some((root, field, is_method)) = function_declaration_table_field(function) else {
        return;
    };
    let is_constructor = is_method && function_body_returns_setmetatable_self(function.body());
    let ty = function_types
        .get(function.name().to_string().trim())
        .cloned()
        .unwrap_or_else(|| function_type_from_body_with_receiver(function.body(), is_method));

    insert_table_field_type(locals, &root, &field, ty, false, is_constructor);
}

fn infer_table_field_assignment_type(
    variable: &Var,
    expression: &Expression,
    context: InferenceQueryContext<'_>,
    locals: &mut BTreeMap<String, Type>,
) -> bool {
    let Some(parts) = plain_field_chain_from_var(variable) else {
        return false;
    };
    let [root, field] = parts.as_slice() else {
        return false;
    };
    let is_self_index = field == "__index" && expression_is_name(expression, root);
    let Some(ty) = context.expression(locals).type_of(expression) else {
        return true;
    };

    insert_table_field_type(locals, root, field, ty, is_self_index, false);
    true
}

fn insert_table_field_type(
    locals: &mut BTreeMap<String, Type>,
    root: &str,
    field: &str,
    ty: Type,
    is_self_index: bool,
    is_constructor: bool,
) {
    let Some(Type::Table(mut table)) = locals.get(root).cloned() else {
        return;
    };

    if is_self_index {
        table.mark_self_index();
    }

    if is_constructor {
        table.mark_constructor_method(field);
    }

    table.insert(field.to_owned(), TableField::required(ty));
    locals.insert(root.to_owned(), Type::Table(table));
}

fn record_setmetatable_call(
    function_call: &FunctionCall,
    context: InferenceQueryContext<'_>,
    locals: &mut BTreeMap<String, Type>,
) {
    let Some((target, meta)) = setmetatable_arguments(function_call) else {
        return;
    };
    let Expression::Var(Var::Name(name)) = target else {
        return;
    };
    let name = token_text(name);
    let Some(base) = locals.get(&name).cloned() else {
        return;
    };
    let Some(meta) = context.expression(locals).type_of(meta) else {
        return;
    };

    locals.insert(name, merge_static_metatable(base, meta));
}

fn locals_with_condition_narrowing(
    condition: &Expression,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    guards: &[ReturnGuard],
) -> BTreeMap<String, Type> {
    let mut scoped = locals.clone();
    let mut narrowed =
        narrowed_locals_for_condition(condition, aliases, |name| locals.get(name).cloned());

    extend_with_return_guards(&mut narrowed, guards);
    scoped.extend(narrowed);
    scoped
}

/// The sibling guards of `local first, ... = call()` for a call with guarded
/// return types.
fn local_return_guards(
    local: &ast::LocalAssignment,
    query: ExpressionInferenceQuery<'_, '_>,
) -> Vec<ReturnGuard> {
    let names = local.names().iter().map(token_text).collect::<Vec<_>>();
    let mut expressions = local.expressions().iter();
    let (Some(Expression::FunctionCall(function_call)), None) =
        (expressions.next(), expressions.next())
    else {
        return Vec::new();
    };
    if names.len() < 2 {
        return Vec::new();
    }

    call_guarded_return_types(query, function_call)
        .map(|guarded| return_guards_for_names(&names, &guarded))
        .unwrap_or_default()
}

fn call_guarded_return_types(
    query: ExpressionInferenceQuery<'_, '_>,
    function_call: &FunctionCall,
) -> Option<Vec<Type>> {
    let calls = function_call
        .suffixes()
        .filter(|suffix| matches!(suffix, ast::Suffix::Call(_)))
        .count();
    if calls != 1 || !matches!(function_call.suffixes().last(), Some(ast::Suffix::Call(_))) {
        return None;
    }
    let parts = field_chain_before_first_call(function_call)?;
    let callee = match parts.as_slice() {
        [name] => query
            .context
            .function_types
            .at(
                query.context.local_contracts,
                name,
                node_span(function_call)?.start,
            )
            .or_else(|| query.locals.get(name))
            .cloned(),
        _ => infer_field_chain_type(
            query.context.parsed,
            query.context.aliases,
            query.locals,
            &parts,
        ),
    }?;

    match resolve_aliases_from_map(query.context.aliases, &callee, 0) {
        Type::Function(function) => function.guarded_return_types,
        _ => None,
    }
}

fn infer_expression_return_type(
    query: ExpressionInferenceQuery<'_, '_>,
    expression: &Expression,
    return_index: usize,
) -> Option<Type> {
    match expression {
        Expression::FunctionCall(function_call) => {
            infer_function_call_return_types(query, function_call)
                .get(return_index)
                .cloned()
        }
        _ if return_index > 0 => None,
        Expression::BinaryOperator { lhs, binop, rhs } => {
            let left = query.type_of(lhs)?;
            let right = query.type_of(rhs)?;

            infer_binary_operator_type(query.context.aliases, binop, &left, &right)
        }
        Expression::UnaryOperator { unop, expression } => {
            let operand = query.type_of(expression)?;

            infer_unary_operator_type(query.context.aliases, unop, &operand)
        }
        Expression::Number(_) => Some(Type::number()),
        Expression::String(_) => Some(Type::string()),
        Expression::Symbol(symbol) => match token_text(symbol).as_str() {
            "true" | "false" => Some(Type::boolean()),
            "nil" => Some(Type::nil()),
            _ => None,
        },
        Expression::TableConstructor(table) => {
            let mut fields = Vec::new();
            let mut array_items = Vec::new();

            for field in table.fields() {
                match field {
                    Field::ExpressionKey { key, value, .. } => {
                        let ty = query.type_of(value).unwrap_or(Type::Any);

                        if let Some(key) = string_literal_expression(key) {
                            fields.push((key, TableField::required(ty)));
                        } else if expression_is_number(key) {
                            array_items.push(ty);
                        }
                    }
                    Field::NameKey { key, value, .. } => {
                        let key_text = token_text(key);
                        let ty = query.type_of(value).unwrap_or(Type::Any);

                        if key_text == "__index" {
                            fields.extend(index_fields_from_expression(value));
                        }

                        fields.push((key_text, TableField::required(ty)));
                    }
                    Field::NoKey(value) => {
                        array_items.push(query.type_of(value).unwrap_or(Type::Any));
                    }
                    _ => {}
                }
            }

            Some(table_or_array_type(fields, array_items))
        }
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => {
            query.return_type(expression, return_index)
        }
        Expression::Function(function) => Some(super::function_type_from_body_with_metadata(
            query.context.parsed,
            function.body(),
        )),
        Expression::Var(var) => infer_var_type(query, var),
        _ => None,
    }
}

fn infer_expression_return_types(
    query: ExpressionInferenceQuery<'_, '_>,
    expression: &Expression,
) -> Vec<Type> {
    match expression {
        Expression::FunctionCall(function_call) => {
            infer_function_call_return_types(query, function_call)
        }
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => query.return_types(expression),
        _ => query.type_of(expression).into_iter().collect(),
    }
}

fn infer_var_type(query: ExpressionInferenceQuery<'_, '_>, var: &Var) -> Option<Type> {
    match var {
        Var::Name(name) => {
            let position = node_span(name)?.start;
            let name = token_text(name);

            query
                .locals
                .get(&name)
                .or_else(|| {
                    query
                        .context
                        .function_types
                        .at(query.context.local_contracts, &name, position)
                })
                .map(|ty| resolve_aliases_from_map(query.context.aliases, ty, 0))
        }
        Var::Expression(expression) => {
            if let Some(parts) = plain_field_chain_from_var_expression(expression) {
                let parser_position = node_span(expression)?.start;
                let position = query
                    .context
                    .parsed
                    .raw
                    .parser_position_origin(parser_position)
                    .map(|origin| origin.source_position)
                    .unwrap_or(parser_position);
                if let Some(ty) = crate::narrowing::guarded_member_type(
                    query.context.parsed,
                    query.context.aliases,
                    &parts,
                    position,
                    |name| {
                        if name == "assert" && query.context.assert_overridden {
                            Some(Type::Any)
                        } else {
                            query.locals.get(name).cloned()
                        }
                    },
                ) {
                    return Some(ty);
                }
                if let Some(ty) = infer_field_chain_type(
                    query.context.parsed,
                    query.context.aliases,
                    query.locals,
                    &parts,
                ) {
                    return Some(ty);
                }
            }

            infer_var_expression_type(query, expression)
        }
        _ => None,
    }
}

fn infer_var_expression_type(
    query: ExpressionInferenceQuery<'_, '_>,
    expression: &ast::VarExpression,
) -> Option<Type> {
    let call_prefix = crate::ast_utils::var_expression_call_prefix(expression);
    let mut current = if let Some((call, _)) = &call_prefix {
        infer_function_call_return_types(query, call)
            .into_iter()
            .next()?
    } else {
        match expression.prefix() {
            Prefix::Name(name) => {
                let name = token_text(name);

                query
                    .locals
                    .get(&name)
                    .or_else(|| {
                        query.context.function_types.at(
                            query.context.local_contracts,
                            &name,
                            node_span(expression)?.start,
                        )
                    })
                    .map(|ty| resolve_aliases_from_map(query.context.aliases, ty, 0))?
            }
            Prefix::Expression(expression) => query.type_of(expression)?,
            _ => return None,
        }
    };

    let suffixes = call_prefix
        .as_ref()
        .map(|(_, rest)| rest.clone())
        .unwrap_or_else(|| expression.suffixes().collect());
    for suffix in suffixes {
        match suffix {
            Suffix::Index(Index::Dot { name, .. }) => {
                current = resolve_table_field_type_from_map(
                    query.context.aliases,
                    &current,
                    &token_text(name),
                )?;
            }
            Suffix::Index(Index::Brackets { expression, .. }) => {
                let key = index_key_type(query, expression)?;
                current = resolve_index_type_from_map(query.context.aliases, &current, &key)?;
            }
            _ => return None,
        }
    }

    Some(current)
}

fn infer_function_call_return_types(
    query: ExpressionInferenceQuery<'_, '_>,
    function_call: &FunctionCall,
) -> Vec<Type> {
    let first = infer_first_function_call_return_types(query, function_call);
    resolve_chained_call_returns(query.context.aliases, function_call, first)
}

/// Apply the remaining suffixes after a call's initial result. Both lexical
/// inference and position-aware editor queries must use the final call's full
/// return list, rather than accidentally reporting its constructor's type.
pub(crate) fn resolve_chained_call_returns(
    aliases: &BTreeMap<String, Type>,
    function_call: &FunctionCall,
    mut returns: Vec<Type>,
) -> Vec<Type> {
    let mut suffixes = function_call
        .suffixes()
        .skip_while(|s| !matches!(s, Suffix::Call(_)));
    suffixes.next();
    for suffix in suffixes {
        let Some(current) = returns.first() else {
            return Vec::new();
        };
        returns = match suffix {
            Suffix::Index(Index::Dot { name, .. }) => {
                resolve_table_field_type_from_map(aliases, current, &token_text(name))
                    .into_iter()
                    .collect()
            }
            Suffix::Call(Call::AnonymousCall(_)) => function_return_types(aliases, current),
            Suffix::Call(Call::MethodCall(method)) => {
                let name = token_text(method.name());
                match resolve_table_field_type_from_map(aliases, current, &name) {
                    Some(callable) => function_return_types(aliases, &callable),
                    None => love_object_method_return_types(aliases, current, &name),
                }
            }
            _ => Vec::new(),
        };
    }
    returns
}

fn infer_first_function_call_return_types(
    query: ExpressionInferenceQuery<'_, '_>,
    function_call: &FunctionCall,
) -> Vec<Type> {
    if let Some(ty) = query
        .context
        .domains
        .runtime_module_type_from_call(function_call)
    {
        return vec![ty];
    }

    if let Some(ty) = setmetatable_return_type(query, function_call) {
        return vec![ty];
    }

    if is_tostring_call(function_call) {
        return vec![Type::string()];
    }

    let Some(parts) = field_chain_before_first_call(function_call) else {
        return Vec::new();
    };

    if parts.len() == 1
        && parts
            .first()
            .is_some_and(|name| name == query.context.current_function)
    {
        return Vec::new();
    }

    if let Some(domain) = query
        .context
        .domains
        .imported_constructor_domain_from_bindings(query.context.runtime_imports, &parts)
    {
        if let Some(ty) = infer_builtin_constructor_return_type(query, domain, function_call) {
            return vec![ty];
        }
        return domain
            .constructor()
            .map(|constructor| {
                function_return_types(
                    query.context.aliases,
                    &Type::Function(constructor.function_type()),
                )
            })
            .unwrap_or_default();
    }

    if parts.as_slice() == ["table", "remove"]
        && !query.context.table_remove_overridden
        && !query.locals.contains_key("table")
        && !query.context.function_types.contains_key("table")
        && !query.context.function_types.contains_key("table.remove")
    {
        if let Some(result) = lua::first_call_argument(function_call)
            .and_then(|argument| query.type_of(argument))
            .and_then(|collection| {
                lua::table_remove_return_type(resolve_aliases_from_map(
                    query.context.aliases,
                    &collection,
                    0,
                ))
            })
        {
            return vec![result];
        }
    }

    if parts.len() == 1 {
        let Some(name) = parts.first() else {
            return Vec::new();
        };

        if name == "rawget"
            && !query.locals.contains_key(name)
            && !query.context.function_types.contains_key(name)
        {
            if let Some(collection) = rawget_collection_argument(function_call)
                .and_then(|argument| query.type_of(argument))
                .and_then(|collection| {
                    optional_index_value_type_from_map(query.context.aliases, &collection)
                })
            {
                return vec![collection];
            }
        }

        if !query.locals.contains_key(name) && !query.context.function_types.contains_key(name) {
            if let Some(ty) = query
                .context
                .domains
                .constructor_domain(name)
                .and_then(|domain| {
                    infer_builtin_constructor_return_type(query, domain, function_call)
                })
            {
                return vec![ty];
            }
        }

        let function_type = query
            .context
            .function_types
            .at(
                query.context.local_contracts,
                name,
                node_span(function_call)
                    .map(|span| span.start)
                    .unwrap_or(crate::SourcePosition { line: 1, column: 1 }),
            )
            .or_else(|| query.locals.get(name))
            .map(|ty| resolve_aliases_from_map(query.context.aliases, ty, 0))
            .or_else(|| asset::loader_kind(name).map(|kind| Type::Function(kind.function_type())))
            .or_else(|| {
                query
                    .context
                    .domains
                    .constructor(name)
                    .map(|constructor| Type::Function(constructor.function_type()))
            });

        return function_type
            .map(|ty| instantiated_call_return_types(query, function_call, ty))
            .unwrap_or_default();
    }

    if let Some(method) = parts.last() {
        if let Some(receiver_parts) = parts.get(..parts.len().saturating_sub(1)) {
            if let Some(receiver_type) = infer_field_chain_type(
                query.context.parsed,
                query.context.aliases,
                query.locals,
                receiver_parts,
            ) {
                if let Some(returns) = query.context.domains.method_return_types_from_call(
                    &receiver_type,
                    function_call,
                    |expression| query.type_of(expression),
                ) {
                    return returns;
                }

                if let Some(ty) = constructor_call_return_type_from_map(
                    query.context.aliases,
                    &receiver_type,
                    method,
                ) {
                    return vec![ty];
                }

                let returns =
                    love_object_method_return_types(query.context.aliases, &receiver_type, method);

                if !returns.is_empty() {
                    return returns;
                }
            }
        }
    }

    infer_field_chain_type(
        query.context.parsed,
        query.context.aliases,
        query.locals,
        &parts,
    )
    .map(|ty| instantiated_call_return_types(query, function_call, ty))
    .or_else(|| {
        let root = parts.first()?;
        if query.locals.contains_key(root) || query.context.function_types.contains_key(root) {
            return None;
        }
        lua::hover_item(&parts)
            .filter(|item| item.kind == lua::LuaItemKind::Function)
            .map(|item| return_types_from_lua_function_detail(item.detail))
    })
    .unwrap_or_default()
}

fn instantiated_call_return_types(
    query: ExpressionInferenceQuery<'_, '_>,
    function_call: &FunctionCall,
    ty: Type,
) -> Vec<Type> {
    let Type::Function(function) = resolve_aliases_from_map(query.context.aliases, &ty, 0) else {
        return Vec::new();
    };
    if function.generic_parameters.is_empty() {
        return function_return_types(query.context.aliases, &Type::Function(function));
    }

    let context = query
        .context
        .aliases
        .iter()
        .fold(TypeContext::new(), |context, (name, ty)| {
            context.with_alias(crate::types::TypeAlias::new(name.clone(), ty.clone()))
        });
    let arguments = function_call_argument_types(query, function_call);
    instantiate_generic_function(&function, &arguments, &context)
        .map(|function| function.return_types_or_any().to_vec())
        .unwrap_or_else(|_| function.return_types_or_any().to_vec())
}

fn function_call_argument_types(
    query: ExpressionInferenceQuery<'_, '_>,
    call: &FunctionCall,
) -> Vec<Option<Type>> {
    let Some(arguments) = call.suffixes().find_map(|suffix| match suffix {
        Suffix::Call(Call::AnonymousCall(args)) => Some(args),
        Suffix::Call(Call::MethodCall(method)) => Some(method.args()),
        _ => None,
    }) else {
        return Vec::new();
    };
    match arguments {
        FunctionArgs::Parentheses { arguments, .. } => arguments
            .iter()
            .map(|expression| query.type_of(expression))
            .collect(),
        FunctionArgs::String(_) => vec![Some(Type::string())],
        FunctionArgs::TableConstructor(table) => {
            let expression = Expression::TableConstructor(table.clone());
            vec![query.type_of(&expression)]
        }
        _ => Vec::new(),
    }
}

fn optional_index_value_type(collection: Type) -> Option<Type> {
    let value = match collection {
        Type::Array(item) => *item,
        Type::Dictionary { value, .. } => *value,
        Type::Tuple(items) => Type::union(items),
        Type::Union(collections) => {
            let values = collections
                .into_iter()
                .filter_map(optional_index_value_type)
                .collect::<Vec<_>>();
            if values.is_empty() {
                return None;
            }
            Type::union(values)
        }
        _ => return None,
    };

    Some(Type::union([value, Type::nil()]))
}

fn optional_index_value_type_from_map(
    aliases: &BTreeMap<String, Type>,
    collection: &Type,
) -> Option<Type> {
    optional_index_value_type(resolve_aliases_from_map(aliases, collection, 0))
}

fn infer_builtin_constructor_return_type(
    query: ExpressionInferenceQuery<'_, '_>,
    domain: BuiltinDomain,
    function_call: &FunctionCall,
) -> Option<Type> {
    let argument = first_parenthesized_call_argument(function_call)
        .and_then(|argument| query.type_of(argument));

    domain.inferred_constructor_return_type(function_call, argument.as_ref())
}

fn first_parenthesized_call_argument(function_call: &FunctionCall) -> Option<&Expression> {
    let Suffix::Call(Call::AnonymousCall(ast::FunctionArgs::Parentheses { arguments, .. })) =
        function_call.suffixes().next()?
    else {
        return None;
    };

    arguments.iter().next()
}

fn rawget_collection_argument(function_call: &FunctionCall) -> Option<&Expression> {
    let Suffix::Call(Call::AnonymousCall(ast::FunctionArgs::Parentheses { arguments, .. })) =
        function_call.suffixes().next()?
    else {
        return None;
    };
    let mut arguments = arguments.iter();
    let collection = arguments.next()?;
    arguments.next()?;

    Some(collection)
}

fn constructor_call_return_type_from_map(
    aliases: &BTreeMap<String, Type>,
    receiver_type: &Type,
    method: &str,
) -> Option<Type> {
    let Type::Table(table) = resolve_aliases_from_map(aliases, receiver_type, 0) else {
        return None;
    };

    (table.has_self_index() && table.is_constructor_method(method)).then(|| receiver_type.clone())
}

fn setmetatable_return_type(
    query: ExpressionInferenceQuery<'_, '_>,
    function_call: &FunctionCall,
) -> Option<Type> {
    let (target, meta) = setmetatable_arguments(function_call)?;
    if let Some(class_type) = class_setmetatable_instance_type(query.context.parsed, meta) {
        return Some(class_type);
    }
    let base = query.type_of(target)?;
    let Some(meta) = query.type_of(meta) else {
        return Some(base);
    };

    Some(merge_static_metatable(base, meta))
}

fn class_setmetatable_instance_type(
    parsed: &ParsedSource,
    expression: &Expression,
) -> Option<Type> {
    let Expression::Var(Var::Name(name)) = expression else {
        return None;
    };
    let name = token_text(name);

    parsed
        .class_declarations
        .iter()
        .any(|class| class.name == name)
        .then_some(Type::alias(name))
}

fn setmetatable_arguments(function_call: &FunctionCall) -> Option<(&Expression, &Expression)> {
    let Prefix::Name(name) = function_call.prefix() else {
        return None;
    };

    if token_text(name) != "setmetatable" {
        return None;
    }

    let Some(Suffix::Call(Call::AnonymousCall(ast::FunctionArgs::Parentheses {
        arguments, ..
    }))) = function_call.suffixes().next()
    else {
        return None;
    };

    Some((arguments.iter().next()?, arguments.iter().nth(1)?))
}

fn is_tostring_call(function_call: &FunctionCall) -> bool {
    let Prefix::Name(name) = function_call.prefix() else {
        return false;
    };

    token_text(name) == "tostring"
}

fn merge_static_metatable(base: Type, meta: Type) -> Type {
    MetatableFacts::from_type(&meta).apply_to_type(base)
}

fn function_return_types(aliases: &BTreeMap<String, Type>, ty: &Type) -> Vec<Type> {
    match resolve_aliases_from_map(aliases, ty, 0) {
        Type::Function(function) => function.return_types_or_any().to_vec(),
        Type::Any => vec![Type::Any],
        Type::Union(types) => {
            let returns = types
                .iter()
                .map(|ty| function_return_types(aliases, ty))
                .collect::<Vec<_>>();

            type_from_return_type_lists(returns).unwrap_or_default()
        }
        Type::Table(table) => table
            .metatable_facts()
            .and_then(|facts| facts.call.as_ref())
            .map(|function| function.return_types_or_any().to_vec())
            .unwrap_or_default(),
        Type::Alias(_)
        | Type::Primitive(_)
        | Type::StringLiteral(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. }
        | Type::GenericParameter { .. }
        | Type::GenericAliasApplication { .. }
        | Type::GenericClassApplication { .. } => Vec::new(),
    }
}

fn love_object_method_return_types(
    aliases: &BTreeMap<String, Type>,
    ty: &Type,
    field: &str,
) -> Vec<Type> {
    match resolve_aliases_from_map(aliases, ty, 0) {
        Type::Alias(name) => lua::type_field_items(&name, field)
            .into_iter()
            .find(|item| item.label == field)
            .map(|item| return_types_from_lua_function_detail(item.detail))
            .unwrap_or_default(),
        Type::Union(types) => {
            let mut returns = Vec::new();

            for ty in types {
                for return_type in love_object_method_return_types(aliases, &ty, field) {
                    push_unique_type(&mut returns, return_type);
                }
            }

            returns
        }
        Type::Any
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::Table(_)
        | Type::StringLiteral(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. }
        | Type::GenericParameter { .. }
        | Type::GenericAliasApplication { .. }
        | Type::GenericClassApplication { .. } => Vec::new(),
    }
}

fn return_types_from_lua_function_detail(detail: &str) -> Vec<Type> {
    let Some(start) = detail.rfind("): ").map(|index| index + "): ".len()) else {
        return Vec::new();
    };

    detail[start..]
        .split(',')
        .map(str::trim)
        .filter_map(type_from_lua_label)
        .collect()
}

fn type_from_lua_label(label: &str) -> Option<Type> {
    let label = label.trim();

    if label.contains('|') {
        let members = label
            .split('|')
            .filter_map(type_from_lua_label)
            .collect::<Vec<_>>();

        return match members.len() {
            0 => None,
            1 => members.into_iter().next(),
            _ => Some(Type::union(members)),
        };
    }

    match label {
        "any" => Some(Type::Any),
        "boolean" => Some(Type::boolean()),
        "nil" => Some(Type::nil()),
        "number" => Some(Type::number()),
        "string" => Some(Type::string()),
        "table" => Some(Type::empty_table()),
        other if !other.is_empty() => Some(Type::alias(other)),
        _ => None,
    }
}

fn infer_binary_operator_type(
    aliases: &BTreeMap<String, Type>,
    binop: &ast::BinOp,
    left: &Type,
    right: &Type,
) -> Option<Type> {
    if is_numeric_operator(binop) && is_number_type(aliases, left) && is_number_type(aliases, right)
    {
        return Some(Type::number());
    }

    if is_ordering_operator(binop)
        && ((is_number_type(aliases, left) && is_number_type(aliases, right))
            || (is_string_type(aliases, left) && is_string_type(aliases, right)))
    {
        return Some(Type::boolean());
    }

    if matches!(binop, ast::BinOp::TwoDots(_))
        && is_concat_operand_type(aliases, left)
        && is_concat_operand_type(aliases, right)
    {
        return Some(Type::string());
    }

    if let Some(name) = arithmetic_metamethod_name(binop) {
        if let Some(function) = binary_metamethod(aliases, left, right, name) {
            return Some(metamethod_return_or(&function, left.clone()));
        }
    }

    if let Some(name) = comparison_metamethod_name(binop) {
        if let Some(function) = binary_metamethod(aliases, left, right, name) {
            return Some(metamethod_return_or(&function, Type::boolean()));
        }
    }

    if matches!(left, Type::Any) || matches!(right, Type::Any) {
        return Some(Type::Any);
    }

    None
}

fn infer_unary_operator_type(
    aliases: &BTreeMap<String, Type>,
    unop: &ast::UnOp,
    operand: &Type,
) -> Option<Type> {
    match unop {
        ast::UnOp::Hash(_) => {
            if let Some(function) =
                resolve_metatable_facts_from_map(aliases, operand).and_then(|facts| facts.len)
            {
                return Some(metamethod_return_or(&function, Type::number()));
            }

            if matches!(operand, Type::Any) {
                Some(Type::Any)
            } else {
                Some(Type::number())
            }
        }
        ast::UnOp::Minus(_) => {
            if is_number_type(aliases, operand) {
                Some(Type::number())
            } else if let Some(function) =
                resolve_metatable_facts_from_map(aliases, operand).and_then(|facts| facts.unm)
            {
                Some(metamethod_return_or(&function, operand.clone()))
            } else if matches!(operand, Type::Any) {
                Some(Type::Any)
            } else {
                None
            }
        }
        ast::UnOp::Not(_) => Some(Type::boolean()),
        _ => {
            if matches!(operand, Type::Any) {
                Some(Type::Any)
            } else {
                Some(Type::number())
            }
        }
    }
}

fn binary_metamethod(
    aliases: &BTreeMap<String, Type>,
    left: &Type,
    right: &Type,
    name: &str,
) -> Option<FunctionType> {
    resolve_metatable_facts_from_map(aliases, left)
        .and_then(|facts| facts.metamethod(name).cloned())
        .or_else(|| {
            resolve_metatable_facts_from_map(aliases, right)
                .and_then(|facts| facts.metamethod(name).cloned())
        })
}

fn metamethod_return_or(function: &FunctionType, fallback: Type) -> Type {
    let ty = function
        .return_types_or_any()
        .first()
        .cloned()
        .unwrap_or(Type::Any);

    if matches!(ty, Type::Any) {
        fallback
    } else {
        ty
    }
}

fn arithmetic_metamethod_name(binop: &ast::BinOp) -> Option<&'static str> {
    match binop {
        ast::BinOp::Plus(_) => Some("__add"),
        ast::BinOp::Minus(_) => Some("__sub"),
        ast::BinOp::Star(_) => Some("__mul"),
        ast::BinOp::Slash(_) => Some("__div"),
        _ => None,
    }
}

fn comparison_metamethod_name(binop: &ast::BinOp) -> Option<&'static str> {
    match binop {
        ast::BinOp::TwoEqual(_) | ast::BinOp::TildeEqual(_) => Some("__eq"),
        ast::BinOp::LessThan(_) | ast::BinOp::GreaterThan(_) => Some("__lt"),
        ast::BinOp::LessThanEqual(_) | ast::BinOp::GreaterThanEqual(_) => Some("__le"),
        _ => None,
    }
}

fn is_numeric_operator(binop: &ast::BinOp) -> bool {
    matches!(
        binop,
        ast::BinOp::Caret(_)
            | ast::BinOp::Minus(_)
            | ast::BinOp::Percent(_)
            | ast::BinOp::Plus(_)
            | ast::BinOp::Slash(_)
            | ast::BinOp::Star(_)
    )
}

fn is_ordering_operator(binop: &ast::BinOp) -> bool {
    matches!(
        binop,
        ast::BinOp::GreaterThan(_)
            | ast::BinOp::GreaterThanEqual(_)
            | ast::BinOp::LessThan(_)
            | ast::BinOp::LessThanEqual(_)
    )
}

fn is_number_type(aliases: &BTreeMap<String, Type>, ty: &Type) -> bool {
    match resolve_aliases_from_map(aliases, ty, 0) {
        Type::Primitive(crate::PrimitiveType::Number) => true,
        Type::Union(types) => types.iter().all(|ty| is_number_type(aliases, ty)),
        _ => false,
    }
}

fn is_string_type(aliases: &BTreeMap<String, Type>, ty: &Type) -> bool {
    match resolve_aliases_from_map(aliases, ty, 0) {
        Type::Primitive(crate::PrimitiveType::String) | Type::StringLiteral(_) => true,
        Type::Union(types) => types.iter().all(|ty| is_string_type(aliases, ty)),
        _ => false,
    }
}

fn is_concat_operand_type(aliases: &BTreeMap<String, Type>, ty: &Type) -> bool {
    match resolve_aliases_from_map(aliases, ty, 0) {
        Type::Primitive(crate::PrimitiveType::Number | crate::PrimitiveType::String)
        | Type::StringLiteral(_) => true,
        Type::Union(types) => types.iter().all(|ty| is_concat_operand_type(aliases, ty)),
        _ => false,
    }
}

fn infer_field_chain_type(
    parsed: &ParsedSource,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    parts: &[String],
) -> Option<Type> {
    resolve_field_chain_type_from_map(aliases, locals, parts).or_else(|| {
        let (root, fields) = parts.split_first()?;

        required_module_export_type_for_fields(parsed, root, fields)
            .map(|ty| resolve_aliases_from_map(aliases, &ty, 0))
    })
}

fn resolve_field_chain_type_from_map(
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    parts: &[String],
) -> Option<Type> {
    let (root, fields) = parts.split_first()?;
    let mut current = locals
        .get(root)
        .map(|ty| resolve_aliases_from_map(aliases, ty, 0))?;

    for field in fields {
        current = resolve_table_field_type_from_map(aliases, &current, field)?;
    }

    Some(current)
}

fn resolve_table_field_type_from_map(
    aliases: &BTreeMap<String, Type>,
    ty: &Type,
    field: &str,
) -> Option<Type> {
    match resolve_aliases_from_map(aliases, ty, 0) {
        Type::Dictionary { key, value } => key_accepts_index(&key, &Type::string_literal(field))
            .then(|| Type::union([*value, Type::nil()])),
        Type::Table(table) => table
            .semantic_field(field)
            .map(|field| resolve_aliases_from_map(aliases, &field.ty, 0)),
        Type::Union(types) => {
            let resolved = types
                .iter()
                .map(|ty| resolve_table_field_type_from_map(aliases, ty, field))
                .collect::<Option<Vec<_>>>()?;

            match resolved.len() {
                0 => None,
                1 => resolved.into_iter().next(),
                _ => Some(Type::union(resolved)),
            }
        }
        _ => None,
    }
}

fn resolve_index_type_from_map(
    aliases: &BTreeMap<String, Type>,
    collection: &Type,
    key: &Type,
) -> Option<Type> {
    match resolve_aliases_from_map(aliases, collection, 0) {
        Type::Array(item) => is_number_like(key).then_some(*item),
        Type::Tuple(items) => is_number_like(key).then(|| union_types(items)),
        Type::Dictionary {
            key: dict_key,
            value,
        } => key_accepts_index(&dict_key, key).then(|| Type::union([*value, Type::nil()])),
        Type::Table(table) => match key {
            Type::StringLiteral(value) => table
                .semantic_field(value)
                .map(|field| resolve_aliases_from_map(aliases, &field.ty, 0)),
            Type::Primitive(crate::PrimitiveType::String) => union_types_optional(
                table
                    .semantic_fields()
                    .into_values()
                    .map(|field| resolve_aliases_from_map(aliases, &field.ty, 0)),
            ),
            _ => None,
        },
        Type::Union(types) => union_types_optional(
            types
                .iter()
                .map(|ty| resolve_index_type_from_map(aliases, ty, key))
                .collect::<Option<Vec<_>>>()?,
        ),
        Type::Any
        | Type::Alias(_)
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::StringLiteral(_)
        | Type::GenericParameter { .. }
        | Type::GenericAliasApplication { .. }
        | Type::GenericClassApplication { .. } => None,
    }
}

fn index_key_type(
    query: ExpressionInferenceQuery<'_, '_>,
    expression: &Expression,
) -> Option<Type> {
    match expression {
        Expression::Number(_) => Some(Type::number()),
        Expression::String(value) => Some(Type::string_literal(unquote_string(&value.to_string()))),
        _ => query.type_of(expression),
    }
}

fn table_or_array_type(fields: Vec<(String, TableField)>, array_items: Vec<Type>) -> Type {
    let table = (!fields.is_empty()).then(|| Type::table(fields));
    let array = (!array_items.is_empty())
        .then(|| Type::array(union_types(crate::types::merge_record_rows(array_items))));

    match (table, array) {
        (Some(table), Some(array)) => Type::union([table, array]),
        (Some(table), None) => table,
        (None, Some(array)) => array,
        (None, None) => Type::empty_table(),
    }
}

fn union_types(types: Vec<Type>) -> Type {
    let unique = deduplicate_inferred_types(types);

    match unique.len() {
        0 => Type::Any,
        1 => unique.into_iter().next().unwrap_or(Type::Any),
        _ => Type::union(unique),
    }
}

fn union_types_optional(types: impl IntoIterator<Item = Type>) -> Option<Type> {
    let types = types.into_iter().collect::<Vec<_>>();

    (!types.is_empty()).then(|| union_types(types))
}

fn is_number_like(ty: &Type) -> bool {
    matches!(ty, Type::Primitive(crate::PrimitiveType::Number))
}

fn key_accepts_index(expected: &Type, actual: &Type) -> bool {
    match (expected, actual) {
        (Type::Any, _) | (_, Type::Any) => true,
        (
            Type::Primitive(crate::PrimitiveType::String),
            Type::Primitive(crate::PrimitiveType::String),
        )
        | (Type::Primitive(crate::PrimitiveType::String), Type::StringLiteral(_))
        | (
            Type::Primitive(crate::PrimitiveType::Number),
            Type::Primitive(crate::PrimitiveType::Number),
        ) => true,
        (Type::StringLiteral(expected), Type::StringLiteral(actual)) => expected == actual,
        (Type::Union(types), actual) => types.iter().any(|ty| key_accepts_index(ty, actual)),
        (expected, Type::Union(types)) => types.iter().any(|ty| key_accepts_index(expected, ty)),
        _ => false,
    }
}

fn resolve_metatable_facts_from_map(
    aliases: &BTreeMap<String, Type>,
    ty: &Type,
) -> Option<MetatableFacts> {
    match resolve_aliases_from_map(aliases, ty, 0) {
        Type::Table(table) => table.metatable_facts().cloned(),
        Type::Union(types) => {
            for ty in types {
                if let Some(facts) = resolve_metatable_facts_from_map(aliases, &ty) {
                    return Some(facts);
                }
            }

            None
        }
        _ => None,
    }
}

/// Hydrate a recursive class back-edge only when it becomes the current receiver.
/// Do not expand its fields recursively: they carry their own finite references.
fn resolve_nominal_reference(aliases: &BTreeMap<String, Type>, ty: &Type) -> Option<Type> {
    let Type::Table(reference) = ty else {
        return None;
    };
    if reference.fields().next().is_some() {
        return None;
    }
    let identity = reference.nominal_identity()?;
    aliases.values().find_map(|candidate| match candidate {
        Type::Table(table)
            if table.nominal_identity() == Some(identity) && table.fields().next().is_some() =>
        {
            Some(candidate.clone())
        }
        _ => None,
    })
}

fn resolve_aliases_from_map(aliases: &BTreeMap<String, Type>, ty: &Type, depth: usize) -> Type {
    fn resolve_chain(
        aliases: &BTreeMap<String, Type>,
        ty: &Type,
        depth: usize,
        visiting: &mut BTreeSet<String>,
    ) -> Type {
        if depth >= MAX_ALIAS_RESOLUTION_DEPTH {
            return ty.clone();
        }
        let Type::Alias(name) = ty else {
            return resolve_nominal_reference(aliases, ty).unwrap_or_else(|| ty.clone());
        };
        let Some(alias) = aliases.get(name) else {
            return ty.clone();
        };
        if !visiting.insert(name.clone()) {
            return Type::Any;
        }
        let resolved = resolve_chain(aliases, alias, depth + 1, visiting);
        visiting.remove(name);
        resolved
    }

    resolve_chain(aliases, ty, depth, &mut BTreeSet::new())
}

fn function_type_from_body(body: &ast::FunctionBody) -> Type {
    super::function_type_from_body(body)
}

fn function_type_from_body_with_receiver(body: &ast::FunctionBody, include_receiver: bool) -> Type {
    let Type::Function(mut function) = function_type_from_body(body) else {
        return Type::Any;
    };

    if include_receiver {
        function
            .parameters
            .insert(0, super::FunctionParameter::anonymous(Type::Any));
    }

    Type::Function(function)
}

fn function_body_returns_setmetatable_self(body: &ast::FunctionBody) -> bool {
    let Some(LastStmt::Return(return_stmt)) = body.block().last_stmt() else {
        return false;
    };
    let Some(Expression::FunctionCall(function_call)) = return_stmt.returns().iter().next() else {
        return false;
    };
    let Some((_, meta)) = setmetatable_arguments(function_call) else {
        return false;
    };

    expression_is_name(meta, "self")
}

fn index_fields_from_expression(expression: &Expression) -> Vec<(String, TableField)> {
    let Expression::Function(function) = expression else {
        return Vec::new();
    };

    index_fields_from_block(function.body().block())
}

fn index_fields_from_block(block: &ast::Block) -> Vec<(String, TableField)> {
    let mut fields = Vec::new();

    for stmt in block.stmts() {
        if let ast::Stmt::If(if_stmt) = stmt {
            fields.extend(index_fields_from_if(if_stmt));
        }
    }

    fields
}

fn index_fields_from_if(if_stmt: &ast::If) -> Vec<(String, TableField)> {
    let Some(key) = string_key_comparison(if_stmt.condition()) else {
        return Vec::new();
    };
    let Some(LastStmt::Return(return_stmt)) = if_stmt.block().last_stmt() else {
        return Vec::new();
    };
    let Some(expression) = return_stmt.returns().iter().next() else {
        return Vec::new();
    };
    let ty = match expression {
        Expression::Number(_) => Type::number(),
        Expression::String(_) => Type::string(),
        Expression::Symbol(symbol) => match token_text(symbol).as_str() {
            "true" | "false" => Type::boolean(),
            "nil" => Type::nil(),
            _ => Type::Any,
        },
        Expression::TableConstructor(table) => {
            Type::table(table.fields().iter().filter_map(|field| {
                let Field::NameKey { key, .. } = field else {
                    return None;
                };

                Some((token_text(key), TableField::required(Type::Any)))
            }))
        }
        _ => Type::Any,
    };

    vec![(key, TableField::required(ty))]
}

fn string_key_comparison(expression: &Expression) -> Option<String> {
    let Expression::BinaryOperator { lhs, binop, rhs } = expression else {
        return None;
    };

    if !matches!(binop, ast::BinOp::TwoEqual(_)) {
        return None;
    }

    string_literal_expression(rhs).or_else(|| string_literal_expression(lhs))
}

fn string_literal_expression(expression: &Expression) -> Option<String> {
    match expression {
        Expression::String(value) => Some(value.to_string().trim_matches(['"', '\'']).to_owned()),
        _ => None,
    }
}

fn expression_is_number(expression: &Expression) -> bool {
    matches!(expression, Expression::Number(_))
}

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.token().to_string().trim().to_owned()
}

fn unquote_string(value: &str) -> String {
    value
        .trim()
        .strip_prefix(['"', '\''])
        .and_then(|value| value.strip_suffix(['"', '\'']))
        .unwrap_or_else(|| value.trim())
        .to_owned()
}

#[derive(Clone, Debug, Default)]
struct TypeResolutionContext {
    self_type: Option<Type>,
    class_bindings: BTreeMap<String, GenericDeclarationId>,
    generic_parameters: Vec<GenericParameter>,
}

impl TypeResolutionContext {
    fn from_self_type(self_type: Option<Type>) -> Self {
        Self {
            self_type,
            ..Self::default()
        }
    }

    fn with_self_type(self_type: Type) -> Self {
        Self {
            self_type: Some(self_type),
            ..Self::default()
        }
    }

    fn with_classes(path: &Path, classes: &[crate::classes::ClassDeclaration]) -> Self {
        let mut context = Self::default();
        for class in classes {
            context
                .class_bindings
                .insert(class.name.clone(), generic_class_identity(path, class));
        }
        context
    }

    fn for_class(
        path: &Path,
        classes: &[crate::classes::ClassDeclaration],
        class: &crate::classes::ClassDeclaration,
    ) -> Self {
        let mut context = Self::with_classes(path, classes);
        context.generic_parameters = class.generic_parameters.clone();
        context.self_type = if class.generic_parameters.is_empty() {
            Some(Type::alias(class.name.clone()))
        } else {
            Some(Type::GenericClassApplication {
                identity: generic_class_identity(path, class),
                name: class.name.clone(),
                arguments: class
                    .generic_parameters
                    .iter()
                    .map(|parameter| Type::GenericParameter {
                        id: parameter.id.clone(),
                        name: parameter.name.clone(),
                    })
                    .collect(),
                span: Some(class.name_span),
            })
        };
        context
    }

    fn self_type(&self) -> Option<&Type> {
        self.self_type.as_ref()
    }
}

fn generic_class_identity(
    path: &Path,
    class: &crate::classes::ClassDeclaration,
) -> GenericDeclarationId {
    class
        .generic_parameters
        .first()
        .map(|parameter| parameter.id.declaration.clone())
        .unwrap_or_else(|| GenericDeclarationId {
            module: crate::vfs::normalize_path(path),
            kind: GenericDeclarationKind::Class,
            name: class.name.clone(),
            declaration: class.name_span,
        })
}

fn resolve_tua_type(
    path: &Path,
    ty: &TuaType,
    span: Option<SourceSpan>,
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    class_declarations: &[crate::classes::ClassDeclaration],
) -> Type {
    let context = TypeResolutionContext::with_classes(path, class_declarations);

    resolve_tua_type_with_context(path, ty, span, alias_names, diagnostics, &context)
}

fn resolve_tua_type_with_context(
    path: &Path,
    ty: &TuaType,
    span: Option<SourceSpan>,
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    context: &TypeResolutionContext,
) -> Type {
    match ty {
        TuaType::Named(name) => {
            resolve_named_type(path, name, span, alias_names, diagnostics, context)
        }
        TuaType::Parameter { id, name } => Type::GenericParameter {
            id: id.clone(),
            name: name.clone(),
        },
        TuaType::Applied {
            name,
            arguments,
            name_span,
            span: application_span,
        } => {
            let arguments = arguments
                .iter()
                .map(|argument| {
                    resolve_tua_type_with_context(
                        path,
                        argument,
                        *application_span,
                        alias_names,
                        diagnostics,
                        context,
                    )
                })
                .collect::<Vec<_>>();
            if let Some(identity) = context.class_bindings.get(name) {
                Type::GenericClassApplication {
                    identity: identity.clone(),
                    name: name.clone(),
                    arguments,
                    span: *application_span,
                }
            } else {
                Type::GenericAliasApplication {
                    reference: GenericAliasReference::Imported {
                        source_module: crate::vfs::normalize_path(path),
                        name: name.clone(),
                        span: name_span.or(*application_span).or(span),
                    },
                    arguments,
                    span: *application_span,
                }
            }
        }
        TuaType::StringLiteral(value) => Type::string_literal(value),
        TuaType::Union(types) => Type::union(types.iter().map(|ty| {
            resolve_tua_type_with_context(path, ty, span, alias_names, diagnostics, context)
        })),
        TuaType::Table(fields) => Type::table(fields.iter().map(|field| {
            (
                field.name.clone(),
                TableField {
                    ty: resolve_table_field_type(path, field, alias_names, diagnostics, context),
                    optional: field.optional,
                },
            )
        })),
        TuaType::Array(item) => Type::array(resolve_tua_type_with_context(
            path,
            item,
            span,
            alias_names,
            diagnostics,
            context,
        )),
        TuaType::Tuple(items) => Type::tuple(items.iter().map(|ty| {
            resolve_tua_type_with_context(path, ty, span, alias_names, diagnostics, context)
        })),
        TuaType::Dictionary { key, value } => Type::dictionary(
            resolve_tua_type_with_context(path, key, span, alias_names, diagnostics, context),
            resolve_tua_type_with_context(path, value, span, alias_names, diagnostics, context),
        ),
        TuaType::Function {
            parameters,
            returns,
        } => Type::Function(function_type_from_tua_signature(
            parameters,
            returns,
            |ty| {
                Some(resolve_tua_type_with_context(
                    path,
                    ty,
                    span,
                    alias_names,
                    diagnostics,
                    context,
                ))
            },
        )),
    }
}

fn resolve_table_field_type(
    path: &Path,
    field: &ParsedTableFieldType,
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    context: &TypeResolutionContext,
) -> Type {
    resolve_tua_type_with_context(
        path,
        &field.ty,
        field.span,
        alias_names,
        diagnostics,
        context,
    )
}

fn resolve_named_type(
    path: &Path,
    name: &str,
    span: Option<SourceSpan>,
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    context: &TypeResolutionContext,
) -> Type {
    let name = name.trim();

    match name {
        "number" => Type::number(),
        "string" => Type::string(),
        "boolean" => Type::boolean(),
        "nil" => Type::nil(),
        "any" => Type::Any,
        "identifier" => Type::string(),
        _ if context
            .generic_parameters
            .iter()
            .any(|parameter| parameter.name == name) =>
        {
            let parameter = context
                .generic_parameters
                .iter()
                .find(|parameter| parameter.name == name)
                .expect("the generic parameter was just found");
            Type::GenericParameter {
                id: parameter.id.clone(),
                name: parameter.name.clone(),
            }
        }
        "Self" => context.self_type().cloned().unwrap_or_else(|| {
            if alias_names.contains(name) {
                Type::alias(name)
            } else {
                diagnostics.push(type_diagnostic(
                    path,
                    span,
                    format!("unknown type `{name}`"),
                    DiagnosticCode::TYPE_UNKNOWN_NAME,
                ));
                Type::Any
            }
        }),
        _ if alias_names.contains(name) => Type::alias(name),
        _ if name.split_once('.').is_some_and(|(namespace, member)| {
            !member.is_empty() && alias_names.contains(namespace)
        }) =>
        {
            // Project analysis resolves this qualified alias after module graph
            // construction. Preserve its identity while keeping an unknown
            // diagnostic that project-aware filtering removes only when the
            // target exists and is publicly exported.
            diagnostics.push(type_diagnostic(
                path,
                span,
                format!("unknown type `{name}`"),
                DiagnosticCode::TYPE_UNKNOWN_NAME,
            ));
            Type::alias(name)
        }
        _ => {
            diagnostics.push(type_diagnostic(
                path,
                span,
                format!("unknown type `{name}`"),
                DiagnosticCode::TYPE_UNKNOWN_NAME,
            ));
            Type::Any
        }
    }
}

fn type_diagnostic(
    path: &Path,
    span: Option<SourceSpan>,
    message: impl Into<String>,
    code: DiagnosticCode,
) -> Diagnostic {
    let position = span
        .map(|span| span.start)
        .unwrap_or(crate::SourcePosition { line: 1, column: 1 });

    let mut diagnostic =
        Diagnostic::error(path, position.line, position.column, message).with_code(code);
    if let Some(span) = span {
        diagnostic = diagnostic.with_range(
            span.start.line,
            span.start.column,
            span.end.line,
            span.end.column,
        );
    }
    diagnostic
}

fn validate_generic_alias_type(
    environment: &TypeEnvironment,
    ty: &Type,
    fallback_span: Option<SourceSpan>,
    parsed: &ParsedSource,
    diagnostics: &mut Vec<Diagnostic>,
    emitted: &mut BTreeSet<(usize, usize, String)>,
) {
    let mut push = |span: Option<SourceSpan>, message: String, code: DiagnosticCode| {
        let span = span.or(fallback_span);
        let position = span
            .map(|span| span.start)
            .unwrap_or(crate::SourcePosition { line: 1, column: 1 });
        if emitted.insert((position.line, position.column, message.clone())) {
            diagnostics.push(type_diagnostic(&parsed.path, span, message, code));
        }
    };

    match ty {
        Type::GenericAliasApplication {
            reference,
            arguments,
            span,
        } => {
            let definition = match reference {
                GenericAliasReference::Declaration(id) => environment.generic_aliases.get(id),
                GenericAliasReference::Imported {
                    source_module,
                    name,
                    ..
                } => environment
                    .generic_alias_bindings
                    .get(&(
                        if source_module.as_os_str().is_empty() {
                            crate::vfs::normalize_path(&environment.path)
                        } else {
                            crate::vfs::normalize_path(source_module)
                        },
                        name.clone(),
                    ))
                    .and_then(|id| environment.generic_aliases.get(id))
                    .or_else(|| {
                        environment.generic_aliases.values().find(|definition| {
                            definition.id.module == *source_module && definition.id.name == *name
                        })
                    }),
            };
            if let Some(definition) = definition {
                if definition.parameters.len() != arguments.len() {
                    let alias_name = &definition.id.name;
                    push(
                        *span,
                        format!(
                            "generic type alias `{alias_name}` expects {} type argument(s), found {}",
                            definition.parameters.len(),
                            arguments.len()
                        ),
                        DiagnosticCode::TYPE_ARGUMENT_COUNT,
                    );
                }
            } else {
                let (name, name_span) = match reference {
                    GenericAliasReference::Declaration(id) => (id.name.as_str(), None),
                    GenericAliasReference::Imported { name, span, .. } => (name.as_str(), *span),
                };
                let imported_here = parsed.type_imports.iter().any(|import| {
                    import.namespace == name
                        || (!import.namespace.is_empty()
                            && name.starts_with(&format!("{}.", import.namespace)))
                        || import.bindings.iter().any(|binding| binding.local == name)
                });
                if let Some(alias) = environment.alias(name) {
                    let _ = alias;
                    push(
                        *span,
                        format!("type alias `{name}` does not accept type arguments"),
                        DiagnosticCode::TYPE_ARGUMENT_COUNT,
                    );
                } else if !imported_here {
                    push(
                        name_span.or(*span),
                        format!("unknown generic type alias `{name}`"),
                        DiagnosticCode::TYPE_UNKNOWN_NAME,
                    );
                }
            }
            for argument in arguments {
                validate_generic_alias_type(
                    environment,
                    argument,
                    *span,
                    parsed,
                    diagnostics,
                    emitted,
                );
            }
        }
        Type::GenericClassApplication {
            identity,
            name,
            arguments,
            span,
        } => {
            if let Some(definition) = environment.generic_classes.get(identity) {
                if definition.parameters.len() != arguments.len() {
                    push(
                        *span,
                        format!(
                            "generic class `{name}` expects {} type argument(s), found {}",
                            definition.parameters.len(),
                            arguments.len()
                        ),
                        DiagnosticCode::TYPE_ARGUMENT_COUNT,
                    );
                } else {
                    let context = environment.to_type_context();
                    for (index, (parameter, actual)) in definition
                        .generic_parameters
                        .iter()
                        .zip(arguments)
                        .enumerate()
                    {
                        let Some(bound) = &parameter.protocol_bound else {
                            continue;
                        };
                        if !context.satisfies_protocol_bound(
                            actual,
                            bound,
                            &parameter.id.declaration.module,
                        ) {
                            push(
                                *span,
                                format!(
                                    "generic class `{name}` argument {} for `{}` must explicitly conform to protocol `{}`, found `{actual}`",
                                    index + 1,
                                    parameter.name,
                                    bound.name
                                ),
                                DiagnosticCode::TYPE_MISMATCH,
                            );
                        }
                    }
                }
            }
            for argument in arguments {
                validate_generic_alias_type(
                    environment,
                    argument,
                    fallback_span,
                    parsed,
                    diagnostics,
                    emitted,
                );
            }
        }
        Type::Alias(name) => {
            let module = crate::vfs::normalize_path(&environment.path);
            if let Some(definition) = environment.generic_classes.values().find(|definition| {
                !definition.parameters.is_empty()
                    && definition.id.module == module
                    && definition.id.name == *name
            }) {
                push(
                    fallback_span,
                    format!(
                        "generic class `{name}` expects {} type argument(s), found 0",
                        definition.parameters.len()
                    ),
                    DiagnosticCode::TYPE_ARGUMENT_COUNT,
                );
            }
            if let Some(id) = environment
                .generic_alias_bindings
                .get(&(crate::vfs::normalize_path(&environment.path), name.clone()))
            {
                if let Some(definition) = environment.generic_aliases.get(id) {
                    push(
                        fallback_span,
                        format!(
                            "generic type alias `{name}` expects {} type argument(s), found 0",
                            definition.parameters.len()
                        ),
                        DiagnosticCode::TYPE_ARGUMENT_COUNT,
                    );
                }
            }
        }
        Type::Union(types) | Type::Tuple(types) => {
            for ty in types {
                validate_generic_alias_type(
                    environment,
                    ty,
                    fallback_span,
                    parsed,
                    diagnostics,
                    emitted,
                );
            }
        }
        Type::Function(function) => {
            for parameter in &function.parameters {
                validate_generic_alias_type(
                    environment,
                    &parameter.ty,
                    fallback_span,
                    parsed,
                    diagnostics,
                    emitted,
                );
            }
            if let Some(vararg) = function.vararg_type.as_deref() {
                validate_generic_alias_type(
                    environment,
                    vararg,
                    fallback_span,
                    parsed,
                    diagnostics,
                    emitted,
                );
            }
            for return_type in function.return_types_or_any() {
                validate_generic_alias_type(
                    environment,
                    return_type,
                    fallback_span,
                    parsed,
                    diagnostics,
                    emitted,
                );
            }
        }
        Type::Table(table) => {
            for (_, field) in table.fields() {
                validate_generic_alias_type(
                    environment,
                    &field.ty,
                    fallback_span,
                    parsed,
                    diagnostics,
                    emitted,
                );
            }
        }
        Type::Array(item) => validate_generic_alias_type(
            environment,
            item,
            fallback_span,
            parsed,
            diagnostics,
            emitted,
        ),
        Type::Dictionary { key, value } => {
            validate_generic_alias_type(
                environment,
                key,
                fallback_span,
                parsed,
                diagnostics,
                emitted,
            );
            validate_generic_alias_type(
                environment,
                value,
                fallback_span,
                parsed,
                diagnostics,
                emitted,
            );
        }
        Type::GenericParameter { .. } | Type::Primitive(_) | Type::Any | Type::StringLiteral(_) => {
        }
    }
}
#[cfg(test)]
mod sharing_tests {
    use super::*;

    #[test]
    fn builtin_catalogs_share_storage_without_leaking_project_aliases() {
        let parsed = crate::parse_tua_source("shared.tua", "return 1").unwrap();
        let first = TypeEnvironment::from_parsed(&parsed).unwrap();
        let second = TypeEnvironment::from_parsed(&parsed).unwrap();
        assert!(Arc::ptr_eq(&first.aliases, &second.aliases));
        assert!(Arc::ptr_eq(&first.locals, &second.locals));
        let changed = first.with_alias_types([("ProjectOnly".to_owned(), Type::number())]);
        assert_eq!(changed.alias("ProjectOnly"), Some(&Type::number()));
        assert!(second.alias("ProjectOnly").is_none());
        assert!(!Arc::ptr_eq(&changed.aliases, &second.aliases));
    }
}

#[cfg(test)]
mod love_context_tests {
    #[test]
    fn love_callback_parameters_use_catalog_without_overriding_annotations() {
        let source = "function love.update(dt)\n  local wrong: string = dt\nend\n";
        let errors = crate::check_tua_source("src/main.tua", source).expect_err("dt is numeric");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("number") && error.message.contains("string")),
            "{errors:?}"
        );
        let hover = crate::hover_tua_source(
            "src/main.tua",
            source,
            crate::SourcePosition {
                line: 2,
                column: 25,
            },
        )
        .unwrap()
        .unwrap();
        assert!(hover.contents.contains("number"), "{}", hover.contents);
        let shadowed =
            "local love = {}\nfunction love.update(dt)\n  local _value: string = dt\nend\n";
        crate::check_tua_source("src/main.tua", shadowed)
            .expect("local love has no catalog context");
        let explicit = "function love.update(dt: string)\n  print(dt)\nend\n";
        let hover = crate::hover_tua_source(
            "src/main.tua",
            explicit,
            crate::SourcePosition { line: 2, column: 9 },
        )
        .unwrap()
        .unwrap();
        assert!(hover.contents.contains("string"), "{}", hover.contents);
    }
}

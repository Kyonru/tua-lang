use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use full_moon::ast::{
    self, Call, Expression, Field, FunctionCall, Index, LastStmt, Prefix, Suffix, Var,
};
use full_moon::node::Node;

use crate::ast_utils::{
    assignment_expression_for_local, field_chain_before_first_call, plain_field_chain_from_var,
    plain_field_chain_from_var_expression,
};
use crate::module_index::required_module_export_type_for_fields;
use crate::narrowing::{directly_assigned_name, narrowed_locals_for_condition};
use crate::parser::{
    FunctionAnnotation as ParsedFunctionAnnotation, FunctionTypeParameter, LuaDocAnnotation,
    LuaDocAnnotationKind, LuaDocParam, LuaDocReturnType, ParsedLanguage, ParsedSource, SourceSpan,
    TableFieldType as ParsedTableFieldType, TuaType,
};
use crate::source_text::function_name_table_field;
use crate::{
    asset, input, lua, object_pool, sprite, state_machine, store, vector, Diagnostic,
    DiagnosticCode,
};

use super::model::{FunctionType, MetatableFacts, TableField, Type, TypeAlias};
use super::{
    function_type_from_tua_signature, widen_inferred_type, FunctionParameter, TypeContext,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeEnvironment {
    path: PathBuf,
    aliases: BTreeMap<String, Type>,
    alias_docs: BTreeMap<String, String>,
    class_names: BTreeSet<String>,
    class_parents: BTreeMap<String, String>,
    locals: BTreeMap<String, Type>,
    local_docs: BTreeMap<String, String>,
    functions: Vec<TypedFunction>,
}

impl TypeEnvironment {
    pub fn from_parsed(parsed: &ParsedSource) -> Result<Self, Vec<Diagnostic>> {
        build_type_environment(parsed)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn alias(&self, name: &str) -> Option<&Type> {
        self.aliases.get(name)
    }

    pub fn alias_documentation(&self, name: &str) -> Option<&str> {
        self.alias_docs.get(name).map(String::as_str)
    }

    pub fn aliases(&self) -> impl Iterator<Item = (&str, &Type)> {
        self.aliases.iter().map(|(name, ty)| (name.as_str(), ty))
    }

    /// Resolves aliases throughout a type while keeping recursive LuaCATS
    /// declarations finite. A repeated alias on the current expansion path is
    /// the recursive back-edge, so only that edge becomes `any`; the outer
    /// table, union, and function structure remains available to semantic and
    /// editor features.
    pub(crate) fn resolve_aliases(&self, ty: &Type) -> Type {
        if !type_contains_alias_cycle(self, ty, 0, &mut BTreeSet::new()) {
            return resolve_shallow_alias_chain(self, ty, 0, &mut BTreeSet::new());
        }

        let mut context = AliasResolutionContext::default();
        context.resolve(self, ty)
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

    pub fn to_type_context(&self) -> TypeContext {
        let mut context = self
            .aliases
            .iter()
            .fold(TypeContext::new(), |context, (name, ty)| {
                context.with_alias(TypeAlias::new(name.clone(), ty.clone()))
            });

        for &(type_name, supertype) in lua::love_type_supertypes() {
            context.insert_nominal_supertype(type_name, supertype);
        }

        for class in &self.class_names {
            context.insert_nominal_alias(class);
        }

        for (class, parent) in &self.class_parents {
            context.insert_nominal_alias_supertype(class, parent);
        }

        context
    }

    pub(crate) fn with_local_types(
        mut self,
        locals: impl IntoIterator<Item = (String, Type)>,
    ) -> Self {
        for (name, ty) in locals {
            self.locals.insert(name, ty);
        }

        self
    }

    pub(crate) fn with_alias_types(
        mut self,
        aliases: impl IntoIterator<Item = (String, Type)>,
    ) -> Self {
        for (name, ty) in aliases {
            self.aliases.entry(name).or_insert(ty);
        }

        self
    }
}

const MAX_ALIAS_RESOLUTION_DEPTH: usize = 64;

fn resolve_shallow_alias_chain(
    environment: &TypeEnvironment,
    ty: &Type,
    depth: usize,
    visiting: &mut BTreeSet<String>,
) -> Type {
    if depth >= MAX_ALIAS_RESOLUTION_DEPTH {
        return Type::Any;
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
        return true;
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
        Type::Primitive(_) | Type::Any | Type::StringLiteral(_) => false,
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
            return Type::Any;
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
                    return Type::Any;
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
                Type::Function(FunctionType::new_signature(
                    parameters,
                    function.required_parameter_count,
                    vararg_type,
                    return_types,
                ))
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
                if let Some(facts) = table.metatable_facts() {
                    resolved.set_metatable_facts(facts.clone());
                }
                if table.has_self_index() {
                    resolved.mark_self_index();
                }
                for method in table.constructor_methods() {
                    resolved.mark_constructor_method(method);
                }
                Type::Table(resolved)
            }
            Type::Array(item) => Type::array(self.resolve(environment, item)),
            Type::Tuple(items) => Type::tuple(items.iter().map(|ty| self.resolve(environment, ty))),
            Type::Dictionary { key, value } => Type::dictionary(
                self.resolve(environment, key),
                self.resolve(environment, value),
            ),
            Type::Primitive(_) | Type::Any | Type::StringLiteral(_) => ty.clone(),
        };
        self.depth -= 1;
        resolved
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedFunction {
    pub name: Option<String>,
    pub parameters: Vec<TypedParameter>,
    pub parameter_count: usize,
    pub required_parameter_count: usize,
    pub has_varargs: bool,
    pub vararg_type: Option<Type>,
    pub return_type: Option<Type>,
    pub return_types: Vec<Type>,
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
            Type::Function(FunctionType::new_signature(
                self.parameters.iter().map(|parameter| {
                    FunctionParameter::named(&parameter.name, parameter.ty.clone())
                }),
                self.required_parameter_count,
                self.vararg_type.clone(),
                self.return_types_or_any(),
            ))
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
    build_type_environment_with_builtin_aliases(parsed, [])
}

pub(crate) fn build_type_environment_with_builtin_aliases(
    parsed: &ParsedSource,
    builtin_aliases: impl IntoIterator<Item = (String, Type)>,
) -> (TypeEnvironment, Vec<Diagnostic>) {
    let builtin_aliases = builtin_aliases.into_iter().collect::<Vec<_>>();
    let mut diagnostics = Vec::new();
    let mut alias_names = collect_alias_names(parsed, &mut diagnostics);
    collect_class_names(parsed, &mut alias_names);
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
        .chain(
            vector::builtin_type_names()
                .iter()
                .map(|name| (*name).to_owned()),
        )
        .chain(
            sprite::builtin_type_names()
                .iter()
                .map(|name| (*name).to_owned()),
        )
        .chain(
            crate::signal::builtin_type_names()
                .iter()
                .map(|name| (*name).to_owned()),
        )
        .chain(
            object_pool::builtin_type_names()
                .iter()
                .map(|name| (*name).to_owned()),
        )
        .chain(
            state_machine::builtin_type_names()
                .iter()
                .map(|name| (*name).to_owned()),
        )
        .chain(
            store::builtin_type_names()
                .iter()
                .map(|name| (*name).to_owned()),
        )
        .chain(
            input::builtin_type_names()
                .iter()
                .map(|name| (*name).to_owned()),
        )
        .chain(builtin_aliases.iter().map(|(name, _)| name.clone()))
        .collect::<BTreeSet<_>>();
    let mut aliases = BTreeMap::new();
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
        if aliases.contains_key(&alias.name) {
            continue;
        }

        let context = TypeResolutionContext::with_self_type(Type::alias(alias.name.clone()));
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
        let ty = resolve_tua_type_with_context(
            &parsed.path,
            &alias.ty,
            Some(alias.span),
            &alias_names,
            &mut diagnostics,
            &context,
        );

        aliases.insert(alias.name.clone(), ty);

        if let Some(documentation) = &alias.documentation {
            alias_docs.insert(alias.name.clone(), documentation.clone());
        }
    }

    aliases
        .entry(vector::TYPE_NAME.to_owned())
        .or_insert_with(vector::vec2_type);
    for (name, ty) in sprite::builtin_aliases() {
        aliases.entry(name).or_insert(ty);
    }
    aliases
        .entry(crate::signal::TYPE_NAME.to_owned())
        .or_insert_with(crate::signal::signal_bus_type);
    for (name, ty) in object_pool::builtin_aliases() {
        aliases.entry(name).or_insert(ty);
    }
    for (name, ty) in state_machine::builtin_aliases() {
        aliases.entry(name).or_insert(ty);
    }
    for (name, ty) in store::builtin_aliases() {
        aliases.entry(name).or_insert(ty);
    }
    for (name, ty) in input::builtin_aliases() {
        aliases.entry(name).or_insert(ty);
    }
    for (name, ty) in builtin_aliases {
        aliases.entry(name).or_insert(ty);
    }

    register_class_aliases(parsed, &alias_names, &mut aliases, &mut diagnostics);

    for local in &parsed.local_annotations {
        let ty = resolve_tua_type(
            &parsed.path,
            &local.ty,
            local.span,
            &alias_names,
            &mut diagnostics,
        );
        locals.insert(local.name.clone(), ty);
    }

    for function in &parsed.function_annotations {
        functions.push(resolve_function_annotation(
            &parsed.path,
            function,
            &alias_names,
            &class_names,
            &mut diagnostics,
        ));
    }

    let mut lua_doc_state = LuaDocLoweringState {
        locals: &mut locals,
        local_docs: &mut local_docs,
        functions: &mut functions,
        diagnostics: &mut diagnostics,
    };
    lower_lua_doc_annotations_into_environment(parsed, &alias_names, &mut lua_doc_state);

    let inferred_returns = infer_function_return_types(parsed, &aliases, &locals, &functions);

    for function in &mut functions {
        if !function.return_types.is_empty() || function.return_type.is_some() {
            continue;
        }

        let Some(name) = &function.name else {
            continue;
        };

        if let Some(return_types) = inferred_returns.get(name) {
            function.return_types = return_types.clone();
            function.return_type = function.return_types.first().cloned();
        }
    }

    register_class_constructor_types(parsed, &mut functions);
    register_class_method_types(parsed, &mut aliases, &functions);

    (
        TypeEnvironment {
            path: parsed.path.clone(),
            aliases,
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
            locals,
            local_docs,
            functions,
        },
        diagnostics,
    )
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
        let instance_type = Type::alias(class.name.clone());

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
    let context = TypeResolutionContext::with_self_type(Type::alias(class_name.to_owned()));
    let mut fields = BTreeMap::new();
    collect_class_alias_fields(
        parsed,
        class_name,
        alias_names,
        diagnostics,
        &context,
        &mut fields,
    );
    Type::table(fields)
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
    diagnostics: &mut Vec<Diagnostic>,
) -> TypedFunction {
    let implicit_self_type = function
        .name
        .as_deref()
        .and_then(|name| function_self_type(name, alias_names));
    let context_self_type = implicit_self_type.clone().or_else(|| {
        function
            .name
            .as_deref()
            .and_then(|name| generated_class_function_owner(name, class_names))
            .map(Type::alias)
    });
    let context = TypeResolutionContext::from_self_type(context_self_type);
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

    let return_type = function.return_type.as_ref().map(|ty| {
        resolve_tua_type_with_context(path, ty, function.span, alias_names, diagnostics, &context)
    });
    let return_types = return_type.iter().cloned().collect();

    TypedFunction {
        name: function.name.clone(),
        parameters,
        parameter_count,
        required_parameter_count,
        has_varargs: function.has_varargs,
        vararg_type: None,
        return_type,
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
        merge_doc_local(parsed, &name, ty, documentation, stmt_span, state);
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
            merge_doc_local(parsed, &name, ty, documentation, group.span, state);
        }

        if let Some(function) =
            lua_doc_function_for_code_line(parsed, &group, alias_names, state.diagnostics)
        {
            merge_doc_function(parsed, function, group.span, state);
        }
    }
}

struct LuaDocGroup<'a> {
    annotations: Vec<&'a LuaDocAnnotation>,
    code: &'a str,
    span: SourceSpan,
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
                    resolve_tua_type_with_context(
                        &parsed.path,
                        &annotation.0.ty,
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
                    .map(|ty| {
                        resolve_lua_doc_return_type(
                            &parsed.path,
                            ty,
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
            LuaDocAnnotationKind::Vararg(vararg) => Some(resolve_tua_type_with_context(
                &parsed.path,
                &vararg.ty,
                Some(vararg.type_span),
                alias_names,
                diagnostics,
                &context,
            )),
            _ => None,
        });

    Some(TypedFunction {
        name: Some(name),
        parameter_count: parameters.len(),
        required_parameter_count: parameters.len(),
        parameters,
        has_varargs: vararg_type.is_some(),
        vararg_type: None,
        return_type,
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
        LuaDocAnnotationKind::Return(return_annotation) => return_annotation
            .returns
            .first()
            .map(|ty| (*annotation, ty)),
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
        resolve_tua_type_with_context(
            &parsed.path,
            &vararg.ty,
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
                    resolve_tua_type_with_context(
                        &parsed.path,
                        &annotation.0.ty,
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
        .map(|(_, ty)| {
            vec![resolve_lua_doc_return_type(
                &parsed.path,
                ty,
                alias_names,
                diagnostics,
                &context,
            )]
        })
        .unwrap_or_default();
    let return_type = return_types.first().cloned();
    let documentation = lua_doc_documentation(docs);

    Some(TypedFunction {
        name,
        parameters,
        parameter_count,
        required_parameter_count: parameter_count,
        has_varargs,
        vararg_type,
        return_type,
        return_types,
        documentation,
        overloads: lua_doc_overloads(parsed, docs, alias_names, diagnostics, &context),
    })
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
    state: &mut LuaDocLoweringState<'_>,
) {
    if let Some(existing) = state.locals.get(name) {
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

fn infer_function_return_types(
    parsed: &ParsedSource,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    functions: &[TypedFunction],
) -> BTreeMap<String, Vec<Type>> {
    let mut returns = BTreeMap::new();
    let parameters = function_parameters_by_name(functions);

    for _ in 0..8 {
        let previous_returns = returns.clone();
        let function_types = function_types_by_name(functions, &returns);

        collect_function_return_types(
            parsed.ast.nodes(),
            parsed,
            aliases,
            locals,
            &function_types,
            &parameters,
            &mut returns,
        );

        if returns == previous_returns {
            break;
        }
    }

    returns
}

fn function_parameters_by_name(
    functions: &[TypedFunction],
) -> BTreeMap<String, Vec<TypedParameter>> {
    functions
        .iter()
        .filter_map(|function| {
            let name = function.name.as_ref()?;

            Some((name.clone(), function.parameters.clone()))
        })
        .collect()
}

fn function_types_by_name(
    functions: &[TypedFunction],
    inferred_returns: &BTreeMap<String, Vec<Type>>,
) -> BTreeMap<String, Type> {
    functions
        .iter()
        .filter_map(|function| {
            let name = function.name.as_ref()?;

            if !function.has_fully_annotated_parameters() {
                return None;
            }

            let return_types = (!function.return_types.is_empty())
                .then(|| function.return_types.clone())
                .or_else(|| inferred_returns.get(name).cloned())
                .or_else(|| function.return_type.clone().map(|ty| vec![ty]))
                .unwrap_or_else(|| vec![Type::Any]);

            Some((
                name.clone(),
                Type::Function(FunctionType::new_signature(
                    function.parameters.iter().map(|parameter| {
                        FunctionParameter::named(&parameter.name, parameter.ty.clone())
                    }),
                    function.required_parameter_count,
                    function.vararg_type.clone(),
                    return_types,
                )),
            ))
        })
        .collect()
}

fn collect_function_return_types(
    block: &ast::Block,
    parsed: &ParsedSource,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    function_types: &BTreeMap<String, Type>,
    parameters: &BTreeMap<String, Vec<TypedParameter>>,
    returns: &mut BTreeMap<String, Vec<Type>>,
) {
    let mut scoped_locals = locals.clone();

    for stmt in block.stmts() {
        match stmt {
            ast::Stmt::Assignment(assignment) => {
                infer_assignment_types(
                    assignment,
                    parsed,
                    aliases,
                    function_types,
                    "",
                    &mut scoped_locals,
                );
            }
            ast::Stmt::LocalAssignment(local) => {
                infer_local_assignment_types(
                    local,
                    parsed,
                    aliases,
                    function_types,
                    "",
                    &mut scoped_locals,
                );
            }
            ast::Stmt::LocalFunction(function) => {
                let name = token_text(function.name());
                let function_locals =
                    locals_with_function_parameters(&scoped_locals, parameters, &name);

                if let Some(return_types) = infer_block_return_types(
                    function.body().block(),
                    parsed,
                    aliases,
                    &function_locals,
                    function_types,
                    &name,
                ) {
                    returns.insert(name, return_types);
                }
                collect_function_return_types(
                    function.body().block(),
                    parsed,
                    aliases,
                    &function_locals,
                    function_types,
                    parameters,
                    returns,
                );
            }
            ast::Stmt::FunctionDeclaration(function) => {
                infer_function_declaration_type(function, function_types, &mut scoped_locals);

                let name = function.name().to_string();
                let function_locals =
                    locals_with_function_parameters(&scoped_locals, parameters, &name);

                if let Some(return_types) = infer_block_return_types(
                    function.body().block(),
                    parsed,
                    aliases,
                    &function_locals,
                    function_types,
                    &name,
                ) {
                    returns.insert(name, return_types);
                }
                collect_function_return_types(
                    function.body().block(),
                    parsed,
                    aliases,
                    &function_locals,
                    function_types,
                    parameters,
                    returns,
                );
            }
            ast::Stmt::Do(block) => {
                collect_function_return_types(
                    block.block(),
                    parsed,
                    aliases,
                    &scoped_locals,
                    function_types,
                    parameters,
                    returns,
                );
            }
            ast::Stmt::GenericFor(for_stmt) => {
                collect_function_return_types(
                    for_stmt.block(),
                    parsed,
                    aliases,
                    &scoped_locals,
                    function_types,
                    parameters,
                    returns,
                );
            }
            ast::Stmt::If(if_stmt) => {
                collect_function_return_types(
                    if_stmt.block(),
                    parsed,
                    aliases,
                    &scoped_locals,
                    function_types,
                    parameters,
                    returns,
                );

                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        collect_function_return_types(
                            else_if.block(),
                            parsed,
                            aliases,
                            &scoped_locals,
                            function_types,
                            parameters,
                            returns,
                        );
                    }
                }

                if let Some(else_block) = if_stmt.else_block() {
                    collect_function_return_types(
                        else_block,
                        parsed,
                        aliases,
                        &scoped_locals,
                        function_types,
                        parameters,
                        returns,
                    );
                }
            }
            ast::Stmt::NumericFor(for_stmt) => {
                collect_function_return_types(
                    for_stmt.block(),
                    parsed,
                    aliases,
                    &scoped_locals,
                    function_types,
                    parameters,
                    returns,
                );
            }
            ast::Stmt::Repeat(repeat) => {
                collect_function_return_types(
                    repeat.block(),
                    parsed,
                    aliases,
                    &scoped_locals,
                    function_types,
                    parameters,
                    returns,
                );
            }
            ast::Stmt::While(while_stmt) => {
                collect_function_return_types(
                    while_stmt.block(),
                    parsed,
                    aliases,
                    &scoped_locals,
                    function_types,
                    parameters,
                    returns,
                );
            }
            _ => {}
        }
    }
}

fn locals_with_function_parameters(
    locals: &BTreeMap<String, Type>,
    parameters: &BTreeMap<String, Vec<TypedParameter>>,
    function_name: &str,
) -> BTreeMap<String, Type> {
    let mut scoped = locals.clone();

    if let Some(parameters) = parameters.get(function_name) {
        for parameter in parameters {
            scoped.insert(parameter.name.clone(), parameter.ty.clone());
        }
    }

    scoped
}

fn infer_block_return_types(
    block: &ast::Block,
    parsed: &ParsedSource,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    function_types: &BTreeMap<String, Type>,
    current_function: &str,
) -> Option<Vec<Type>> {
    let mut returns = Vec::new();

    collect_return_types_in_block(
        block,
        parsed,
        aliases,
        locals,
        function_types,
        current_function,
        &mut returns,
    );

    if returns.is_empty() {
        return None;
    }

    if block_may_fall_through(block) {
        returns.push(vec![Type::nil()]);
    }

    type_from_return_type_lists(returns)
}

fn collect_return_types_in_block(
    block: &ast::Block,
    parsed: &ParsedSource,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    function_types: &BTreeMap<String, Type>,
    current_function: &str,
    returns: &mut Vec<Vec<Type>>,
) {
    let mut scoped_locals = locals.clone();

    for stmt in block.stmts() {
        match stmt {
            ast::Stmt::Assignment(assignment) => {
                infer_assignment_types(
                    assignment,
                    parsed,
                    aliases,
                    function_types,
                    current_function,
                    &mut scoped_locals,
                );
            }
            ast::Stmt::LocalAssignment(local) => {
                infer_local_assignment_types(
                    local,
                    parsed,
                    aliases,
                    function_types,
                    current_function,
                    &mut scoped_locals,
                );
            }
            ast::Stmt::FunctionDeclaration(function) => {
                infer_function_declaration_type(function, function_types, &mut scoped_locals);
            }
            ast::Stmt::FunctionCall(function_call) => {
                record_setmetatable_call(
                    function_call,
                    parsed,
                    aliases,
                    function_types,
                    current_function,
                    &mut scoped_locals,
                );
            }
            ast::Stmt::Do(block) => collect_return_types_in_block(
                block.block(),
                parsed,
                aliases,
                &scoped_locals,
                function_types,
                current_function,
                returns,
            ),
            ast::Stmt::GenericFor(for_stmt) => collect_return_types_in_block(
                for_stmt.block(),
                parsed,
                aliases,
                &scoped_locals,
                function_types,
                current_function,
                returns,
            ),
            ast::Stmt::If(if_stmt) => {
                let branch_locals =
                    locals_with_condition_narrowing(if_stmt.condition(), aliases, &scoped_locals);
                collect_return_types_in_block(
                    if_stmt.block(),
                    parsed,
                    aliases,
                    &branch_locals,
                    function_types,
                    current_function,
                    returns,
                );

                if let Some(else_ifs) = if_stmt.else_if() {
                    for else_if in else_ifs {
                        let branch_locals = locals_with_condition_narrowing(
                            else_if.condition(),
                            aliases,
                            &scoped_locals,
                        );
                        collect_return_types_in_block(
                            else_if.block(),
                            parsed,
                            aliases,
                            &branch_locals,
                            function_types,
                            current_function,
                            returns,
                        );
                    }
                }

                if let Some(else_block) = if_stmt.else_block() {
                    collect_return_types_in_block(
                        else_block,
                        parsed,
                        aliases,
                        &scoped_locals,
                        function_types,
                        current_function,
                        returns,
                    );
                }
            }
            ast::Stmt::NumericFor(for_stmt) => collect_return_types_in_block(
                for_stmt.block(),
                parsed,
                aliases,
                &scoped_locals,
                function_types,
                current_function,
                returns,
            ),
            ast::Stmt::Repeat(repeat) => collect_return_types_in_block(
                repeat.block(),
                parsed,
                aliases,
                &scoped_locals,
                function_types,
                current_function,
                returns,
            ),
            ast::Stmt::While(while_stmt) => collect_return_types_in_block(
                while_stmt.block(),
                parsed,
                aliases,
                &scoped_locals,
                function_types,
                current_function,
                returns,
            ),
            ast::Stmt::LocalFunction(_) => {}
            _ => {}
        }
    }

    if let Some(LastStmt::Return(return_stmt)) = block.last_stmt() {
        if let Some(return_types) = infer_return_statement_types(
            return_stmt,
            parsed,
            aliases,
            &scoped_locals,
            function_types,
            current_function,
        ) {
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
    parsed: &ParsedSource,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    function_types: &BTreeMap<String, Type>,
    current_function: &str,
) -> Option<Vec<Type>> {
    let expressions = return_stmt.returns().iter().collect::<Vec<_>>();

    if expressions.is_empty() {
        return Some(vec![Type::nil()]);
    }

    let fixed_expression_count = expressions.len().saturating_sub(1);
    let mut return_types = Vec::new();

    for (index, expression) in expressions.iter().enumerate() {
        if index < fixed_expression_count {
            return_types.push(infer_expression_return_type(
                parsed,
                expression,
                aliases,
                locals,
                function_types,
                current_function,
                0,
            )?);
            continue;
        }

        let expression_returns = infer_expression_return_types(
            parsed,
            expression,
            aliases,
            locals,
            function_types,
            current_function,
        );

        if expression_returns.is_empty() {
            return_types.push(infer_expression_return_type(
                parsed,
                expression,
                aliases,
                locals,
                function_types,
                current_function,
                0,
            )?);
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
    parsed: &ParsedSource,
    aliases: &BTreeMap<String, Type>,
    function_types: &BTreeMap<String, Type>,
    current_function: &str,
    locals: &mut BTreeMap<String, Type>,
) {
    let expressions = local.expressions().iter().collect::<Vec<_>>();

    for (index, name) in local.names().iter().enumerate() {
        let ty = match assignment_expression_for_local(&expressions, index) {
            Some((expression, return_index)) => {
                let Some(ty) = infer_expression_return_type(
                    parsed,
                    expression,
                    aliases,
                    locals,
                    function_types,
                    current_function,
                    return_index,
                ) else {
                    continue;
                };
                ty
            }
            None if parsed.raw.language == ParsedLanguage::Tua => Type::nil(),
            None => continue,
        };

        let preserve_literal_unions = match assignment_expression_for_local(&expressions, index) {
            Some((Expression::FunctionCall(call), _)) => {
                (state_machine::machine_type_from_call(call).is_some()
                    && !locals.contains_key(state_machine::CONSTRUCTOR_NAME)
                    && !function_types.contains_key(state_machine::CONSTRUCTOR_NAME))
                    || (input::input_type_from_call(call).is_some()
                        && !locals.contains_key(input::CONSTRUCTOR_NAME)
                        && !function_types.contains_key(input::CONSTRUCTOR_NAME))
            }
            _ => false,
        };
        let ty = if parsed.raw.language == ParsedLanguage::Tua && !preserve_literal_unions {
            widen_inferred_type(&ty)
        } else {
            ty
        };
        locals.insert(token_text(name), ty);
    }
}

fn infer_assignment_types(
    assignment: &ast::Assignment,
    parsed: &ParsedSource,
    aliases: &BTreeMap<String, Type>,
    function_types: &BTreeMap<String, Type>,
    current_function: &str,
    locals: &mut BTreeMap<String, Type>,
) {
    for (variable, expression) in assignment.variables().iter().zip(assignment.expressions()) {
        if infer_table_field_assignment_type(
            variable,
            expression,
            parsed,
            aliases,
            function_types,
            current_function,
            locals,
        ) {
            continue;
        }

        let Some(name) = directly_assigned_name(variable) else {
            continue;
        };
        let Some(ty) = infer_expression_type(
            parsed,
            expression,
            aliases,
            locals,
            function_types,
            current_function,
        ) else {
            locals.remove(&name);
            continue;
        };

        if parsed.raw.language == ParsedLanguage::Tua {
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
    function_types: &BTreeMap<String, Type>,
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
    parsed: &ParsedSource,
    aliases: &BTreeMap<String, Type>,
    function_types: &BTreeMap<String, Type>,
    current_function: &str,
    locals: &mut BTreeMap<String, Type>,
) -> bool {
    let Some(parts) = plain_field_chain_from_var(variable) else {
        return false;
    };
    let [root, field] = parts.as_slice() else {
        return false;
    };
    let is_self_index = field == "__index" && expression_is_name(expression, root);
    let Some(ty) = infer_expression_type(
        parsed,
        expression,
        aliases,
        locals,
        function_types,
        current_function,
    ) else {
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
    parsed: &ParsedSource,
    aliases: &BTreeMap<String, Type>,
    function_types: &BTreeMap<String, Type>,
    current_function: &str,
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
    let Some(meta) = infer_expression_type(
        parsed,
        meta,
        aliases,
        locals,
        function_types,
        current_function,
    ) else {
        return;
    };

    locals.insert(name, merge_static_metatable(base, meta));
}

fn locals_with_condition_narrowing(
    condition: &Expression,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
) -> BTreeMap<String, Type> {
    let mut scoped = locals.clone();

    scoped.extend(narrowed_locals_for_condition(condition, aliases, |name| {
        locals.get(name).cloned()
    }));

    scoped
}

fn infer_expression_type(
    parsed: &ParsedSource,
    expression: &Expression,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    function_types: &BTreeMap<String, Type>,
    current_function: &str,
) -> Option<Type> {
    infer_expression_return_type(
        parsed,
        expression,
        aliases,
        locals,
        function_types,
        current_function,
        0,
    )
}

fn infer_expression_return_type(
    parsed: &ParsedSource,
    expression: &Expression,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    function_types: &BTreeMap<String, Type>,
    current_function: &str,
    return_index: usize,
) -> Option<Type> {
    match expression {
        Expression::FunctionCall(function_call) => infer_function_call_return_types(
            parsed,
            function_call,
            aliases,
            locals,
            function_types,
            current_function,
        )
        .get(return_index)
        .cloned(),
        _ if return_index > 0 => None,
        Expression::BinaryOperator { lhs, binop, rhs } => {
            let left = infer_expression_type(
                parsed,
                lhs,
                aliases,
                locals,
                function_types,
                current_function,
            )?;
            let right = infer_expression_type(
                parsed,
                rhs,
                aliases,
                locals,
                function_types,
                current_function,
            )?;

            infer_binary_operator_type(aliases, binop, &left, &right)
        }
        Expression::UnaryOperator { unop, expression } => {
            let operand = infer_expression_type(
                parsed,
                expression,
                aliases,
                locals,
                function_types,
                current_function,
            )?;

            infer_unary_operator_type(aliases, unop, &operand)
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
                        let ty = infer_expression_type(
                            parsed,
                            value,
                            aliases,
                            locals,
                            function_types,
                            current_function,
                        )
                        .unwrap_or(Type::Any);

                        if let Some(key) = string_literal_expression(key) {
                            fields.push((key, TableField::required(ty)));
                        } else if expression_is_number(key) {
                            array_items.push(ty);
                        }
                    }
                    Field::NameKey { key, value, .. } => {
                        let key_text = token_text(key);
                        let ty = infer_expression_type(
                            parsed,
                            value,
                            aliases,
                            locals,
                            function_types,
                            current_function,
                        )
                        .unwrap_or(Type::Any);

                        if key_text == "__index" {
                            fields.extend(index_fields_from_expression(value));
                        }

                        fields.push((key_text, TableField::required(ty)));
                    }
                    Field::NoKey(value) => {
                        array_items.push(
                            infer_expression_type(
                                parsed,
                                value,
                                aliases,
                                locals,
                                function_types,
                                current_function,
                            )
                            .unwrap_or(Type::Any),
                        );
                    }
                    _ => {}
                }
            }

            Some(table_or_array_type(fields, array_items))
        }
        Expression::Parentheses { expression, .. } => infer_expression_return_type(
            parsed,
            expression,
            aliases,
            locals,
            function_types,
            current_function,
            return_index,
        ),
        Expression::TypeAssertion { expression, .. } => infer_expression_return_type(
            parsed,
            expression,
            aliases,
            locals,
            function_types,
            current_function,
            return_index,
        ),
        Expression::Function(function) => Some(function_type_from_body(function.body())),
        Expression::Var(var) => infer_var_type(
            parsed,
            var,
            aliases,
            locals,
            function_types,
            current_function,
        ),
        _ => None,
    }
}

fn infer_expression_return_types(
    parsed: &ParsedSource,
    expression: &Expression,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    function_types: &BTreeMap<String, Type>,
    current_function: &str,
) -> Vec<Type> {
    match expression {
        Expression::FunctionCall(function_call) => infer_function_call_return_types(
            parsed,
            function_call,
            aliases,
            locals,
            function_types,
            current_function,
        ),
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => infer_expression_return_types(
            parsed,
            expression,
            aliases,
            locals,
            function_types,
            current_function,
        ),
        _ => infer_expression_type(
            parsed,
            expression,
            aliases,
            locals,
            function_types,
            current_function,
        )
        .into_iter()
        .collect(),
    }
}

fn infer_var_type(
    parsed: &ParsedSource,
    var: &Var,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    function_types: &BTreeMap<String, Type>,
    current_function: &str,
) -> Option<Type> {
    match var {
        Var::Name(name) => {
            let name = token_text(name);

            locals
                .get(&name)
                .or_else(|| function_types.get(&name))
                .map(|ty| resolve_aliases_from_map(aliases, ty, 0))
        }
        Var::Expression(expression) => {
            if let Some(parts) = plain_field_chain_from_var_expression(expression) {
                if let Some(ty) = infer_field_chain_type(parsed, aliases, locals, &parts) {
                    return Some(ty);
                }
            }

            infer_var_expression_type(
                parsed,
                expression,
                aliases,
                locals,
                function_types,
                current_function,
            )
        }
        _ => None,
    }
}

fn infer_var_expression_type(
    parsed: &ParsedSource,
    expression: &ast::VarExpression,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    function_types: &BTreeMap<String, Type>,
    current_function: &str,
) -> Option<Type> {
    let mut current = match expression.prefix() {
        Prefix::Name(name) => {
            let name = token_text(name);

            locals
                .get(&name)
                .or_else(|| function_types.get(&name))
                .map(|ty| resolve_aliases_from_map(aliases, ty, 0))?
        }
        Prefix::Expression(expression) => infer_expression_type(
            parsed,
            expression,
            aliases,
            locals,
            function_types,
            current_function,
        )?,
        _ => return None,
    };

    for suffix in expression.suffixes() {
        match suffix {
            Suffix::Index(Index::Dot { name, .. }) => {
                current = resolve_table_field_type_from_map(aliases, &current, &token_text(name))?;
            }
            Suffix::Index(Index::Brackets { expression, .. }) => {
                let key = index_key_type(
                    parsed,
                    expression,
                    aliases,
                    locals,
                    function_types,
                    current_function,
                )?;
                current = resolve_index_type_from_map(aliases, &current, &key)?;
            }
            _ => return None,
        }
    }

    Some(current)
}

fn infer_function_call_return_types(
    parsed: &ParsedSource,
    function_call: &FunctionCall,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    function_types: &BTreeMap<String, Type>,
    current_function: &str,
) -> Vec<Type> {
    if let Some(ty) = setmetatable_return_type(
        parsed,
        function_call,
        aliases,
        locals,
        function_types,
        current_function,
    ) {
        return vec![ty];
    }

    if is_tostring_call(function_call) {
        return vec![Type::string()];
    }

    let Some(parts) = field_chain_before_first_call(function_call) else {
        return Vec::new();
    };

    if parts.len() == 1 && parts.first().is_some_and(|name| name == current_function) {
        return Vec::new();
    }

    if parts.len() == 1 {
        let Some(name) = parts.first() else {
            return Vec::new();
        };

        if state_machine::is_constructor(name)
            && !locals.contains_key(name)
            && !function_types.contains_key(name)
        {
            return vec![state_machine::machine_type_from_call(function_call)
                .unwrap_or_else(|| Type::alias(state_machine::TYPE_NAME))];
        }

        if input::is_constructor(name)
            && !locals.contains_key(name)
            && !function_types.contains_key(name)
        {
            return vec![input::input_type_from_call(function_call)
                .unwrap_or_else(|| Type::alias(input::TYPE_NAME))];
        }

        if object_pool::is_constructor(name)
            && !locals.contains_key(name)
            && !function_types.contains_key(name)
        {
            let pool = first_parenthesized_call_argument(function_call)
                .and_then(|options| {
                    infer_expression_type(
                        parsed,
                        options,
                        aliases,
                        locals,
                        function_types,
                        current_function,
                    )
                })
                .map(|options| object_pool::pool_type_from_options(&options))
                .unwrap_or_else(|| Type::alias(object_pool::TYPE_NAME));

            return vec![pool];
        }

        if store::is_constructor(name)
            && !locals.contains_key(name)
            && !function_types.contains_key(name)
        {
            let store = first_parenthesized_call_argument(function_call)
                .and_then(|initial| {
                    infer_expression_type(
                        parsed,
                        initial,
                        aliases,
                        locals,
                        function_types,
                        current_function,
                    )
                })
                .map(|initial| store::store_type_from_initial(&initial))
                .unwrap_or_else(|| Type::alias(store::TYPE_NAME));

            return vec![store];
        }

        let function_type = function_types
            .get(name)
            .or_else(|| locals.get(name))
            .map(|ty| resolve_aliases_from_map(aliases, ty, 0))
            .or_else(|| asset::loader_kind(name).map(|kind| Type::Function(kind.function_type())))
            .or_else(|| {
                vector::is_constructor(name).then(|| Type::Function(vector::constructor_type()))
            })
            .or_else(|| sprite::is_helper(name).then(|| Type::Function(sprite::constructor_type())))
            .or_else(|| {
                crate::signal::is_constructor(name)
                    .then(|| Type::Function(crate::signal::constructor_type()))
            })
            .or_else(|| {
                state_machine::is_constructor(name)
                    .then(|| Type::Function(state_machine::constructor_type()))
            })
            .or_else(|| {
                store::is_constructor(name).then(|| Type::Function(store::constructor_type()))
            })
            .or_else(|| {
                input::is_constructor(name).then(|| Type::Function(input::constructor_type()))
            });

        return function_type
            .map(|ty| function_return_types(aliases, &ty))
            .unwrap_or_default();
    }

    if let Some(method) = parts.last() {
        if let Some(receiver_parts) = parts.get(..parts.len().saturating_sub(1)) {
            if let Some(receiver_type) =
                infer_field_chain_type(parsed, aliases, locals, receiver_parts)
            {
                if let Some(returns) =
                    input::method_return_types_from_direct_type(&receiver_type, function_call)
                {
                    return returns;
                }

                if let Some(ty) =
                    constructor_call_return_type_from_map(aliases, &receiver_type, method)
                {
                    return vec![ty];
                }

                let returns = love_object_method_return_types(aliases, &receiver_type, method);

                if !returns.is_empty() {
                    return returns;
                }
            }
        }
    }

    infer_field_chain_type(parsed, aliases, locals, &parts)
        .map(|ty| function_return_types(aliases, &ty))
        .unwrap_or_default()
}

fn first_parenthesized_call_argument(function_call: &FunctionCall) -> Option<&Expression> {
    let Suffix::Call(Call::AnonymousCall(ast::FunctionArgs::Parentheses { arguments, .. })) =
        function_call.suffixes().next()?
    else {
        return None;
    };

    arguments.iter().next()
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
    parsed: &ParsedSource,
    function_call: &FunctionCall,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    function_types: &BTreeMap<String, Type>,
    current_function: &str,
) -> Option<Type> {
    let (target, meta) = setmetatable_arguments(function_call)?;
    if let Some(class_type) = class_setmetatable_instance_type(parsed, meta) {
        return Some(class_type);
    }
    let base = infer_expression_type(
        parsed,
        target,
        aliases,
        locals,
        function_types,
        current_function,
    )?;
    let meta = infer_expression_type(
        parsed,
        meta,
        aliases,
        locals,
        function_types,
        current_function,
    )?;

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
        | Type::Dictionary { .. } => Vec::new(),
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
        | Type::Dictionary { .. } => Vec::new(),
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
        Type::Table(table) => table
            .semantic_field(field)
            .map(|field| resolve_aliases_from_map(aliases, &field.ty, 0)),
        Type::Union(types) => {
            let mut resolved = Vec::new();

            for ty in types {
                if let Some(field_type) = resolve_table_field_type_from_map(aliases, &ty, field) {
                    resolved.push(field_type);
                }
            }

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
        } => key_accepts_index(&dict_key, key).then_some(*value),
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
                .filter_map(|ty| resolve_index_type_from_map(aliases, ty, key)),
        ),
        Type::Any
        | Type::Alias(_)
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::StringLiteral(_) => None,
    }
}

fn index_key_type(
    parsed: &ParsedSource,
    expression: &Expression,
    aliases: &BTreeMap<String, Type>,
    locals: &BTreeMap<String, Type>,
    function_types: &BTreeMap<String, Type>,
    current_function: &str,
) -> Option<Type> {
    match expression {
        Expression::Number(_) => Some(Type::number()),
        Expression::String(value) => Some(Type::string_literal(unquote_string(&value.to_string()))),
        _ => infer_expression_type(
            parsed,
            expression,
            aliases,
            locals,
            function_types,
            current_function,
        ),
    }
}

fn table_or_array_type(fields: Vec<(String, TableField)>, array_items: Vec<Type>) -> Type {
    let table = (!fields.is_empty()).then(|| Type::table(fields));
    let array = (!array_items.is_empty()).then(|| Type::array(union_types(array_items)));

    match (table, array) {
        (Some(table), Some(array)) => Type::union([table, array]),
        (Some(table), None) => table,
        (None, Some(array)) => array,
        (None, None) => Type::empty_table(),
    }
}

fn union_types(types: Vec<Type>) -> Type {
    let mut unique = Vec::new();
    for ty in types {
        if !unique.contains(&ty) {
            unique.push(ty);
        }
    }

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

fn resolve_aliases_from_map(aliases: &BTreeMap<String, Type>, ty: &Type, depth: usize) -> Type {
    fn resolve_chain(
        aliases: &BTreeMap<String, Type>,
        ty: &Type,
        depth: usize,
        visiting: &mut BTreeSet<String>,
    ) -> Type {
        if depth >= MAX_ALIAS_RESOLUTION_DEPTH {
            return Type::Any;
        }
        let Type::Alias(name) = ty else {
            return ty.clone();
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
    token.to_string().trim().to_owned()
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
}

impl TypeResolutionContext {
    fn from_self_type(self_type: Option<Type>) -> Self {
        Self { self_type }
    }

    fn with_self_type(self_type: Type) -> Self {
        Self {
            self_type: Some(self_type),
        }
    }

    fn self_type(&self) -> Option<&Type> {
        self.self_type.as_ref()
    }
}

fn resolve_tua_type(
    path: &Path,
    ty: &TuaType,
    span: Option<SourceSpan>,
    alias_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Type {
    let context = TypeResolutionContext::default();

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

    Diagnostic::error(path, position.line, position.column, message).with_code(code)
}

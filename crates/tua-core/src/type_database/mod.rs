use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::ecs;
use crate::input;
use crate::module_graph::{ModuleExportOrigin, ModuleGraph, RequireResolution};
use crate::sprite;
use crate::store;
use crate::vector;
use crate::vfs::normalize_path;
use crate::{
    AnalysisSourceKind, FunctionType, ModuleExportField, ScopeDeclarationKind, ScopeMap, Type,
    TypeEnvironment, TypedFunction,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectTypeDatabase {
    modules: BTreeMap<PathBuf, ProjectTypeModule>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTypeModule {
    pub path: PathBuf,
    pub source_kind: AnalysisSourceKind,
    class_names: BTreeSet<String>,
    aliases: BTreeMap<String, ProjectTypeBinding>,
    locals: BTreeMap<String, ProjectTypeBinding>,
    functions: BTreeMap<String, TypedFunction>,
    module_aliases: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTypeBinding {
    pub module_path: PathBuf,
    pub name: String,
    pub ty: Type,
    pub origin: ProjectTypeOrigin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectTypeOrigin {
    Tua,
    LuaDoc,
    ModuleExport,
    Builtin,
}

#[derive(Clone, Debug)]
pub struct ProjectTypeDatabaseInput<'a> {
    pub path: &'a Path,
    pub source_kind: AnalysisSourceKind,
    pub environment: Option<&'a TypeEnvironment>,
    pub scope: Option<&'a ScopeMap>,
}

impl ProjectTypeDatabase {
    pub fn build<'a>(
        inputs: impl IntoIterator<Item = ProjectTypeDatabaseInput<'a>>,
        module_graph: &ModuleGraph,
    ) -> Self {
        let mut database = Self::default();

        for input in inputs {
            let path = normalize_path(input.path);
            let origin = origin_for_source_kind(input.source_kind);
            let mut module = ProjectTypeModule {
                path: path.clone(),
                source_kind: input.source_kind,
                class_names: BTreeSet::new(),
                aliases: BTreeMap::new(),
                locals: BTreeMap::new(),
                functions: BTreeMap::new(),
                module_aliases: module_aliases(input.scope),
            };

            if let Some(environment) = input.environment {
                for (name, ty) in environment.aliases() {
                    if environment.is_class(name) {
                        module.class_names.insert(name.to_owned());
                    }
                    module.aliases.insert(
                        name.to_owned(),
                        ProjectTypeBinding {
                            module_path: path.clone(),
                            name: name.to_owned(),
                            ty: ty.clone(),
                            origin,
                        },
                    );
                }

                for (name, ty) in environment.locals() {
                    module.locals.insert(
                        name.to_owned(),
                        ProjectTypeBinding {
                            module_path: path.clone(),
                            name: name.to_owned(),
                            ty: ty.clone(),
                            origin,
                        },
                    );
                }

                for function in environment.functions() {
                    if let Some(name) = &function.name {
                        module.functions.insert(name.clone(), function.clone());
                    }
                }
            }

            if let Some(graph_module) = module_graph.module(&path) {
                for export in &graph_module.exports {
                    module.locals.insert(
                        export.public_name.clone(),
                        ProjectTypeBinding {
                            module_path: path.clone(),
                            name: export.public_name.clone(),
                            ty: export.field.ty.clone(),
                            origin: ProjectTypeOrigin::ModuleExport,
                        },
                    );

                    if let Type::Function(_) = &export.field.ty {
                        module.functions.insert(
                            export.public_name.clone(),
                            typed_function_from_export(&export.public_name, &export.field),
                        );
                    }

                    if export.origin == ModuleExportOrigin::LuaDoc {
                        module.aliases.entry(export.public_name.clone()).or_insert(
                            ProjectTypeBinding {
                                module_path: path.clone(),
                                name: export.public_name.clone(),
                                ty: export.field.ty.clone(),
                                origin: ProjectTypeOrigin::LuaDoc,
                            },
                        );
                    }
                }
            }

            database.modules.insert(path, module);
        }

        database
    }

    pub fn module(&self, path: impl AsRef<Path>) -> Option<&ProjectTypeModule> {
        self.modules.get(&normalize_path(path))
    }

    pub fn modules(&self) -> impl Iterator<Item = &ProjectTypeModule> {
        self.modules.values()
    }

    pub fn resolve_type_name(
        &self,
        module_graph: &ModuleGraph,
        source_path: impl AsRef<Path>,
        name: &str,
    ) -> Option<ProjectTypeBinding> {
        let source_path = normalize_path(source_path);
        let name = name.trim();
        let binding = self.resolve_type_name_raw(module_graph, &source_path, name)?;
        let resolved = self.resolve_type(module_graph, &binding.module_path, &binding.ty);

        Some(ProjectTypeBinding {
            ty: resolved,
            ..binding
        })
    }

    pub fn resolve_type(
        &self,
        module_graph: &ModuleGraph,
        source_path: impl AsRef<Path>,
        ty: &Type,
    ) -> Type {
        let mut resolving = Vec::new();

        self.resolve_type_inner(
            module_graph,
            &normalize_path(source_path),
            ty,
            &mut resolving,
        )
    }

    pub fn module_export_type(
        &self,
        module_graph: &ModuleGraph,
        source_path: impl AsRef<Path>,
        receiver: &str,
        fields: &[String],
    ) -> Option<Type> {
        let source_path = normalize_path(source_path);
        let module_path = self.module_path_for_alias(module_graph, &source_path, receiver)?;
        let field = self.module_export_field(module_graph, &module_path, fields)?;

        Some(self.resolve_type(module_graph, &module_path, &field.ty))
    }

    pub fn typed_function_for_fields(
        &self,
        module_graph: &ModuleGraph,
        source_path: impl AsRef<Path>,
        receiver: &str,
        fields: &[String],
    ) -> Option<TypedFunction> {
        let source_path = normalize_path(source_path);
        let module_path = self.module_path_for_alias(module_graph, &source_path, receiver)?;
        if fields.len() == 1 {
            if let Some(function) = self.module(&module_path)?.function(&fields[0]) {
                return Some(function.clone());
            }
        }

        let field = self.module_export_field(module_graph, &module_path, fields)?;

        Some(typed_function_from_export(
            fields.last().map(String::as_str).unwrap_or(receiver),
            &field,
        ))
    }

    fn resolve_type_inner(
        &self,
        module_graph: &ModuleGraph,
        source_path: &Path,
        ty: &Type,
        resolving: &mut Vec<(PathBuf, String)>,
    ) -> Type {
        match ty {
            Type::Alias(name) => {
                let Some(binding) = self.resolve_type_name_raw(module_graph, source_path, name)
                else {
                    return ty.clone();
                };
                let key = (binding.module_path.clone(), binding.name.clone());

                if resolving.contains(&key) {
                    // Recursive LuaCATS aliases are valid (for example a
                    // linked-list node referring to `Node` again).  Do not
                    // expand the cycle indefinitely: replacing the back-edge
                    // with `any` preserves the outer table shape and allows
                    // shallow completion while keeping all resolution callers
                    // stack-safe.
                    return Type::Any;
                }

                resolving.push(key);
                let resolved = self.resolve_type_inner(
                    module_graph,
                    &binding.module_path,
                    &binding.ty,
                    resolving,
                );
                resolving.pop();
                resolved
            }
            Type::Union(types) => Type::union(
                types
                    .iter()
                    .map(|ty| self.resolve_type_inner(module_graph, source_path, ty, resolving)),
            ),
            Type::Function(function) => {
                let parameters = function
                    .parameters
                    .iter()
                    .map(|parameter| crate::FunctionParameter {
                        name: parameter.name.clone(),
                        ty: self.resolve_type_inner(
                            module_graph,
                            source_path,
                            &parameter.ty,
                            resolving,
                        ),
                    })
                    .collect::<Vec<_>>();
                let vararg_type = function
                    .vararg_type
                    .as_deref()
                    .map(|ty| self.resolve_type_inner(module_graph, source_path, ty, resolving));
                let return_types = function
                    .return_types_or_any()
                    .iter()
                    .map(|ty| self.resolve_type_inner(module_graph, source_path, ty, resolving))
                    .collect::<Vec<_>>();

                Type::Function(FunctionType::new_signature(
                    parameters,
                    function.required_parameter_count,
                    vararg_type,
                    return_types,
                ))
            }
            Type::Table(table) => {
                let mut resolved_table =
                    crate::TableShape::from_fields(table.fields().map(|(name, field)| {
                        (
                            name.to_owned(),
                            crate::TableField {
                                ty: self.resolve_type_inner(
                                    module_graph,
                                    source_path,
                                    &field.ty,
                                    resolving,
                                ),
                                optional: field.optional,
                            },
                        )
                    }));
                resolved_table.copy_semantic_metadata_from(table);

                Type::Table(resolved_table)
            }
            Type::Array(item) => {
                Type::array(self.resolve_type_inner(module_graph, source_path, item, resolving))
            }
            Type::Tuple(items) => Type::tuple(
                items
                    .iter()
                    .map(|ty| self.resolve_type_inner(module_graph, source_path, ty, resolving)),
            ),
            Type::Dictionary { key, value } => Type::dictionary(
                self.resolve_type_inner(module_graph, source_path, key, resolving),
                self.resolve_type_inner(module_graph, source_path, value, resolving),
            ),
            Type::Primitive(_) | Type::Any | Type::StringLiteral(_) => ty.clone(),
        }
    }

    fn resolve_type_name_raw(
        &self,
        module_graph: &ModuleGraph,
        source_path: &Path,
        name: &str,
    ) -> Option<ProjectTypeBinding> {
        let name = name.trim();

        if let Some(ty) = builtin_type(name) {
            return Some(ProjectTypeBinding {
                module_path: source_path.to_path_buf(),
                name: name.to_owned(),
                ty,
                origin: ProjectTypeOrigin::Builtin,
            });
        }

        if let Some((alias, member)) = name.split_once('.') {
            let alias = alias.trim();
            let member = member.trim();
            let module_path = self.module_path_for_alias(module_graph, source_path, alias)?;

            return self
                .module(&module_path)?
                .aliases
                .get(member)
                .cloned()
                .or_else(|| self.module(&module_path)?.locals.get(member).cloned());
        }

        self.module(source_path)?.aliases.get(name).cloned()
    }

    fn module_path_for_alias(
        &self,
        module_graph: &ModuleGraph,
        source_path: &Path,
        alias: &str,
    ) -> Option<PathBuf> {
        let module = self.module(source_path)?.module_aliases.get(alias)?;

        match module_graph.resolve_require(source_path, module)? {
            RequireResolution::Resolved { path }
            | RequireResolution::Duplicate { selected: path, .. } => Some(path.clone()),
            RequireResolution::Missing { .. } | RequireResolution::Invalid { .. } => None,
        }
    }

    fn module_export_field(
        &self,
        module_graph: &ModuleGraph,
        module_path: &Path,
        fields: &[String],
    ) -> Option<ModuleExportField> {
        let (first, rest) = fields.split_first()?;
        let entry = module_graph.export(module_path, first)?;

        if rest.is_empty() {
            return Some(entry.field.clone());
        }

        let mut current = self.resolve_type(module_graph, module_path, &entry.field.ty);
        for field in rest {
            current = match current {
                Type::Table(table) => table.semantic_field(field)?.ty.clone(),
                Type::Union(types) => {
                    let mut resolved = Vec::new();

                    for ty in types {
                        if let Type::Table(table) =
                            self.resolve_type(module_graph, module_path, &ty)
                        {
                            if let Some(field) = table.semantic_field(field) {
                                resolved.push(field.ty.clone());
                            }
                        }
                    }

                    match resolved.len() {
                        0 => return None,
                        1 => resolved.remove(0),
                        _ => Type::union(resolved),
                    }
                }
                Type::Alias(_) => self.resolve_type(module_graph, module_path, &current),
                Type::Any
                | Type::Function(_)
                | Type::Primitive(_)
                | Type::StringLiteral(_)
                | Type::Array(_)
                | Type::Tuple(_)
                | Type::Dictionary { .. } => return None,
            };
        }

        Some(ModuleExportField {
            ty: current,
            documentation: None,
            deprecation: None,
            parameter_names: Vec::new(),
            parameter_documentation: Vec::new(),
        })
    }
}

impl ProjectTypeModule {
    pub fn alias(&self, name: &str) -> Option<&ProjectTypeBinding> {
        self.aliases.get(name)
    }

    pub fn local(&self, name: &str) -> Option<&ProjectTypeBinding> {
        self.locals.get(name)
    }

    pub fn function(&self, name: &str) -> Option<&TypedFunction> {
        self.functions.get(name)
    }

    pub fn module_alias(&self, name: &str) -> Option<&str> {
        self.module_aliases.get(name).map(String::as_str)
    }
}

fn module_aliases(scope: Option<&ScopeMap>) -> BTreeMap<String, String> {
    scope
        .into_iter()
        .flat_map(ScopeMap::declarations)
        .filter_map(|declaration| match &declaration.kind {
            ScopeDeclarationKind::ModuleAlias { module } => {
                Some((declaration.name.clone(), module.clone()))
            }
            ScopeDeclarationKind::Local
            | ScopeDeclarationKind::Parameter
            | ScopeDeclarationKind::Function
            | ScopeDeclarationKind::TableField => None,
        })
        .collect()
}

fn origin_for_source_kind(kind: AnalysisSourceKind) -> ProjectTypeOrigin {
    match kind {
        AnalysisSourceKind::Tua => ProjectTypeOrigin::Tua,
        AnalysisSourceKind::Lua => ProjectTypeOrigin::LuaDoc,
    }
}

fn builtin_type(name: &str) -> Option<Type> {
    match name {
        "number" | "string" | "boolean" | "nil" | "any" => Some(Type::named(name)),
        vector::TYPE_NAME => Some(vector::vec2_type()),
        crate::signal::TYPE_NAME => Some(crate::signal::signal_bus_type()),
        name if store::builtin_type_names().contains(&name) => store::builtin_aliases()
            .into_iter()
            .find_map(|(alias, ty)| (alias == name).then_some(ty)),
        name if input::builtin_type_names().contains(&name) => input::builtin_aliases()
            .into_iter()
            .find_map(|(alias, ty)| (alias == name).then_some(ty)),
        name if sprite::builtin_type_names().contains(&name) => sprite::builtin_aliases()
            .into_iter()
            .find_map(|(alias, ty)| (alias == name).then_some(ty)),
        name if ecs::builtin_type_names().contains(&name) => ecs::builtin_aliases()
            .into_iter()
            .find_map(|(alias, ty)| (alias == name).then_some(ty)),
        _ => None,
    }
}

fn typed_function_from_export(name: &str, field: &ModuleExportField) -> TypedFunction {
    match &field.ty {
        Type::Function(function) => TypedFunction {
            name: Some(name.to_owned()),
            parameters: function
                .parameters
                .iter()
                .enumerate()
                .map(|(index, parameter)| crate::TypedParameter {
                    name: parameter
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("arg{}", index + 1)),
                    ty: parameter.ty.clone(),
                    documentation: field
                        .parameter_documentation
                        .get(index)
                        .and_then(Clone::clone),
                })
                .collect(),
            parameter_count: function.parameters.len(),
            required_parameter_count: function.required_parameter_count,
            has_varargs: function.vararg_type.is_some(),
            vararg_type: function.vararg_type.as_deref().cloned(),
            return_type: Some((*function.return_type).clone()),
            return_types: function.return_types_or_any().to_vec(),
            documentation: field.documentation.clone(),
            overloads: Vec::new(),
        },
        _ => TypedFunction {
            name: Some(name.to_owned()),
            parameters: Vec::new(),
            parameter_count: 0,
            required_parameter_count: 0,
            has_varargs: true,
            vararg_type: Some(Type::Any),
            return_type: Some(field.ty.clone()),
            return_types: vec![field.ty.clone()],
            documentation: field.documentation.clone(),
            overloads: Vec::new(),
        },
    }
}

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::module_graph::{ModuleExportOrigin, ModuleGraph, RequireResolution};
use crate::vector;
use crate::vfs::normalize_path;
use crate::{
    AnalysisSourceKind, ModuleExportField, ScopeDeclarationKind, ScopeMap, Type, TypeEnvironment,
    TypedFunction,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectTypeDatabase {
    modules: BTreeMap<PathBuf, ProjectTypeModule>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTypeModule {
    pub path: PathBuf,
    pub source_kind: AnalysisSourceKind,
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
                aliases: BTreeMap::new(),
                locals: BTreeMap::new(),
                functions: BTreeMap::new(),
                module_aliases: module_aliases(input.scope),
            };

            if let Some(environment) = input.environment {
                for (name, ty) in environment.aliases() {
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
                    return ty.clone();
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
                    .map(|ty| self.resolve_type_inner(module_graph, source_path, ty, resolving))
                    .collect::<Vec<_>>();
                let return_types = function
                    .return_types_or_any()
                    .iter()
                    .map(|ty| self.resolve_type_inner(module_graph, source_path, ty, resolving))
                    .collect::<Vec<_>>();

                Type::function_returns(parameters, return_types)
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

                if let Some(facts) = table.metatable_facts() {
                    resolved_table.set_metatable_facts(facts.clone());
                }

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
            RequireResolution::Missing { .. } => None,
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
                .map(|(index, ty)| crate::TypedParameter {
                    name: format!("arg{}", index + 1),
                    ty: ty.clone(),
                    documentation: field
                        .parameter_documentation
                        .get(index)
                        .and_then(Clone::clone),
                })
                .collect(),
            parameter_count: function.parameters.len(),
            has_varargs: false,
            vararg_type: None,
            return_type: Some((*function.return_type).clone()),
            return_types: function.return_types_or_any().to_vec(),
            documentation: field.documentation.clone(),
            overloads: Vec::new(),
        },
        _ => TypedFunction {
            name: Some(name.to_owned()),
            parameters: Vec::new(),
            parameter_count: 0,
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

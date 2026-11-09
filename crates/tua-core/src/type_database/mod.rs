use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use crate::audio;
use crate::ecs;
use crate::input;
use crate::module_graph::{ModuleExportOrigin, ModuleGraph, RequireResolution};
use crate::parser::{GenericDeclarationId, TypeAliasId};
use crate::sprite;
use crate::store;
use crate::types::{GenericAliasDefinition, GenericAliasReference, GenericClassDefinition};
use crate::vector;
use crate::vfs::normalize_path;
use crate::{
    AnalysisSourceKind, Definition, FunctionType, ModuleExportField, ParsedSource,
    ScopeDeclarationKind, ScopeMap, Type, TypeEnvironment, TypedFunction,
};

mod protocol_index;

use protocol_index::ProjectProtocolIndex;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectTypeDatabase {
    modules: BTreeMap<PathBuf, ProjectTypeModule>,
    protocols: ProjectProtocolIndex,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTypeModule {
    pub path: PathBuf,
    pub source_kind: AnalysisSourceKind,
    class_names: BTreeSet<String>,
    aliases: BTreeMap<String, ProjectTypeBinding>,
    generic_aliases: BTreeMap<String, GenericAliasDefinition>,
    generic_classes: HashMap<GenericDeclarationId, GenericClassDefinition>,
    locals: BTreeMap<String, ProjectTypeBinding>,
    functions: BTreeMap<String, TypedFunction>,
    module_aliases: BTreeMap<String, String>,
    named_type_imports: BTreeMap<String, ImportedTypeReference>,
    reexported_aliases: BTreeMap<String, ImportedTypeReference>,
    type_import_namespaces: BTreeSet<String>,
    exported_aliases: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ImportedTypeReference {
    module: String,
    imported: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTypeBinding {
    pub module_path: PathBuf,
    pub name: String,
    pub ty: Type,
    pub origin: ProjectTypeOrigin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTypeExport {
    pub module_path: PathBuf,
    pub name: String,
    pub ty: Type,
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
    pub parsed: Option<&'a ParsedSource>,
}

const MAX_PROJECT_TYPE_RESOLUTION_WORK: usize = 8192;
const MAX_GENERIC_ALIAS_INSTANTIATION_DEPTH: usize = 128;

pub(crate) fn protocol_nominal_identity(module_path: &Path, name: &str) -> String {
    format!(
        "{}::protocol::{name}",
        normalize_path(module_path).to_string_lossy()
    )
}

#[derive(Default)]
struct ProjectTypeResolutionContext {
    aliases: Vec<(PathBuf, String)>,
    generic_aliases: Vec<(TypeAliasId, Vec<Type>)>,
    work: usize,
}

impl ProjectTypeDatabase {
    pub fn build<'a>(
        inputs: impl IntoIterator<Item = ProjectTypeDatabaseInput<'a>>,
        module_graph: &ModuleGraph,
    ) -> Self {
        let mut database = Self::default();
        let inputs = inputs.into_iter().collect::<Vec<_>>();

        for input in &inputs {
            let path = normalize_path(input.path);
            let origin = origin_for_source_kind(input.source_kind);
            let mut module = ProjectTypeModule {
                path: path.clone(),
                source_kind: input.source_kind,
                class_names: BTreeSet::new(),
                aliases: BTreeMap::new(),
                generic_aliases: BTreeMap::new(),
                generic_classes: HashMap::new(),
                locals: BTreeMap::new(),
                functions: BTreeMap::new(),
                module_aliases: module_aliases(input.scope, input.parsed),
                named_type_imports: named_type_imports(input.parsed, false),
                reexported_aliases: named_type_imports(input.parsed, true),
                type_import_namespaces: input
                    .parsed
                    .into_iter()
                    .flat_map(|parsed| &parsed.type_imports)
                    .filter(|import| !import.reexport && !import.namespace.is_empty())
                    .map(|import| import.namespace.clone())
                    .chain(
                        input
                            .parsed
                            .into_iter()
                            .flat_map(|parsed| &parsed.type_imports)
                            .filter(|import| import.reexport)
                            .flat_map(|import| import.bindings.iter())
                            .map(|binding| binding.local.clone()),
                    )
                    .collect(),
                exported_aliases: input
                    .parsed
                    .into_iter()
                    .flat_map(|parsed| &parsed.type_aliases)
                    .filter(|alias| alias.exported)
                    .map(|alias| alias.name.clone())
                    .chain(
                        input
                            .parsed
                            .into_iter()
                            .flat_map(|parsed| &parsed.protocol_declarations)
                            .filter(|protocol| protocol.exported)
                            .map(|protocol| protocol.name.clone()),
                    )
                    .collect(),
            };

            if let Some(environment) = input.environment {
                for definition in environment.generic_alias_definitions() {
                    module
                        .generic_aliases
                        .insert(definition.id.name.clone(), definition.clone());
                }

                for definition in environment.generic_class_definitions() {
                    module
                        .generic_classes
                        .insert(definition.id.clone(), definition.clone());
                }

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

        database.protocols = ProjectProtocolIndex::build(
            inputs.iter().filter_map(|input| {
                input
                    .parsed
                    .map(|parsed| (normalize_path(input.path), parsed))
            }),
            module_graph,
            &database,
        );

        database
    }

    pub(crate) fn protocol_method_definition(
        &self,
        module_graph: &ModuleGraph,
        source_path: impl AsRef<Path>,
        protocol_name: &str,
        method: &str,
    ) -> Option<Definition> {
        let identity = self.protocol_identity(module_graph, source_path, protocol_name)?;
        self.protocols.method_definition(&identity, method).cloned()
    }

    pub(crate) fn protocol_method_definitions(
        &self,
        module_graph: &ModuleGraph,
        source_path: impl AsRef<Path>,
        protocol_name: &str,
    ) -> Option<Vec<(String, Definition)>> {
        let identity = self.protocol_identity(module_graph, source_path, protocol_name)?;
        Some(
            self.protocols
                .method_definitions(&identity)?
                .iter()
                .map(|(method, definition)| (method.clone(), definition.clone()))
                .collect(),
        )
    }

    pub(crate) fn protocol_method_implementations(
        &self,
        module_graph: &ModuleGraph,
        source_path: impl AsRef<Path>,
        protocol_name: &str,
        method: &str,
    ) -> Option<Vec<Definition>> {
        let identity = self.protocol_identity(module_graph, source_path, protocol_name)?;
        self.protocols
            .method_implementations(&identity, method)
            .map(<[Definition]>::to_vec)
    }

    pub(crate) fn protocol_related_method_definitions(
        &self,
        target: &Definition,
    ) -> Option<Vec<Definition>> {
        self.protocols.related_method_definitions(target)
    }

    pub(crate) fn is_protocol_type(
        &self,
        module_graph: &ModuleGraph,
        source_path: impl AsRef<Path>,
        protocol_name: &str,
    ) -> bool {
        self.protocol_identity(module_graph, source_path, protocol_name)
            .is_some()
    }

    pub(crate) fn class_protocol_conformances(
        &self,
        class_identity: &str,
    ) -> Vec<(PathBuf, String)> {
        self.protocols
            .class_conformances(class_identity)
            .cloned()
            .collect()
    }

    pub(crate) fn all_class_protocol_conformances(&self) -> Vec<(String, PathBuf, String)> {
        self.protocols.all_class_conformances()
    }

    pub(crate) fn protocol_method_definition_at_position(
        &self,
        path: impl AsRef<Path>,
        position: crate::SourcePosition,
    ) -> Option<Definition> {
        self.protocols
            .method_definition_at_position(path.as_ref(), position)
    }

    pub(crate) fn protocol_identity(
        &self,
        module_graph: &ModuleGraph,
        source_path: impl AsRef<Path>,
        protocol_name: &str,
    ) -> Option<(PathBuf, String)> {
        let binding = self.resolve_type_name(module_graph, source_path, protocol_name)?;
        self.protocols
            .contains_protocol(&binding.module_path, &binding.name)
            .then_some((binding.module_path, binding.name))
    }

    pub fn module(&self, path: impl AsRef<Path>) -> Option<&ProjectTypeModule> {
        self.modules.get(&normalize_path(path))
    }

    pub fn modules(&self) -> impl Iterator<Item = &ProjectTypeModule> {
        self.modules.values()
    }

    pub fn exported_types(&self, module_graph: &ModuleGraph) -> Vec<ProjectTypeExport> {
        self.exported_types_matching(module_graph, |_| true)
    }

    pub fn exported_types_matching(
        &self,
        module_graph: &ModuleGraph,
        mut matches: impl FnMut(&str) -> bool,
    ) -> Vec<ProjectTypeExport> {
        let mut exports = Vec::new();

        for module in self.modules() {
            for name in module.exported_alias_names().filter(|name| matches(name)) {
                let Some(binding) =
                    self.resolve_exported_alias_binding(module_graph, &module.path, name)
                else {
                    continue;
                };
                exports.push(ProjectTypeExport {
                    module_path: module.path.clone(),
                    name: name.to_owned(),
                    ty: self.resolve_type(module_graph, &binding.module_path, &binding.ty),
                });
            }
        }

        exports
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
        let resolved = if self
            .protocols
            .contains_protocol(&binding.module_path, &binding.name)
        {
            match resolved {
                Type::Table(mut table) => {
                    table.mark_nominal(
                        protocol_nominal_identity(&binding.module_path, &binding.name),
                        binding.name.clone(),
                        std::iter::empty(),
                    );
                    Type::Table(table)
                }
                other => other,
            }
        } else {
            resolved
        };

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
        let mut context = ProjectTypeResolutionContext::default();

        self.resolve_type_inner(module_graph, &normalize_path(source_path), ty, &mut context)
    }

    pub(crate) fn generic_alias_definition(
        &self,
        module_graph: &ModuleGraph,
        source_path: impl AsRef<Path>,
        name: &str,
    ) -> Option<GenericAliasDefinition> {
        self.resolve_generic_alias_name(
            module_graph,
            source_path.as_ref(),
            name,
            &mut BTreeSet::new(),
        )
        .cloned()
    }

    pub(crate) fn generic_class_definition(
        &self,
        module_graph: &ModuleGraph,
        identity: &GenericDeclarationId,
    ) -> Option<GenericClassDefinition> {
        let mut definition = self
            .module(&identity.module)?
            .generic_classes
            .get(identity)
            .cloned()?;
        for parameter in &mut definition.generic_parameters {
            if let Some(bound) = &mut parameter.protocol_bound {
                if bound.identity.is_none() {
                    bound.identity = self.protocol_identity(
                        module_graph,
                        &parameter.id.declaration.module,
                        &bound.name,
                    );
                }
            }
        }
        Some(definition)
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
        context: &mut ProjectTypeResolutionContext,
    ) -> Type {
        if context.work >= MAX_PROJECT_TYPE_RESOLUTION_WORK {
            return ty.clone();
        }
        context.work += 1;
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
                    .map(|argument| {
                        self.resolve_type_inner(module_graph, source_path, argument, context)
                    })
                    .collect(),
                span: *span,
            },
            Type::GenericAliasApplication {
                reference,
                arguments,
                span,
            } => {
                if context.generic_aliases.len() >= MAX_GENERIC_ALIAS_INSTANTIATION_DEPTH {
                    return ty.clone();
                }
                let lookup_path = match reference {
                    GenericAliasReference::Declaration(id) => normalize_path(&id.module),
                    GenericAliasReference::Imported { source_module, .. }
                        if !source_module.as_os_str().is_empty() =>
                    {
                        normalize_path(source_module)
                    }
                    GenericAliasReference::Imported { .. } => normalize_path(source_path),
                };
                let arguments = arguments
                    .iter()
                    .map(|argument| {
                        self.resolve_type_inner(module_graph, &lookup_path, argument, context)
                    })
                    .collect::<Vec<_>>();
                let definition = match reference {
                    GenericAliasReference::Declaration(id) => self
                        .module(&id.module)
                        .and_then(|module| module.generic_aliases.get(&id.name))
                        .filter(|definition| definition.id == *id),
                    GenericAliasReference::Imported {
                        source_module,
                        name,
                        ..
                    } => self.resolve_generic_alias_name(
                        module_graph,
                        if source_module.as_os_str().is_empty() {
                            source_path
                        } else {
                            source_module
                        },
                        name,
                        &mut BTreeSet::new(),
                    ),
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
                        reference: GenericAliasReference::Declaration(definition.id.clone()),
                        arguments,
                        span: *span,
                    };
                }

                let key = (definition.id.clone(), arguments.clone());
                if context
                    .generic_aliases
                    .iter()
                    .any(|(active_id, _)| active_id == &definition.id)
                {
                    // Recursive generic aliases may change their arguments on
                    // every edge. Preserve the named edge on the first return
                    // to a declaration instead of expanding a deep prefix.
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
                    .collect::<std::collections::HashMap<_, _>>();
                let body =
                    crate::types::substitute_type_parameters(&definition.body, &substitutions);
                context.generic_aliases.push(key);
                let resolved =
                    self.resolve_type_inner(module_graph, &definition.id.module, &body, context);
                context.generic_aliases.pop();
                resolved
            }
            Type::Alias(name) => {
                let Some(binding) = self.resolve_type_name_raw(module_graph, source_path, name)
                else {
                    return ty.clone();
                };
                if binding.origin == ProjectTypeOrigin::Builtin {
                    return ty.clone();
                }
                let key = (binding.module_path.clone(), binding.name.clone());

                if context.aliases.contains(&key) {
                    if self
                        .protocols
                        .contains_protocol(&binding.module_path, &binding.name)
                    {
                        if let Type::Table(table) = &binding.ty {
                            let mut table = table.clone();
                            table.mark_nominal(
                                protocol_nominal_identity(&binding.module_path, &binding.name),
                                binding.name.clone(),
                                std::iter::empty(),
                            );
                            return table
                                .nominal_reference()
                                .map(Type::Table)
                                .unwrap_or(Type::Any);
                        }
                    }

                    // Recursive LuaCATS aliases are valid (for example a
                    // linked-list node referring to `Node` again).  Do not
                    // expand the cycle indefinitely: replacing the back-edge
                    // with `any` preserves the outer table shape and allows
                    // shallow completion while keeping all resolution callers
                    // stack-safe. Native classes instead retain a finite nominal
                    // reference, which the receiver resolver can hydrate later.
                    return match &binding.ty {
                        Type::Table(table) => table.nominal_reference().map(Type::Table),
                        _ => None,
                    }
                    .unwrap_or(Type::Any);
                }

                context.aliases.push(key);
                let resolved = self.resolve_type_inner(
                    module_graph,
                    &binding.module_path,
                    &binding.ty,
                    context,
                );
                context.aliases.pop();
                if self
                    .protocols
                    .contains_protocol(&binding.module_path, &binding.name)
                {
                    return match resolved {
                        Type::Table(mut table) => {
                            table.mark_nominal(
                                protocol_nominal_identity(&binding.module_path, &binding.name),
                                binding.name,
                                std::iter::empty(),
                            );
                            Type::Table(table)
                        }
                        other => other,
                    };
                }
                resolved
            }
            Type::Union(types) => Type::union(
                types
                    .iter()
                    .map(|ty| self.resolve_type_inner(module_graph, source_path, ty, context)),
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
                            context,
                        ),
                    })
                    .collect::<Vec<_>>();
                let vararg_type = function
                    .vararg_type
                    .as_deref()
                    .map(|ty| self.resolve_type_inner(module_graph, source_path, ty, context));
                let return_types = function
                    .return_types_or_any()
                    .iter()
                    .map(|ty| self.resolve_type_inner(module_graph, source_path, ty, context))
                    .collect::<Vec<_>>();
                let guarded = function.guarded_return_types.as_ref().map(|types| {
                    types
                        .iter()
                        .map(|ty| self.resolve_type_inner(module_graph, source_path, ty, context))
                        .collect()
                });

                let overloads = function
                    .overloads
                    .iter()
                    .map(|overload| {
                        let Type::Function(overload) = self.resolve_type_inner(
                            module_graph,
                            source_path,
                            &Type::Function(overload.clone()),
                            context,
                        ) else {
                            unreachable!()
                        };
                        overload
                    })
                    .collect::<Vec<_>>();
                let generic_parameters = function
                    .generic_parameters
                    .iter()
                    .cloned()
                    .map(|mut parameter| {
                        if let Some(bound) = &mut parameter.protocol_bound {
                            if bound.identity.is_none() {
                                bound.identity = self.protocol_identity(
                                    module_graph,
                                    &parameter.id.declaration.module,
                                    &bound.name,
                                );
                            }
                        }
                        parameter
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
                    .with_overloads(overloads)
                    .with_generic_parameters(generic_parameters),
                )
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
                                    context,
                                ),
                                optional: field.optional,
                            },
                        )
                    }));
                resolved_table.copy_semantic_metadata_from(table);

                Type::Table(resolved_table)
            }
            Type::Array(item) => {
                Type::array(self.resolve_type_inner(module_graph, source_path, item, context))
            }
            Type::Tuple(items) => Type::tuple(
                items
                    .iter()
                    .map(|ty| self.resolve_type_inner(module_graph, source_path, ty, context)),
            ),
            Type::Dictionary { key, value } => Type::dictionary(
                self.resolve_type_inner(module_graph, source_path, key, context),
                self.resolve_type_inner(module_graph, source_path, value, context),
            ),
            Type::Primitive(_)
            | Type::Any
            | Type::StringLiteral(_)
            | Type::GenericParameter { .. } => ty.clone(),
        }
    }

    fn resolve_generic_alias_name(
        &self,
        module_graph: &ModuleGraph,
        source_path: &Path,
        name: &str,
        visiting: &mut BTreeSet<(PathBuf, String)>,
    ) -> Option<&GenericAliasDefinition> {
        let source_path = normalize_path(source_path);
        if !visiting.insert((source_path.clone(), name.to_owned())) {
            return None;
        }
        let module = self.module(&source_path)?;

        if let Some((alias, member)) = name.split_once('.') {
            let target = self.module_path_for_alias(module_graph, &source_path, alias.trim())?;
            let result =
                self.resolve_exported_generic_alias(module_graph, &target, member.trim(), visiting);
            visiting.remove(&(source_path, name.to_owned()));
            return result;
        }

        if let Some(definition) = module.generic_aliases.get(name) {
            visiting.remove(&(source_path, name.to_owned()));
            return Some(definition);
        }

        let imported = module.named_type_imports.get(name)?;
        let target =
            self.module_path_for_specifier(module_graph, &source_path, &imported.module)?;
        let result = self.resolve_exported_generic_alias(
            module_graph,
            &target,
            &imported.imported,
            visiting,
        );
        visiting.remove(&(source_path, name.to_owned()));
        result
    }

    fn resolve_exported_generic_alias(
        &self,
        module_graph: &ModuleGraph,
        module_path: &Path,
        name: &str,
        visiting: &mut BTreeSet<(PathBuf, String)>,
    ) -> Option<&GenericAliasDefinition> {
        let module_path = normalize_path(module_path);
        if !visiting.insert((module_path.clone(), name.to_owned())) {
            return None;
        }
        let module = self.module(&module_path)?;
        if let Some(definition) = module.generic_aliases.get(name) {
            visiting.remove(&(module_path, name.to_owned()));
            return module.exported_aliases.contains(name).then_some(definition);
        }
        let reexport = module.reexported_aliases.get(name)?;
        let target =
            self.module_path_for_specifier(module_graph, &module_path, &reexport.module)?;
        let result = self.resolve_exported_generic_alias(
            module_graph,
            &target,
            &reexport.imported,
            visiting,
        );
        visiting.remove(&(module_path, name.to_owned()));
        result
    }

    fn resolve_type_name_raw(
        &self,
        module_graph: &ModuleGraph,
        source_path: &Path,
        name: &str,
    ) -> Option<ProjectTypeBinding> {
        let name = name.trim();

        if let Some(definition) =
            self.resolve_generic_alias_name(module_graph, source_path, name, &mut BTreeSet::new())
        {
            return Some(ProjectTypeBinding {
                module_path: definition.id.module.clone(),
                name: definition.id.name.clone(),
                ty: Type::GenericAliasApplication {
                    reference: GenericAliasReference::Declaration(definition.id.clone()),
                    arguments: Vec::new(),
                    span: None,
                },
                origin: ProjectTypeOrigin::Tua,
            });
        }

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
            let source_module = self.module(source_path)?;
            let target_module = self.module(&module_path)?;
            if source_module.type_import_namespaces.contains(alias)
                || target_module.source_kind == AnalysisSourceKind::Tua
            {
                return self.resolve_exported_alias_binding(module_graph, &module_path, member);
            }

            return target_module
                .aliases
                .get(member)
                .cloned()
                .or_else(|| self.module(&module_path)?.locals.get(member).cloned());
        }

        let source_module = self.module(source_path)?;
        if let Some(binding) = source_module.aliases.get(name) {
            return Some(binding.clone());
        }
        let imported = source_module.named_type_imports.get(name)?;
        let module_path =
            self.module_path_for_specifier(module_graph, source_path, &imported.module)?;
        self.resolve_exported_alias_binding(module_graph, &module_path, &imported.imported)
    }

    fn resolve_exported_alias_binding(
        &self,
        module_graph: &ModuleGraph,
        module_path: &Path,
        name: &str,
    ) -> Option<ProjectTypeBinding> {
        let mut current_path = module_path.to_path_buf();
        let mut current_name = name.to_owned();
        let mut visiting = BTreeSet::new();

        loop {
            if !visiting.insert((current_path.clone(), current_name.clone())) {
                return None;
            }

            let module = self.module(&current_path)?;
            if let Some(binding) = module.aliases.get(&current_name) {
                return module
                    .exported_aliases
                    .contains(&current_name)
                    .then(|| binding.clone());
            }

            if let Some(definition) = module.generic_aliases.get(&current_name) {
                return module.exported_aliases.contains(&current_name).then(|| {
                    ProjectTypeBinding {
                        module_path: definition.id.module.clone(),
                        name: definition.id.name.clone(),
                        ty: Type::GenericAliasApplication {
                            reference: GenericAliasReference::Declaration(definition.id.clone()),
                            arguments: Vec::new(),
                            span: None,
                        },
                        origin: ProjectTypeOrigin::Tua,
                    }
                });
            }

            let reexport = module.reexported_aliases.get(&current_name)?;
            current_path =
                self.module_path_for_specifier(module_graph, &current_path, &reexport.module)?;
            current_name.clone_from(&reexport.imported);
        }
    }

    pub(crate) fn exported_alias_binding(
        &self,
        module_graph: &ModuleGraph,
        module_path: &Path,
        name: &str,
    ) -> Option<ProjectTypeBinding> {
        self.resolve_exported_alias_binding(module_graph, module_path, name)
    }

    pub(crate) fn reexport_cycle(
        &self,
        module_graph: &ModuleGraph,
        module_path: &Path,
        name: &str,
    ) -> bool {
        let mut visiting = BTreeSet::new();
        let mut current_path = module_path.to_path_buf();
        let mut current_name = name.to_owned();
        loop {
            if !visiting.insert((current_path.clone(), current_name.clone())) {
                return true;
            }
            let Some(module) = self.module(&current_path) else {
                return false;
            };
            if module.aliases.contains_key(&current_name) {
                return false;
            }
            let Some(reexport) = module.reexported_aliases.get(&current_name) else {
                return false;
            };
            let Some(next_path) =
                self.module_path_for_specifier(module_graph, &current_path, &reexport.module)
            else {
                return false;
            };
            current_path = next_path;
            current_name = reexport.imported.clone();
        }
    }

    fn module_path_for_alias(
        &self,
        module_graph: &ModuleGraph,
        source_path: &Path,
        alias: &str,
    ) -> Option<PathBuf> {
        let module = self.module(source_path)?.module_aliases.get(alias)?;
        self.module_path_for_specifier(module_graph, source_path, module)
    }

    fn module_path_for_specifier(
        &self,
        module_graph: &ModuleGraph,
        source_path: &Path,
        module: &str,
    ) -> Option<PathBuf> {
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
                // Generic class member projection is currently local to the
                // owning TypeEnvironment. Do not retry general project type
                // resolution here: it deliberately preserves the nominal
                // application, which would otherwise leave this field walk
                // stuck on the same receiver.
                Type::GenericClassApplication { .. } => return None,
                Type::Any
                | Type::Function(_)
                | Type::Primitive(_)
                | Type::StringLiteral(_)
                | Type::Array(_)
                | Type::Tuple(_)
                | Type::Dictionary { .. }
                | Type::GenericParameter { .. }
                | Type::GenericAliasApplication { .. } => return None,
            };
        }

        Some(ModuleExportField {
            ty: current,
            documentation: None,
            deprecation: None,
            must_use: false,
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

    pub fn exported_alias_names(&self) -> impl Iterator<Item = &str> {
        self.exported_aliases.iter().map(String::as_str).chain(
            self.reexported_aliases
                .keys()
                .filter(|name| !self.exported_aliases.contains(*name))
                .map(String::as_str),
        )
    }
}

fn module_aliases(
    scope: Option<&ScopeMap>,
    parsed: Option<&ParsedSource>,
) -> BTreeMap<String, String> {
    let mut aliases = scope
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
        .collect::<BTreeMap<_, _>>();
    aliases.extend(parsed.into_iter().flat_map(|parsed| {
        parsed
            .type_imports
            .iter()
            .filter(|import| !import.reexport && !import.namespace.is_empty())
            .map(|import| (import.namespace.clone(), import.module.clone()))
    }));
    aliases
}

fn named_type_imports(
    parsed: Option<&ParsedSource>,
    reexport: bool,
) -> BTreeMap<String, ImportedTypeReference> {
    parsed
        .into_iter()
        .flat_map(|parsed| &parsed.type_imports)
        .filter(|import| import.reexport == reexport)
        .flat_map(|import| {
            import.bindings.iter().map(|binding| {
                (
                    binding.local.clone(),
                    ImportedTypeReference {
                        module: import.module.clone(),
                        imported: binding.imported.clone(),
                    },
                )
            })
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
        name if audio::builtin_type_names().contains(&name) => audio::builtin_aliases()
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
            generic_parameters: function.generic_parameters.clone(),
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
            guarded_return_types: None,
            return_types: function.return_types_or_any().to_vec(),
            documentation: field.documentation.clone(),
            overloads: crate::semantic::typed_function_from_function_type(None, function.clone())
                .overloads,
        },
        _ => TypedFunction {
            generic_parameters: Vec::new(),
            name: Some(name.to_owned()),
            parameters: Vec::new(),
            parameter_count: 0,
            required_parameter_count: 0,
            has_varargs: true,
            vararg_type: Some(Type::Any),
            return_type: Some(field.ty.clone()),
            guarded_return_types: None,
            return_types: vec![field.ty.clone()],
            documentation: field.documentation.clone(),
            overloads: Vec::new(),
        },
    }
}

#[cfg(test)]
mod tests;

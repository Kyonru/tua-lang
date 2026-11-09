use super::*;
use crate::Deprecation;
use full_moon::ast::{Call, FunctionArgs, Suffix};
use std::collections::HashSet;

impl AnalysisSnapshot {
    pub fn is_type_completion_position(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
    ) -> bool {
        let path = normalize_path(path.as_ref());
        self.file(&path)
            .and_then(|file| file.parsed.as_ref().ok())
            .is_some_and(|parsed| {
                matches!(
                    crate::completion::context::completion_context(&parsed.source, position),
                    Some(crate::completion::context::CompletionContext::TypeAnnotation { .. })
                )
            })
    }

    pub fn type_import_candidates(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
    ) -> Vec<ProjectTypeExport> {
        let path = normalize_path(path.as_ref());
        let Some(parsed) = self.file(&path).and_then(|file| file.parsed.as_ref().ok()) else {
            return Vec::new();
        };
        let Some(crate::completion::context::CompletionContext::TypeAnnotation { prefix }) =
            crate::completion::context::completion_context(&parsed.source, position)
        else {
            return Vec::new();
        };

        self.type_database
            .exported_types_matching(&self.module_graph, |name| text_matches(name, &prefix))
            .into_iter()
            .filter(|candidate| candidate.module_path != path)
            .collect()
    }

    pub fn resolved_type_import_path(
        &self,
        path: impl AsRef<Path>,
        specifier: &str,
    ) -> Option<PathBuf> {
        let path = normalize_path(path.as_ref());
        let parsed = self.file(&path)?.parsed.as_ref().ok()?;
        self.indexed_module_path_for_specifier(parsed, specifier)
            .map(Path::to_path_buf)
    }

    pub(super) fn project_module_candidates(
        &self,
        current_path: &Path,
        specifier: &str,
    ) -> Vec<PathBuf> {
        if specifier.is_empty() || has_parent_path_component(specifier) {
            return Vec::new();
        }

        let Some(source_root) = self.source_root_for_path(current_path) else {
            return Vec::new();
        };
        let relative = module_relative_path(specifier, self.require_path_separator);
        let extensions = module_extensions(&self.config.extension);

        module_candidates(&source_root, &relative, &extensions)
            .into_iter()
            .map(normalize_path)
            .filter(|candidate| {
                self.vfs.file(candidate).is_some()
                    && self.module_name_for_known_path(candidate).is_some()
            })
            .collect()
    }

    pub(super) fn module_export_for_fields(
        &self,
        module_path: &Path,
        fields: &[String],
    ) -> Option<ModuleExportField> {
        let module_fields = self.indexed_module_export_fields(module_path)?;
        crate::module_index::exported_field_for_fields(
            &ModuleExports {
                path: module_path.to_path_buf(),
                value_type: Type::table(
                    module_fields.iter().map(|(name, field)| {
                        (name.clone(), TableField::required(field.ty.clone()))
                    }),
                ),
                fields: module_fields,
            },
            fields,
        )
    }

    pub(super) fn indexed_module_export_fields(
        &self,
        module_path: &Path,
    ) -> Option<BTreeMap<String, ModuleExportField>> {
        let mut fields = self
            .module_exports(module_path)
            .map(|exports| exports.fields.clone())
            .unwrap_or_default();
        if let Some(module) = self.module_graph.module(module_path) {
            fields.extend(
                module
                    .exports
                    .iter()
                    .map(|entry| (entry.public_name.clone(), entry.field.clone())),
            );
        }

        (!fields.is_empty()).then_some(fields)
    }

    pub(super) fn indexed_module_value_type(&self, module_path: &Path) -> Option<Type> {
        let direct = self
            .module_exports(module_path)
            .map(|exports| exports.value_type.clone());
        let fields = self.indexed_module_export_fields(module_path);

        match (direct, fields) {
            (Some(Type::Table(mut table)), Some(fields)) => {
                for (name, field) in fields {
                    table.insert(name, TableField::required(field.ty));
                }
                Some(Type::Table(table))
            }
            (Some(ty), _) => Some(ty),
            (None, Some(fields)) => Some(Type::table(
                fields
                    .into_iter()
                    .map(|(name, field)| (name, TableField::required(field.ty))),
            )),
            (None, None) => None,
        }
    }

    pub(super) fn project_type_hover(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<Hover> {
        if !is_type_annotation_position(parsed, position) {
            return None;
        }

        let chain = dotted_chain_at_position(&parsed.source, position)?;
        let current_part = chain.parts.get(chain.cursor_index)?;
        let name = chain
            .parts
            .iter()
            .take(chain.cursor_index + 1)
            .map(|part| part.text.as_str())
            .collect::<Vec<_>>()
            .join(".");
        let binding =
            self.type_database
                .resolve_type_name(&self.module_graph, &parsed.path, &name)?;
        let mut hover = Hover {
            is_markdown: false,
            contents: format_type_with_prefix(format!("type {name} = "), &binding.ty),
            span: Some(current_part.span),
            documentation: None,
            context: Vec::new(),
        };

        if binding.module_path != normalize_path(&parsed.path) {
            hover
                .context
                .push(format!("module: {}", binding.module_path.display()));
        }

        Some(hover)
    }

    pub(super) fn project_symbol_hover_with_environment(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
        environment: &TypeEnvironment,
    ) -> Option<Hover> {
        let chain = dotted_chain_at_position(&parsed.source, position)?;

        if chain.cursor_index != 0 || chain.parts.len() != 1 {
            return None;
        }

        let current_part = chain.parts.first()?;
        let semantic = SemanticModel::new(parsed, environment);
        let ty = semantic.type_of_name(&current_part.text, position)?;

        if matches!(ty, Type::Any) {
            return None;
        }

        Some(Hover {
            is_markdown: false,
            contents: format_type_with_prefix(format!("local {}: ", current_part.text), &ty),
            span: Some(current_part.span),
            documentation: None,
            context: Vec::new(),
        })
    }

    pub(super) fn project_annotated_local_hover(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<Hover> {
        let chain = dotted_chain_at_position(&parsed.source, position)?;

        if chain.cursor_index != 0 || chain.parts.len() != 1 {
            return None;
        }

        let current_part = chain.parts.first()?;
        let annotation = parsed
            .local_annotations
            .iter()
            .filter(|annotation| {
                annotation.name == current_part.text
                    && tua_type_uses_qualified_name(&annotation.ty)
                    && annotation
                        .name_span
                        .is_some_and(|span| position_key(span.start) <= position_key(position))
            })
            .max_by_key(|annotation| annotation.name_span.map(|span| position_key(span.start)))?;
        let ty = self.project_type_from_tua_type(parsed, &annotation.ty)?;

        if matches!(ty, Type::Any) {
            return None;
        }

        Some(Hover {
            is_markdown: false,
            contents: format_type_with_prefix(format!("local {}: ", current_part.text), &ty),
            span: Some(current_part.span),
            documentation: None,
            context: Vec::new(),
        })
    }

    pub(super) fn project_completions(
        &self,
        parsed: &ParsedSource,
        source: &str,
        position: SourcePosition,
        file_scope_hash: u64,
    ) -> Vec<Completion> {
        if let Some(completions) = self.shader_uniform_completions(parsed, source, position) {
            return completions;
        }
        let asset_completions = self.project_asset_path_completions(parsed, source, position);

        if !asset_completions.is_empty() {
            return asset_completions;
        }

        let named_type_completions =
            self.project_named_type_import_completions(parsed, source, position);
        if !named_type_completions.is_empty() {
            return named_type_completions;
        }

        let type_import_completions =
            self.project_type_import_completions(parsed, source, position);

        if !type_import_completions.is_empty() {
            return type_import_completions;
        }

        let module_completions = self.project_module_field_completions(parsed, source, position);

        if !module_completions.is_empty() {
            return module_completions;
        }

        let contextual_completions =
            self.project_contextual_parameter_completions(parsed, source, position);

        if !contextual_completions.is_empty() {
            return contextual_completions;
        }

        let table_literal_completions =
            self.project_table_literal_completions(parsed, source, position);

        if !table_literal_completions.is_empty() {
            return table_literal_completions;
        }

        if !matches!(
            crate::completion::context::completion_context(source, position),
            Some(crate::completion::context::CompletionContext::FieldAccess { .. })
        ) {
            return Vec::new();
        }

        let environment = self.semantic_environment(parsed);

        let field_completions = complete_field_access_in_source_with_environment_and_ranking_inputs(
            source,
            parsed,
            position,
            &environment,
            self.completion_ranking_inputs(file_scope_hash),
        );

        if !field_completions.is_empty() {
            field_completions
        } else {
            Vec::new()
        }
    }

    pub(super) fn project_completions_with_environment(
        &self,
        parsed: &ParsedSource,
        source: &str,
        position: SourcePosition,
        environment: &TypeEnvironment,
        file_scope_hash: u64,
    ) -> Vec<Completion> {
        let _profile = profile::span("analysis.completion.render");
        if let Some(completions) = self.shader_uniform_completions(parsed, source, position) {
            return completions;
        }

        let asset_completions = self.project_asset_path_completions(parsed, source, position);

        if !asset_completions.is_empty() {
            return asset_completions;
        }

        let named_type_completions =
            self.project_named_type_import_completions(parsed, source, position);
        if !named_type_completions.is_empty() {
            return named_type_completions;
        }

        let type_import_completions =
            self.project_type_import_completions(parsed, source, position);

        if !type_import_completions.is_empty() {
            return type_import_completions;
        }

        let module_completions = self.project_module_field_completions(parsed, source, position);

        if !module_completions.is_empty() {
            return module_completions;
        }

        let contextual_completions =
            self.project_contextual_parameter_completions(parsed, source, position);

        if !contextual_completions.is_empty() {
            return contextual_completions;
        }

        let field_completions = complete_field_access_in_source_with_environment_and_ranking_inputs(
            source,
            parsed,
            position,
            environment,
            self.completion_ranking_inputs(file_scope_hash),
        );

        if !field_completions.is_empty() {
            return field_completions;
        }

        self.project_table_literal_completions(parsed, source, position)
    }

    pub(super) fn project_asset_path_completions(
        &self,
        parsed: &ParsedSource,
        source: &str,
        position: SourcePosition,
    ) -> Vec<Completion> {
        let Some(crate::completion::context::CompletionContext::StringArgument {
            callee_parts,
            active_parameter,
            prefix,
            ..
        }) = crate::completion::context::completion_context(source, position)
        else {
            return Vec::new();
        };

        if active_parameter != 0 {
            return Vec::new();
        }

        let Some(kind) = asset::asset_loader_kind_for_callee_parts(&callee_parts) else {
            return Vec::new();
        };

        let fallback_scope;
        let scope = match self.file(&parsed.path).and_then(AnalysisFile::scope) {
            Some(scope) => scope,
            None => {
                fallback_scope = scope_map_parsed_source(parsed);
                &fallback_scope
            }
        };
        if !asset::builtin_loader_at_position(parsed, scope, position, &callee_parts[0]) {
            return Vec::new();
        }

        self.asset_index
            .as_ref()
            .map(|index| index.complete_paths(kind, &prefix))
            .unwrap_or_else(|| {
                asset::complete_asset_paths(self.effective_asset_roots(), kind, &prefix)
            })
            .into_iter()
            .map(|candidate| Completion {
                label: candidate.path.clone(),
                kind: CompletionKind::Value,
                detail: Some(format!(
                    "{} asset path -> {}",
                    candidate.kind.asset_label(),
                    candidate.kind.return_type_name()
                )),
                documentation: candidate.extension.map(|extension| {
                    format!(
                        "Supported {} asset with .{} extension.",
                        kind.asset_label(),
                        extension
                    )
                }),
                deprecation: None,
                insert_text: Some(crate::completion::escape_completion_string(
                    source,
                    position,
                    &candidate.path,
                )),
                filter_text: None,
                is_snippet: false,
                module_path: None,
                source_module: None,
                additional_text_edits: Vec::new(),
            })
            .collect()
    }

    pub(super) fn semantic_facts_for_parsed(
        &self,
        parsed: &ParsedSource,
    ) -> Arc<AnalysisSemanticFacts> {
        let path = normalize_path(&parsed.path);

        if let Some(facts) = self
            .semantic_facts
            .lock()
            .expect("semantic facts lock poisoned")
            .get(&path)
            .cloned()
        {
            return facts;
        }

        let file = self
            .file(&path)
            .expect("parsed semantic query must reference an analysis file");
        let semantics = file
            .semantics()
            .expect("parsed semantic query must reference valid semantic facts");
        let _profile = profile::span_for_path("analysis.semantic.facts.file", &path);
        let project_inference = self.project_local_inference(parsed);
        let environment = self.project_type_environment_for_parsed(
            parsed,
            semantics.environment.clone(),
            &project_inference,
        );
        let facts = Arc::new(AnalysisSemanticFacts {
            semantic_index: SemanticIndex::build(parsed, Some(&semantics.hir), &environment),
            environment,
            project_inference,
        });

        self.semantic_facts
            .lock()
            .expect("semantic facts lock poisoned")
            .insert(path, Arc::clone(&facts));

        facts
    }

    pub(super) fn semantic_environment(&self, parsed: &ParsedSource) -> TypeEnvironment {
        if let Some(file) = self.file(&parsed.path) {
            if file.semantics().is_none() {
                if let Some(facts) = self.degraded_semantic_facts(&parsed.path, file) {
                    return facts.environment.clone();
                }
            }
        }
        self.semantic_facts_for_parsed(parsed).environment.clone()
    }

    pub(super) fn project_module_globals(&self) -> &BTreeSet<String> {
        self.project_module_globals.get_or_init(|| {
            self.files
                .values()
                .filter_map(|file| file.semantics())
                .flat_map(|semantics| semantics.scope.globals().values())
                .filter(|global| {
                    global.kind == crate::scope::ScopeGlobalKind::Assigned
                        && crate::builtin_module::module(&global.name).is_some()
                })
                .map(|global| global.name.clone())
                .collect()
        })
    }

    pub(super) fn project_type_environment_for_parsed(
        &self,
        parsed: &ParsedSource,
        base_environment: TypeEnvironment,
        project_inference: &ProjectLocalInference,
    ) -> TypeEnvironment {
        let _profile = profile::span("analysis.semantic.environment");
        let environment = base_environment
            .with_project_module_globals(parsed, self.project_module_globals())
            .with_alias_types(self.project_imported_alias_types(parsed))
            .with_generic_alias_bindings(self.project_imported_generic_alias_bindings(parsed))
            .with_generic_class_definitions(self.project_imported_generic_class_definitions(parsed))
            .with_protocol_names(database::project_imported_protocol_names(
                parsed,
                &self.module_graph,
                &self.type_database,
            ))
            .with_class_protocol_conformances(
                self.project_imported_class_protocol_conformances(parsed),
            )
            .with_protocol_conformance_identities(
                self.type_database.all_class_protocol_conformances(),
            )
            .with_local_types(self.project_module_binding_types(parsed))
            .with_required_module_types(
                self.module_graph
                    .module(&parsed.path)
                    .into_iter()
                    .flat_map(|module| &module.require_edges)
                    .filter_map(|edge| {
                        let path = self.indexed_module_path_for_specifier(parsed, &edge.module)?;
                        let ty = self.indexed_module_value_type(path)?;
                        Some((
                            edge.module.clone(),
                            self.type_database
                                .resolve_type(&self.module_graph, path, &ty),
                        ))
                    }),
            )
            .with_exact_module_bindings(self.project_exact_module_bindings(parsed))
            .with_project_module_signatures(self.project_module_signatures(parsed))
            .with_deprecated_accesses(self.project_module_binding_deprecations(parsed))
            .with_must_use_accesses(self.project_module_binding_must_use_accesses(parsed))
            .with_local_types(project_inference.local_types.clone())
            .with_local_contract_types(self.project_local_annotation_types(parsed));

        if self.config.ecs.enabled {
            environment
                .with_alias_types(crate::ecs::builtin_aliases())
                .with_local_types(crate::ecs::builtin_locals())
        } else {
            environment
        }
    }

    fn project_imported_generic_alias_bindings(
        &self,
        parsed: &ParsedSource,
    ) -> Vec<(PathBuf, String, crate::types::GenericAliasDefinition)> {
        let mut bindings = Vec::new();
        let source_path = normalize_path(&parsed.path);
        for import in parsed.type_imports.iter().filter(|import| !import.reexport) {
            for binding in &import.bindings {
                if let Some(definition) = self.type_database.generic_alias_definition(
                    &self.module_graph,
                    &parsed.path,
                    &binding.local,
                ) {
                    bindings.push((source_path.clone(), binding.local.clone(), definition));
                }
            }

            if import.namespace.is_empty() {
                continue;
            }
            let Some(module_path) = self.indexed_module_path_for_specifier(parsed, &import.module)
            else {
                continue;
            };
            let Some(module) = self.type_database.module(module_path) else {
                continue;
            };
            let exported_names = module
                .exported_alias_names()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            for name in exported_names {
                let local_name = format!("{}.{}", import.namespace, name);
                if let Some(definition) = self.type_database.generic_alias_definition(
                    &self.module_graph,
                    &parsed.path,
                    &local_name,
                ) {
                    bindings.push((source_path.clone(), local_name, definition));
                }
            }
        }
        bindings
    }

    fn project_imported_generic_class_definitions(
        &self,
        parsed: &ParsedSource,
    ) -> Vec<crate::types::GenericClassDefinition> {
        let mut pending = HashSet::new();

        for (_, ty) in self.project_imported_alias_types(parsed) {
            collect_generic_class_identities(&ty, &mut pending);
        }
        for (_, _, definition) in self.project_imported_generic_alias_bindings(parsed) {
            collect_generic_class_identities(&definition.body, &mut pending);
        }

        for binding in require_bindings_for_parsed_source(parsed) {
            let Some(module_path) = self.indexed_required_module_path(parsed, &binding.alias)
            else {
                continue;
            };
            let Some(module_type) = self.indexed_module_value_type(module_path) else {
                continue;
            };
            let resolved =
                self.type_database
                    .resolve_type(&self.module_graph, module_path, &module_type);
            collect_generic_class_identities(&resolved, &mut pending);
        }

        let mut definitions = Vec::new();
        let mut resolved = HashSet::new();
        let mut pending = pending.into_iter().collect::<Vec<_>>();
        while let Some(identity) = pending.pop() {
            if !resolved.insert(identity.clone()) {
                continue;
            }
            let Some(definition) = self
                .type_database
                .generic_class_definition(&self.module_graph, &identity)
            else {
                continue;
            };
            let mut nested = HashSet::new();
            if let Some(parent) = &definition.parent {
                collect_generic_class_identities(parent, &mut nested);
            }
            collect_generic_class_identities(&definition.body, &mut nested);
            pending.extend(nested);
            definitions.push(definition);
        }

        definitions
    }

    fn project_imported_class_protocol_conformances(
        &self,
        parsed: &ParsedSource,
    ) -> Vec<(String, String)> {
        let names = database::referenced_type_names(parsed);
        let mut visible_type_names = BTreeMap::<(PathBuf, String), Vec<String>>::new();
        let mut class_types = Vec::new();
        for name in names {
            let Some(binding) =
                self.type_database
                    .resolve_type_name(&self.module_graph, &parsed.path, &name)
            else {
                continue;
            };
            visible_type_names
                .entry((binding.module_path.clone(), binding.name.clone()))
                .or_default()
                .push(name.clone());
            if let Type::Table(class_type) = &binding.ty {
                if let Some(class_identity) = class_type.nominal_identity() {
                    class_types.push((name, class_identity.to_owned()));
                }
            }
        }

        let mut conformances = BTreeSet::new();

        for (class_type_name, class_identity) in class_types {
            for (protocol_module, protocol_name) in self
                .type_database
                .class_protocol_conformances(&class_identity)
            {
                if let Some(protocol_names) =
                    visible_type_names.get(&(protocol_module, protocol_name))
                {
                    for protocol_name in protocol_names {
                        conformances.insert((class_type_name.clone(), protocol_name.clone()));
                    }
                }
            }
        }

        conformances.into_iter().collect()
    }

    pub(super) fn project_module_binding_types(
        &self,
        parsed: &ParsedSource,
    ) -> Vec<(String, Type)> {
        require_bindings_for_parsed_source(parsed)
            .into_iter()
            .filter_map(|binding| {
                let module_path = self.indexed_required_module_path(parsed, &binding.alias)?;
                let module_type = self.indexed_module_value_type(module_path)?;
                let ty =
                    self.type_database
                        .resolve_type(&self.module_graph, module_path, &module_type);

                Some((binding.alias, ty))
            })
            .collect()
    }

    // Preserve authored alias labels and documentation for presentation without
    // weakening the resolved module types used by inference and checking.
    fn project_module_signatures(
        &self,
        parsed: &ParsedSource,
    ) -> Vec<(String, (Type, TypedFunction))> {
        let mut signatures = Vec::new();
        for binding in require_bindings_for_parsed_source(parsed) {
            let Some(path) = self.indexed_required_module_path(parsed, &binding.alias) else {
                continue;
            };
            let Some(raw) = self.file(path).and_then(|file| file.exports.as_ref()) else {
                continue;
            };
            let Some(exports) = self.indexed_module_export_fields(path) else {
                continue;
            };
            for (name, export) in exports {
                if !matches!(
                    raw.fields.get(&name).map(|field| &field.ty),
                    Some(Type::Function(_))
                ) {
                    continue;
                }
                let Type::Function(resolved) =
                    self.type_database
                        .resolve_type(&self.module_graph, path, &export.ty)
                else {
                    continue;
                };
                let contract = Type::Function(resolved.clone());
                let qualified = format!("{}.{}", binding.alias, name);
                let mut function = crate::semantic::typed_function_from_function_type(
                    Some(qualified.clone()),
                    resolved,
                );
                function.documentation = export.documentation.clone();
                for (index, parameter) in function.parameters.iter_mut().enumerate() {
                    parameter.documentation =
                        export.parameter_documentation.get(index).cloned().flatten();
                }
                if let Some(authored) = raw.fields.get(&name) {
                    if let Type::Function(authored_type) = &authored.ty {
                        for (parameter, authored_parameter) in function
                            .parameters
                            .iter_mut()
                            .zip(&authored_type.parameters)
                        {
                            if matches!(authored_parameter.ty, Type::Alias(_))
                                && self.type_database.resolve_type(
                                    &self.module_graph,
                                    path,
                                    &authored_parameter.ty,
                                ) == parameter.ty
                            {
                                parameter.ty = authored_parameter.ty.clone();
                            }
                        }
                        for (result, authored_result) in function
                            .return_types
                            .iter_mut()
                            .zip(authored_type.return_types_or_any())
                        {
                            if matches!(authored_result, Type::Alias(_))
                                && self.type_database.resolve_type(
                                    &self.module_graph,
                                    path,
                                    authored_result,
                                ) == *result
                            {
                                *result = authored_result.clone();
                            }
                        }
                        function.return_type = function.return_types.first().cloned();
                    }
                }
                signatures.push((qualified, (contract, function)));
            }
        }
        signatures
    }

    fn project_exact_module_bindings(&self, parsed: &ParsedSource) -> Vec<(String, String)> {
        require_bindings_for_parsed_source(parsed)
            .into_iter()
            .filter_map(|binding| {
                let module_path = self.indexed_required_module_path(parsed, &binding.alias)?;
                let module_type = self.indexed_module_value_type(module_path)?;
                matches!(
                    self.type_database
                        .resolve_type(&self.module_graph, module_path, &module_type),
                    Type::Table(_)
                )
                .then_some((binding.alias, binding.module))
            })
            .collect()
    }

    fn project_imported_alias_types(&self, parsed: &ParsedSource) -> Vec<(String, Type)> {
        database::project_imported_alias_types(parsed, &self.module_graph, &self.type_database)
    }

    fn project_module_binding_deprecations(
        &self,
        parsed: &ParsedSource,
    ) -> Vec<(Vec<String>, Deprecation)> {
        let mut deprecations = Vec::new();

        for binding in require_bindings_for_parsed_source(parsed) {
            let Some(module_path) = self.indexed_required_module_path(parsed, &binding.alias)
            else {
                continue;
            };
            let Some(exports) = self.indexed_module_export_fields(module_path) else {
                continue;
            };

            deprecations.extend(exports.iter().filter_map(|(name, field)| {
                field
                    .deprecation
                    .clone()
                    .map(|deprecation| (vec![binding.alias.clone(), name.clone()], deprecation))
            }));
        }

        deprecations
    }

    fn project_module_binding_must_use_accesses(&self, parsed: &ParsedSource) -> Vec<Vec<String>> {
        let mut accesses = Vec::new();

        for binding in require_bindings_for_parsed_source(parsed) {
            let Some(module_path) = self.indexed_required_module_path(parsed, &binding.alias)
            else {
                continue;
            };
            let Some(exports) = self.indexed_module_export_fields(module_path) else {
                continue;
            };

            accesses.extend(
                exports
                    .iter()
                    .filter(|(_, field)| field.must_use)
                    .map(|(name, _)| vec![binding.alias.clone(), name.clone()]),
            );
        }

        accesses
    }

    pub(super) fn project_local_annotation_types(
        &self,
        parsed: &ParsedSource,
    ) -> Vec<(String, crate::SourcePosition, Type)> {
        parsed
            .local_annotations
            .iter()
            .filter(|annotation| {
                tua_type_uses_qualified_name(&annotation.ty)
                    || tua_type_uses_named_import(parsed, &annotation.ty)
            })
            .filter_map(|annotation| {
                let position = annotation.name_span?.start;
                self.project_type_from_tua_type(parsed, &annotation.ty)
                    .map(|ty| (annotation.name.clone(), position, ty))
            })
            .collect()
    }

    pub(super) fn project_local_inference(&self, parsed: &ParsedSource) -> ProjectLocalInference {
        let mut local_types = BTreeMap::<String, Type>::new();
        let mut field_origins = BTreeMap::<String, ProjectAliasFieldOrigin>::new();
        let mut function_return_origins = BTreeMap::<String, ProjectAliasFieldOrigin>::new();

        for stmt in parsed.ast.nodes().stmts() {
            let ast::Stmt::LocalAssignment(local) = stmt else {
                continue;
            };
            let expressions = local.expressions().iter().collect::<Vec<_>>();
            let type_specifiers = local.type_specifiers().collect::<Vec<_>>();
            let mut pending_types = Vec::new();
            let mut pending_field_origins = Vec::new();
            let mut pending_function_return_origins = Vec::new();

            for (index, local_name) in local.names().iter().enumerate() {
                let name = token_text(local_name);
                if type_specifiers
                    .get(index)
                    .is_some_and(|specifier| specifier.is_some())
                {
                    continue;
                }
                let Some((expression, return_index)) =
                    assignment_expression_for_local(&expressions, index)
                else {
                    continue;
                };

                if let Some(ty) =
                    self.project_type_for_expression(parsed, expression, return_index, &local_types)
                {
                    pending_types.push((name.clone(), ty));
                }

                if let Some(origin) = self.project_field_origin_for_expression(
                    parsed,
                    expression,
                    return_index,
                    &field_origins,
                    &function_return_origins,
                ) {
                    pending_field_origins.push((name.clone(), origin));
                }

                if return_index == 0 {
                    if let Some(origin) = self.project_function_return_alias_origin_for_expression(
                        parsed,
                        expression,
                        &function_return_origins,
                    ) {
                        pending_function_return_origins.push((name, origin));
                    }
                }
            }

            for (name, ty) in pending_types {
                local_types.insert(name, ty);
            }

            for (name, origin) in pending_field_origins {
                field_origins.insert(name, origin);
            }

            for (name, origin) in pending_function_return_origins {
                function_return_origins.insert(name, origin);
            }
        }

        ProjectLocalInference {
            local_types: local_types.into_iter().collect(),
            field_origins,
        }
    }

    pub(super) fn project_type_for_expression(
        &self,
        parsed: &ParsedSource,
        expression: &Expression,
        return_index: usize,
        local_types: &BTreeMap<String, Type>,
    ) -> Option<Type> {
        match expression {
            Expression::FunctionCall(function_call) => {
                if function_call
                    .suffixes()
                    .filter(|suffix| matches!(suffix, full_moon::ast::Suffix::Call(_)))
                    .count()
                    > 1
                {
                    let first =
                        self.project_type_for_function_call(parsed, function_call, 0, local_types)?;
                    let aliases = self
                        .project_imported_alias_types(parsed)
                        .into_iter()
                        .collect();
                    return crate::types::resolve_chained_call_returns(
                        &aliases,
                        function_call,
                        vec![first],
                    )
                    .get(return_index)
                    .cloned();
                }
                self.project_type_for_function_call(
                    parsed,
                    function_call,
                    return_index,
                    local_types,
                )
            }
            Expression::Var(var) if return_index == 0 => {
                let parts = plain_field_chain_from_var(var)?;

                self.project_type_for_field_chain(parsed, &parts)
                    .or_else(|| project_local_field_chain_type(local_types, &parts))
            }
            Expression::Parentheses { expression, .. }
            | Expression::TypeAssertion { expression, .. } => {
                self.project_type_for_expression(parsed, expression, return_index, local_types)
            }
            _ => None,
        }
    }

    fn project_type_for_function_call(
        &self,
        parsed: &ParsedSource,
        function_call: &FunctionCall,
        return_index: usize,
        local_types: &BTreeMap<String, Type>,
    ) -> Option<Type> {
        let parts = field_chain_before_first_call(function_call)?;
        let (root, fields) = parts.split_first()?;
        let ty = if fields.is_empty() {
            local_types.get(root).cloned()
        } else {
            self.project_type_for_field_chain(parsed, &parts)
        }?;

        match ty {
            Type::Function(function) => {
                if function.generic_parameters.is_empty() {
                    return function.return_types_or_any().get(return_index).cloned();
                }

                let mut arguments = project_call_argument_expressions(function_call)?
                    .iter()
                    .map(|argument| {
                        self.project_type_for_expression(parsed, argument, 0, local_types)
                            .or_else(|| project_literal_type(argument))
                    })
                    .collect::<Vec<_>>();
                if function_call_has_method_receiver(function_call) {
                    let receiver_parts = parts.get(..parts.len().saturating_sub(1))?;
                    let receiver = self
                        .project_type_for_field_chain(parsed, receiver_parts)
                        .or_else(|| project_local_field_chain_type(local_types, receiver_parts));
                    arguments.insert(0, receiver);
                }

                crate::types::instantiate_generic_function(
                    &function,
                    &arguments,
                    &crate::types::TypeContext::default(),
                )
                .ok()?
                .return_types_or_any()
                .get(return_index)
                .cloned()
            }
            _ => None,
        }
    }

    pub(super) fn project_type_for_field_chain(
        &self,
        parsed: &ParsedSource,
        parts: &[String],
    ) -> Option<Type> {
        let (root, fields) = parts.split_first()?;

        if fields.is_empty() {
            return None;
        }

        self.project_indexed_module_export_type(parsed, root, fields)
            .or_else(|| {
                self.type_database.module_export_type(
                    &self.module_graph,
                    &parsed.path,
                    root,
                    fields,
                )
            })
    }

    pub(super) fn project_indexed_module_export_type(
        &self,
        parsed: &ParsedSource,
        root: &str,
        fields: &[String],
    ) -> Option<Type> {
        let (first, rest) = fields.split_first()?;
        let module_path = self.indexed_required_module_path(parsed, root)?;
        let entry = self
            .indexed_module_export_fields(module_path)?
            .remove(first)?;
        let mut current =
            self.type_database
                .resolve_type(&self.module_graph, module_path, &entry.ty);

        for field in rest {
            current = project_table_field_type(&current, field)?;
        }

        Some(current)
    }

    pub(super) fn project_field_origin_for_expression(
        &self,
        parsed: &ParsedSource,
        expression: &Expression,
        return_index: usize,
        field_origins: &BTreeMap<String, ProjectAliasFieldOrigin>,
        function_return_origins: &BTreeMap<String, ProjectAliasFieldOrigin>,
    ) -> Option<ProjectAliasFieldOrigin> {
        match expression {
            Expression::FunctionCall(function_call) => {
                let parts = field_chain_before_first_call(function_call)?;

                self.project_field_origin_for_call_parts(
                    parsed,
                    &parts,
                    return_index,
                    function_return_origins,
                )
            }
            Expression::Var(var) if return_index == 0 => {
                let parts = plain_field_chain_from_var(var)?;
                let [root] = parts.as_slice() else {
                    return None;
                };

                field_origins.get(root).cloned()
            }
            Expression::Parentheses { expression, .. }
            | Expression::TypeAssertion { expression, .. } => self
                .project_field_origin_for_expression(
                    parsed,
                    expression,
                    return_index,
                    field_origins,
                    function_return_origins,
                ),
            _ => None,
        }
    }

    pub(super) fn project_field_origin_for_call_parts(
        &self,
        parsed: &ParsedSource,
        parts: &[String],
        return_index: usize,
        function_return_origins: &BTreeMap<String, ProjectAliasFieldOrigin>,
    ) -> Option<ProjectAliasFieldOrigin> {
        let (root, fields) = parts.split_first()?;

        if fields.is_empty() {
            return (return_index == 0)
                .then(|| function_return_origins.get(root).cloned())
                .flatten();
        }

        self.project_module_return_alias_origin(parsed, parts, return_index)
    }

    pub(super) fn project_function_return_alias_origin_for_expression(
        &self,
        parsed: &ParsedSource,
        expression: &Expression,
        function_return_origins: &BTreeMap<String, ProjectAliasFieldOrigin>,
    ) -> Option<ProjectAliasFieldOrigin> {
        match expression {
            Expression::Var(var) => {
                let parts = plain_field_chain_from_var(var)?;
                let (root, fields) = parts.split_first()?;

                if fields.is_empty() {
                    return function_return_origins.get(root).cloned();
                }

                self.project_module_return_alias_origin(parsed, &parts, 0)
            }
            Expression::Parentheses { expression, .. }
            | Expression::TypeAssertion { expression, .. } => self
                .project_function_return_alias_origin_for_expression(
                    parsed,
                    expression,
                    function_return_origins,
                ),
            _ => None,
        }
    }

    pub(super) fn project_module_return_alias_origin(
        &self,
        parsed: &ParsedSource,
        parts: &[String],
        return_index: usize,
    ) -> Option<ProjectAliasFieldOrigin> {
        let (root, fields) = parts.split_first()?;
        let [export] = fields else {
            return None;
        };
        let module_path = self
            .indexed_required_module_path(parsed, root)?
            .to_path_buf();
        let entry = self.module_graph.export(&module_path, export)?;
        let Type::Function(function) = &entry.field.ty else {
            return None;
        };
        let Type::Alias(alias_name) = function.return_types_or_any().get(return_index)? else {
            return None;
        };

        Some(ProjectAliasFieldOrigin {
            module_path,
            alias_name: alias_name.clone(),
        })
    }

    pub(super) fn project_imported_return_alias_field_definition(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
        degraded_inference: Option<&ProjectLocalInference>,
    ) -> Option<Definition> {
        let chain = dotted_chain_at_position(&parsed.source, position)?;

        if chain.cursor_index == 0 {
            return None;
        }

        let field = &chain.parts.get(chain.cursor_index)?.text;
        let receiver_parts = chain
            .parts
            .iter()
            .take(chain.cursor_index)
            .map(|part| part.text.clone())
            .collect::<Vec<_>>();
        let [receiver] = receiver_parts.as_slice() else {
            return None;
        };

        if let Some(inference) = degraded_inference {
            let origin = inference.field_origins.get(receiver)?;

            return self.project_alias_field_definition(origin, field);
        }

        let facts = self.semantic_facts_for_parsed(parsed);
        let origin = facts.project_inference.field_origins.get(receiver)?;

        self.project_alias_field_definition(origin, field)
    }

    pub(super) fn project_alias_field_definition(
        &self,
        origin: &ProjectAliasFieldOrigin,
        field: &str,
    ) -> Option<Definition> {
        let file = self.file(&origin.module_path)?;
        let parsed = file.parsed.as_ref().ok()?;
        let span = project_alias_field_span(parsed, &origin.alias_name, field, 0)?;

        Some(Definition {
            path: origin.module_path.clone(),
            span,
            kind: DefinitionKind::TableField,
        })
    }

    pub(super) fn project_type_from_tua_type(
        &self,
        parsed: &ParsedSource,
        ty: &TuaType,
    ) -> Option<Type> {
        match ty {
            TuaType::Named(name) => {
                if self
                    .type_database
                    .is_protocol_type(&self.module_graph, &parsed.path, name)
                {
                    // Keep imported protocol references nominal in annotations.
                    // Expanding them to their structural method table loses the
                    // protocol view required to guard mutable array widening.
                    Some(Type::alias(name.trim()))
                } else {
                    self.type_database
                        .resolve_type_name(&self.module_graph, &parsed.path, name)
                        .map(|binding| {
                            let resolved = self.type_database.resolve_type(
                                &self.module_graph,
                                &binding.module_path,
                                &binding.ty,
                            );
                            if matches!(&resolved, Type::Table(table) if table.nominal_identity().is_some()) {
                                // Keep authored class-instance aliases in the
                                // local annotation so diagnostics and hovers can
                                // show the imported name while TypeContext still
                                // expands it for member lookup and assignability.
                                Type::alias(name.trim())
                            } else {
                                resolved
                            }
                        })
                        .or_else(|| self.required_module_type(parsed, name))
                        .or_else(|| Some(Type::named(name.trim())))
                }
            }
            TuaType::Applied {
                name,
                arguments,
                name_span,
                span,
            } => Some(Type::GenericAliasApplication {
                reference: crate::types::GenericAliasReference::Imported {
                    source_module: normalize_path(&parsed.path),
                    name: name.trim().to_owned(),
                    span: *name_span,
                },
                arguments: arguments
                    .iter()
                    .filter_map(|argument| self.project_type_from_tua_type(parsed, argument))
                    .collect(),
                span: *span,
            }),
            TuaType::Parameter { id, name } => Some(Type::GenericParameter {
                id: id.clone(),
                name: name.clone(),
            }),
            TuaType::StringLiteral(value) => Some(Type::string_literal(value)),
            TuaType::Union(types) => {
                Some(Type::union(types.iter().filter_map(|ty| {
                    self.project_type_from_tua_type(parsed, ty)
                })))
            }
            TuaType::Table(fields) => Some(Type::table(fields.iter().filter_map(|field| {
                self.project_type_from_tua_type(parsed, &field.ty)
                    .map(|ty| {
                        (
                            field.name.clone(),
                            TableField {
                                ty,
                                optional: field.optional,
                            },
                        )
                    })
            }))),
            TuaType::Array(item) => self
                .project_type_from_tua_type(parsed, item)
                .map(Type::array),
            TuaType::Tuple(items) => {
                Some(Type::tuple(items.iter().filter_map(|ty| {
                    self.project_type_from_tua_type(parsed, ty)
                })))
            }
            TuaType::Dictionary { key, value } => Some(Type::dictionary(
                self.project_type_from_tua_type(parsed, key)?,
                self.project_type_from_tua_type(parsed, value)?,
            )),
            TuaType::Function {
                parameters,
                returns,
            } => Some(Type::Function(
                crate::types::function_type_from_tua_signature(parameters, returns, |ty| {
                    self.project_type_from_tua_type(parsed, ty)
                }),
            )),
        }
    }

    pub(super) fn required_module_type(&self, parsed: &ParsedSource, name: &str) -> Option<Type> {
        let (receiver, member) = name.trim().split_once('.')?;

        if let Some(path) = self.indexed_required_module_path(parsed, receiver) {
            let module = self.type_database.module(path)?;
            let binding = if module.source_kind == AnalysisSourceKind::Tua {
                self.type_database.exported_alias_binding(
                    &self.module_graph,
                    path,
                    member.trim(),
                )?
            } else {
                module.alias(member.trim())?.clone()
            };

            return Some(self.type_database.resolve_type(
                &self.module_graph,
                &binding.module_path,
                &binding.ty,
            ));
        }

        None
    }

    pub(super) fn indexed_required_module_path(
        &self,
        parsed: &ParsedSource,
        receiver: &str,
    ) -> Option<&Path> {
        let module = require_bindings_for_parsed_source(parsed)
            .into_iter()
            .find(|binding| binding.alias == receiver)?
            .module;

        self.indexed_module_path_for_specifier(parsed, &module)
    }

    fn indexed_type_import_path(&self, parsed: &ParsedSource, namespace: &str) -> Option<&Path> {
        let module = &parsed
            .type_imports
            .iter()
            .find(|import| import.namespace == namespace)?
            .module;

        self.indexed_module_path_for_specifier(parsed, module)
    }

    fn indexed_module_path_for_specifier(
        &self,
        parsed: &ParsedSource,
        module: &str,
    ) -> Option<&Path> {
        let has_declaration = self
            .module_graph
            .declaration_module_for_path(&parsed.path, module)
            .and_then(|path| self.type_database.module(path))
            .is_some();
        if let Some(path) = self
            .module_graph
            .resolve_require(&parsed.path, module)
            .filter(|_| !has_declaration)
            .and_then(|resolution| match resolution {
                crate::module_graph::RequireResolution::Resolved { path }
                | crate::module_graph::RequireResolution::Duplicate { selected: path, .. } => {
                    Some(path)
                }
                crate::module_graph::RequireResolution::Missing { .. }
                | crate::module_graph::RequireResolution::Invalid { .. } => None,
            })
        {
            return self
                .type_database
                .module(path)
                .map(|module| module.path.as_path());
        }
        if let Some(path) = self
            .module_graph
            .declaration_module_for_path(&parsed.path, module)
        {
            if let Some(module) = self.type_database.module(path) {
                return Some(module.path.as_path());
            }
        }

        // An indexed edge is authoritative, including a missing dependency in
        // this project. Do not search unrelated workspace roots as a fallback.
        if self
            .module_graph
            .resolve_require(&parsed.path, module)
            .is_some()
        {
            return None;
        }

        let type_only = parsed
            .type_imports
            .iter()
            .any(|import| import.module == module);
        let relative =
            module_relative_path(module.trim_start_matches("./"), self.require_path_separator);
        let roots = if module.starts_with("./") {
            parsed.path.parent().into_iter().collect::<Vec<_>>()
        } else {
            self.source_roots().iter().map(PathBuf::as_path).collect()
        };
        for root in roots {
            let base = root.join(&relative);
            let mut candidates = Vec::new();
            if type_only && self.config.extension != "lua" {
                candidates.push(base.with_extension(format!("d.{}", self.config.extension)));
                candidates.push(base.join(format!("init.d.{}", self.config.extension)));
            }
            candidates.push(base.with_extension(&self.config.extension));
            candidates.push(base.join(format!("init.{}", self.config.extension)));
            if self.config.extension != "lua" {
                candidates.push(base.with_extension("lua"));
                candidates.push(base.join("init.lua"));
            }
            if let Some(path) = candidates
                .into_iter()
                .map(normalize_path)
                .find(|candidate| {
                    self.vfs.file(candidate).is_some()
                        && self.type_database.module(candidate).is_some()
                })
            {
                return self
                    .type_database
                    .module(&path)
                    .map(|module| module.path.as_path());
            }
        }
        None
    }

    pub(super) fn project_table_literal_completions(
        &self,
        parsed: &ParsedSource,
        source: &str,
        position: SourcePosition,
    ) -> Vec<Completion> {
        let Some(expected) = self.expected_table_type(parsed, position) else {
            return Vec::new();
        };
        if let Some(field) = table_string_value_field(&expected.table, position) {
            let Some(prefix) = string_literal_prefix_before(source, position) else {
                return Vec::new();
            };
            let Some(field_ty) = self.expected_table_field_type(parsed, &expected, &field) else {
                return Vec::new();
            };
            let mut values = BTreeSet::new();

            collect_string_literal_values(&field_ty, &mut values);

            return values
                .into_iter()
                .filter(|value| text_matches(value, &prefix))
                .map(|value| Completion {
                    label: value.clone(),
                    kind: CompletionKind::Value,
                    detail: Some(format!("literal: \"{value}\"")),
                    documentation: None,
                    deprecation: None,
                    insert_text: None,
                    filter_text: None,
                    is_snippet: false,
                    module_path: None,
                    source_module: None,
                    additional_text_edits: Vec::new(),
                })
                .collect();
        }
        if !table_field_key_position(&expected.table, position) {
            return Vec::new();
        }

        let prefix = identifier_prefix_before(source, position).unwrap_or_default();
        let existing_fields = table_named_fields(&expected.table);
        let mut fields = BTreeMap::new();

        collect_type_fields(&expected.ty, &mut fields);

        fields
            .into_iter()
            .filter(|(name, _)| !existing_fields.contains(name) && text_matches(name, &prefix))
            .map(|(name, ty)| Completion {
                label: name,
                kind: CompletionKind::Field,
                detail: Some(format!("field: {ty}")),
                documentation: None,
                deprecation: None,
                insert_text: None,
                filter_text: None,
                is_snippet: false,
                module_path: None,
                source_module: None,
                additional_text_edits: Vec::new(),
            })
            .collect()
    }

    pub(super) fn project_contextual_parameter_hover(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<Hover> {
        let chain = dotted_chain_at_position(&parsed.source, position)?;
        let root = chain.parts.first()?;
        let root_position = parsed
            .raw
            .parser_position_for_source(root.span.start)
            .unwrap_or(root.span.start);
        let scope = scope_map_parsed_source(parsed);
        let resolves_to_parameter = scope
            .symbol_at_position(root_position)
            .and_then(|symbol| scope.declaration(symbol))
            .is_some_and(|declaration| declaration.kind == ScopeDeclarationKind::Parameter);
        if !resolves_to_parameter {
            return None;
        }
        let part = chain.parts.get(chain.cursor_index)?;
        let (contextual_ty, origin) =
            self.contextual_callback_parameter_type(parsed, &root.text, root.span.start)?;
        let (environment, _) = build_type_environment_with_diagnostics(parsed);
        let declared_ty = SemanticModel::new(parsed, &environment)
            .parameter_type_at_source_position(&root.text, root.span.start);
        let mut ty = match declared_ty {
            Some(ty) if ty != Type::Any => ty,
            _ => contextual_ty,
        };
        for field in chain.parts.iter().take(chain.cursor_index + 1).skip(1) {
            ty = self.field_type_for_type(&origin, &ty, &field.text)?;
        }
        let prefix = if chain.cursor_index == 0 {
            "parameter"
        } else {
            "(field)"
        };

        Some(Hover {
            is_markdown: false,
            contents: format_type_with_prefix(format!("{prefix} {}: ", part.text), &ty),
            span: Some(part.span),
            documentation: None,
            context: vec!["context: expected callback type".to_owned()],
        })
    }

    pub(super) fn project_expected_table_field_hover(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<Hover> {
        let expected = self.expected_table_type(parsed, position)?;
        let (field, span) = table_field_at_position(&expected.table, position)?;
        let table_field =
            self.expected_field_for_type(parsed, &expected.origin_path, &expected.ty, &field)?;
        let ty = self.resolve_expected_type(parsed, &expected.origin_path, &table_field.ty);
        let optional = if table_field.optional { "?" } else { "" };

        Some(Hover {
            is_markdown: false,
            contents: format_type_with_prefix(format!("(field) {field}{optional}: "), &ty),
            span: Some(span),
            documentation: None,
            context: expected
                .builtin_type_name
                .map(|name| format!("context: {name}"))
                .into_iter()
                .collect(),
        })
    }

    pub(super) fn project_store_watch_key_hover(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<Hover> {
        let reference = store_watch_key_reference_at_position(parsed, position)?;
        let (environment, _) = build_type_environment_with_diagnostics(parsed);
        let semantic = SemanticModel::new(parsed, &environment);
        let receiver_type = semantic.type_of_name(&reference.receiver, position)?;
        let state_type = crate::store::store_state_type(&environment, &receiver_type)?;
        let state_type = resolve_aliases(&environment, &state_type, 0);
        let Type::Table(table) = state_type else {
            return None;
        };
        let field = table.semantic_field(&reference.key)?;
        let ty = if field.optional {
            Type::union([field.ty.clone(), Type::nil()])
        } else {
            field.ty.clone()
        };

        Some(Hover {
            is_markdown: false,
            contents: format_type_with_prefix(format!("(field) {}: ", reference.key), &ty),
            span: Some(reference.span),
            documentation: Some(format!(
                "Keyed watcher for the `{}` store state field.",
                reference.key
            )),
            context: vec!["context: Store:watch key".to_owned()],
        })
    }

    pub(super) fn project_derived_helper_hover(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<Hover> {
        let chain = dotted_chain_at_position(&parsed.source, position)?;
        if chain.cursor_index != 0 {
            return None;
        }
        let part = chain.parts.first()?;

        if let Some(animation) = parsed
            .sprite_declarations
            .iter()
            .flat_map(|declaration| &declaration.animations)
            .find(|animation| {
                animation.name == part.text && span_contains_position(animation.name_span, position)
            })
        {
            return Some(Hover {
                is_markdown: false,
                contents: format!(
                    "(animation) {}: {}",
                    animation.name,
                    crate::sprite::ANIMATION_CONFIG_TYPE_NAME
                ),
                span: Some(animation.name_span),
                documentation: Some("Named sprite-sheet animation configuration.".to_owned()),
                context: vec!["context: spriteSheet animation".to_owned()],
            });
        }

        let mut syntax = ExpectedTableSyntaxCollector::default();
        syntax.visit_ast(&parsed.ast);

        syntax.calls.into_iter().find_map(|call| {
            let call_span = node_span(&call)?;
            if !span_contains_position(call_span, position)
                || field_chain_before_first_call(&call)?.as_slice()
                    != [crate::state_machine::CONSTRUCTOR_NAME]
            {
                return None;
            }
            let ty = crate::state_machine::derived_member_type(&call, &part.text)?;

            Some(Hover {
                is_markdown: false,
                contents: format_type_with_prefix(format!("(field) {}: ", part.text), &ty),
                span: Some(part.span),
                documentation: Some(
                    "Callback derived from this state machine's configured events and states."
                        .to_owned(),
                ),
                context: vec!["context: stateMachine callback".to_owned()],
            })
        })
    }

    pub(super) fn project_store_watch_key_definition(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<Definition> {
        let reference = store_watch_key_reference_at_position(parsed, position)?;
        let (environment, _) = build_type_environment_with_diagnostics(parsed);
        let call = helper_constructor_call_for_local(
            parsed,
            &environment,
            &reference.receiver,
            position,
            crate::store::CONSTRUCTOR_NAME,
        )?;
        let span = store_state_field_span(&call, &reference.key)?;

        Some(Definition {
            path: parsed.path.clone(),
            span,
            kind: DefinitionKind::TableField,
        })
    }

    pub(super) fn project_expected_table_field_definition(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<Definition> {
        let expected = self.expected_table_type(parsed, position)?;
        let (field, _) = table_field_at_position(&expected.table, position)?;
        let type_name = expected.builtin_type_name?;
        let definition = crate::lua::builtin_type_field_definition(&type_name, &field)?;

        Some(Definition {
            path: definition.path,
            span: definition.span,
            kind: DefinitionKind::Builtin,
        })
    }

    fn project_contextual_parameter_completions(
        &self,
        parsed: &ParsedSource,
        source: &str,
        position: SourcePosition,
    ) -> Vec<Completion> {
        let Some(crate::completion::context::CompletionContext::FieldAccess {
            receiver_parts,
            prefix,
            ..
        }) = crate::completion::context::completion_context(source, position)
        else {
            return Vec::new();
        };
        let Some((root, fields)) = receiver_parts.split_first() else {
            return Vec::new();
        };
        let Some((mut ty, origin_path)) =
            self.contextual_callback_parameter_type(parsed, root, position)
        else {
            return Vec::new();
        };

        for field in fields {
            let Some(next) = self.field_type_for_type(&origin_path, &ty, field) else {
                return Vec::new();
            };
            ty = next;
        }
        ty = self
            .type_database
            .resolve_type(&self.module_graph, &origin_path, &ty);
        let mut completions = BTreeMap::new();

        collect_type_fields(&ty, &mut completions);

        completions
            .into_iter()
            .filter(|(name, _)| text_matches(name, &prefix))
            .map(|(name, ty)| Completion {
                label: name,
                kind: completion_kind_for_type(&ty),
                detail: Some(format!("field: {ty}")),
                documentation: None,
                deprecation: None,
                insert_text: None,
                filter_text: None,
                is_snippet: false,
                module_path: None,
                source_module: None,
                additional_text_edits: Vec::new(),
            })
            .collect()
    }

    fn contextual_callback_parameter_type(
        &self,
        parsed: &ParsedSource,
        name: &str,
        position: SourcePosition,
    ) -> Option<(Type, PathBuf)> {
        if let Some((receiver_parts, event_name, parameter_index)) =
            crate::signal::handler_parameter_context(parsed, name, position)
        {
            let schema = self.usage_index.signal_event_index().schema_for_receiver(
                &parsed.path,
                &receiver_parts,
                &event_name,
                position,
            )?;
            let parameter = schema.payload.get(parameter_index)?;
            let ty = self.type_database.resolve_type(
                &self.module_graph,
                &schema.origin_path,
                &parameter.ty,
            );
            return Some((ty, schema.origin_path));
        }

        let expected = self.expected_table_type(parsed, position)?;
        let (field, function) = table_callback_field_at_position(&expected.table, position)?;
        let parameter_index =
            function
                .body()
                .parameters()
                .iter()
                .enumerate()
                .find_map(|(index, parameter)| match parameter {
                    ast::Parameter::Name(parameter) if token_text(parameter) == name => Some(index),
                    ast::Parameter::Name(_) | ast::Parameter::Ellipsis(_) => None,
                    _ => None,
                })?;
        let field_ty = self
            .state_machine_callback_type_at_position(parsed, position, &field)
            .or_else(|| self.expected_table_field_type(parsed, &expected, &field))?;
        let mut parameter_types = Vec::new();

        collect_function_parameter_types(&field_ty, parameter_index, &mut parameter_types);

        let ty = match parameter_types.len() {
            0 => return None,
            1 => parameter_types.pop()?,
            _ => Type::union(parameter_types),
        };

        Some((
            self.resolve_expected_type(parsed, &expected.origin_path, &ty),
            expected.origin_path,
        ))
    }

    fn state_machine_callback_type_at_position(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
        field: &str,
    ) -> Option<Type> {
        let (environment, _) = build_type_environment_with_diagnostics(parsed);
        if environment
            .local(crate::state_machine::CONSTRUCTOR_NAME)
            .is_some()
            || environment
                .function(crate::state_machine::CONSTRUCTOR_NAME)
                .is_some()
        {
            return None;
        }
        let mut syntax = ExpectedTableSyntaxCollector::default();
        syntax.visit_ast(&parsed.ast);

        syntax.calls.into_iter().find_map(|call| {
            node_span(&call)
                .is_some_and(|span| span_contains_position(span, position))
                .then_some(())?;
            (field_chain_before_first_call(&call)?.as_slice()
                == [crate::state_machine::CONSTRUCTOR_NAME])
            .then_some(())?;

            crate::state_machine::derived_member_type(&call, field)
        })
    }

    fn expected_table_type(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<ExpectedTableType> {
        let syntax = expected_table_syntax(parsed, position)?;
        let target_span = node_span(&syntax.table)?;
        let mut calls = syntax
            .calls
            .iter()
            .filter(|call| {
                node_span(*call).is_some_and(|span| span_contains_span(span, target_span))
            })
            .collect::<Vec<_>>();

        calls.sort_by_key(|call| node_span(*call).map(table_span_size));

        for call in calls {
            let Some((argument_index, steps)) = call_argument_table_path(call, target_span) else {
                continue;
            };
            let Some((ty, origin_path)) =
                self.expected_call_argument_type(parsed, call, argument_index)
            else {
                continue;
            };
            let Some(resolved) =
                self.expected_type_after_table_steps(parsed, &origin_path, ty, &steps)
            else {
                continue;
            };
            let builtin_type_name = resolved.builtin_type_name.or_else(|| {
                let environment = self.semantic_environment(parsed);
                let semantic = SemanticModel::new(parsed, &environment);
                let parts = field_chain_before_first_call(call)?;
                let generic =
                    semantic.typed_function_for_call_parts(&parts, node_span(call)?.start)?;
                let is_method = call
                    .suffixes()
                    .any(|suffix| matches!(suffix, ast::Suffix::Call(ast::Call::MethodCall(_))));
                let ty = typed_function_parameter_type(&generic, argument_index, is_method)?;
                self.expected_type_after_table_steps(parsed, &origin_path, ty, &steps)?
                    .builtin_type_name
            });
            let ty = self.narrow_expected_table_type(&origin_path, resolved.ty, &syntax.table);

            return Some(ExpectedTableType {
                ty,
                origin_path,
                table: syntax.table,
                builtin_type_name,
            });
        }

        let mut locals = syntax
            .locals
            .iter()
            .filter(|local| {
                node_span(*local).is_some_and(|span| span_contains_span(span, target_span))
            })
            .collect::<Vec<_>>();

        locals.sort_by_key(|local| node_span(*local).map(table_span_size));

        for local in locals {
            let expressions = local.expressions().iter().collect::<Vec<_>>();

            for (index, name) in local.names().iter().enumerate() {
                let Some((expression, return_index)) =
                    assignment_expression_for_local(&expressions, index)
                else {
                    continue;
                };
                if return_index != 0 {
                    continue;
                }
                let Some(steps) = expression_path_to_table(expression, target_span) else {
                    continue;
                };
                let name = token_text(name);
                let Some(annotation) = parsed.local_annotations.iter().find(|annotation| {
                    annotation.name == name
                        && annotation.name_span.is_some_and(|span| {
                            node_span(local)
                                .is_some_and(|local_span| span_contains_span(local_span, span))
                        })
                }) else {
                    continue;
                };
                let Some(ty) = self.project_type_from_tua_type(parsed, &annotation.ty) else {
                    continue;
                };
                let origin_path = parsed.path.clone();
                let Some(resolved) =
                    self.expected_type_after_table_steps(parsed, &origin_path, ty, &steps)
                else {
                    continue;
                };
                let ty = self.narrow_expected_table_type(&origin_path, resolved.ty, &syntax.table);

                return Some(ExpectedTableType {
                    ty,
                    origin_path,
                    table: syntax.table,
                    builtin_type_name: resolved.builtin_type_name,
                });
            }
        }

        None
    }

    fn expected_call_argument_type(
        &self,
        parsed: &ParsedSource,
        call: &FunctionCall,
        argument_index: usize,
    ) -> Option<(Type, PathBuf)> {
        let parts = field_chain_before_first_call(call)?;
        let (root, fields) = parts.split_first()?;
        let is_method = call
            .suffixes()
            .any(|suffix| matches!(suffix, ast::Suffix::Call(ast::Call::MethodCall(_))));

        if !fields.is_empty() {
            if let Some(module_path) = self.indexed_required_module_path(parsed, root) {
                let function = if fields.len() == 1 {
                    self.type_database
                        .module(module_path)
                        .and_then(|module| module.function(&fields[0]))
                        .cloned()
                } else {
                    self.type_database.typed_function_for_fields(
                        &self.module_graph,
                        &parsed.path,
                        root,
                        fields,
                    )
                };

                if let Some(function) = function {
                    let ty = typed_function_parameter_type(&function, argument_index, is_method)?;

                    return Some((ty, module_path.to_path_buf()));
                }
            }
        }

        let environment = self.semantic_environment(parsed);
        let semantic = SemanticModel::new(parsed, &environment);
        let function = semantic.typed_function_for_call(call, node_span(call)?.start)?;
        let ty = typed_function_parameter_type(&function, argument_index, is_method)?;

        Some((ty, parsed.path.clone()))
    }

    fn expected_type_after_table_steps(
        &self,
        parsed: &ParsedSource,
        origin_path: &Path,
        mut ty: Type,
        steps: &[ExpectedTableStep],
    ) -> Option<ExpectedResolvedType> {
        let mut builtin_name = builtin_type_name(&ty);

        for step in steps {
            ty = match step {
                ExpectedTableStep::Field(field) => {
                    unresolved_expected_table_field(&ty, field)
                        .or_else(|| self.expected_field_for_type(parsed, origin_path, &ty, field))?
                        .ty
                }
                ExpectedTableStep::Element => {
                    self.expected_table_element_type(parsed, origin_path, &ty)?
                }
            };
            builtin_name = builtin_type_name(&ty);
        }

        let ty = self.resolve_expected_type(parsed, origin_path, &ty);
        Some(ExpectedResolvedType {
            ty,
            builtin_type_name: builtin_name,
        })
    }

    fn expected_table_element_type(
        &self,
        parsed: &ParsedSource,
        origin_path: &Path,
        ty: &Type,
    ) -> Option<Type> {
        match ty {
            Type::Array(item) => Some(item.as_ref().clone()),
            Type::Tuple(items) => Some(Type::union(items.clone())),
            Type::Dictionary { value, .. } => Some(value.as_ref().clone()),
            _ => match self.resolve_expected_type(parsed, origin_path, ty) {
                Type::Array(item) => Some(*item),
                Type::Tuple(items) => Some(Type::union(items)),
                Type::Dictionary { value, .. } => Some(*value),
                Type::Union(types) => {
                    let items = types
                        .iter()
                        .filter_map(|ty| self.expected_table_element_type(parsed, origin_path, ty))
                        .collect::<Vec<_>>();

                    (!items.is_empty()).then(|| Type::union(items))
                }
                Type::Any
                | Type::Alias(_)
                | Type::Function(_)
                | Type::Primitive(_)
                | Type::StringLiteral(_)
                | Type::Table(_) => None,
                Type::GenericParameter { .. }
                | Type::GenericAliasApplication { .. }
                | Type::GenericClassApplication { .. } => None,
            },
        }
    }

    fn expected_table_field_type(
        &self,
        parsed: &ParsedSource,
        expected: &ExpectedTableType,
        field: &str,
    ) -> Option<Type> {
        self.expected_field_for_type(parsed, &expected.origin_path, &expected.ty, field)
            .map(|field| self.resolve_expected_type(parsed, &expected.origin_path, &field.ty))
    }

    fn expected_field_for_type(
        &self,
        parsed: &ParsedSource,
        origin_path: &Path,
        ty: &Type,
        field: &str,
    ) -> Option<TableField> {
        match self.resolve_expected_type(parsed, origin_path, ty) {
            Type::Table(table) => table.semantic_field(field).cloned(),
            Type::Dictionary { key, value }
                if dictionary_key_accepts_field(key.as_ref(), field) =>
            {
                Some(TableField::required(*value))
            }
            Type::Union(types) => {
                let fields = types
                    .iter()
                    .filter_map(|ty| self.expected_field_for_type(parsed, origin_path, ty, field))
                    .collect::<Vec<_>>();
                let optional = fields.iter().any(|field| field.optional);
                let types = fields.into_iter().map(|field| field.ty).collect::<Vec<_>>();

                (!types.is_empty()).then(|| TableField {
                    ty: Type::union(types),
                    optional,
                })
            }
            Type::Alias(_)
            | Type::Any
            | Type::Function(_)
            | Type::Primitive(_)
            | Type::StringLiteral(_)
            | Type::Array(_)
            | Type::Tuple(_)
            | Type::Dictionary { .. }
            | Type::GenericParameter { .. }
            | Type::GenericAliasApplication { .. }
            | Type::GenericClassApplication { .. } => None,
        }
    }

    fn resolve_expected_type(&self, parsed: &ParsedSource, origin_path: &Path, ty: &Type) -> Type {
        let ty = self
            .type_database
            .resolve_type(&self.module_graph, origin_path, ty);

        if origin_path == parsed.path {
            let (environment, _) = build_type_environment_with_diagnostics(parsed);

            resolve_aliases(&environment, &ty, 0)
        } else {
            ty
        }
    }

    fn narrow_expected_table_type(
        &self,
        origin_path: &Path,
        mut ty: Type,
        table: &TableConstructor,
    ) -> Type {
        for (field, literal) in table_string_literal_fields(table) {
            let matching = self.table_types_matching_literal(origin_path, &ty, &field, &literal);

            if !matching.is_empty() {
                ty = Type::union(matching);
            }
        }

        ty
    }

    fn table_types_matching_literal(
        &self,
        origin_path: &Path,
        ty: &Type,
        field: &str,
        literal: &str,
    ) -> Vec<Type> {
        match self
            .type_database
            .resolve_type(&self.module_graph, origin_path, ty)
        {
            Type::Union(types) => types
                .iter()
                .flat_map(|ty| self.table_types_matching_literal(origin_path, ty, field, literal))
                .collect(),
            candidate @ Type::Table(_) => self
                .field_type_for_type(origin_path, &candidate, field)
                .filter(|field_ty| type_contains_string_literal(field_ty, literal))
                .map(|_| vec![candidate])
                .unwrap_or_default(),
            Type::Alias(_)
            | Type::Any
            | Type::Function(_)
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

    pub(super) fn project_module_field_completions(
        &self,
        parsed: &ParsedSource,
        source: &str,
        position: SourcePosition,
    ) -> Vec<Completion> {
        let _profile = profile::span("analysis.completion.module_exports");
        let Some(crate::completion::context::CompletionContext::FieldAccess {
            receiver_parts,
            prefix,
            ..
        }) = crate::completion::context::completion_context(source, position)
        else {
            return Vec::new();
        };
        let Some((receiver, fields)) = receiver_parts.split_first() else {
            return Vec::new();
        };
        let Some(module_path) = self.indexed_required_module_path(parsed, receiver) else {
            return Vec::new();
        };

        if fields.is_empty() {
            let Some(module) = self.module_graph.module(module_path) else {
                return Vec::new();
            };

            return module
                .exports
                .iter()
                .filter(|entry| text_matches(&entry.public_name, &prefix))
                .map(|entry| Completion {
                    label: entry.public_name.clone(),
                    kind: completion_kind_for_type(&entry.field.ty),
                    detail: Some(format!("field: {}", entry.field.ty)),
                    documentation: entry.field.documentation.clone(),
                    deprecation: entry.field.deprecation.clone(),
                    insert_text: None,
                    filter_text: None,
                    is_snippet: false,
                    module_path: Some(module_path.to_path_buf()),
                    source_module: None,
                    additional_text_edits: Vec::new(),
                })
                .collect();
        }

        let Some(entry) = self.module_graph.export(module_path, &fields[0]) else {
            return Vec::new();
        };
        let mut current = entry.field.ty.clone();

        for field in fields.iter().skip(1) {
            current = match current {
                Type::Table(table) => match table.semantic_field(field) {
                    Some(field) => field.ty.clone(),
                    None => return Vec::new(),
                },
                Type::Union(types) => {
                    let mut resolved = Vec::new();

                    for ty in types {
                        if let Type::Table(table) = ty {
                            if let Some(field) = table.semantic_field(field) {
                                resolved.push(field.ty.clone());
                            }
                        }
                    }

                    if resolved.is_empty() {
                        return Vec::new();
                    }

                    Type::union(resolved)
                }
                _ => return Vec::new(),
            };
        }
        let mut field_types = BTreeMap::new();

        collect_type_fields(&current, &mut field_types);

        field_types
            .into_iter()
            .filter(|(name, _)| text_matches(name, &prefix))
            .map(|(name, ty)| Completion {
                kind: completion_kind_for_type(&ty),
                detail: Some(format!("field: {ty}")),
                label: name,
                documentation: entry.field.documentation.clone(),
                deprecation: None,
                insert_text: None,
                filter_text: None,
                is_snippet: false,
                module_path: Some(module_path.to_path_buf()),
                source_module: None,
                additional_text_edits: Vec::new(),
            })
            .collect()
    }

    fn project_type_import_completions(
        &self,
        parsed: &ParsedSource,
        source: &str,
        position: SourcePosition,
    ) -> Vec<Completion> {
        if !is_type_annotation_position(parsed, position) {
            return Vec::new();
        }
        let Some(crate::completion::context::CompletionContext::FieldAccess {
            receiver_parts,
            prefix,
            ..
        }) = crate::completion::context::completion_context(source, position)
        else {
            return Vec::new();
        };
        let [namespace] = receiver_parts.as_slice() else {
            return Vec::new();
        };
        let Some(module_path) = self.indexed_type_import_path(parsed, namespace) else {
            return Vec::new();
        };
        let Some(module) = self.type_database.module(module_path) else {
            return Vec::new();
        };

        module
            .exported_alias_names()
            .filter(|name| text_matches(name, &prefix))
            .filter_map(|name| {
                let binding = self.type_database.exported_alias_binding(
                    &self.module_graph,
                    module_path,
                    name,
                )?;
                Some(Completion {
                    label: name.to_owned(),
                    kind: CompletionKind::TypeAlias,
                    detail: Some(format!("type: {}", binding.ty)),
                    documentation: None,
                    deprecation: None,
                    insert_text: None,
                    filter_text: None,
                    is_snippet: false,
                    module_path: Some(module_path.to_path_buf()),
                    source_module: None,
                    additional_text_edits: Vec::new(),
                })
            })
            .collect()
    }

    fn project_named_type_import_completions(
        &self,
        parsed: &ParsedSource,
        source: &str,
        position: SourcePosition,
    ) -> Vec<Completion> {
        let Some(crate::completion::context::CompletionContext::TypeAnnotation { prefix }) =
            crate::completion::context::completion_context(source, position)
        else {
            return Vec::new();
        };

        let mut completions = Vec::new();
        for import in parsed.type_imports.iter().filter(|import| !import.reexport) {
            let Some(module_path) = self.indexed_module_path_for_specifier(parsed, &import.module)
            else {
                continue;
            };
            for binding in &import.bindings {
                if !text_matches(&binding.local, &prefix) {
                    continue;
                }
                let Some(resolved) = self.type_database.exported_alias_binding(
                    &self.module_graph,
                    module_path,
                    &binding.imported,
                ) else {
                    continue;
                };
                completions.push(Completion {
                    label: binding.local.clone(),
                    kind: CompletionKind::TypeAlias,
                    detail: Some(format!("type: {}", resolved.ty)),
                    documentation: None,
                    deprecation: None,
                    insert_text: None,
                    filter_text: None,
                    is_snippet: false,
                    module_path: Some(module_path.to_path_buf()),
                    source_module: None,
                    additional_text_edits: Vec::new(),
                });
            }
        }
        completions
    }

    fn type_import_binding_diagnostics(&self, parsed: &ParsedSource) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for import in &parsed.type_imports {
            let Some(module_path) = self.indexed_module_path_for_specifier(parsed, &import.module)
            else {
                continue;
            };
            for binding in &import.bindings {
                if self
                    .type_database
                    .exported_alias_binding(&self.module_graph, module_path, &binding.imported)
                    .is_some()
                {
                    continue;
                }
                let cycle = self.type_database.reexport_cycle(
                    &self.module_graph,
                    module_path,
                    &binding.imported,
                );
                let message = if cycle {
                    format!("cyclic type re-export involving `{}`", binding.imported)
                } else {
                    format!(
                        "module `{}` does not export type `{}`",
                        import.module, binding.imported
                    )
                };
                diagnostics.push(
                    Diagnostic::error(
                        &parsed.path,
                        binding.imported_span.start.line,
                        binding.imported_span.start.column,
                        message,
                    )
                    .with_code(if cycle {
                        crate::DiagnosticCode::TYPE_IMPORT_CYCLE
                    } else {
                        crate::DiagnosticCode::TYPE_UNKNOWN_NAME
                    }),
                );
            }
        }
        diagnostics
    }

    pub(super) fn project_aware_diagnostics(
        &self,
        path: &Path,
        diagnostics: Vec<Diagnostic>,
    ) -> Vec<Diagnostic> {
        deduplicate_diagnostics(diagnostics)
            .into_iter()
            .filter(|diagnostic| {
                if diagnostic.code != Some(crate::DiagnosticCode::TYPE_UNKNOWN_NAME) {
                    return true;
                }

                let Some(name) = unknown_type_name(&diagnostic.message) else {
                    return true;
                };

                self.type_database
                    .resolve_type_name(&self.module_graph, path, name)
                    .is_none()
            })
            .collect()
    }

    pub(super) fn project_type_diagnostics(
        &self,
        path: &Path,
        file: &AnalysisFile,
    ) -> Vec<Diagnostic> {
        if !self.project_type_check_required(file) {
            return Vec::new();
        }

        let path = normalize_path(path);

        if let Some(diagnostics) = self
            .project_diagnostics_cache
            .lock()
            .expect("project diagnostics cache lock poisoned")
            .get(&path)
            .cloned()
        {
            return diagnostics;
        }

        let _profile = profile::span_for_path("analysis.check.project", &path);
        let diagnostics = file
            .parsed
            .as_ref()
            .ok()
            .map(|parsed| {
                let facts = self.semantic_facts_for_parsed(parsed);
                let mut diagnostics_config = self.config.diagnostics.clone();

                if facts.environment.has_busted_globals() {
                    for name in crate::busted::GLOBAL_NAMES {
                        if !diagnostics_config.globals.iter().any(|global| global == name) {
                            diagnostics_config.globals.push((*name).to_owned());
                        }
                    }
                }

                diagnostics_config.inline_ignores = false;

                let mut diagnostics =
                    check_parsed_source_with_environment_asset_roots_diagnostics_jit_config_and_asset_index(
                        parsed,
                        &facts.environment,
                        analysis_asset_roots(self.source_roots(), &self.asset_roots),
                        &diagnostics_config,
                        &self.config.jit,
                        self.asset_index.as_deref(),
                    )
                    .err()
                    .unwrap_or_default();

                diagnostics.extend(facts.environment.generic_alias_diagnostics(parsed));
                diagnostics.extend(self.generic_protocol_bound_diagnostics(parsed));

                diagnostics.retain(|diagnostic| {
                    diagnostic.code != Some(crate::DiagnosticCode::TYPE_UNKNOWN_NAME)
                });
                diagnostics.extend(self.type_import_binding_diagnostics(parsed));
                if self.config.ecs.enabled {
                    diagnostics.extend(crate::ecs::diagnostics(parsed, &facts.environment));
                }

                diagnostics
            })
            .unwrap_or_default();

        self.project_diagnostics_cache
            .lock()
            .expect("project diagnostics cache lock poisoned")
            .insert(path, diagnostics.clone());

        diagnostics
    }

    fn generic_protocol_bound_diagnostics(&self, parsed: &ParsedSource) -> Vec<Diagnostic> {
        let parameters = parsed
            .function_annotations
            .iter()
            .flat_map(|function| function.generic_parameters.iter())
            .chain(
                parsed
                    .class_declarations
                    .iter()
                    .flat_map(|class| class.generic_parameters.iter()),
            );
        parameters
            .filter_map(|parameter| {
                let bound = parameter.protocol_bound.as_ref()?;
                if self.type_database.is_protocol_type(
                    &self.module_graph,
                    &parameter.id.declaration.module,
                    &bound.name,
                ) {
                    return None;
                }
                let mut diagnostic = Diagnostic::error(
                    &parsed.path,
                    bound.span.start.line,
                    bound.span.start.column,
                    format!(
                        "unknown protocol bound `{}` on generic parameter `{}`",
                        bound.name, parameter.name
                    ),
                )
                .with_code(crate::DiagnosticCode::TYPE_UNKNOWN_NAME);
                diagnostic.end_line = bound.span.end.line;
                diagnostic.end_column = bound.span.end.column.max(bound.span.start.column + 1);
                Some(diagnostic)
            })
            .collect()
    }

    pub(super) fn project_type_check_required(&self, file: &AnalysisFile) -> bool {
        let has_type_imports = file
            .parsed
            .as_ref()
            .is_ok_and(|parsed| !parsed.type_imports.is_empty());
        let has_runtime_imports = file
            .hir()
            .is_some_and(|hir| !hir.require_bindings.is_empty());

        (has_type_imports || has_runtime_imports)
            && (has_type_imports
                || file
                    .semantics()
                    .is_none_or(|semantics| semantics.environment_diagnostics.is_empty()))
    }

    pub(super) fn project_diagnostic_enabled(&self, diagnostic: &Diagnostic) -> bool {
        match diagnostic.code {
            Some(crate::DiagnosticCode::LINT_MISSING_REQUIRE) => {
                self.config.diagnostics.missing_requires
            }
            Some(crate::DiagnosticCode::LINT_DUPLICATE_REQUIRE_CANDIDATE)
            | Some(crate::DiagnosticCode::LINT_REQUIRE_CYCLE) => {
                self.config.diagnostics.bad_requires
            }
            Some(crate::DiagnosticCode::LINT_CANONICAL_MODULE_SPELLING) => {
                self.config.diagnostics.rules
            }
            _ => true,
        }
    }
}

fn collect_tua_type_names(ty: &TuaType, names: &mut BTreeSet<String>) {
    match ty {
        TuaType::Named(name) => {
            names.insert(name.clone());
        }
        TuaType::Applied {
            name, arguments, ..
        } => {
            names.insert(name.clone());
            for argument in arguments {
                collect_tua_type_names(argument, names);
            }
        }
        TuaType::Parameter { .. } => {}
        TuaType::StringLiteral(_) => {}
        TuaType::Union(types) | TuaType::Tuple(types) => {
            for ty in types {
                collect_tua_type_names(ty, names);
            }
        }
        TuaType::Table(fields) => {
            for field in fields {
                collect_tua_type_names(&field.ty, names);
            }
        }
        TuaType::Array(item) => collect_tua_type_names(item, names),
        TuaType::Dictionary { key, value } => {
            collect_tua_type_names(key, names);
            collect_tua_type_names(value, names);
        }
        TuaType::Function {
            parameters,
            returns,
        } => {
            for parameter in parameters {
                collect_tua_type_names(&parameter.ty, names);
            }
            for ty in returns {
                collect_tua_type_names(ty, names);
            }
        }
    }
}

fn tua_type_uses_named_import(parsed: &ParsedSource, ty: &TuaType) -> bool {
    let mut names = BTreeSet::new();
    collect_tua_type_names(ty, &mut names);

    parsed
        .type_imports
        .iter()
        .filter(|import| !import.reexport)
        .flat_map(|import| &import.bindings)
        .any(|binding| names.contains(&binding.local))
}

fn builtin_type_name(ty: &Type) -> Option<String> {
    let Type::Alias(name) = ty else {
        return None;
    };

    crate::semantic_domains::BuiltinDomainRegistry::standard()
        .builtin_type_names()
        .contains(&name.as_str())
        .then(|| name.clone())
}

fn unresolved_expected_table_field(ty: &Type, field: &str) -> Option<TableField> {
    match ty {
        Type::Table(table) => table.semantic_field(field).cloned(),
        Type::Dictionary { key, value } if dictionary_key_accepts_field(key, field) => {
            Some(TableField::required(value.as_ref().clone()))
        }
        Type::Alias(type_name) => crate::semantic_domains::BuiltinDomainRegistry::standard()
            .builtin_aliases()
            .into_iter()
            .find_map(|(name, ty)| {
                if name != *type_name {
                    return None;
                }
                let Type::Table(table) = ty else {
                    return None;
                };

                table.semantic_field(field).cloned()
            }),
        Type::Any
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::StringLiteral(_)
        | Type::Union(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. }
        | Type::GenericParameter { .. }
        | Type::GenericAliasApplication { .. }
        | Type::GenericClassApplication { .. } => None,
    }
}

fn dictionary_key_accepts_field(ty: &Type, field: &str) -> bool {
    match ty {
        Type::Primitive(crate::PrimitiveType::String) => true,
        Type::StringLiteral(value) => value == field,
        Type::Union(types) => types
            .iter()
            .any(|ty| dictionary_key_accepts_field(ty, field)),
        Type::Any
        | Type::Alias(_)
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::Table(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. }
        | Type::GenericParameter { .. }
        | Type::GenericAliasApplication { .. }
        | Type::GenericClassApplication { .. } => false,
    }
}

fn project_call_argument_expressions(function_call: &FunctionCall) -> Option<Vec<&Expression>> {
    let call = function_call.suffixes().find_map(|suffix| match suffix {
        Suffix::Call(call) => Some(call),
        _ => None,
    })?;
    match call {
        Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }) => {
            Some(arguments.iter().collect())
        }
        Call::AnonymousCall(FunctionArgs::String(value)) => {
            // String-call sugar supplies its string as the only argument.
            // The owned AST node cannot be borrowed from this helper, so the
            // caller will retain the unspecialized return in this uncommon
            // syntax form.
            let _ = value;
            None
        }
        Call::AnonymousCall(FunctionArgs::TableConstructor(value)) => {
            let _ = value;
            None
        }
        Call::MethodCall(method) => match method.args() {
            FunctionArgs::Parentheses { arguments, .. } => Some(arguments.iter().collect()),
            FunctionArgs::String(_) | FunctionArgs::TableConstructor(_) => None,
            _ => Some(Vec::new()),
        },
        _ => None,
    }
}

fn function_call_has_method_receiver(function_call: &FunctionCall) -> bool {
    function_call
        .suffixes()
        .find_map(|suffix| match suffix {
            Suffix::Call(call) => Some(matches!(call, Call::MethodCall(_))),
            _ => None,
        })
        .unwrap_or(false)
}

fn project_literal_type(expression: &Expression) -> Option<Type> {
    match expression {
        Expression::Number(_) => Some(Type::number()),
        Expression::String(_) => Some(Type::string()),
        Expression::Symbol(symbol)
            if token_text(symbol) == "true" || token_text(symbol) == "false" =>
        {
            Some(Type::boolean())
        }
        Expression::Symbol(symbol) if token_text(symbol) == "nil" => Some(Type::nil()),
        _ => None,
    }
}

fn collect_generic_class_identities(
    ty: &Type,
    identities: &mut HashSet<crate::parser::GenericDeclarationId>,
) {
    fn collect(
        ty: &Type,
        identities: &mut HashSet<crate::parser::GenericDeclarationId>,
        depth: usize,
    ) {
        if depth >= 64 {
            return;
        }
        match ty {
            Type::GenericClassApplication {
                identity,
                arguments,
                ..
            } => {
                identities.insert(identity.clone());
                for argument in arguments {
                    collect(argument, identities, depth + 1);
                }
            }
            Type::GenericAliasApplication { arguments, .. } => {
                for argument in arguments {
                    collect(argument, identities, depth + 1);
                }
            }
            Type::Union(types) | Type::Tuple(types) => {
                for ty in types {
                    collect(ty, identities, depth + 1);
                }
            }
            Type::Function(function) => {
                for parameter in &function.parameters {
                    collect(&parameter.ty, identities, depth + 1);
                }
                if let Some(vararg) = function.vararg_type.as_deref() {
                    collect(vararg, identities, depth + 1);
                }
                for ty in function
                    .return_types_or_any()
                    .iter()
                    .chain(function.guarded_return_types.iter().flatten())
                {
                    collect(ty, identities, depth + 1);
                }
                for overload in &function.overloads {
                    collect(&Type::Function(overload.clone()), identities, depth + 1);
                }
            }
            Type::Table(table) => {
                for (_, field) in table.fields() {
                    collect(&field.ty, identities, depth + 1);
                }
            }
            Type::Array(item) => collect(item, identities, depth + 1),
            Type::Dictionary { key, value } => {
                collect(key, identities, depth + 1);
                collect(value, identities, depth + 1);
            }
            Type::Alias(_)
            | Type::GenericParameter { .. }
            | Type::Primitive(_)
            | Type::Any
            | Type::StringLiteral(_) => {}
        }
    }

    collect(ty, identities, 0);
}

#[cfg(test)]
mod declaration_overlay_tests {
    use super::*;
    use crate::Fixture;

    #[test]
    fn declaration_overlay_controls_callable_and_result_types_with_runtime_present() {
        let fixture = Fixture::parse(
            "-- file: src/main.tua\nlocal F = require(\"lib.feel\")\nlocal result = F.echo(<|signature|>\"hello\")\nprint(<|hover|>result)\n",
        );
        let root = std::env::temp_dir().join(format!(
            "tua-declaration-overlay-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("src/lib")).unwrap();
        std::fs::create_dir_all(root.join("src/vendor-types")).unwrap();
        std::fs::create_dir_all(root.join("build/lib")).unwrap();
        std::fs::write(
            root.join("build/lib/feel.lua"),
            "local M={}\nfunction M.echo(value) return value end\nreturn M\n",
        )
        .unwrap();
        std::fs::write(root.join("tua.toml"), "source=\"src\"\nout=\"build\"\n").unwrap();
        std::fs::write(
            root.join("src/lib/feel.lua"),
            "local M={}\nfunction M.echo(value) return value end\nreturn M\n",
        )
        .unwrap();
        std::fs::write(root.join("src/vendor-types/feel.lua"), "---@meta feel\nlocal M={}\n---@param value string echoed value\n---@return string\nfunction M.echo(value) end\nreturn M\n").unwrap();
        let main = root.join("src/main.tua");
        std::fs::write(&main, &fixture.files()[0].source).unwrap();
        let mut host = AnalysisHost::new();
        host.set_source_roots([root.join("src")]);
        host.set_output_roots([root.join("build")]);
        host.set_type_libraries([ProjectTypeLibrary::new(
            root.join("src/vendor-types"),
            "feel",
            "lib.feel",
        )]);
        host.set_file(&main, fixture.files()[0].source.clone());
        host.set_file_with_kind(root.join("src/vendor-types/feel.lua"), "---@meta feel\nlocal M={}\n---@param value string echoed value\n---@return string\nfunction M.echo(value) end\nreturn M\n", AnalysisSourceKind::Lua);
        host.set_file_with_kind(
            root.join("src/lib/feel.lua"),
            "local M={}\nfunction M.echo(value) return value end\nreturn M\n",
            AnalysisSourceKind::Lua,
        );
        host.set_file_with_kind(
            root.join("build/lib/feel.lua"),
            "local M={}\nfunction M.echo(value) return value end\nreturn M\n",
            AnalysisSourceKind::Lua,
        );
        assert!(
            host.current_file_hover(&main, fixture.position("hover"))
                .is_none(),
            "module-call results must use project-aware hover"
        );
        let snapshot = host.snapshot();
        let signature = snapshot
            .signature_help(&main, fixture.position("signature"))
            .unwrap();
        assert!(
            signature.signatures[0].label.contains("value: string"),
            "{signature:?}"
        );
        assert!(
            signature.signatures[0].label.ends_with(": string"),
            "{signature:?}"
        );
        assert_eq!(
            signature.signatures[0].parameters[0]
                .documentation
                .as_deref(),
            Some("echoed value")
        );
        let hover = snapshot.hover(&main, fixture.position("hover")).unwrap();
        assert!(hover.contents.contains("result: string"), "{hover:?}");
        for runtime in ["src/lib/feel.lua", "build/lib/feel.lua"] {
            std::fs::remove_file(root.join(runtime)).unwrap();
            host.remove_file(root.join(runtime));
            let snapshot = host.snapshot();
            let signature = snapshot
                .signature_help(&main, fixture.position("signature"))
                .unwrap();
            assert!(
                signature.signatures[0].label.contains("value: string"),
                "{signature:?}"
            );
            assert!(snapshot
                .hover(&main, fixture.position("hover"))
                .unwrap()
                .contents
                .contains("result: string"));
        }
        host.set_file(&main, "local F=require(\"lib.feel\")\nF.echo(42)\n");
        assert!(
            host.snapshot()
                .diagnostics(&main)
                .iter()
                .any(|diagnostic| diagnostic.message.contains("string")),
            "bad argument must use declaration contract"
        );
        let shadow = Fixture::parse(
            "-- file: src/main.tua\nlocal F = require(\"lib.feel\")\ndo\n local F = {echo = function(value: number): number return value end}\n local number = F.echo(<|signature|>42)\nend\n",
        );
        host.set_file(&main, shadow.files()[0].source.clone());
        let signature = host
            .snapshot()
            .signature_help(&main, shadow.position("signature"))
            .unwrap();
        assert!(
            signature.signatures[0].label.contains("value: number"),
            "shadowed module: {signature:?}"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

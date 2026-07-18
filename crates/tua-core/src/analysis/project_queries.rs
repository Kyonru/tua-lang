use super::*;

impl AnalysisSnapshot {
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
        let exports = self.module_exports(module_path)?;

        exported_field_for_fields(exports, fields)
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
            contents: format!("type {name} = {}", binding.ty),
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
            contents: format!("local {}: {ty}", current_part.text),
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
            contents: format!("local {}: {ty}", current_part.text),
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
        let asset_completions = self.project_asset_path_completions(source, position);

        if !asset_completions.is_empty() {
            return asset_completions;
        }

        let module_completions = self.project_module_field_completions(parsed, source, position);

        if !module_completions.is_empty() {
            return module_completions;
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

        let asset_completions = self.project_asset_path_completions(source, position);

        if !asset_completions.is_empty() {
            return asset_completions;
        }

        let module_completions = self.project_module_field_completions(parsed, source, position);

        if !module_completions.is_empty() {
            return module_completions;
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
                insert_text: Some(candidate.path),
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
        self.semantic_facts_for_parsed(parsed).environment.clone()
    }

    pub(super) fn project_type_environment_for_parsed(
        &self,
        parsed: &ParsedSource,
        base_environment: TypeEnvironment,
        project_inference: &ProjectLocalInference,
    ) -> TypeEnvironment {
        let _profile = profile::span("analysis.semantic.environment");
        let environment = base_environment
            .with_local_types(self.project_module_binding_types(parsed))
            .with_local_types(project_inference.local_types.clone())
            .with_local_types(self.project_local_annotation_types(parsed));

        if self.config.ecs.enabled {
            environment
                .with_alias_types(crate::ecs::builtin_aliases())
                .with_local_types(crate::ecs::builtin_locals())
        } else {
            environment
        }
    }

    pub(super) fn project_module_binding_types(
        &self,
        parsed: &ParsedSource,
    ) -> Vec<(String, Type)> {
        require_bindings_for_parsed_source(parsed)
            .into_iter()
            .filter_map(|binding| {
                let module_path = self.indexed_required_module_path(parsed, &binding.alias)?;
                let exports = self.module_exports(module_path)?;
                let ty = Type::table(exports.fields.iter().map(|(name, field)| {
                    (
                        name.clone(),
                        TableField::required(self.type_database.resolve_type(
                            &self.module_graph,
                            module_path,
                            &field.ty,
                        )),
                    )
                }));

                Some((binding.alias, ty))
            })
            .collect()
    }

    pub(super) fn project_local_annotation_types(
        &self,
        parsed: &ParsedSource,
    ) -> Vec<(String, Type)> {
        parsed
            .local_annotations
            .iter()
            .filter(|annotation| tua_type_uses_qualified_name(&annotation.ty))
            .filter_map(|annotation| {
                self.project_type_from_tua_type(parsed, &annotation.ty)
                    .map(|ty| (annotation.name.clone(), ty))
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
            let mut pending_types = Vec::new();
            let mut pending_field_origins = Vec::new();
            let mut pending_function_return_origins = Vec::new();

            for (index, local_name) in local.names().iter().enumerate() {
                let name = token_text(local_name);
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
                let parts = field_chain_before_first_call(function_call)?;

                self.project_type_for_function_call_parts(parsed, &parts, return_index, local_types)
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

    pub(super) fn project_type_for_function_call_parts(
        &self,
        parsed: &ParsedSource,
        parts: &[String],
        return_index: usize,
        local_types: &BTreeMap<String, Type>,
    ) -> Option<Type> {
        let (root, fields) = parts.split_first()?;
        let ty = if fields.is_empty() {
            local_types.get(root).cloned()
        } else {
            self.project_type_for_field_chain(parsed, parts)
        }?;

        match ty {
            Type::Function(function) => function.return_types_or_any().get(return_index).cloned(),
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

        self.type_database
            .module_export_type(&self.module_graph, &parsed.path, root, fields)
            .or_else(|| self.project_indexed_module_export_type(parsed, root, fields))
    }

    pub(super) fn project_indexed_module_export_type(
        &self,
        parsed: &ParsedSource,
        root: &str,
        fields: &[String],
    ) -> Option<Type> {
        let (first, rest) = fields.split_first()?;
        let module_path = self.indexed_required_module_path(parsed, root)?;
        let entry = self.module_graph.export(module_path, first)?;
        let mut current =
            self.type_database
                .resolve_type(&self.module_graph, module_path, &entry.field.ty);

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
            TuaType::Named(name) => self
                .type_database
                .resolve_type_name(&self.module_graph, &parsed.path, name)
                .map(|binding| binding.ty)
                .or_else(|| self.required_module_type(parsed, name))
                .or_else(|| Some(Type::named(name.trim()))),
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
            let binding = self.type_database.module(path)?.alias(member.trim())?;

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

        for candidate in self.indexed_module_candidates(&module) {
            if self.type_database.module(&candidate).is_some() {
                return self
                    .type_database
                    .module(&candidate)
                    .map(|module| module.path.as_path());
            }
        }

        None
    }

    pub(super) fn indexed_module_candidates(&self, module: &str) -> Vec<PathBuf> {
        let mut relative = PathBuf::new();

        for part in module.split('.').filter(|part| !part.is_empty()) {
            relative.push(part);
        }

        let mut candidates = Vec::new();
        let extensions = if self.config.extension == "lua" {
            vec!["lua"]
        } else {
            vec![self.config.extension.as_str(), "lua"]
        };

        for root in self.source_roots() {
            for extension in &extensions {
                candidates.push(normalize_path(
                    root.join(&relative).with_extension(extension),
                ));
                candidates.push(normalize_path(
                    root.join(&relative).join(format!("init.{extension}")),
                ));
            }
        }

        candidates
    }

    pub(super) fn project_table_literal_completions(
        &self,
        parsed: &ParsedSource,
        source: &str,
        position: SourcePosition,
    ) -> Vec<Completion> {
        let Some((ty, prefix)) = self.table_literal_annotation_type(parsed, source, position)
        else {
            return Vec::new();
        };
        let ty = self
            .type_database
            .resolve_type(&self.module_graph, &parsed.path, &ty);
        let mut fields = BTreeMap::new();

        collect_type_fields(&ty, &mut fields);

        fields
            .into_iter()
            .filter(|(name, _)| text_matches(name, &prefix))
            .map(|(name, ty)| Completion {
                label: name,
                kind: CompletionKind::Field,
                detail: Some(format!("field: {ty}")),
                documentation: None,
                insert_text: None,
                filter_text: None,
                is_snippet: false,
                module_path: None,
                source_module: None,
                additional_text_edits: Vec::new(),
            })
            .collect()
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
                insert_text: None,
                filter_text: None,
                is_snippet: false,
                module_path: Some(module_path.to_path_buf()),
                source_module: None,
                additional_text_edits: Vec::new(),
            })
            .collect()
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
        if file.hir().is_none_or(|hir| hir.require_bindings.is_empty())
            || file
                .semantics()
                .is_some_and(|semantics| !semantics.environment_diagnostics.is_empty())
        {
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

                diagnostics_config.inline_ignores = false;

                let mut diagnostics =
                    check_parsed_source_with_environment_asset_roots_diagnostics_jit_config_and_asset_index(
                        parsed,
                        &facts.environment,
                        analysis_asset_roots(self.source_roots(), &self.asset_roots),
                        &diagnostics_config,
                        &self.config.jit,
                        self.asset_index.as_ref(),
                        &self.config.format.options(),
                    )
                    .err()
                    .unwrap_or_default();

                diagnostics.retain(|diagnostic| {
                    diagnostic.code != Some(crate::DiagnosticCode::TYPE_UNKNOWN_NAME)
                });

                diagnostics
            })
            .unwrap_or_default();

        self.project_diagnostics_cache
            .lock()
            .expect("project diagnostics cache lock poisoned")
            .insert(path, diagnostics.clone());

        diagnostics
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
            _ => true,
        }
    }
}

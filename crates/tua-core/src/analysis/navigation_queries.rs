use super::*;

impl AnalysisSnapshot {
    pub fn definition(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
    ) -> Option<Definition> {
        let _profile = profile::span("analysis.query.definition");
        let path = path.as_ref();

        let file = self.file(path)?;
        let key = definition_request_key(path, &file.source, position);

        if let Some(definition) = self
            .definition_cache
            .lock()
            .expect("definition cache lock poisoned")
            .get(&key)
            .cloned()
        {
            return definition;
        }

        let definition = self
            .signal_event_definition_at_position(path, position)
            .or_else(|| self.asset_definition_at_position(path, position))
            .or_else(|| match &file.parsed {
                Ok(parsed) => self.definition_parsed_snapshot(parsed, file.hir(), position),
                Err(_) => {
                    let facts = self.degraded_semantic_facts(path, file)?;

                    self.definition_parsed_snapshot_with_facts(
                        &facts.parsed,
                        Some(&facts.hir),
                        position,
                        Some(&facts),
                    )
                }
            });

        let mut cache = self
            .definition_cache
            .lock()
            .expect("definition cache lock poisoned");

        if let Some(span) = definition_lookup_span(&file.source, position) {
            for column in span.start.column..=span.end.column {
                cache.insert(
                    definition_request_key(
                        path,
                        &file.source,
                        SourcePosition {
                            line: span.start.line,
                            column,
                        },
                    ),
                    definition.clone(),
                );
            }
        } else {
            cache.insert(key, definition.clone());
        }

        definition
    }

    fn signal_event_definition_at_position(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<Definition> {
        let event = self.usage_index.signal_event_at(path, position)?;
        let definition = self.usage_index.signal_event_definition(event)?;

        Some(Definition {
            path: definition.path.clone(),
            span: definition.mention.name_span,
            kind: DefinitionKind::SignalEvent,
        })
    }

    fn asset_definition_at_position(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<Definition> {
        let file = self.file(path)?;
        let metadata = asset::asset_at_position(&file.assets, position)?;
        let path = asset::resolved_asset_path(
            metadata,
            self.effective_asset_roots(),
            self.asset_index.as_ref(),
        )?;
        let start = SourcePosition { line: 1, column: 1 };

        Some(Definition {
            path,
            span: SourceSpan { start, end: start },
            kind: DefinitionKind::Asset,
        })
    }

    pub(super) fn definition_parsed_snapshot(
        &self,
        parsed: &ParsedSource,
        hir: Option<&HirModule>,
        position: SourcePosition,
    ) -> Option<Definition> {
        self.definition_parsed_snapshot_with_facts(parsed, hir, position, None)
    }

    pub(super) fn definition_parsed_snapshot_with_facts(
        &self,
        parsed: &ParsedSource,
        hir: Option<&HirModule>,
        position: SourcePosition,
        degraded_facts: Option<&DegradedSemanticFacts>,
    ) -> Option<Definition> {
        if let Some(definition) = self.project_store_watch_key_definition(parsed, position) {
            return Some(definition);
        }

        if let Some(definition) = self.project_expected_table_field_definition(parsed, position) {
            return Some(definition);
        }

        if let Some(definition) =
            self.project_derived_helper_definition_at_position(parsed, position)
        {
            return Some(definition);
        }

        if let Some(definition) =
            self.project_class_member_definition_at_position(parsed, position, degraded_facts)
        {
            return Some(definition);
        }

        if let Some(definition) = self.module_definition(parsed, position) {
            return Some(definition);
        }

        if self.is_module_reference(parsed, position) {
            return None;
        }

        if let Some(definition) = self.project_imported_return_alias_field_definition(
            parsed,
            position,
            degraded_facts.map(|facts| &facts.project_inference),
        ) {
            return Some(definition);
        }

        if let Some(definition) =
            degraded_facts.and_then(|facts| facts.semantic_index.definition_at(position))
        {
            return Some(definition);
        }

        if degraded_facts.is_none() {
            if let Some(definition) = self
                .semantic_facts_for_parsed(parsed)
                .semantic_index
                .definition_at(position)
            {
                return Some(definition);
            }
        }

        definition_parsed_source_with_hir(parsed, hir, position)
            .ok()
            .flatten()
    }

    pub(super) fn project_class_member_definition_at_position(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
        degraded_facts: Option<&DegradedSemanticFacts>,
    ) -> Option<Definition> {
        let chain = dotted_chain_at_position(&parsed.source, position)?;

        if chain.cursor_index == 0 {
            return None;
        }

        let member = &chain.parts.get(chain.cursor_index)?.text;
        let receiver = chain
            .parts
            .iter()
            .take(chain.cursor_index)
            .map(|part| part.text.clone())
            .collect::<Vec<_>>();
        let environment = self.semantic_query_environment(parsed, degraded_facts);
        let receiver_type = self.type_for_parts(parsed, &environment, &receiver, position)?;

        self.class_member_definition_for_type(&parsed.path, &receiver_type, member, 0)
    }

    pub(super) fn project_derived_helper_definition_at_position(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<Definition> {
        let chain = dotted_chain_at_position(&parsed.source, position)?;
        let current = chain.parts.get(chain.cursor_index)?;

        if chain.cursor_index == 0 {
            let mut syntax = ExpectedTableSyntaxCollector::default();
            syntax.visit_ast(&parsed.ast);

            if let Some(span) = syntax.calls.into_iter().find_map(|call| {
                let call_span = node_span(&call)?;
                if !span_contains_position(call_span, position)
                    || field_chain_before_first_call(&call)?.as_slice()
                        != [crate::state_machine::CONSTRUCTOR_NAME]
                {
                    return None;
                }

                crate::state_machine::derived_member_definition_span(&call, &current.text)
            }) {
                return Some(Definition {
                    path: parsed.path.clone(),
                    span,
                    kind: DefinitionKind::TableField,
                });
            }
        }

        if chain.cursor_index == 2
            && !chain.parts.is_empty()
            && chain
                .parts
                .get(1)
                .is_some_and(|part| part.text == "animations")
        {
            let receiver = &chain.parts[0].text;
            let (environment, _) = build_type_environment_with_diagnostics(parsed);
            let initializer_span = SemanticModel::new(parsed, &environment)
                .local_initializer_inference(receiver, position)?
                .expression_span?;
            if let Some(animation) = parsed
                .sprite_declarations
                .iter()
                .find(|declaration| {
                    declaration.name == *receiver
                        && span_contains_span(declaration.span, initializer_span)
                })
                .and_then(|declaration| {
                    declaration
                        .animations
                        .iter()
                        .find(|animation| animation.name == current.text)
                })
            {
                return Some(Definition {
                    path: parsed.path.clone(),
                    span: animation.name_span,
                    kind: DefinitionKind::TableField,
                });
            }
        }

        if chain.cursor_index == 1 {
            let receiver = &chain.parts[0].text;
            let (environment, _) = build_type_environment_with_diagnostics(parsed);
            let call = helper_constructor_call_for_local(
                parsed,
                &environment,
                receiver,
                position,
                crate::state_machine::CONSTRUCTOR_NAME,
            )?;
            let span = crate::state_machine::derived_member_definition_span(&call, &current.text)?;

            return Some(Definition {
                path: parsed.path.clone(),
                span,
                kind: DefinitionKind::TableField,
            });
        }

        None
    }

    pub(super) fn class_member_definition_for_type(
        &self,
        source_path: &Path,
        ty: &Type,
        member: &str,
        depth: usize,
    ) -> Option<Definition> {
        if depth >= 64 {
            return None;
        }

        match self
            .type_database
            .resolve_type(&self.module_graph, source_path, ty)
        {
            Type::Table(table) => {
                let identity = table.nominal_identity()?;

                for file in self.files.values() {
                    let Ok(parsed) = &file.parsed else {
                        continue;
                    };
                    let Some(class) = parsed.class_declarations.iter().find(|class| {
                        nominal_class_identity(&parsed.path, &class.name) == identity
                    }) else {
                        continue;
                    };

                    return class_member_definition_in_parsed(
                        parsed,
                        &class.name,
                        member,
                        depth + 1,
                    );
                }

                None
            }
            Type::Union(types) => {
                let mut definitions = types
                    .iter()
                    .filter_map(|ty| {
                        self.class_member_definition_for_type(source_path, ty, member, depth + 1)
                    })
                    .collect::<Vec<_>>();

                definitions.sort_by_key(|definition| {
                    (
                        definition.path.clone(),
                        position_key(definition.span.start),
                        position_key(definition.span.end),
                        definition_kind_key(definition.kind),
                    )
                });
                definitions.dedup();

                (definitions.len() == 1).then(|| definitions.remove(0))
            }
            Type::Alias(_)
            | Type::Primitive(_)
            | Type::Any
            | Type::StringLiteral(_)
            | Type::Function(_)
            | Type::Array(_)
            | Type::Tuple(_)
            | Type::Dictionary { .. } => None,
        }
    }

    pub fn type_definition(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
    ) -> Option<Definition> {
        let _profile = profile::span("analysis.query.type_definition");
        let path = path.as_ref();
        let file = self.file(path)?;

        match &file.parsed {
            Ok(parsed) => self.type_definition_parsed_snapshot(parsed, file.hir(), position, None),
            Err(_) => {
                let facts = self.degraded_semantic_facts(path, file)?;

                self.type_definition_parsed_snapshot(
                    &facts.parsed,
                    Some(&facts.hir),
                    position,
                    Some(&facts),
                )
            }
        }
    }

    pub(super) fn type_definition_parsed_snapshot(
        &self,
        parsed: &ParsedSource,
        hir: Option<&HirModule>,
        position: SourcePosition,
        degraded_facts: Option<&DegradedSemanticFacts>,
    ) -> Option<Definition> {
        if is_type_reference_position(parsed, position) {
            if let Some(definition) = self.type_alias_definition_at_position(parsed, hir, position)
            {
                return Some(definition);
            }
        }

        let definition =
            self.definition_parsed_snapshot_with_facts(parsed, hir, position, degraded_facts);

        if definition.as_ref().is_some_and(is_type_definition_kind) {
            return definition;
        }

        let environment = self.semantic_query_environment(parsed, degraded_facts);
        let ty = self.type_at_position(parsed, &environment, position)?;

        self.type_origin_definition(parsed, hir, &ty)
    }

    pub fn implementation(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
    ) -> Vec<Definition> {
        let _profile = profile::span("analysis.query.implementation");
        let path = path.as_ref();
        let Some(file) = self.file(path) else {
            return Vec::new();
        };

        let definition = match &file.parsed {
            Ok(parsed) => self.implementation_parsed_snapshot(parsed, file.hir(), position, None),
            Err(_) => {
                let Some(facts) = self.degraded_semantic_facts(path, file) else {
                    return Vec::new();
                };

                self.implementation_parsed_snapshot(
                    &facts.parsed,
                    Some(&facts.hir),
                    position,
                    Some(&facts),
                )
            }
        };

        definition.into_iter().collect()
    }

    pub(super) fn implementation_parsed_snapshot(
        &self,
        parsed: &ParsedSource,
        hir: Option<&HirModule>,
        position: SourcePosition,
        degraded_facts: Option<&DegradedSemanticFacts>,
    ) -> Option<Definition> {
        let environment = self.semantic_query_environment(parsed, degraded_facts);

        if let Some(definition) =
            self.typed_receiver_method_implementation(parsed, &environment, position)
        {
            return Some(definition);
        }

        if let Some(definition) = self.module_export_definition_at_position(parsed, position) {
            return (definition.kind == DefinitionKind::Function).then_some(definition);
        }

        let chain = dotted_chain_at_position(&parsed.source, position)?;

        if chain.cursor_index > 0 {
            return None;
        }

        let definition =
            self.definition_parsed_snapshot_with_facts(parsed, hir, position, degraded_facts)?;

        if definition.kind != DefinitionKind::Function {
            return None;
        }

        if let Some(name) = global_function_name_for_definition(parsed, &definition) {
            return (global_function_definition_count(parsed, &name) == 1).then_some(definition);
        }

        Some(definition)
    }

    pub(super) fn type_alias_definition_at_position(
        &self,
        parsed: &ParsedSource,
        hir: Option<&HirModule>,
        position: SourcePosition,
    ) -> Option<Definition> {
        let chain = dotted_chain_at_position(&parsed.source, position)?;
        let name = chain
            .parts
            .iter()
            .take(chain.cursor_index + 1)
            .map(|part| part.text.as_str())
            .collect::<Vec<_>>()
            .join(".");

        self.type_alias_definition_for_name(parsed, hir, &name)
    }

    pub(super) fn type_origin_definition(
        &self,
        parsed: &ParsedSource,
        hir: Option<&HirModule>,
        ty: &Type,
    ) -> Option<Definition> {
        match ty {
            Type::Alias(name) => self.type_alias_definition_for_name(parsed, hir, name),
            Type::Union(types) => {
                let mut definitions = types
                    .iter()
                    .filter(|ty| !matches!(ty, Type::Primitive(crate::PrimitiveType::Nil)))
                    .filter_map(|ty| self.type_origin_definition(parsed, hir, ty))
                    .collect::<Vec<_>>();

                (definitions.len() == 1).then(|| definitions.remove(0))
            }
            Type::Primitive(_)
            | Type::Any
            | Type::StringLiteral(_)
            | Type::Function(_)
            | Type::Table(_)
            | Type::Array(_)
            | Type::Tuple(_)
            | Type::Dictionary { .. } => None,
        }
    }

    pub(super) fn type_alias_definition_for_name(
        &self,
        parsed: &ParsedSource,
        hir: Option<&HirModule>,
        name: &str,
    ) -> Option<Definition> {
        if !name.contains('.') {
            if let Some(definition) = type_alias_definition_in_parsed(parsed, hir, name) {
                return Some(definition);
            }
        }

        let binding =
            self.type_database
                .resolve_type_name(&self.module_graph, &parsed.path, name)?;
        let file = self.file(&binding.module_path)?;
        let parsed = file.parsed.as_ref().ok()?;

        type_alias_definition_in_parsed(parsed, file.hir(), &binding.name).or_else(|| {
            self.module_graph
                .export(&binding.module_path, &binding.name)
                .map(|entry| Definition {
                    path: binding.module_path.clone(),
                    span: entry.definition_span,
                    kind: DefinitionKind::TableField,
                })
        })
    }

    pub(super) fn typed_receiver_method_implementation(
        &self,
        parsed: &ParsedSource,
        environment: &TypeEnvironment,
        position: SourcePosition,
    ) -> Option<Definition> {
        let chain = dotted_chain_at_position(&parsed.source, position)?;

        if chain.cursor_index == 0 {
            return None;
        }

        let method = chain.parts.get(chain.cursor_index)?.text.as_str();
        let receiver_parts = chain
            .parts
            .iter()
            .take(chain.cursor_index)
            .map(|part| part.text.clone())
            .collect::<Vec<_>>();
        let receiver_type = self.type_for_parts(parsed, environment, &receiver_parts, position)?;
        let alias = receiver_alias_name(&receiver_type)?;

        self.method_implementation_for_alias(parsed, &alias, method)
    }

    pub(super) fn method_implementation_for_alias(
        &self,
        parsed: &ParsedSource,
        alias: &str,
        method: &str,
    ) -> Option<Definition> {
        let (path, owner) = self
            .type_database
            .resolve_type_name(&self.module_graph, &parsed.path, alias)
            .map(|binding| (binding.module_path, binding.name))
            .unwrap_or_else(|| (parsed.path.clone(), alias.to_owned()));
        let file = self.file(&path)?;
        let parsed = file.parsed.as_ref().ok()?;

        method_implementation_in_parsed(parsed, &owner, method)
    }

    pub fn file_rename_edits(
        &self,
        old_path: impl AsRef<Path>,
        new_path: impl AsRef<Path>,
    ) -> Vec<TextEdit> {
        let old_path = normalize_path(old_path.as_ref());
        let new_path = normalize_path(new_path.as_ref());
        let Some(plan) = self.module_file_rename_plan(&old_path, &new_path) else {
            return Vec::new();
        };
        let mut edits = Vec::new();

        for (path, file) in &self.files {
            if !path.starts_with(&plan.source_root) || self.is_generated_path(path) {
                continue;
            }

            for literal in module_specifier_literals(&file.source) {
                let candidates =
                    self.module_specifier_candidates(path, &literal.module, literal.kind);

                if candidates.len() != 1 || candidates.first() != Some(&plan.old_path) {
                    continue;
                }

                let Some(new_text) =
                    self.renamed_module_specifier(path, &literal.module, literal.kind, &plan)
                else {
                    continue;
                };

                if new_text == literal.module {
                    continue;
                }

                edits.push(TextEdit {
                    path: path.clone(),
                    span: literal.span,
                    new_text,
                });
            }
        }

        edits.sort_by_key(text_edit_key);
        edits.dedup_by_key(|edit| text_edit_key(edit));
        edits
    }

    pub(super) fn module_file_rename_plan(
        &self,
        old_path: &Path,
        new_path: &Path,
    ) -> Option<ModuleFileRenamePlan> {
        if old_path == new_path
            || self.vfs.file(old_path).is_none()
            || self.vfs.is_excluded(new_path)
            || self.is_generated_path(old_path)
            || self.is_generated_path(new_path)
        {
            return None;
        }

        let old_root = self.source_root_for_path(old_path)?;
        let new_root = self.source_root_for_path(new_path)?;

        if old_root != new_root {
            return None;
        }

        let old_module_name = self.module_name_for_known_path(old_path)?;
        let new_module_name = self.module_name_for_known_path(new_path)?;
        let old_candidates = self.known_module_paths_for_name(&old_module_name, &old_root);
        let new_conflicts = self
            .known_module_paths_for_name(&new_module_name, &old_root)
            .into_iter()
            .filter(|path| path != old_path && path != new_path)
            .collect::<Vec<_>>();

        if old_candidates != vec![old_path.to_path_buf()] || !new_conflicts.is_empty() {
            return None;
        }

        Some(ModuleFileRenamePlan {
            old_path: old_path.to_path_buf(),
            new_path: new_path.to_path_buf(),
            source_root: old_root,
            new_module_name,
        })
    }

    pub(super) fn source_root_for_path(&self, path: &Path) -> Option<PathBuf> {
        self.source_roots()
            .iter()
            .filter(|root| path.starts_with(root))
            .max_by_key(|root| root.components().count())
            .cloned()
    }

    pub(super) fn module_name_for_known_path(&self, path: &Path) -> Option<String> {
        module_name_for_path(
            self.source_root_for_path(path)?,
            path,
            self.require_path_separator,
        )
    }

    pub(super) fn known_module_paths_for_name(
        &self,
        name: &str,
        source_root: &Path,
    ) -> Vec<PathBuf> {
        self.files
            .keys()
            .filter(|path| path.starts_with(source_root))
            .filter(|path| self.module_name_for_known_path(path).as_deref() == Some(name))
            .cloned()
            .collect()
    }

    pub(super) fn is_generated_path(&self, path: &Path) -> bool {
        self.vfs
            .options()
            .output_roots
            .iter()
            .any(|root| path.starts_with(root))
    }

    pub(super) fn module_specifier_candidates(
        &self,
        current_path: &Path,
        specifier: &str,
        kind: ModuleSpecifierKind,
    ) -> Vec<PathBuf> {
        match kind {
            ModuleSpecifierKind::Require => self.project_module_candidates(current_path, specifier),
            ModuleSpecifierKind::Import => {
                if is_child_relative_import_specifier(specifier) {
                    self.child_import_module_candidates(current_path, specifier)
                } else if is_project_import_specifier(specifier) {
                    self.project_module_candidates(current_path, specifier)
                } else {
                    Vec::new()
                }
            }
        }
    }

    pub(super) fn child_import_module_candidates(
        &self,
        current_path: &Path,
        specifier: &str,
    ) -> Vec<PathBuf> {
        let Some(parent) = current_path.parent() else {
            return Vec::new();
        };
        let base = normalize_path(parent.join(specifier));

        if let Some(extension) = base.extension().and_then(|extension| extension.to_str()) {
            if extension != "lua" && extension != self.config.extension {
                return Vec::new();
            }
        }

        let extensions = module_extensions(&self.config.extension);
        let candidates = if base.extension().is_some() {
            vec![base]
        } else {
            import_module_candidates(&base, &extensions)
        };

        candidates
            .into_iter()
            .map(normalize_path)
            .filter(|candidate| {
                self.vfs.file(candidate).is_some()
                    && self.module_name_for_known_path(candidate).is_some()
            })
            .collect()
    }

    pub(super) fn renamed_module_specifier(
        &self,
        current_path: &Path,
        old_specifier: &str,
        kind: ModuleSpecifierKind,
        plan: &ModuleFileRenamePlan,
    ) -> Option<String> {
        match kind {
            ModuleSpecifierKind::Require => Some(plan.new_module_name.clone()),
            ModuleSpecifierKind::Import => {
                if is_child_relative_import_specifier(old_specifier) {
                    self.import_specifier_for_new_path(current_path, &plan.new_path)
                } else {
                    Some(plan.new_module_name.clone())
                }
            }
        }
    }

    pub(super) fn import_specifier_for_new_path(
        &self,
        current_path: &Path,
        target_path: &Path,
    ) -> Option<String> {
        self.module_name_for_known_path(target_path)?;

        let current_dir = normalize_path(current_path.parent()?);
        let target_module_path = module_path_without_runtime_suffix(target_path);
        let mut parts = relative_path_parts(&current_dir, &target_module_path)?;

        if parts.is_empty() {
            return None;
        }

        if parts.first().is_some_and(|part| part == "..") {
            return self.module_name_for_known_path(target_path);
        }

        parts.insert(0, ".".to_owned());

        Some(parts.join("/"))
    }

    pub fn prepare_call_hierarchy(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
    ) -> Vec<CallHierarchySymbol> {
        let path = normalize_path(path.as_ref());
        let Some(file) = self.file(&path) else {
            return Vec::new();
        };
        let Ok(parsed) = &file.parsed else {
            return Vec::new();
        };

        if let Some(symbol) = call_hierarchy_prepare_parsed_source(parsed, position) {
            return vec![symbol];
        }

        self.call_hierarchy_calls
            .get(&path)
            .into_iter()
            .flatten()
            .find(|call| contains_position(call.span, position))
            .and_then(|call| self.resolve_call_hierarchy_target(&path, call))
            .map(|symbol| vec![symbol])
            .unwrap_or_default()
    }

    pub fn call_hierarchy_incoming(
        &self,
        path: impl AsRef<Path>,
        selection_range: SourceSpan,
    ) -> Vec<CallHierarchyIncomingCall> {
        let Some(target) = self.call_hierarchy_symbol_for_identity(path.as_ref(), selection_range)
        else {
            return Vec::new();
        };
        let mut calls = Vec::new();

        for (path, file_calls) in &self.call_hierarchy_calls {
            for call in file_calls {
                let Some(callee) = self.resolve_call_hierarchy_target(path, call) else {
                    continue;
                };

                if !same_call_hierarchy_symbol(&callee, &target) {
                    continue;
                }

                let Some(caller) = call
                    .caller
                    .as_ref()
                    .and_then(|definition| self.call_hierarchy_symbol_for_definition(definition))
                else {
                    continue;
                };

                push_incoming_call(&mut calls, caller, call.span);
            }
        }

        sort_incoming_calls(&mut calls);
        calls
    }

    pub fn call_hierarchy_outgoing(
        &self,
        path: impl AsRef<Path>,
        selection_range: SourceSpan,
    ) -> Vec<CallHierarchyOutgoingCall> {
        let Some(caller) = self.call_hierarchy_symbol_for_identity(path.as_ref(), selection_range)
        else {
            return Vec::new();
        };
        let mut calls = Vec::new();

        for call in self
            .call_hierarchy_calls
            .get(&caller.path)
            .into_iter()
            .flatten()
        {
            if !call
                .caller
                .as_ref()
                .is_some_and(|definition| definition_matches_call_symbol(definition, &caller))
            {
                continue;
            }

            let Some(callee) = self.resolve_call_hierarchy_target(&caller.path, call) else {
                continue;
            };

            push_outgoing_call(&mut calls, callee, call.span);
        }

        sort_outgoing_calls(&mut calls);
        calls
    }

    pub fn references(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
        include_declaration: bool,
    ) -> Vec<Reference> {
        let _profile = profile::span("analysis.query.references");
        let path = path.as_ref();

        if let Some(references) =
            self.signal_event_references_at_position(path, position, include_declaration)
        {
            return references;
        }

        if let Some(references) = self.asset_references_at_position(path, position) {
            return references;
        }

        if let Some(references) =
            self.scope_references_at_position(path, position, include_declaration)
        {
            return references;
        }

        let Some(target) = self.definition(path, position) else {
            return Vec::new();
        };

        if let Some(references) = self.parameter_scope_references(&target, include_declaration) {
            return references;
        }

        let _profile = profile::span("analysis.references.reverse_index.lookup");

        if let Some(references) = self
            .reverse_reference_index()
            .references(&target, include_declaration)
        {
            return references;
        }

        self.references_by_scan(&target, include_declaration)
    }

    fn signal_event_references_at_position(
        &self,
        path: &Path,
        position: SourcePosition,
        include_declaration: bool,
    ) -> Option<Vec<Reference>> {
        let event = self.usage_index.signal_event_at(path, position)?;
        let definition = self.usage_index.signal_event_definition(event);
        let mut references = self
            .usage_index
            .signal_event_index()
            .related_events(event)
            .into_iter()
            .filter(|candidate| {
                include_declaration
                    || definition.is_none_or(|definition| {
                        candidate.path != definition.path
                            || candidate.mention.name_span != definition.mention.name_span
                    })
            })
            .map(|event| Reference {
                path: event.path.clone(),
                span: event.mention.name_span,
                kind: ReferenceKind::SignalEvent,
            })
            .collect::<Vec<_>>();

        sort_and_dedup_references(&mut references);
        Some(references)
    }

    fn asset_references_at_position(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<Vec<Reference>> {
        let file = self.file(path)?;
        let metadata = asset::asset_at_position(&file.assets, position)?;
        let Some(target) = asset::resolved_asset_path(
            metadata,
            self.effective_asset_roots(),
            self.asset_index.as_ref(),
        ) else {
            return Some(Vec::new());
        };
        let mut references = Vec::new();

        for (file_path, file) in &self.files {
            for metadata in &file.assets {
                let Some(candidate) = asset::resolved_asset_path(
                    metadata,
                    self.effective_asset_roots(),
                    self.asset_index.as_ref(),
                ) else {
                    continue;
                };

                if candidate == target {
                    references.push(Reference {
                        path: file_path.clone(),
                        span: metadata.span,
                        kind: ReferenceKind::Asset,
                    });
                }
            }
        }

        sort_and_dedup_references(&mut references);
        Some(references)
    }

    pub(super) fn scope_references_at_position(
        &self,
        path: &Path,
        position: SourcePosition,
        include_declaration: bool,
    ) -> Option<Vec<Reference>> {
        let _profile = profile::span("analysis.references.scope_position_fast_path");
        let file = self.file(path)?;
        let parsed = file.parsed.as_ref().ok()?;
        let scope = file.scope()?;
        let symbol = scope.symbol_at_position(position)?;
        let declaration = scope.declaration(symbol)?;
        let kind = match declaration.kind {
            ScopeDeclarationKind::ModuleAlias { .. } => ReferenceKind::Module,
            ScopeDeclarationKind::Parameter => ReferenceKind::Parameter,
            ScopeDeclarationKind::Local
            | ScopeDeclarationKind::Function
            | ScopeDeclarationKind::TableField => return None,
        };
        let mut references = scope
            .references_to_symbol(symbol, include_declaration)
            .into_iter()
            .map(|reference| Reference {
                path: parsed.path.clone(),
                span: reference.span,
                kind,
            })
            .collect::<Vec<_>>();

        sort_and_dedup_references(&mut references);
        Some(references)
    }

    pub(super) fn parameter_scope_references(
        &self,
        target: &Definition,
        include_declaration: bool,
    ) -> Option<Vec<Reference>> {
        if target.kind != DefinitionKind::Parameter {
            return None;
        }

        let _profile = profile::span("analysis.references.parameter_scope_fast_path");
        let file = self.file(&target.path)?;
        let parsed = file.parsed.as_ref().ok()?;

        references_to_definition_parsed_source_with_artifacts(
            parsed,
            target,
            include_declaration,
            file.hir(),
            file.scope(),
            None,
        )
        .ok()
    }

    pub(super) fn references_by_scan(
        &self,
        target: &Definition,
        include_declaration: bool,
    ) -> Vec<Reference> {
        let _profile = profile::span("analysis.references.fallback_scan");
        let global_name = self
            .file(&target.path)
            .and_then(|file| file.parsed.as_ref().ok())
            .and_then(|parsed| global_function_name_for_definition(parsed, target));
        let alias_name = self
            .file(&target.path)
            .and_then(|file| file.parsed.as_ref().ok())
            .and_then(|parsed| global_type_alias_name_for_definition(parsed, target));
        let mut references = Vec::new();

        for file in self.files.values() {
            let Ok(parsed) = &file.parsed else {
                continue;
            };
            let semantic_facts = (target.kind == DefinitionKind::TableField)
                .then(|| self.semantic_facts_for_parsed(parsed));
            let Ok(mut file_references) = references_to_definition_parsed_source_with_artifacts(
                parsed,
                target,
                include_declaration,
                file.hir(),
                file.scope(),
                semantic_facts.as_ref().map(|facts| &facts.semantic_index),
            ) else {
                continue;
            };

            references.append(&mut file_references);

            if let Some(global_name) = global_name.as_deref() {
                let Ok(mut global_references) =
                    global_references_to_definition_parsed_source_with_hir(
                        parsed,
                        target,
                        global_name,
                        include_declaration,
                        file.hir(),
                    )
                else {
                    continue;
                };

                references.append(&mut global_references);
            }

            if let Some(alias_name) = alias_name.as_deref() {
                let Ok(mut alias_references) =
                    global_type_alias_references_to_definition_parsed_source_with_hir(
                        parsed,
                        target,
                        alias_name,
                        include_declaration,
                        file.hir(),
                    )
                else {
                    continue;
                };

                references.append(&mut alias_references);
            }
        }

        references.sort_by_key(reference_key);
        references.dedup_by_key(|reference| reference_key(reference));
        references
    }

    pub(super) fn build_reverse_reference_index(&self) -> ReverseReferenceIndex {
        let _profile = profile::span("analysis.references.index.build");
        let global_functions = self.global_function_definitions();
        let global_aliases = self.global_type_alias_definitions();
        let mut type_alias_names = self.symbol_index.names_for_kind(SymbolIndexKind::TypeAlias);
        type_alias_names.extend(self.symbol_index.names_for_kind(SymbolIndexKind::Class));
        let mut index = ReverseReferenceIndex::default();

        for file in self.files.values() {
            let Ok(parsed) = &file.parsed else {
                continue;
            };

            if let Some(scope) = file.scope() {
                self.index_scope_references(parsed, file.hir(), scope, &mut index);
                self.index_global_function_references(parsed, scope, &global_functions, &mut index);
            }

            if let Some(hir) = file.hir() {
                self.index_hir_declarations(parsed, hir, &mut index);
            }

            let module_export_candidates =
                self.module_export_candidates_for_file(parsed, file.hir());

            self.index_token_references(
                parsed,
                &global_aliases,
                &type_alias_names,
                &module_export_candidates,
                &mut index,
            );
        }

        index.finish();
        index
    }

    pub(super) fn reverse_reference_index(&self) -> &ReverseReferenceIndex {
        self.reverse_reference_index.get_or_init(|| {
            let _profile = profile::span("analysis.index.references");

            self.build_reverse_reference_index()
        })
    }

    pub(super) fn index_scope_references(
        &self,
        parsed: &ParsedSource,
        hir: Option<&HirModule>,
        scope: &ScopeMap,
        index: &mut ReverseReferenceIndex,
    ) {
        for declaration in scope.declarations() {
            let Some(definition_kind) = definition_kind_for_scope_declaration(&declaration.kind)
            else {
                continue;
            };
            let definition = Definition {
                path: parsed.path.clone(),
                span: declaration.name_span,
                kind: definition_kind,
            };

            for reference in scope.references_to_symbol(declaration.id, true) {
                index.insert(
                    &definition,
                    Reference {
                        path: parsed.path.clone(),
                        span: reference.span,
                        kind: reference_kind_for_definition(definition.kind),
                    },
                );
            }

            if let Some(hir) = hir {
                self.index_export_key_references(parsed, hir, &definition, index);
            }
        }
    }

    pub(super) fn index_export_key_references(
        &self,
        parsed: &ParsedSource,
        hir: &HirModule,
        definition: &Definition,
        index: &mut ReverseReferenceIndex,
    ) {
        let Some(symbol) = hir
            .symbols()
            .iter()
            .find(|symbol| symbol.name_span == definition.span)
        else {
            return;
        };

        for export in &hir.exports {
            if export.value != HirExportValue::Symbol(symbol.id) {
                continue;
            }

            index.insert(
                definition,
                Reference {
                    path: parsed.path.clone(),
                    span: export.name_span,
                    kind: reference_kind_for_definition(definition.kind),
                },
            );
        }
    }

    pub(super) fn index_global_function_references(
        &self,
        parsed: &ParsedSource,
        scope: &ScopeMap,
        global_functions: &BTreeMap<String, Vec<Definition>>,
        index: &mut ReverseReferenceIndex,
    ) {
        for reference in scope.references() {
            if !is_project_global_reference(&reference.resolution) {
                continue;
            }
            let Some(definitions) = global_functions.get(&reference.name) else {
                continue;
            };

            for definition in definitions {
                index.insert(
                    definition,
                    Reference {
                        path: parsed.path.clone(),
                        span: reference.span,
                        kind: ReferenceKind::Function,
                    },
                );
            }
        }
    }

    pub(super) fn index_token_references(
        &self,
        parsed: &ParsedSource,
        global_aliases: &BTreeMap<String, Vec<Definition>>,
        type_alias_names: &BTreeSet<String>,
        module_export_candidates: &ModuleExportCandidates,
        index: &mut ReverseReferenceIndex,
    ) {
        for identifier in source_identifiers(&parsed.source) {
            let span = identifier.span;
            let name = identifier.text.clone();
            let is_type_reference = is_type_reference_position(parsed, span.start);

            let direct_definition = self.definition_parsed_snapshot(
                parsed,
                self.file(&parsed.path).and_then(|file| file.hir()),
                span.start,
            );

            if let Some(definition) = direct_definition.as_ref() {
                index.insert(
                    definition,
                    Reference {
                        path: parsed.path.clone(),
                        span,
                        kind: reference_kind_for_definition(definition.kind),
                    },
                );
            }

            if is_type_reference && !type_alias_names.contains(&name) {
                continue;
            }

            if !is_type_reference
                && !is_module_export_candidate(parsed, span.start, module_export_candidates)
            {
                continue;
            }

            if is_type_reference {
                if direct_definition.is_some() {
                    continue;
                }

                self.index_type_alias_token_reference(parsed, &identifier, global_aliases, index);
                continue;
            }

            if let Some(definition) = self.module_export_definition_at_position(parsed, span.start)
            {
                index.insert(
                    &definition,
                    Reference {
                        path: parsed.path.clone(),
                        span,
                        kind: reference_kind_for_definition(definition.kind),
                    },
                );
            }
        }
    }

    pub(super) fn index_type_alias_token_reference(
        &self,
        parsed: &ParsedSource,
        identifier: &SourceIdentifier,
        global_aliases: &BTreeMap<String, Vec<Definition>>,
        index: &mut ReverseReferenceIndex,
    ) {
        let name = &identifier.text;
        let Some(definitions) = global_aliases.get(name) else {
            return;
        };

        for definition in definitions {
            index.insert(
                definition,
                Reference {
                    path: parsed.path.clone(),
                    span: identifier.span,
                    kind: ReferenceKind::TypeAlias,
                },
            );
        }
    }

    pub(super) fn module_export_definition_at_position(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<Definition> {
        let chain = dotted_chain_at_position(&parsed.source, position)?;

        if chain.cursor_index != 1 {
            return None;
        }

        let root = chain.parts.first()?;
        let binding = self.require_binding(parsed, &root.text, position)?;
        let path = self.resolved_module_path(&parsed.path, &binding.module)?;
        let public_name = chain.parts.get(1)?.text.as_str();
        let entry = self.module_graph.export(path, public_name)?;

        Some(Definition {
            path: path.to_path_buf(),
            span: entry.definition_span,
            kind: self.module_export_definition_kind(path, entry),
        })
    }

    pub(super) fn global_function_definitions(&self) -> BTreeMap<String, Vec<Definition>> {
        let mut definitions: BTreeMap<String, Vec<Definition>> = BTreeMap::new();

        for file in self.files.values() {
            let Ok(parsed) = &file.parsed else {
                continue;
            };
            let Some(hir) = file.hir() else {
                continue;
            };

            for symbol in hir.symbols() {
                if symbol.kind != HirSymbolKind::Function {
                    continue;
                }
                let definition = definition_from_hir_symbol_for_index(&parsed.path, symbol);
                let Some(name) = global_function_name_for_definition(parsed, &definition) else {
                    continue;
                };

                definitions.entry(name).or_default().push(definition);
            }
        }

        definitions
    }

    pub(super) fn global_type_alias_definitions(&self) -> BTreeMap<String, Vec<Definition>> {
        let mut definitions: BTreeMap<String, Vec<Definition>> = BTreeMap::new();

        for file in self.files.values() {
            let Ok(parsed) = &file.parsed else {
                continue;
            };
            let Some(hir) = file.hir() else {
                continue;
            };

            for symbol in hir.symbols() {
                if symbol.kind != HirSymbolKind::TypeAlias {
                    continue;
                }
                let definition = definition_from_hir_symbol_for_index(&parsed.path, symbol);
                let Some(name) = global_type_alias_name_for_definition(parsed, &definition) else {
                    continue;
                };

                definitions.entry(name).or_default().push(definition);
            }
        }

        definitions
    }

    pub fn prepare_rename(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
    ) -> Option<RenameTarget> {
        let _profile = profile::span("analysis.query.prepare_rename");
        let path = path.as_ref();

        if let Some(event) = self.usage_index.signal_event_at(path, position) {
            return Some(RenameTarget {
                path: event.path.clone(),
                span: event.mention.name_span,
                placeholder: event.mention.name.clone(),
                kind: crate::RenameKind::SignalEvent,
            });
        }

        let target = self.definition(path, position)?;
        let target_parsed = self.file(&target.path)?.parsed.as_ref().ok()?;
        let mut rename_target = rename_target_for_definition(target_parsed, target)?;
        let reference_kind = reference_kind_for_rename(rename_target.kind);
        let current_reference =
            self.references(path, position, true)
                .into_iter()
                .find(|reference| {
                    reference.path == path
                        && reference.kind == reference_kind
                        && contains_position(reference.span, position)
                })?;

        rename_target.path = current_reference.path;
        rename_target.span = current_reference.span;

        Some(rename_target)
    }

    pub fn rename(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
        new_name: &str,
    ) -> Option<RenameEdit> {
        let _profile = profile::span("analysis.query.rename");
        let path = path.as_ref();

        if let Some(event) = self.usage_index.signal_event_at(path, position) {
            if !crate::signal::is_valid_event_name(new_name) {
                return None;
            }

            let mut edits = self
                .usage_index
                .signal_event_index()
                .related_events(event)
                .into_iter()
                .map(|event| TextEdit {
                    path: event.path.clone(),
                    span: event.mention.name_span,
                    new_text: new_name.to_owned(),
                })
                .collect::<Vec<_>>();

            edits.sort_by_key(text_edit_key);
            edits.dedup_by_key(|edit| text_edit_key(edit));

            return (!edits.is_empty()).then_some(RenameEdit { edits });
        }

        if !is_valid_rename_identifier(new_name) {
            return None;
        }

        let target = self.definition(path, position)?;
        let target_parsed = self.file(&target.path)?.parsed.as_ref().ok()?;
        let rename_target = rename_target_for_definition(target_parsed, target.clone())?;

        if self.is_ambiguous_project_rename(&target, &rename_target) {
            return None;
        }

        let reference_kind = reference_kind_for_rename(rename_target.kind);
        let mut edits = self
            .references(path, position, true)
            .into_iter()
            .filter(|reference| reference.kind == reference_kind)
            .map(|reference| TextEdit {
                path: reference.path,
                span: reference.span,
                new_text: new_name.to_owned(),
            })
            .collect::<Vec<_>>();

        edits.sort_by_key(text_edit_key);
        edits.dedup_by_key(|edit| text_edit_key(edit));

        for edit in &edits {
            let source = self.source(&edit.path)?;
            if crate::rename::rename_name_conflicts(
                source,
                new_name,
                rename_target.kind == crate::RenameKind::TableField,
            ) {
                return None;
            }
        }
        (!edits.is_empty()).then_some(RenameEdit { edits })
    }

    pub(super) fn call_hierarchy_symbol_for_identity(
        &self,
        path: &Path,
        selection_range: SourceSpan,
    ) -> Option<CallHierarchySymbol> {
        let path = normalize_path(path);

        self.call_hierarchy_symbols
            .get(&path)?
            .iter()
            .find(|symbol| symbol.path == path && symbol.selection_range == selection_range)
            .cloned()
    }

    pub(super) fn call_hierarchy_symbol_for_definition(
        &self,
        definition: &Definition,
    ) -> Option<CallHierarchySymbol> {
        self.call_hierarchy_symbols
            .get(&normalize_path(&definition.path))?
            .iter()
            .find(|symbol| definition_matches_call_symbol(definition, symbol))
            .cloned()
    }

    pub(super) fn resolve_call_hierarchy_target(
        &self,
        current_path: &Path,
        call: &CallHierarchyCallSite,
    ) -> Option<CallHierarchySymbol> {
        match &call.target {
            CallHierarchyCallTarget::Name(name) => {
                self.resolve_local_call_symbol(current_path, name, call.span.start)
            }
            CallHierarchyCallTarget::Field(parts) => self
                .resolve_required_module_call_symbol(current_path, parts)
                .or_else(|| self.resolve_qualified_call_symbol(current_path, parts)),
            CallHierarchyCallTarget::Method { receiver, method } => {
                let receiver_type = self.receiver_type_name(current_path, receiver, call)?;

                self.resolve_method_call_symbol(current_path, &receiver_type, method)
            }
        }
    }

    pub(super) fn resolve_local_call_symbol(
        &self,
        path: &Path,
        name: &str,
        position: SourcePosition,
    ) -> Option<CallHierarchySymbol> {
        self.call_hierarchy_symbols
            .get(&normalize_path(path))?
            .iter()
            .filter(|symbol| call_symbol_targets_name(symbol, name))
            .filter(|symbol| position_key(symbol.selection_range.start) <= position_key(position))
            .max_by_key(|symbol| call_hierarchy_symbol_key(symbol))
            .cloned()
    }

    pub(super) fn resolve_qualified_call_symbol(
        &self,
        path: &Path,
        parts: &[String],
    ) -> Option<CallHierarchySymbol> {
        self.call_hierarchy_symbols
            .get(&normalize_path(path))?
            .iter()
            .find(|symbol| symbol.target == CallHierarchySymbolTarget::Qualified(parts.to_vec()))
            .cloned()
    }

    pub(super) fn resolve_required_module_call_symbol(
        &self,
        current_path: &Path,
        parts: &[String],
    ) -> Option<CallHierarchySymbol> {
        let (receiver, fields) = parts.split_first()?;
        let field = fields.first()?;
        let file = self.file(current_path)?;
        let binding = file
            .hir()?
            .require_bindings
            .iter()
            .find(|binding| binding.alias == *receiver)?;
        let resolution = self
            .module_graph
            .resolve_require(current_path, &binding.module)?;
        let (RequireResolution::Resolved { path }
        | RequireResolution::Duplicate { selected: path, .. }) = resolution
        else {
            return None;
        };

        self.exported_call_symbol(path, field)
    }

    pub(super) fn exported_call_symbol(
        &self,
        module_path: &Path,
        field: &str,
    ) -> Option<CallHierarchySymbol> {
        let path = normalize_path(module_path);
        let symbols = self.call_hierarchy_symbols.get(&path)?;

        if let Some(symbol) = symbols
            .iter()
            .find(|symbol| symbol.target == CallHierarchySymbolTarget::Export(field.to_owned()))
        {
            return Some(symbol.clone());
        }

        let export = self.module_graph.export(&path, field)?;

        symbols
            .iter()
            .find(|symbol| symbol.selection_range == export.definition_span)
            .or_else(|| {
                symbols
                    .iter()
                    .find(|symbol| call_symbol_targets_name(symbol, &export.public_name))
            })
            .cloned()
    }

    pub(super) fn receiver_type_name(
        &self,
        path: &Path,
        receiver: &str,
        call: &CallHierarchyCallSite,
    ) -> Option<String> {
        let parsed = self.file(path)?.parsed.as_ref().ok()?;
        let local = parsed
            .local_annotations
            .iter()
            .filter(|annotation| annotation.name == receiver)
            .filter(|annotation| {
                annotation
                    .name_span
                    .or(annotation.span)
                    .is_some_and(|span| position_key(span.start) <= position_key(call.span.start))
            })
            .max_by_key(|annotation| {
                annotation
                    .name_span
                    .or(annotation.span)
                    .map(|span| position_key(span.start))
                    .unwrap_or((0, 0))
            })
            .and_then(|annotation| type_name_from_tua_type(&annotation.ty));

        if local.is_some() {
            return local;
        }

        let caller = call.caller.as_ref()?;

        parsed
            .function_annotations
            .iter()
            .find(|function| function.name_span == Some(caller.span))
            .and_then(|function| {
                function
                    .parameters
                    .iter()
                    .find(|parameter| parameter.name == receiver)
                    .and_then(|parameter| type_name_from_tua_type(&parameter.ty))
            })
    }

    pub(super) fn resolve_method_call_symbol(
        &self,
        current_path: &Path,
        receiver_type: &str,
        method: &str,
    ) -> Option<CallHierarchySymbol> {
        let current_path = normalize_path(current_path);
        let mut matches = self
            .call_hierarchy_symbols
            .values()
            .flatten()
            .filter(|symbol| {
                symbol.target
                    == CallHierarchySymbolTarget::Method {
                        receiver: receiver_type.to_owned(),
                        method: method.to_owned(),
                    }
            })
            .cloned()
            .collect::<Vec<_>>();

        let same_file = matches
            .iter()
            .filter(|symbol| symbol.path == current_path)
            .cloned()
            .collect::<Vec<_>>();

        if same_file.len() == 1 {
            return same_file.into_iter().next();
        }

        (matches.len() == 1).then(|| matches.remove(0))
    }

    pub(super) fn module_definition(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<Definition> {
        if let Some(specifier) = module_specifier_at_position(&parsed.source, position) {
            let path = self.resolved_module_path(&parsed.path, &specifier.module)?;

            return Some(module_file_definition(
                path,
                self.module_definition_span(path),
            ));
        }

        let chain = dotted_chain_at_position(&parsed.source, position)?;
        let root = chain.parts.first()?;
        let binding = self.require_binding(parsed, &root.text, position)?;
        let path = self.resolved_module_path(&parsed.path, &binding.module)?;

        if chain.cursor_index == 0 {
            return Some(module_file_definition(
                path,
                self.module_definition_span(path),
            ));
        }
        if chain.cursor_index != 1 {
            return None;
        }

        let public_name = chain.parts.get(1)?.text.as_str();
        let entry = self.module_graph.export(path, public_name)?;

        Some(Definition {
            path: path.to_path_buf(),
            span: entry.definition_span,
            kind: self.module_export_definition_kind(path, entry),
        })
    }

    pub(super) fn module_export_definition_kind(
        &self,
        path: &Path,
        entry: &crate::ModuleExportEntry,
    ) -> DefinitionKind {
        let is_class = entry.symbol_id.is_some_and(|symbol_id| {
            self.file(path)
                .and_then(|file| file.hir())
                .and_then(|hir| hir.symbol(symbol_id))
                .is_some_and(|symbol| symbol.kind == HirSymbolKind::Class)
        });

        if is_class {
            DefinitionKind::TypeAlias
        } else if matches!(entry.field.ty, Type::Function(_)) {
            DefinitionKind::Function
        } else {
            DefinitionKind::TableField
        }
    }

    pub(super) fn module_definition_span(&self, path: &Path) -> SourceSpan {
        self.module_graph
            .module(path)
            .and_then(|module| {
                module
                    .exports
                    .iter()
                    .map(|entry| entry.definition_span)
                    .min_by_key(|span| position_key(span.start))
            })
            .unwrap_or_else(default_span)
    }

    pub(super) fn module_export_source_documentation(
        &self,
        path: &Path,
        fields: &[String],
    ) -> Option<String> {
        let public_name = fields.first()?;
        let entry = self.module_graph.export(path, public_name)?;
        let source = &self.file(path)?.source;

        leading_plain_documentation(source, entry.definition_span.start.line)
    }

    pub(super) fn is_module_reference(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> bool {
        if module_specifier_at_position(&parsed.source, position).is_some() {
            return true;
        }

        let Some(chain) = dotted_chain_at_position(&parsed.source, position) else {
            return false;
        };
        let Some(root) = chain.parts.first() else {
            return false;
        };

        self.require_binding(parsed, &root.text, position).is_some()
    }

    pub(super) fn require_binding(
        &self,
        parsed: &ParsedSource,
        name: &str,
        position: SourcePosition,
    ) -> Option<HirRequireBinding> {
        self.file(&parsed.path)?
            .hir()?
            .require_bindings
            .iter()
            .filter(|binding| {
                binding.alias == name
                    && position_key(binding.alias_span.start) <= position_key(position)
            })
            .max_by_key(|binding| position_key(binding.alias_span.start))
            .cloned()
    }

    pub(super) fn resolved_module_path(&self, source_path: &Path, module: &str) -> Option<&Path> {
        match self.module_graph.resolve_require(source_path, module)? {
            RequireResolution::Resolved { path }
            | RequireResolution::Duplicate { selected: path, .. } => Some(path.as_path()),
            RequireResolution::Missing { .. } | RequireResolution::Invalid { .. } => None,
        }
    }

    pub(super) fn is_ambiguous_project_rename(
        &self,
        target: &Definition,
        rename_target: &RenameTarget,
    ) -> bool {
        match rename_target.kind {
            crate::RenameKind::SignalEvent => false,
            crate::RenameKind::Function => {
                let Some(global_name) = self
                    .file(&target.path)
                    .and_then(|file| file.parsed.as_ref().ok())
                    .and_then(|parsed| global_function_name_for_definition(parsed, target))
                else {
                    return false;
                };

                self.files
                    .values()
                    .filter_map(|file| file.parsed.as_ref().ok())
                    .map(|parsed| global_function_definition_count(parsed, &global_name))
                    .sum::<usize>()
                    > 1
            }
            crate::RenameKind::TypeAlias => {
                if self
                    .file(&target.path)
                    .and_then(|file| file.hir())
                    .and_then(|hir| hir.symbol_at_span(target.span))
                    .is_some_and(|symbol| symbol.kind == HirSymbolKind::Class)
                {
                    return false;
                }

                let Some(alias_name) = self
                    .file(&target.path)
                    .and_then(|file| file.parsed.as_ref().ok())
                    .and_then(|parsed| global_type_alias_name_for_definition(parsed, target))
                else {
                    return false;
                };

                self.files
                    .values()
                    .filter_map(|file| file.parsed.as_ref().ok())
                    .map(|parsed| type_alias_definition_count(parsed, &alias_name))
                    .sum::<usize>()
                    > 1
            }
            crate::RenameKind::Local
            | crate::RenameKind::Parameter
            | crate::RenameKind::TableField => false,
        }
    }
}

fn class_member_definition_in_parsed(
    parsed: &ParsedSource,
    class_name: &str,
    member: &str,
    depth: usize,
) -> Option<Definition> {
    if depth >= 64 {
        return None;
    }

    let target = class_member_source_target(parsed, class_name, member)?;

    Some(Definition {
        path: parsed.path.clone(),
        span: target.span,
        kind: match target.kind {
            ClassMemberKind::Field => DefinitionKind::TableField,
            ClassMemberKind::Method => DefinitionKind::Function,
        },
    })
}

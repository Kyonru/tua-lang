use super::*;

impl AnalysisSnapshot {
    pub fn hover(&self, path: impl AsRef<Path>, position: SourcePosition) -> Option<Hover> {
        let _profile = profile::span("analysis.query.hover");
        let path = path.as_ref();
        if let Some(hover) = self.shader_uniform_hover(path, position) {
            return Some(hover);
        }
        let file = self.file(path)?;

        match &file.parsed {
            Ok(parsed) => {
                if let Some(hover) = self.signal_event_hover(path, position) {
                    return Some(hover);
                }

                if let Some(asset) = asset::asset_at_position(&file.assets, position) {
                    return Some(Hover {
                        contents: asset.hover_markdown(),
                        span: Some(asset.span),
                        documentation: None,
                        context: Vec::new(),
                    });
                }

                self.hover_parsed_snapshot(parsed, position)
            }
            Err(_) => {
                let facts = self.degraded_semantic_facts(path, file)?;

                self.hover_parsed_snapshot_with_environment(
                    &facts.parsed,
                    position,
                    Some(&facts.environment),
                )
            }
        }
    }

    fn signal_event_hover(&self, path: &Path, position: SourcePosition) -> Option<Hover> {
        signal_event_hover(
            self.usage_index.signal_event_index(),
            &normalize_path(path),
            position,
        )
    }

    pub(super) fn hover_parsed_snapshot(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<Hover> {
        self.hover_parsed_snapshot_with_environment(parsed, position, None)
    }

    pub(super) fn hover_parsed_snapshot_with_environment(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
        degraded_environment: Option<&TypeEnvironment>,
    ) -> Option<Hover> {
        if let Some(hover) = self.project_type_hover(parsed, position) {
            return Some(hover);
        }

        if let Some(hover) = self.module_hover(parsed, position) {
            return Some(hover);
        }

        if self.is_module_reference(parsed, position) {
            return None;
        }

        if let Some(hover) = self.project_annotated_local_hover(parsed, position) {
            return Some(hover);
        }

        if let Some(hover) = self.project_store_watch_key_hover(parsed, position) {
            return Some(hover);
        }

        if let Some(hover) = self.project_derived_helper_hover(parsed, position) {
            return Some(hover);
        }

        if let Some(hover) = self.project_expected_table_field_hover(parsed, position) {
            return Some(hover);
        }

        if let Some(hover) = self.project_contextual_parameter_hover(parsed, position) {
            return Some(hover);
        }

        if let Some(environment) = degraded_environment {
            return hover_parsed_source_with_environment(parsed, position, environment).or_else(
                || self.project_symbol_hover_with_environment(parsed, position, environment),
            );
        }

        let facts = self.semantic_facts_for_parsed(parsed);

        hover_parsed_source_with_environment(parsed, position, &facts.environment).or_else(|| {
            self.project_symbol_hover_with_environment(parsed, position, &facts.environment)
        })
    }

    pub fn completion(&self, path: impl AsRef<Path>, position: SourcePosition) -> Vec<Completion> {
        let _profile = profile::span("analysis.query.completion");
        let path = path.as_ref();
        let Some(file) = self.file(path) else {
            return Vec::new();
        };

        let revision = self.usage_index.revision();
        debug_assert_eq!(revision, self.revision);

        if let Some(request) = reusable_completion_request(revision, path, &file.source, position) {
            if let Some(completions) = self
                .completion_cache
                .lock()
                .expect("completion cache lock poisoned")
                .get(&request.key)
                .cloned()
            {
                self.record_completion_cache_hit();

                return filter_and_rank_completions(completions, &request.prefix);
            }

            self.record_completion_cache_miss();

            let completions = self.completion_uncached(
                path,
                file,
                Some(request.source.as_str()),
                request.position,
            );
            self.completion_cache
                .lock()
                .expect("completion cache lock poisoned")
                .insert(request.key, completions.clone());

            return filter_and_rank_completions(completions, &request.prefix);
        }

        let key = completion_request_key(revision, path, &file.source, position);

        if let Some(completions) = self
            .completion_cache
            .lock()
            .expect("completion cache lock poisoned")
            .get(&key)
            .cloned()
        {
            self.record_completion_cache_hit();

            return completions;
        }

        self.record_completion_cache_miss();

        let completions = self.completion_uncached(path, file, None, position);

        self.completion_cache
            .lock()
            .expect("completion cache lock poisoned")
            .insert(key, completions.clone());

        completions
    }

    pub(super) fn record_completion_cache_hit(&self) {
        self.completion_cache_stats
            .lock()
            .expect("completion cache stats lock poisoned")
            .hits += 1;
    }

    pub(super) fn record_completion_cache_miss(&self) {
        self.completion_cache_stats
            .lock()
            .expect("completion cache stats lock poisoned")
            .misses += 1;
    }

    pub(super) fn completion_uncached(
        &self,
        path: &Path,
        file: &AnalysisFile,
        source_override: Option<&str>,
        position: SourcePosition,
    ) -> Vec<Completion> {
        let source = source_override.unwrap_or(&file.source);

        if let Some(completions) =
            complete_indexed_module_paths(path, source, position, &self.module_path_index)
        {
            return completions;
        }

        let file_scope_hash = CompletionAdaptiveStats::file_scope_hash(path);
        let completions = match &file.parsed {
            Ok(parsed) => {
                let completions =
                    self.project_completions(parsed, source, position, file_scope_hash);

                if !completions.is_empty() {
                    completions
                } else {
                    let facts = self.semantic_facts_for_parsed(parsed);

                    complete_source_with_parsed_environment_and_ranking_inputs(
                        source,
                        parsed,
                        position,
                        &facts.environment,
                        self.completion_ranking_inputs(file_scope_hash),
                    )
                }
            }
            Err(_) => {
                if let Some(facts) = self.degraded_semantic_facts(path, file) {
                    let completions = self.project_completions_with_environment(
                        &facts.parsed,
                        source,
                        position,
                        &facts.environment,
                        file_scope_hash,
                    );

                    if !completions.is_empty() {
                        completions
                    } else {
                        let completions =
                            complete_source_with_parsed_environment_and_ranking_inputs(
                                source,
                                &facts.parsed,
                                position,
                                &facts.environment,
                                self.completion_ranking_inputs(file_scope_hash),
                            );

                        completions
                    }
                } else {
                    degraded_completion_fallback(path, source, position)
                }
            }
        };

        completions
    }

    pub(super) fn completion_ranking_inputs(
        &self,
        file_scope_hash: u64,
    ) -> CompletionRankingInputs<'_> {
        CompletionRankingInputs::from_usage_index(
            &self.usage_index,
            Some(&self.completion_adaptive_stats),
            file_scope_hash,
            &self.completion_sequence_ranking,
        )
    }

    pub fn multi_token_completion(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
        primary_candidate_count: usize,
    ) -> Vec<Completion> {
        let _profile = profile::span("analysis.query.completion.multi_token");
        let path = path.as_ref();
        let Some(file) = self.file(path) else {
            return Vec::new();
        };

        complete_multi_token_source(
            path,
            &file.source,
            position,
            primary_candidate_count,
            &self.completion_multi_token,
        )
    }

    pub(super) fn type_at_position(
        &self,
        parsed: &ParsedSource,
        environment: &TypeEnvironment,
        position: SourcePosition,
    ) -> Option<Type> {
        let chain = dotted_chain_at_position(&parsed.source, position)?;
        let parts = chain
            .parts
            .iter()
            .take(chain.cursor_index + 1)
            .map(|part| part.text.clone())
            .collect::<Vec<_>>();

        self.type_for_parts(parsed, environment, &parts, position)
    }

    pub(super) fn type_for_parts(
        &self,
        parsed: &ParsedSource,
        environment: &TypeEnvironment,
        parts: &[String],
        position: SourcePosition,
    ) -> Option<Type> {
        let (root, fields) = parts.split_first()?;
        let mut current = environment
            .local(root)
            .cloned()
            .or_else(|| parameter_type_before_position(parsed, root, position))?;

        for field in fields {
            current = self.field_type_for_type(&parsed.path, &current, field)?;
        }

        Some(current)
    }

    pub(super) fn field_type_for_type(
        &self,
        source_path: &Path,
        ty: &Type,
        field: &str,
    ) -> Option<Type> {
        match self
            .type_database
            .resolve_type(&self.module_graph, source_path, ty)
        {
            Type::Table(table) => table.semantic_field(field).map(|field| field.ty.clone()),
            Type::Union(types) => {
                let fields = types
                    .iter()
                    .map(|ty| self.field_type_for_type(source_path, ty, field))
                    .collect::<Option<Vec<_>>>()?;

                match fields.len() {
                    0 => None,
                    1 => fields.into_iter().next(),
                    _ => Some(Type::union(fields)),
                }
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

    pub fn resolve_completion(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
        label: &str,
        kind: Option<CompletionKind>,
        module_path: Option<&Path>,
        source_module: Option<&str>,
    ) -> Option<Completion> {
        let _profile = profile::span("analysis.query.completion.resolve");
        self.completion(path, position)
            .into_iter()
            .find(|completion| {
                completion.label == label
                    && kind.is_none_or(|kind| completion.kind == kind)
                    && module_path
                        .is_none_or(|path| completion.module_path.as_deref() == Some(path))
                    && source_module
                        .is_none_or(|module| completion.source_module.as_deref() == Some(module))
            })
    }

    pub fn signature_help(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
    ) -> Option<SignatureHelp> {
        let _profile = profile::span("analysis.query.signature_help");
        let path = path.as_ref();
        let file = self.file(path)?;

        match &file.parsed {
            Ok(parsed) => {
                let facts = self.semantic_facts_for_parsed(parsed);

                signature_help_parsed_source_with_environment_and_signal_index(
                    parsed,
                    position,
                    &facts.environment,
                    Some(self.usage_index.signal_event_index()),
                )
            }
            Err(_) => self
                .degraded_semantic_facts(path, file)
                .and_then(|facts| {
                    signature_help_parsed_source_with_environment_and_signal_index(
                        &facts.parsed,
                        position,
                        &facts.environment,
                        Some(self.usage_index.signal_event_index()),
                    )
                })
                .or_else(|| degraded_signature_help_fallback(path, &file.source, position)),
        }
    }

    pub fn document_links(&self, path: impl AsRef<Path>) -> Vec<ModuleLink> {
        self.file(path.as_ref())
            .map(|file| file.require_edges.clone())
            .unwrap_or_default()
    }

    pub fn document_highlights(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
    ) -> Vec<DocumentHighlight> {
        self.file(path.as_ref())
            .and_then(|file| file.parsed.as_ref().ok())
            .and_then(|parsed| document_highlights_parsed_source(parsed, position).ok())
            .unwrap_or_default()
    }

    pub fn document_symbols(&self, path: impl AsRef<Path>) -> Vec<DocumentSymbol> {
        self.file(path.as_ref())
            .and_then(|file| {
                Some(document_symbols_parsed_source_with_hir(
                    file.parsed.as_ref().ok()?,
                    file.hir()?,
                ))
            })
            .unwrap_or_default()
    }

    pub fn code_lenses(&self, path: impl AsRef<Path>) -> Vec<CodeLens> {
        let path = normalize_path(path.as_ref());
        let Some(file) = self.file(&path) else {
            return Vec::new();
        };
        let Some(hir) = file.hir() else {
            return Vec::new();
        };
        if file.parsed.is_err() {
            return Vec::new();
        }

        let mut lenses = Vec::new();

        lenses.extend(self.exported_function_code_lenses(&path, hir));
        lenses.extend(self.type_alias_code_lenses(&path));
        lenses.sort_by_key(code_lens_key);
        lenses
    }

    pub(super) fn exported_function_code_lenses(
        &self,
        path: &Path,
        hir: &HirModule,
    ) -> Vec<CodeLens> {
        let mut exported_symbols = BTreeSet::new();

        for export in &hir.exports {
            let HirExportValue::Symbol(symbol_id) = export.value else {
                continue;
            };
            exported_symbols.insert(symbol_id);
        }

        hir.symbols()
            .iter()
            .filter(|symbol| symbol.kind == HirSymbolKind::Function)
            .filter(|symbol| exported_symbols.contains(&symbol.id))
            .map(|symbol| {
                reference_code_lens(
                    symbol.name_span,
                    path,
                    symbol.name_span.start,
                    symbol.name.clone(),
                    CodeLensReferenceKind::ExportedFunction,
                )
            })
            .collect()
    }

    pub(super) fn type_alias_code_lenses(&self, path: &Path) -> Vec<CodeLens> {
        let alias_counts = self.type_alias_name_counts();

        self.symbol_index
            .entries_for_path(path)
            .into_iter()
            .filter(|entry| {
                matches!(
                    entry.kind,
                    SymbolIndexKind::Class | SymbolIndexKind::TypeAlias
                )
            })
            .filter(|entry| alias_counts.get(&entry.name).copied().unwrap_or_default() == 1)
            .map(|entry| {
                reference_code_lens(
                    entry.selection_range,
                    &entry.path,
                    entry.selection_range.start,
                    entry.name.clone(),
                    CodeLensReferenceKind::TypeAlias,
                )
            })
            .collect()
    }

    pub(super) fn type_alias_name_counts(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();

        for entry in self.symbol_index.entries() {
            if !matches!(
                entry.kind,
                SymbolIndexKind::Class | SymbolIndexKind::TypeAlias
            ) {
                continue;
            }

            *counts.entry(entry.name.clone()).or_insert(0) += 1;
        }

        counts
    }

    pub fn selection_ranges(
        &self,
        path: impl AsRef<Path>,
        positions: &[SourcePosition],
    ) -> Vec<Option<SelectionRange>> {
        let Some(file) = self.file(path.as_ref()) else {
            return Vec::new();
        };
        let file_span = file.parsed.as_ref().ok().and_then(|parsed| parsed.span);

        selection_ranges_source(&file.source, file_span, positions)
    }

    pub fn folding_ranges(&self, path: impl AsRef<Path>) -> Vec<FoldingRange> {
        let Some(file) = self.file(path.as_ref()) else {
            return Vec::new();
        };
        let type_alias_spans = file
            .parsed
            .as_ref()
            .ok()
            .map(|parsed| {
                parsed
                    .type_aliases
                    .iter()
                    .filter_map(|alias| alias.span)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        folding_ranges_source(&file.source, &type_alias_spans)
    }

    pub fn assists(
        &self,
        path: impl AsRef<Path>,
        range: AssistRange,
        diagnostics: &[Diagnostic],
    ) -> Vec<Assist> {
        let path = path.as_ref();
        let Some(parsed) = self.file(path).and_then(|file| file.parsed.as_ref().ok()) else {
            return Vec::new();
        };
        let source_dir = self.assist_source_root_for_path(path);

        code_actions_parsed_source_with_kinds_and_diagnostics_config(
            parsed,
            diagnostics,
            range,
            &source_dir,
            &self.config.extension,
            &[],
            &self.config.diagnostics,
        )
    }

    pub fn code_actions(
        &self,
        path: impl AsRef<Path>,
        range: AssistRange,
        diagnostics: &[Diagnostic],
        requested_kinds: &[AssistKind],
    ) -> Vec<Assist> {
        let path = path.as_ref();
        let Some(parsed) = self.file(path).and_then(|file| file.parsed.as_ref().ok()) else {
            return Vec::new();
        };
        let source_dir = self.assist_source_root_for_path(path);

        code_actions_parsed_source_with_kinds_and_diagnostics_config(
            parsed,
            diagnostics,
            range,
            &source_dir,
            &self.config.extension,
            requested_kinds,
            &self.config.diagnostics,
        )
    }

    pub(super) fn index_hir_declarations(
        &self,
        parsed: &ParsedSource,
        hir: &HirModule,
        index: &mut ReverseReferenceIndex,
    ) {
        for symbol in hir.symbols() {
            let Some(definition_kind) = definition_kind_for_hir_symbol(symbol.kind) else {
                continue;
            };
            let definition = definition_from_hir_symbol_for_index(&parsed.path, symbol);

            if definition.kind != definition_kind {
                continue;
            }

            index.insert(
                &definition,
                Reference {
                    path: parsed.path.clone(),
                    span: symbol.name_span,
                    kind: reference_kind_for_definition(definition.kind),
                },
            );
        }
    }

    pub(super) fn module_export_candidates_for_file(
        &self,
        parsed: &ParsedSource,
        hir: Option<&HirModule>,
    ) -> ModuleExportCandidates {
        let Some(hir) = hir else {
            return ModuleExportCandidates::new();
        };
        let mut candidates = ModuleExportCandidates::new();

        for binding in &hir.require_bindings {
            let Some(path) = self.resolved_module_path(&parsed.path, &binding.module) else {
                continue;
            };
            let names = self.symbol_index.export_names_for_module_path(path);

            if names.is_empty() {
                continue;
            }

            candidates.insert(binding.alias.clone(), names);
        }

        candidates
    }

    pub fn inlay_hints(&self, path: impl AsRef<Path>) -> Vec<InlayHint> {
        let _profile = profile::span("analysis.query.inlay_hints");
        let path = path.as_ref();
        let Some(file) = self.file(path) else {
            return Vec::new();
        };
        let parsed = match &file.parsed {
            Ok(parsed) => parsed,
            Err(_) => {
                return self
                    .degraded_semantic_facts(path, file)
                    .map(|facts| {
                        inlay_hints_parsed_source_with_environment(
                            &facts.parsed,
                            &facts.environment,
                        )
                    })
                    .unwrap_or_default();
            }
        };
        let facts = self.semantic_facts_for_parsed(parsed);

        inlay_hints_parsed_source_with_environment(parsed, &facts.environment)
    }

    pub fn inlay_hints_in_range(
        &self,
        path: impl AsRef<Path>,
        range: SourceSpan,
    ) -> Vec<InlayHint> {
        let _profile = profile::span("analysis.query.inlay_hints.range");
        let path = path.as_ref();
        let Some(file) = self.file(path) else {
            return Vec::new();
        };
        let parsed = match &file.parsed {
            Ok(parsed) => parsed,
            Err(_) => {
                return self
                    .degraded_semantic_facts(path, file)
                    .map(|facts| {
                        inlay_hints_parsed_source_with_environment_in_range(
                            &facts.parsed,
                            &facts.environment,
                            Some(range),
                        )
                    })
                    .unwrap_or_default();
            }
        };
        let facts = self.semantic_facts_for_parsed(parsed);

        inlay_hints_parsed_source_with_environment_in_range(parsed, &facts.environment, Some(range))
    }

    pub fn semantic_tokens(&self, path: impl AsRef<Path>) -> Vec<SemanticToken> {
        let path = path.as_ref();
        let Some(file) = self.file(path) else {
            return Vec::new();
        };

        match &file.parsed {
            Ok(parsed) => {
                let facts = self.semantic_facts_for_parsed(parsed);

                crate::semantic_token::semantic_tokens_parsed_source_with_environment(
                    parsed,
                    &facts.environment,
                )
                .unwrap_or_default()
            }
            Err(_) => degraded_semantic_tokens(path, file),
        }
    }

    pub(super) fn module_hover(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<Hover> {
        if let Some(specifier) = module_specifier_at_position(&parsed.source, position) {
            let path = self.resolved_module_path(&parsed.path, &specifier.module)?;

            return Some(Hover {
                contents: format!("(module) {} -> {}", specifier.module, path.display()),
                span: None,
                documentation: None,
                context: Vec::new(),
            });
        }

        let chain = dotted_chain_at_position(&parsed.source, position)?;
        let current_part = chain.parts.get(chain.cursor_index)?;
        let root = chain.parts.first()?;
        let binding = self.require_binding(parsed, &root.text, position)?;

        if chain.cursor_index == 0 {
            let path = self.resolved_module_path(&parsed.path, &binding.module)?;

            return Some(Hover {
                contents: format!(
                    "(module) {} = {}(\"{}\") -> {}",
                    binding.alias,
                    binding.kind.label(),
                    binding.module,
                    path.display()
                ),
                span: Some(current_part.span),
                documentation: None,
                context: Vec::new(),
            });
        }

        let fields = chain
            .parts
            .iter()
            .skip(1)
            .take(chain.cursor_index)
            .map(|part| part.text.clone())
            .collect::<Vec<_>>();
        let path = self.resolved_module_path(&parsed.path, &binding.module)?;
        let export = self.module_export_for_fields(path, &fields)?;
        let documentation = module_export_documentation(
            &export,
            self.module_export_source_documentation(path, &fields),
        );
        let kind = if matches!(export.ty, Type::Function(_)) {
            "function"
        } else {
            "field"
        };

        Some(Hover {
            contents: format_module_export_hover(
                &chain
                    .parts
                    .iter()
                    .take(chain.cursor_index + 1)
                    .map(|part| part.text.as_str())
                    .collect::<Vec<_>>()
                    .join("."),
                &export.ty,
            ),
            span: Some(current_part.span),
            documentation,
            context: vec![
                format!("kind: {kind}"),
                format!("module: {}", path.display()),
            ],
        })
    }

    pub(super) fn assist_source_root_for_path(&self, path: &Path) -> PathBuf {
        self.source_roots()
            .iter()
            .filter(|root| path.starts_with(root))
            .max_by_key(|root| root.components().count())
            .cloned()
            .or_else(|| path.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

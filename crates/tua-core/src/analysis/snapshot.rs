use super::*;

impl AnalysisSnapshot {
    pub(super) fn i18n_wrapper_accepts_key(
        &self,
        parsed: &ParsedSource,
        callee_parts: &[String],
    ) -> bool {
        if callee_parts.len() == 1 && crate::i18n::function_accepts_key(parsed, &callee_parts[0], 0)
        {
            return true;
        }
        let Some((module, field)) = crate::i18n::imported_wrapper(parsed, callee_parts) else {
            return false;
        };
        let Some(path) = self.indexed_module_path_for_specifier(parsed, &module) else {
            return false;
        };
        self.file(path)
            .and_then(|file| file.parsed.as_ref().ok())
            .is_some_and(|target| crate::i18n::function_accepts_key(target, &field, 0))
    }

    pub(super) fn i18n_key_uses_for_file(&self, file: &AnalysisFile) -> Vec<crate::i18n::KeyUse> {
        let mut uses = file.i18n_key_uses().to_vec();
        let Some(parsed) = file.parsed.as_ref().ok() else {
            return uses;
        };
        uses.extend(
            file.i18n_wrapper_key_uses()
                .iter()
                .filter(|candidate| self.i18n_wrapper_accepts_key(parsed, &candidate.callee_parts))
                .map(|candidate| candidate.usage.clone()),
        );
        uses
    }

    pub(super) fn i18n_key_use_at(
        &self,
        file: &AnalysisFile,
        position: SourcePosition,
    ) -> Option<crate::i18n::KeyUse> {
        if let Some(usage) = file
            .i18n_key_uses()
            .iter()
            .find(|usage| super::span_contains_position(usage.span, position))
        {
            return Some(usage.clone());
        }
        let parsed = file.parsed.as_ref().ok()?;
        file.i18n_wrapper_key_uses()
            .iter()
            .find(|candidate| {
                super::span_contains_position(candidate.usage.span, position)
                    && self.i18n_wrapper_accepts_key(parsed, &candidate.callee_parts)
            })
            .map(|candidate| candidate.usage.clone())
    }
    pub(super) fn i18n_result_types(&self) -> Arc<BTreeMap<String, Type>> {
        self.i18n_result_types
            .get_or_init(|| {
                Arc::new(
                    self.catalog_index()
                        .guaranteed_result_types(&self.config.i18n),
                )
            })
            .clone()
    }

    pub(super) fn i18n_result_types_for_path(&self, path: &Path) -> Arc<BTreeMap<String, Type>> {
        let Some(root) = self.workspace_root_for_path(path) else {
            return self.i18n_result_types();
        };

        self.workspace_i18n_result_types
            .get_or_init(|| {
                self.workspace_configs
                    .iter()
                    .map(|(root, config)| {
                        (
                            root.clone(),
                            Arc::new(
                                self.catalog_index_for_path(root)
                                    .guaranteed_result_types(&config.i18n),
                            ),
                        )
                    })
                    .collect()
            })
            .get(root)
            .expect("workspace i18n result types must exist")
            .clone()
    }
    pub(super) fn catalog_index(&self) -> &crate::i18n::CatalogIndex {
        self.catalog_index.get_or_init(|| {
            let Some(root) = self.vfs.source_roots().first() else {
                return crate::i18n::CatalogIndex::default();
            };
            self.build_catalog_index(root, &self.config.i18n)
        })
    }

    pub(super) fn catalog_index_for_path(&self, path: &Path) -> &crate::i18n::CatalogIndex {
        let Some(root) = self.workspace_root_for_path(path) else {
            return self.catalog_index();
        };

        self.workspace_catalog_indexes
            .get_or_init(|| {
                self.workspace_configs
                    .iter()
                    .map(|(root, config)| {
                        (
                            root.clone(),
                            self.build_catalog_index(&root.join(&config.source), &config.i18n),
                        )
                    })
                    .collect()
            })
            .get(root)
            .expect("workspace catalog index must exist")
    }

    fn build_catalog_index(
        &self,
        root: &Path,
        config: &crate::I18nConfig,
    ) -> crate::i18n::CatalogIndex {
        let _profile = profile::span("analysis.index.i18n");
        let mut index = crate::i18n::CatalogIndex::default();
        if config.enabled {
            for catalog in &config.catalogs {
                let path = normalize_path(root.join(&catalog.path));
                let parsed = self
                    .files
                    .get(&path)
                    .and_then(|file| file.parsed.as_ref().ok());
                index.add_catalog(&catalog.locale, path, parsed.map(Arc::as_ref));
            }
            index.finish(config);
        }
        index
    }

    /// Catalog diagnostics are project facts, including catalogs that do not
    /// currently have a source file in the VFS.
    pub fn i18n_catalog_diagnostics(&self) -> Vec<Diagnostic> {
        if !self.config.i18n.enabled {
            return Vec::new();
        }
        self.catalog_index()
            .diagnostics
            .values()
            .flatten()
            .cloned()
            .collect()
    }

    pub fn localization_page(
        &self,
        search: &str,
        offset: usize,
        limit: usize,
    ) -> crate::LocalizationPage {
        self.localization_page_with_catalog(
            &self.config.i18n,
            self.catalog_index(),
            search,
            offset,
            limit,
        )
    }

    pub fn localization_page_for_path(
        &self,
        path: &Path,
        search: &str,
        offset: usize,
        limit: usize,
    ) -> crate::LocalizationPage {
        self.localization_page_with_catalog(
            &self.config_for_path(path).i18n,
            self.catalog_index_for_path(path),
            search,
            offset,
            limit,
        )
    }

    fn localization_page_with_catalog(
        &self,
        config: &crate::I18nConfig,
        index: &crate::i18n::CatalogIndex,
        search: &str,
        offset: usize,
        limit: usize,
    ) -> crate::LocalizationPage {
        if !config.enabled {
            return crate::LocalizationPage {
                enabled: false,
                locales: Vec::new(),
                total: 0,
                rows: Vec::new(),
            };
        }
        let locales = config
            .catalogs
            .iter()
            .map(|catalog| catalog.locale.clone())
            .collect::<Vec<_>>();
        let search = search
            .trim()
            .chars()
            .take(100)
            .collect::<String>()
            .to_lowercase();
        let matching = index
            .entries
            .iter()
            .filter(|(key, _)| key.to_lowercase().contains(&search))
            .collect::<Vec<_>>();
        let total = matching.len();
        let rows = matching
            .into_iter()
            .skip(offset)
            .take(limit.min(200))
            .map(|(key, entries)| {
                let cells = locales
                    .iter()
                    .map(|locale| {
                        let exact = entries.get(locale);
                        let resolved = index.resolved_entry(key, locale, &config.fallback_locale);
                        let status = match (exact, resolved) {
                            (Some(entry), _)
                                if matches!(entry.value, crate::i18n::CatalogValue::Dynamic) =>
                            {
                                "dynamic"
                            }
                            (Some(_), _) => "translated",
                            (None, Some(entry))
                                if matches!(entry.value, crate::i18n::CatalogValue::Dynamic) =>
                            {
                                "dynamic"
                            }
                            (None, Some(_)) => "fallback",
                            (None, None) => "missing",
                        };
                        let (preview, forms) = match resolved.map(|entry| &entry.value) {
                            Some(crate::i18n::CatalogValue::Text(value)) => {
                                (Some(value.clone()), Vec::new())
                            }
                            Some(crate::i18n::CatalogValue::Dialogue(lines)) => {
                                (Some(format!("{} dialogue lines", lines.len())), Vec::new())
                            }
                            Some(crate::i18n::CatalogValue::Plural(forms)) => (
                                None,
                                forms
                                    .iter()
                                    .map(|(name, text)| (name.clone(), text.clone()))
                                    .collect(),
                            ),
                            _ => (None, Vec::new()),
                        };
                        crate::LocalizationCell {
                            locale: locale.clone(),
                            status,
                            preview,
                            forms,
                            placeholders: resolved.map_or_else(Vec::new, |entry| {
                                entry.placeholders.iter().cloned().collect()
                            }),
                            path: resolved.map(|entry| entry.path.clone()),
                            span: resolved.map(|entry| entry.span),
                        }
                    })
                    .collect();
                crate::LocalizationRow {
                    key: key.clone(),
                    cells,
                }
            })
            .collect();
        crate::LocalizationPage {
            enabled: true,
            locales,
            total,
            rows,
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn config(&self) -> &ProjectConfig {
        &self.config
    }

    pub(super) fn config_for_path(&self, path: &Path) -> &ProjectConfig {
        super::config_for_path(&self.config, &self.workspace_configs, path)
    }

    pub(super) fn workspace_root_for_path(&self, path: &Path) -> Option<&Path> {
        self.workspace_configs
            .iter()
            .filter(|(root, _)| path.starts_with(root))
            .max_by_key(|(root, _)| root.components().count())
            .map(|(root, _)| root.as_path())
    }

    pub fn source_roots(&self) -> &[PathBuf] {
        self.vfs.source_roots()
    }

    pub fn asset_roots(&self) -> &[PathBuf] {
        &self.asset_roots
    }

    pub fn builtin_catalogs(&self) -> &BuiltinCatalogs {
        &self.builtin_catalogs
    }

    pub fn is_open_document(&self, path: impl AsRef<Path>) -> bool {
        self.vfs.is_open(path)
    }

    pub fn source(&self, path: impl AsRef<Path>) -> Option<&str> {
        self.file(path.as_ref()).map(|file| file.source.as_str())
    }

    pub fn semantic_file(&self, path: impl AsRef<Path>) -> Option<AnalysisSemanticFile<'_>> {
        let file = self.file(path.as_ref())?;
        let parsed = file.parsed.as_ref().ok()?;

        Some(AnalysisSemanticFile {
            parsed,
            hir: file.hir().expect("parsed file must have HIR"),
            scope: file.scope().expect("parsed file must have a scope map"),
        })
    }

    pub fn parse(&self, path: impl AsRef<Path>) -> Option<Result<&ParsedSource, &[Diagnostic]>> {
        self.file(path.as_ref())
            .map(|file| file.parsed.as_ref().map(Arc::as_ref).map_err(Vec::as_slice))
    }

    pub fn diagnostics(&self, path: impl AsRef<Path>) -> Vec<Diagnostic> {
        self.pipeline_diagnostics(path)
    }

    pub fn fast_diagnostics(&self, path: impl AsRef<Path>) -> Vec<Diagnostic> {
        if let Some(error) = self.input_errors.get(&normalize_path(path.as_ref())) {
            return vec![error.clone()];
        }
        let Some(file) = self.file(path.as_ref()) else {
            return Vec::new();
        };

        match &file.parsed {
            Ok(_) => Vec::new(),
            Err(diagnostics) => diagnostics.clone(),
        }
    }

    pub fn fresh_diagnostics(&self, path: impl AsRef<Path>) -> Vec<Diagnostic> {
        self.pipeline_diagnostics(path)
    }

    pub fn pipeline_diagnostics(&self, path: impl AsRef<Path>) -> Vec<Diagnostic> {
        let config = self.config_for_path(path.as_ref());
        if let Some(error) = self.input_errors.get(&normalize_path(path.as_ref())) {
            return vec![error.clone()];
        }
        let Some(file) = self.file(path.as_ref()) else {
            return Vec::new();
        };
        if file.kind == AnalysisSourceKind::Dialogue {
            let mut diagnostics = file.diagnostics.clone();
            diagnostics.extend(
                self.dialogue_project_diagnostics()
                    .get(&normalize_path(path.as_ref()))
                    .into_iter()
                    .flatten()
                    .cloned(),
            );
            return diagnostics;
        }
        let mut diagnostics = file.diagnostics.clone();
        diagnostics.extend(self.checker_diagnostics(path.as_ref(), file));
        diagnostics.extend(self.emission_diagnostics(path.as_ref(), file));

        let project_type_diagnostics = self.project_type_diagnostics(path.as_ref(), file);
        if self.project_type_check_required(file) {
            diagnostics.retain(|diagnostic| {
                !matches!(
                    diagnostic.code,
                    Some(
                        crate::DiagnosticCode::TYPE_MISMATCH
                            | crate::DiagnosticCode::TYPE_GENERIC_INFERENCE
                            | crate::DiagnosticCode::DOMAIN_ECS_COMPONENT_VALUE
                            | crate::DiagnosticCode::DOMAIN_ECS_QUERY
                    )
                )
            });
        }
        diagnostics.extend(project_type_diagnostics);
        diagnostics.extend(self.unsent_shader_diagnostics(path.as_ref()));
        diagnostics.extend(self.shader_scan_runtime_path_diagnostics(path.as_ref(), file));
        diagnostics.extend(self.usage_index.signal_event_diagnostics(path.as_ref()));
        if config.i18n.enabled {
            if let Some(catalog_diagnostics) = self
                .catalog_index_for_path(path.as_ref())
                .diagnostics
                .get(&normalize_path(path.as_ref()))
            {
                diagnostics.extend(catalog_diagnostics.iter().cloned());
            }
            if let Ok(parsed) = &file.parsed {
                if parsed.raw.language == crate::ParsedLanguage::Tua {
                    let uses = self.i18n_key_uses_for_file(file);
                    diagnostics.extend(crate::i18n::use_diagnostics(
                        parsed,
                        &uses,
                        file.i18n_locale_uses(),
                        self.catalog_index_for_path(path.as_ref()),
                        &config.i18n,
                    ));
                }
            }
        }

        // Source-only checking can validate on-disk modules, but it cannot see
        // VFS overlays or the complete workspace module map. The graph below
        // is authoritative inside a snapshot and republishes missing/invalid
        // static module diagnostics with project-aware resolution.
        diagnostics.retain(|diagnostic| {
            !matches!(
                diagnostic.code,
                Some(
                    crate::DiagnosticCode::LINT_MISSING_REQUIRE
                        | crate::DiagnosticCode::LINT_BAD_REQUIRE
                )
            )
        });

        diagnostics.extend(
            self.module_graph
                .diagnostics(path.as_ref())
                .iter()
                .filter(|diagnostic| self.project_diagnostic_enabled(path.as_ref(), diagnostic))
                .cloned(),
        );
        diagnostics.extend(self.unparsed_lua_module_diagnostics(path.as_ref()));

        let diagnostics = self.project_aware_diagnostics(
            path.as_ref(),
            deduplicate_diagnostics(diagnostics),
            file.parsed
                .as_ref()
                .ok()
                .map(Arc::as_ref)
                .or_else(|| file.degraded_semantics.as_ref().map(|facts| &facts.parsed)),
        );

        apply_inline_ignores(
            path.as_ref(),
            &file.source,
            diagnostics,
            &config.diagnostics,
        )
    }

    fn checker_diagnostics(&self, path: &Path, file: &AnalysisFile) -> Vec<Diagnostic> {
        let config = self.config_for_path(path);
        file.checker_diagnostics
            .get_or_init(|| {
                let default_ecs = crate::config::EcsConfig::default();
                let (parsed, environment, environment_diagnostics, ecs) = match &file.parsed {
                    Ok(parsed) => {
                        let semantics = file
                            .semantics
                            .as_ref()
                            .expect("parsed file must have semantic facts");
                        (
                            parsed.as_ref(),
                            &semantics.environment,
                            &semantics.environment_diagnostics,
                            &default_ecs,
                        )
                    }
                    Err(_) => {
                        let Some(degraded) = &file.degraded_semantics else {
                            return Vec::new();
                        };
                        (
                            &degraded.parsed,
                            &degraded.environment,
                            &degraded.environment_diagnostics,
                            &config.ecs,
                        )
                    }
                };
                if !environment_diagnostics.is_empty() {
                    return Vec::new();
                }
                let i18n_environment = (config.i18n.enabled && !file.i18n_key_uses().is_empty())
                    .then(|| {
                        environment
                            .clone()
                            .with_i18n_result_types(self.i18n_result_types_for_path(path))
                    });
                let environment = i18n_environment.as_ref().unwrap_or(environment);
                let _profile = profile::span_for_path("analysis.check", path);
                let mut diagnostics_config = config.diagnostics.clone();
                if parsed.raw.language == crate::ParsedLanguage::Tua {
                    diagnostics_config
                        .globals
                        .push(crate::save::MODULE_NAME.to_owned());
                }
                if config.i18n.enabled && parsed.raw.language == crate::ParsedLanguage::Tua {
                    diagnostics_config
                        .globals
                        .push(crate::i18n::MODULE_NAME.to_owned());
                }
                if environment.has_busted_globals() {
                    for name in crate::busted::GLOBAL_NAMES {
                        if !diagnostics_config
                            .globals
                            .iter()
                            .any(|global| global == name)
                        {
                            diagnostics_config.globals.push((*name).to_owned());
                        }
                    }
                }
                diagnostics_config.inline_ignores = false;
                check_parsed_source_with_environment_and_ecs_config(
                    parsed,
                    environment,
                    crate::check::CheckQueryOptions::new(
                        analysis_asset_roots(self.vfs.source_roots(), &self.asset_roots),
                        &diagnostics_config,
                        &config.jit,
                        ecs,
                        self.asset_index.as_deref(),
                        None,
                    ),
                )
                .err()
                .unwrap_or_default()
            })
            .clone()
    }

    fn emission_diagnostics(&self, path: &Path, file: &AnalysisFile) -> Vec<Diagnostic> {
        if file.kind != AnalysisSourceKind::Tua {
            return Vec::new();
        }
        let Ok(parsed) = &file.parsed else {
            return Vec::new();
        };
        let path = normalize_path(path);
        let config = self.config_for_path(&path);
        if let Some(cached) = self
            .emission_diagnostics_cache
            .lock()
            .expect("emission diagnostics cache poisoned")
            .get(&path)
        {
            return cached.clone();
        }
        let _profile = profile::span_for_path("analysis.emit.check", &path);
        let known_paths = self.files.keys().cloned().collect::<BTreeSet<_>>();
        let resolver = |specifier: &str| {
            module_name_for_import_specifier(
                &path,
                specifier,
                &known_paths,
                self.vfs.source_roots(),
                &self.vfs.options().output_roots,
                &config.extension,
                self.require_path_separator,
            )
        };
        let environment = file.environment().cloned().map(|environment| {
            environment.with_project_module_globals(parsed, self.project_module_globals())
        });
        let diagnostics = crate::emit::emit_lua_artifact_with_mode_import_resolver_and_environment(
            parsed,
            config.emit.annotation_mode,
            Some(&resolver),
            config.ecs.enabled,
            config.i18n.enabled,
            environment.as_ref(),
        )
        .err()
        .unwrap_or_default();
        self.emission_diagnostics_cache
            .lock()
            .expect("emission diagnostics cache poisoned")
            .insert(path, diagnostics.clone());
        diagnostics
    }

    pub(super) fn file(&self, path: &Path) -> Option<&AnalysisFile> {
        self.files.get(&normalize_path(path))
    }

    /// Plain Lua files are not checked themselves, so a required one that
    /// fails to parse would otherwise leave its importer silently untyped.
    fn unparsed_lua_module_diagnostics(&self, path: &Path) -> Vec<Diagnostic> {
        let Some(module) = self.module_graph.module(path) else {
            return Vec::new();
        };

        module
            .require_edges
            .iter()
            .filter_map(|edge| {
                let target = match &edge.resolution {
                    RequireResolution::Resolved { path } => path,
                    RequireResolution::Duplicate { selected, .. } => selected,
                    RequireResolution::Missing { .. } | RequireResolution::Invalid { .. } => {
                        return None;
                    }
                };
                let file = self.file(target)?;
                if file.kind != AnalysisSourceKind::Lua {
                    return None;
                }
                let error = file.parsed.as_ref().err()?.first()?;

                Some(
                    Diagnostic::new(
                        edge.source.clone(),
                        edge.span.start.line,
                        edge.span.start.column,
                        crate::DiagnosticSeverity::Warning,
                        format!(
                            "module `{}` is untyped because {}:{}:{} does not parse: {}",
                            edge.module,
                            target.to_string_lossy().replace('\\', "/"),
                            error.line,
                            error.column,
                            error.message
                        ),
                    )
                    .with_code(crate::DiagnosticCode::TYPE_UNPARSED_MODULE),
                )
            })
            .collect()
    }

    pub(super) fn effective_asset_roots(&self) -> &[PathBuf] {
        analysis_asset_roots(self.source_roots(), &self.asset_roots)
    }

    pub fn type_environment(&self, path: impl AsRef<Path>) -> Option<&TypeEnvironment> {
        self.file(path.as_ref())?.environment()
    }

    /// Returns the project-aware environment used by semantic queries. Unlike
    /// `type_environment`, this includes imported aliases, required-module
    /// values, and project-local inference, so compiler lowering can make the
    /// same type-driven decisions as diagnostics and editor features.
    pub fn project_type_environment(&self, path: impl AsRef<Path>) -> Option<TypeEnvironment> {
        let file = self.file(path.as_ref())?;
        let parsed = file.parsed.as_ref().ok()?;

        // Reuse facts an editor query already built, but don't build a whole
        // semantic index just to read the environment: for a large file that
        // costs far more than the environment itself.
        if let Some(facts) = self
            .semantic_facts
            .lock()
            .expect("semantic facts lock poisoned")
            .get(&normalize_path(&parsed.path))
        {
            return Some(facts.environment.clone());
        }
        let Some(semantics) = file.semantics() else {
            return Some(self.semantic_environment(parsed));
        };
        let inference = self.project_local_inference(parsed);

        Some(self.project_type_environment_for_parsed(
            parsed,
            semantics.environment.clone(),
            &inference,
        ))
    }

    pub fn semantic_index(&self, path: impl AsRef<Path>) -> Option<SemanticIndex> {
        let file = self.file(path.as_ref())?;
        let parsed = file.parsed.as_ref().ok()?;

        Some(
            self.semantic_facts_for_parsed(parsed)
                .semantic_index
                .clone(),
        )
    }

    pub fn scope_map(&self, path: impl AsRef<Path>) -> Option<&ScopeMap> {
        self.file(path.as_ref())?.scope()
    }

    pub fn module_exports(&self, path: impl AsRef<Path>) -> Option<&ModuleExports> {
        self.file(path.as_ref())?.exports.as_ref()
    }

    pub fn require_edges(&self, path: impl AsRef<Path>) -> &[ModuleLink] {
        self.file(path.as_ref())
            .map(|file| file.require_edges.as_slice())
            .unwrap_or_default()
    }

    pub fn module_graph(&self) -> &ModuleGraph {
        &self.module_graph
    }

    pub fn type_database(&self) -> &ProjectTypeDatabase {
        &self.type_database
    }

    pub fn symbol_index(&self) -> &SymbolIndex {
        &self.symbol_index
    }

    pub fn symbol_index_for_path(&self, path: impl AsRef<Path>) -> Vec<&SymbolIndexEntry> {
        self.symbol_index.entries_for_path(path)
    }

    pub fn workspace_symbols(&self, query: &str) -> Vec<WorkspaceSymbol> {
        self.symbol_index.workspace_symbols(query)
    }

    pub fn signal_graph(&self) -> SignalGraph {
        let mut events = self
            .usage_index
            .signal_event_index()
            .grouped_events()
            .into_iter()
            .filter_map(|group| {
                let first = group.first()?;
                let name = first.mention.name.clone();
                let bus = bus_label(&first.identity, &first.mention.receiver_parts);
                let id = graph_event_id(&first.identity, &name);
                let bus_path = first.identity.origin.clone();
                let (resolved_payload, schema_conflict) = graph_schema_summary(&group);
                let mut calls = group
                    .into_iter()
                    .map(|event| {
                        graph_call_from_mention(
                            event.path.clone(),
                            event.mention.clone(),
                            self.source(&event.path),
                        )
                    })
                    .collect::<Vec<_>>();

                calls.sort_by(|left, right| {
                    left.path
                        .cmp(&right.path)
                        .then_with(|| left.span.start.line.cmp(&right.span.start.line))
                        .then_with(|| left.span.start.column.cmp(&right.span.start.column))
                        .then_with(|| left.method.cmp(&right.method))
                });

                Some(SignalGraphEvent {
                    id,
                    name,
                    bus,
                    bus_path,
                    resolved_payload,
                    schema_conflict,
                    calls,
                })
            })
            .collect::<Vec<_>>();

        events.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.bus.cmp(&right.bus))
                .then_with(|| left.id.cmp(&right.id))
        });

        SignalGraph { events }
    }

    pub fn importable_symbols(&self, prefix: &str) -> Vec<&SymbolIndexEntry> {
        self.symbol_index.importable_symbols(prefix)
    }
}

impl AnalysisSnapshot {
    fn dialogue_project_diagnostics(&self) -> &BTreeMap<PathBuf, Vec<Diagnostic>> {
        self.dialogue_diagnostics.get_or_init(|| {
            let mut ids: BTreeMap<(PathBuf, String), Vec<(PathBuf, SourceSpan)>> = BTreeMap::new();
            for (path, file) in &self.files {
                if file.kind != AnalysisSourceKind::Dialogue || file.parsed.is_err() {
                    continue;
                }
                let model = file.dialogue_model.get_or_init(|| {
                    crate::dialogue_asset::parse_dialogue_source(path, &file.source)
                        .ok()
                        .map(Arc::new)
                });
                let Some(model) = model else {
                    continue;
                };
                let workspace = self
                    .workspace_configs
                    .iter()
                    .filter(|(root, _)| path.starts_with(root))
                    .max_by_key(|(root, _)| root.components().count())
                    .map(|(root, _)| root.clone())
                    .unwrap_or_default();
                for section in &model.sections {
                    for line in &section.lines {
                        for text in std::iter::once(&line.text)
                            .chain(line.choices.iter().map(|choice| &choice.text))
                        {
                            ids.entry((workspace.clone(), text.id.value.clone()))
                                .or_default()
                                .push((path.clone(), text.id.span));
                        }
                    }
                }
            }
            let mut diagnostics: BTreeMap<PathBuf, Vec<Diagnostic>> = BTreeMap::new();
            for ((_, id), locations) in ids {
                if locations.len() < 2 {
                    continue;
                }
                for (path, span) in &locations {
                    diagnostics.entry(path.clone()).or_default().push(
                        Diagnostic::error(
                            path,
                            span.start.line,
                            span.start.column,
                            format!("dialogue text ID `{id}` is duplicated across project assets"),
                        )
                        .with_code(crate::DiagnosticCode::DOMAIN_DIALOGUE)
                        .with_range(
                            span.start.line,
                            span.start.column,
                            span.end.line,
                            span.end.column,
                        ),
                    );
                }
            }
            diagnostics
        })
    }
}

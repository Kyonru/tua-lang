use super::*;

impl AnalysisSnapshot {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn config(&self) -> &ProjectConfig {
        &self.config
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
            .map(|file| file.parsed.as_ref().map_err(Vec::as_slice))
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
        if let Some(error) = self.input_errors.get(&normalize_path(path.as_ref())) {
            return vec![error.clone()];
        }
        let Some(file) = self.file(path.as_ref()) else {
            return Vec::new();
        };
        let mut diagnostics = file.diagnostics.clone();

        let project_type_diagnostics = self.project_type_diagnostics(path.as_ref(), file);
        if self.project_type_check_required(file) {
            diagnostics.retain(|diagnostic| {
                !matches!(
                    diagnostic.code,
                    Some(
                        crate::DiagnosticCode::TYPE_MISMATCH
                            | crate::DiagnosticCode::DOMAIN_ECS_COMPONENT_VALUE
                            | crate::DiagnosticCode::DOMAIN_ECS_QUERY
                    )
                )
            });
        }
        diagnostics.extend(project_type_diagnostics);
        diagnostics.extend(self.unsent_shader_diagnostics(path.as_ref()));
        diagnostics.extend(self.usage_index.signal_event_diagnostics(path.as_ref()));

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
                .filter(|diagnostic| self.project_diagnostic_enabled(diagnostic))
                .cloned(),
        );
        diagnostics.extend(self.unparsed_lua_module_diagnostics(path.as_ref()));

        let diagnostics =
            self.project_aware_diagnostics(path.as_ref(), deduplicate_diagnostics(diagnostics));

        apply_inline_ignores(
            path.as_ref(),
            &file.source,
            diagnostics,
            &self.config.diagnostics,
        )
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
                let bus = bus_label(&first.identity);
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

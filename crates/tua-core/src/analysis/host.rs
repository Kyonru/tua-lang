use super::*;

fn declaration_modules(
    files: &BTreeMap<PathBuf, AnalysisFile>,
    libraries: &[ProjectTypeLibrary],
) -> BTreeMap<String, PathBuf> {
    let mut modules = BTreeMap::new();

    for library in libraries {
        for (path, file) in files {
            if !path.starts_with(library.root()) {
                continue;
            }

            let Some(meta_name) = file.parsed.as_ref().ok().and_then(declaration_meta_name) else {
                continue;
            };
            let prefix = library.declaration_prefix();
            let suffix = if meta_name == prefix {
                ""
            } else if let Some(suffix) = meta_name.strip_prefix(prefix) {
                if !suffix.starts_with('.') {
                    continue;
                }
                suffix
            } else {
                continue;
            };
            let runtime_name = format!("{}{suffix}", library.runtime_prefix());

            modules.entry(runtime_name).or_insert_with(|| path.clone());
        }
    }

    modules
}

fn declaration_meta_name(parsed: &ParsedSource) -> Option<&str> {
    parsed.lua_doc_annotations.iter().find_map(|annotation| {
        let LuaDocAnnotationKind::Meta(meta) = &annotation.kind else {
            return None;
        };
        let expression = meta.expression.as_deref()?.trim();

        Some(
            expression
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
                .or_else(|| {
                    expression
                        .strip_prefix('\'')
                        .and_then(|value| value.strip_suffix('\''))
                })
                .unwrap_or(expression),
        )
    })
}

impl AnalysisHost {
    /// Record a rejected disk input without retaining stale semantic results.
    pub fn set_input_error(&mut self, path: impl AsRef<Path>, message: impl Into<String>) {
        let path = normalize_path(path.as_ref());
        self.remove_file(&path);
        self.input_errors.insert(
            path.clone(),
            Diagnostic::error(&path, 1, 1, message).with_code(crate::DiagnosticCode::CONFIG_IO),
        );
    }

    fn admit_source(&mut self, path: &Path, bytes: usize) -> bool {
        use crate::input_limits::{MAX_PROJECT_FILES, MAX_PROJECT_SOURCE_BYTES, MAX_SOURCE_BYTES};
        let path = normalize_path(path);
        let previous = self.input_sizes.get(&path).copied().unwrap_or(0);
        // Keep the larger disk/overlay footprint until removal: an open
        // overlay may coexist with a different saved source in the VFS.
        let retained = previous.max(bytes);
        let total = self.input_sizes.values().sum::<usize>() - previous + retained;
        let message = if bytes > MAX_SOURCE_BYTES {
            Some("source exceeds 8 MiB input limit")
        } else if total > MAX_PROJECT_SOURCE_BYTES {
            Some("project exceeds 128 MiB aggregate source input limit")
        } else if !self.input_sizes.contains_key(&path)
            && self.input_sizes.len() >= MAX_PROJECT_FILES
        {
            Some("project exceeds 20000 source file limit")
        } else {
            None
        };
        if let Some(message) = message {
            self.set_input_error(path, message);
            return false;
        }
        self.input_sizes.insert(path.clone(), retained);
        if self.input_errors.remove(&path).is_some() {
            self.bump_revision();
        }
        true
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(config: ProjectConfig) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }

    pub fn config(&self) -> &ProjectConfig {
        &self.config
    }

    pub fn set_config(&mut self, config: ProjectConfig) {
        if self.config == config {
            return;
        }

        self.config = config;
        self.invalidate_all_analysis();
        self.bump_revision();
    }

    pub fn set_require_path_separator(&mut self, separator: ModulePathSeparator) {
        if self.require_path_separator == separator {
            return;
        }

        self.require_path_separator = separator;
        self.bump_module_path_revision();
        self.invalidate_all_analysis();
        self.bump_revision();
    }

    pub fn set_type_libraries(&mut self, libraries: impl IntoIterator<Item = ProjectTypeLibrary>) {
        let libraries = libraries.into_iter().collect::<Vec<_>>();

        if self.type_libraries == libraries {
            return;
        }

        self.type_libraries = libraries;
        self.invalidate_all_analysis();
        self.bump_revision();
    }

    pub fn set_module_path_workspaces(
        &mut self,
        workspaces: impl IntoIterator<Item = ProjectModuleWorkspace>,
    ) {
        let mut workspaces = workspaces.into_iter().collect::<Vec<_>>();

        workspaces.sort_by(|left, right| left.root().cmp(right.root()));
        workspaces.dedup();
        if self.module_path_workspaces == workspaces {
            return;
        }

        self.module_path_workspaces = workspaces;
        self.bump_module_path_revision();
        self.clear_completion_cache();
        self.bump_revision();
    }

    pub fn set_indexed_module_paths(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        let paths = paths
            .into_iter()
            .map(normalize_path)
            .collect::<BTreeSet<_>>();

        if self.indexed_module_paths == paths {
            return;
        }

        self.indexed_module_paths = paths;
        self.bump_module_path_revision();
        self.clear_completion_cache();
        self.bump_revision();
    }

    pub fn add_indexed_module_path(&mut self, path: impl Into<PathBuf>) {
        if self
            .indexed_module_paths
            .insert(normalize_path(path.into()))
        {
            self.bump_module_path_revision();
            self.clear_completion_cache();
            self.bump_revision();
        }
    }

    pub fn source_roots(&self) -> &[PathBuf] {
        &self.vfs.options().source_roots
    }

    pub fn set_source_roots(&mut self, roots: impl IntoIterator<Item = PathBuf>) {
        let roots = roots.into_iter().map(normalize_path).collect::<Vec<_>>();

        if self.vfs.options().source_roots == roots {
            return;
        }

        self.vfs.set_source_roots(roots);
        self.bump_module_path_revision();
        self.asset_index = None;
        self.invalidate_all_analysis();
        self.bump_revision();
    }

    pub fn set_output_roots(&mut self, roots: impl IntoIterator<Item = PathBuf>) {
        let roots = roots.into_iter().map(normalize_path).collect::<Vec<_>>();

        if self.vfs.options().output_roots == roots {
            return;
        }

        self.vfs.set_output_roots(roots);
        self.bump_module_path_revision();
        self.invalidate_all_analysis();
        self.bump_revision();
    }

    pub fn asset_roots(&self) -> &[PathBuf] {
        &self.asset_roots
    }

    pub fn set_completion_adaptive_stats(&mut self, stats: CompletionAdaptiveStats) {
        if self.completion_adaptive_stats == stats {
            return;
        }

        self.completion_adaptive_stats = stats;
        self.clear_completion_cache();
        self.bump_revision();
    }

    pub fn set_completion_sequence_ranking(&mut self, config: CompletionSequenceRankingConfig) {
        if self.completion_sequence_ranking == config {
            return;
        }

        self.completion_sequence_ranking = config;
        self.clear_completion_cache();
        self.bump_revision();
    }

    pub fn set_completion_multi_token(&mut self, config: CompletionMultiTokenConfig) {
        if self.completion_multi_token == config {
            return;
        }

        self.completion_multi_token = config;
        self.bump_revision();
    }

    /// Enables capture of the compact scope facts used by the optional
    /// statistical inline source. Keeping this explicit preserves a zero-work
    /// default path: ordinary analysis never extracts or stores these facts.
    pub fn set_inline_statistical_enabled(&mut self, enabled: bool) {
        if self.inline_statistical_enabled == enabled {
            return;
        }
        self.inline_statistical_enabled = enabled;
        if !enabled {
            Arc::make_mut(self.inline_statistical_scopes.get_mut()).clear();
        }
    }

    /// Select whether background corpus builds retain exact line-retrieval
    /// tables. `None` is the core/test default for backwards compatibility;
    /// the LSP always sets the explicit workspace configuration.
    pub fn set_inline_project_enabled(&mut self, enabled: bool) {
        self.project_pattern_exact_enabled = Some(enabled);
    }

    #[cfg(test)]
    pub(crate) fn retained_statistical_scope_files(&self) -> usize {
        self.inline_statistical_scopes.borrow().len()
    }

    #[cfg(test)]
    pub(crate) fn retained_last_good_files(&self) -> usize {
        self.last_good_files.borrow().len()
    }

    #[cfg(any(test, feature = "test-fixture"))]
    pub(crate) fn forget_inline_statistical_scope(&self, path: impl AsRef<Path>) {
        Arc::make_mut(&mut self.inline_statistical_scopes.borrow_mut())
            .remove(&normalize_path(path.as_ref()));
    }

    #[cfg(test)]
    pub(crate) fn statistical_scope_storage_ptr(&self) -> *const () {
        Arc::as_ptr(&self.inline_statistical_scopes.borrow()).cast()
    }

    pub fn replace_completion_adaptive_file_observations(
        &mut self,
        file_scope_hash: u64,
        source: &str,
        now_epoch: u64,
    ) {
        self.completion_adaptive_stats.replace_file_observations(
            file_scope_hash,
            source,
            now_epoch,
        );
        self.clear_completion_cache();
        self.bump_revision();
    }

    pub fn remove_completion_adaptive_file_observations(&mut self, file_scope_hash: u64) {
        self.completion_adaptive_stats
            .remove_file_observations(file_scope_hash);
        self.clear_completion_cache();
        self.bump_revision();
    }

    pub fn reset_completion_adaptive_stats(&mut self) {
        if self.completion_adaptive_stats == CompletionAdaptiveStats::default() {
            return;
        }

        self.completion_adaptive_stats = CompletionAdaptiveStats::default();
        self.clear_completion_cache();
        self.bump_revision();
    }

    pub fn set_asset_roots(&mut self, roots: impl IntoIterator<Item = PathBuf>) {
        let roots = roots.into_iter().map(normalize_path).collect::<Vec<_>>();

        if self.asset_roots == roots {
            return;
        }

        self.asset_roots = roots;
        self.asset_index = None;
        self.invalidate_all_analysis();
        self.bump_revision();
    }

    /// Update an editor shader overlay without parsing GLSL as Tua or reading disk on queries.
    pub fn set_shader_source_overlay(&mut self, path: impl AsRef<Path>, source: Option<String>) {
        let path = normalize_path(path.as_ref());
        if let Some(source) = source.as_ref() {
            self.shader_overlays.insert(path.clone(), source.clone());
        } else {
            self.shader_overlays.remove(&path);
        }
        if let Some(index) = &mut self.asset_index {
            index.update_shader(&path, source);
        }
        self.cache.borrow_mut().invalidate_all();
        self.clear_query_caches();
        self.bump_revision();
    }

    pub fn refresh_asset_index_from_disk(&mut self) {
        let roots = analysis_asset_roots(self.source_roots(), &self.asset_roots).to_vec();
        let mut index =
            asset::AssetIndex::from_roots_with_previous(&roots, self.asset_index.as_ref());
        for (path, source) in &self.shader_overlays {
            index.update_shader(path, Some(source.clone()));
        }

        if self.asset_index.as_ref() == Some(&index) {
            return;
        }

        self.asset_index = Some(index);
        self.cache.borrow_mut().invalidate_all();
        self.clear_query_caches();
        self.bump_revision();
    }

    pub fn set_ignored_paths(&mut self, patterns: impl IntoIterator<Item = String>) {
        let patterns = patterns.into_iter().collect::<Vec<_>>();

        if self.vfs.options().ignored_paths == patterns {
            return;
        }

        self.vfs.set_ignored_paths(patterns);
        self.bump_module_path_revision();
        self.invalidate_all_analysis();
        self.bump_revision();
    }

    pub fn builtin_catalogs(&self) -> &BuiltinCatalogs {
        &self.builtin_catalogs
    }

    pub fn set_builtin_catalogs(&mut self, catalogs: BuiltinCatalogs) {
        if self.builtin_catalogs == catalogs {
            return;
        }

        self.builtin_catalogs = catalogs;
        self.invalidate_all_analysis();
        self.bump_revision();
    }

    pub fn set_file(&mut self, path: impl Into<PathBuf>, source: impl Into<String>) {
        let path = path.into();
        let kind = infer_source_kind(&path, &self.config);

        self.set_file_with_kind(path, source, kind);
    }

    pub fn set_file_with_kind(
        &mut self,
        path: impl Into<PathBuf>,
        source: impl Into<String>,
        kind: AnalysisSourceKind,
    ) {
        let _ = self.update_file_with_kind(path, source, kind);
    }

    /// Updates a file and reports whether cached importers were invalidated.
    #[must_use]
    pub fn update_file_with_kind(
        &mut self,
        path: impl Into<PathBuf>,
        source: impl Into<String>,
        kind: AnalysisSourceKind,
    ) -> bool {
        let path = path.into();
        let source = source.into();
        if !self.admit_source(&path, source.len()) {
            return true;
        }
        let vfs_kind = vfs_source_kind(kind);
        let had_module_path = self.has_module_path(&path);

        if self.vfs.disk_file_matches(&path, &source, vfs_kind) {
            return false;
        }

        let invalidated_dependents =
            self.cache
                .borrow_mut()
                .invalidate_changed_file(&path, &source, kind, &self.config);
        self.clear_completion_cache();
        if invalidated_dependents {
            self.clear_definition_cache();
        }
        self.vfs.set_disk_file(&path, source, vfs_kind);
        if had_module_path != self.has_module_path(&path) {
            self.bump_module_path_revision();
        }
        self.bump_revision();

        invalidated_dependents
    }

    pub fn remove_file(&mut self, path: impl AsRef<Path>) {
        let path = normalize_path(path.as_ref());
        self.input_sizes.remove(&path);
        self.input_errors.remove(&path);
        let had_module_path = self.has_module_path(&path);
        self.indexed_module_paths.remove(&path);
        self.cache.borrow_mut().invalidate_file(path.as_ref(), true);
        self.clear_query_caches();
        self.last_good_files.borrow_mut().remove(&path);
        Arc::make_mut(self.inline_statistical_scopes.get_mut()).remove(&path);
        self.vfs.remove_file(&path);
        if had_module_path != self.has_module_path(&path) {
            self.bump_module_path_revision();
        }
        self.bump_revision();
    }

    pub fn invalidate_file_analysis(&mut self, path: impl AsRef<Path>) {
        self.cache
            .borrow_mut()
            .invalidate_file(path.as_ref(), false);
        self.clear_query_caches();
        self.bump_revision();
    }

    pub fn open_document(&mut self, path: impl Into<PathBuf>, source: impl Into<String>) {
        let path = path.into();
        let kind = infer_source_kind(&path, &self.config);

        self.open_document_with_kind(path, source, kind);
    }

    pub fn open_document_with_kind(
        &mut self,
        path: impl Into<PathBuf>,
        source: impl Into<String>,
        kind: AnalysisSourceKind,
    ) {
        let path = path.into();
        let source = source.into();
        if !self.admit_source(&path, source.len()) {
            return;
        }
        let vfs_kind = vfs_source_kind(kind);
        let had_module_path = self.has_module_path(&path);

        if self.vfs.open_file_matches(&path, &source, vfs_kind) {
            return;
        }

        let invalidated_dependents =
            self.cache
                .borrow_mut()
                .invalidate_changed_file(&path, &source, kind, &self.config);
        self.clear_completion_cache();
        if invalidated_dependents {
            self.clear_definition_cache();
        }
        self.vfs.open_file(&path, source, vfs_kind);
        if had_module_path != self.has_module_path(&path) {
            self.bump_module_path_revision();
        }
        self.bump_revision();
    }

    pub fn update_open_document(&mut self, path: impl Into<PathBuf>, source: impl Into<String>) {
        self.open_document(path, source);
    }

    pub fn close_document(&mut self, path: impl AsRef<Path>) {
        let path = normalize_path(path.as_ref());
        let had_module_path = self.has_module_path(&path);
        self.cache.borrow_mut().invalidate_file(path.as_ref(), true);
        self.clear_query_caches();
        self.vfs.close_file(&path);
        // Closing an overlay can reveal a different disk source. Do not let
        // request-side facts captured from the former open document survive
        // until the background warmer reaches the newly visible source.
        Arc::make_mut(self.inline_statistical_scopes.get_mut()).remove(&path);
        if had_module_path != self.has_module_path(&path) {
            self.bump_module_path_revision();
        }
        self.bump_revision();
    }

    pub fn snapshot(&self) -> AnalysisSnapshot {
        let _profile = profile::span("analysis.snapshot");
        if let Some(cached) = self
            .snapshot_cache
            .lock()
            .expect("analysis snapshot cache lock poisoned")
            .as_ref()
        {
            if cached.revision == self.revision {
                return cached.snapshot.clone();
            }
        }

        let vfs = self.vfs.snapshot();
        let source_roots = vfs.source_roots().to_vec();
        let live_paths = vfs
            .files()
            .map(|file| file.path().to_path_buf())
            .collect::<BTreeSet<_>>();
        let files = {
            let mut cache = self.cache.borrow_mut();

            cache.retain_paths(&live_paths);

            vfs.files()
                .map(|file| {
                    let analysis_file = cache.cached_file(
                        file.path(),
                        file.source(),
                        analysis_source_kind(file.kind()),
                        AnalysisBuildContext {
                            config: &self.config,
                            source_roots: &source_roots,
                            output_roots: &vfs.options().output_roots,
                            known_paths: &live_paths,
                            require_path_separator: self.require_path_separator,
                            asset_roots: &self.asset_roots,
                            asset_index: self.asset_index.as_ref(),
                        },
                    );

                    (file.path().to_path_buf(), analysis_file)
                })
                .collect::<BTreeMap<_, _>>()
        };
        let module_graph = {
            let _profile = profile::span("analysis.index.module_graph");
            let _trace = profile::diagnostics_span(profile::DiagnosticsTraceCategory::ModuleGraph);

            ModuleGraph::build(
                files.iter().map(|(path, file)| ModuleGraphInput {
                    path,
                    parsed: file.parsed.as_ref().ok(),
                    hir: file.hir(),
                    exports: file.exports.as_ref(),
                    source_kind: file.kind,
                }),
                ModuleGraphOptions {
                    source_roots: vfs.source_roots().to_vec(),
                    output_roots: vfs.options().output_roots.clone(),
                    extension: self.config.extension.clone(),
                    require_path_separator: self.require_path_separator,
                    declaration_modules: declaration_modules(&files, &self.type_libraries),
                },
            )
        };
        let type_database = {
            let _profile = profile::span("analysis.index.type_database");
            let _trace =
                profile::diagnostics_span(profile::DiagnosticsTraceCategory::TypeEnvironment);

            ProjectTypeDatabase::build(
                files.iter().map(|(path, file)| ProjectTypeDatabaseInput {
                    path,
                    source_kind: file.kind,
                    environment: file.environment(),
                    scope: file.scope(),
                    parsed: file.parsed.as_ref().ok(),
                }),
                &module_graph,
            )
        };
        let symbol_index = {
            let _profile = profile::span("analysis.index.symbol_index");

            SymbolIndex::build(
                files.iter().map(|(path, file)| (path, file.hir())),
                &module_graph,
                vfs.source_roots(),
            )
        };
        let usage_index = {
            let _profile = profile::span("analysis.index.usage.project");

            ProjectUsageIndex::build(
                self.revision,
                files
                    .iter()
                    .map(|(path, file)| (path.as_path(), &file.usage_index)),
                &module_graph,
                &type_database,
            )
        };
        let module_path_index = self.module_path_index(&vfs);
        let (call_hierarchy_symbols, call_hierarchy_calls) = {
            let _profile = profile::span("analysis.index.call_hierarchy");
            let symbols = files
                .iter()
                .filter_map(|(path, file)| {
                    let parsed = file.parsed.as_ref().ok()?;

                    Some((path.clone(), call_hierarchy_symbols_parsed_source(parsed)))
                })
                .collect::<BTreeMap<_, _>>();
            let calls = files
                .iter()
                .filter_map(|(path, file)| {
                    let parsed = file.parsed.as_ref().ok()?;

                    Some((path.clone(), call_hierarchy_calls_parsed_source(parsed)))
                })
                .collect::<BTreeMap<_, _>>();

            (symbols, calls)
        };
        self.refresh_last_good_files(&files);

        let snapshot = AnalysisSnapshot {
            input_errors: self.input_errors.clone(),
            revision: self.revision,
            config: self.config.clone(),
            vfs,
            asset_roots: self.asset_roots.clone(),
            asset_index: self.asset_index.clone(),
            files,
            require_path_separator: self.require_path_separator,
            module_path_index,
            builtin_catalogs: self.builtin_catalogs.clone(),
            module_graph: Arc::new(module_graph),
            type_database: Arc::new(type_database),
            symbol_index: Arc::new(symbol_index),
            usage_index: Arc::new(usage_index),
            last_good_files: self.last_good_files.borrow().clone(),
            completion_adaptive_stats: self.completion_adaptive_stats.clone(),
            completion_sequence_ranking: self.completion_sequence_ranking.clone(),
            completion_multi_token: self.completion_multi_token.clone(),
            semantic_facts: Arc::new(Mutex::new(BTreeMap::new())),
            degraded_facts: Arc::new(Mutex::new(BTreeMap::new())),
            completion_cache: Arc::clone(&self.completion_cache),
            completion_cache_stats: Arc::clone(&self.completion_cache_stats),
            definition_cache: Arc::clone(&self.definition_cache),
            shader_lint_cache: Arc::new(OnceLock::new()),
            project_diagnostics_cache: Arc::new(Mutex::new(BTreeMap::new())),
            reverse_reference_index: Arc::new(OnceLock::new()),
            call_hierarchy_symbols,
            call_hierarchy_calls,
        };

        let mut snapshot_cache = self
            .snapshot_cache
            .lock()
            .expect("analysis snapshot cache lock poisoned");
        if snapshot_cache
            .as_ref()
            .is_none_or(|cached| cached.revision <= self.revision)
        {
            snapshot_cache.replace(CachedAnalysisSnapshot {
                revision: self.revision,
                snapshot: snapshot.clone(),
            });
        }

        snapshot
    }

    pub fn cached_snapshot(&self) -> Option<AnalysisSnapshot> {
        self.snapshot_cache
            .lock()
            .expect("analysis snapshot cache lock poisoned")
            .as_ref()
            .filter(|cached| cached.revision == self.revision)
            .map(|cached| cached.snapshot.clone())
    }

    pub(crate) fn detached_clone_for_ephemeral_analysis(&self) -> Self {
        let mut detached = self.clone();
        // Keep the VFS clone intact: its source-version allocator is shared by
        // one VFS lineage, so coordinator and ephemeral edits cannot assign
        // the same version to different effective sources.
        detached.completion_cache = Arc::new(Mutex::new(BTreeMap::new()));
        detached.completion_cache_stats = Arc::new(Mutex::new(CompletionCacheStats::default()));
        detached.definition_cache = Arc::new(Mutex::new(BTreeMap::new()));
        detached.module_path_index_cache = Arc::new(Mutex::new(None));
        detached.snapshot_cache = Arc::new(Mutex::new(None));
        detached
    }

    pub fn file_paths(&self) -> Vec<PathBuf> {
        self.vfs.paths().into_iter().collect()
    }

    pub fn warm_file(&self, path: impl AsRef<Path>, priority: AnalysisWarmPriority) -> bool {
        let span = match priority {
            AnalysisWarmPriority::High => "analysis.warm.file.high",
            AnalysisWarmPriority::Low => "analysis.warm.file.low",
        };
        let _profile = profile::span_for_path(span, path.as_ref());

        let path = normalize_path(path.as_ref());
        let Some(file) = self.current_analysis_file(&path) else {
            return false;
        };
        self.publish_inline_statistical_scope(&path, &file);
        true
    }

    pub fn adopt_warmed_file(&mut self, warmed: &AnalysisHost, path: impl AsRef<Path>) -> bool {
        if self.revision != warmed.revision
            || self.config != warmed.config
            || self.require_path_separator != warmed.require_path_separator
            || self.module_path_workspaces != warmed.module_path_workspaces
            || self.indexed_module_paths != warmed.indexed_module_paths
            || self.asset_roots != warmed.asset_roots
            || self.builtin_catalogs != warmed.builtin_catalogs
            || self.vfs.options() != warmed.vfs.options()
            || self.inline_statistical_enabled != warmed.inline_statistical_enabled
        {
            return false;
        }

        let path = normalize_path(path.as_ref());
        let Some(source) = self.vfs.source(&path) else {
            return false;
        };
        let Some(kind) = self.vfs.kind(&path) else {
            return false;
        };
        let (cached, parsed_files) = {
            let cache = warmed.cache.borrow();
            let Some(cached) = cache.files.get(&path).cloned() else {
                return false;
            };
            let parsed_files = cache
                .parsed_files
                .iter()
                .filter(|(key, _)| key.path == path)
                .map(|(key, parsed)| (key.clone(), parsed.clone()))
                .collect::<Vec<_>>();

            (cached, parsed_files)
        };

        if cached.source != source
            || vfs_source_kind(cached.kind) != kind
            || warmed.vfs.source(&path) != Some(source)
            || warmed.vfs.kind(&path) != Some(kind)
        {
            return false;
        }

        let parsed = cached.file.parsed.is_ok();
        {
            let mut cache = self.cache.borrow_mut();
            cache.update_dependencies(&path, &cached.file);
            cache.files.insert(path.clone(), cached);
            cache.parsed_files.extend(parsed_files);
        }

        // Invalid edits keep the previous valid facts. The warmed worker
        // publishes new facts only for a valid current source, so adoption can
        // copy immutable/Arc-backed values without walking a ScopeMap on the
        // coordinator thread or mutating the shared last-good semantic cache.
        if self.inline_statistical_enabled && parsed {
            if let Some(scope) = warmed
                .inline_statistical_scopes
                .borrow()
                .get(&path)
                .cloned()
            {
                insert_statistical_scope(
                    Arc::make_mut(&mut self.inline_statistical_scopes.borrow_mut()),
                    path,
                    scope,
                );
            }
        }

        true
    }

    pub fn warm_related_paths(&self, path: impl AsRef<Path>) -> Vec<PathBuf> {
        let path = normalize_path(path.as_ref());
        let cache = self.cache.borrow();
        let mut related = cache
            .dependencies
            .get(&path)
            .into_iter()
            .flatten()
            .chain(cache.importers.get(&path).into_iter().flatten())
            .cloned()
            .collect::<BTreeSet<_>>();

        related.remove(&path);
        related.into_iter().collect()
    }

    pub fn cache_stats(&self) -> AnalysisCacheStats {
        let mut stats = self.cache.borrow().stats.clone();
        let completion_stats = self
            .completion_cache_stats
            .lock()
            .expect("completion cache stats lock poisoned");

        stats.completion_context_hits = completion_stats.hits;
        stats.completion_context_misses = completion_stats.misses;
        stats.inline_completion_requests = self
            .inline_completion_stats
            .requests
            .load(std::sync::atomic::Ordering::Relaxed);
        stats.inline_skipped_oversized = self
            .inline_completion_stats
            .skipped_oversized
            .load(std::sync::atomic::Ordering::Relaxed);
        stats.inline_cache_peek_hits = self
            .inline_completion_stats
            .cache_peek_hits
            .load(std::sync::atomic::Ordering::Relaxed);
        stats.inline_cache_peek_misses = self
            .inline_completion_stats
            .cache_peek_misses
            .load(std::sync::atomic::Ordering::Relaxed);
        stats.inline_validation_attempts = self
            .inline_completion_stats
            .validation_attempts
            .load(std::sync::atomic::Ordering::Relaxed);
        stats.inline_candidate_parses = self
            .inline_completion_stats
            .candidate_parses
            .load(std::sync::atomic::Ordering::Relaxed);
        stats.inline_baseline_parses = self
            .inline_completion_stats
            .baseline_parses
            .load(std::sync::atomic::Ordering::Relaxed);
        stats.inline_edited_parse_sufficient = self
            .inline_completion_stats
            .edited_parse_sufficient
            .load(std::sync::atomic::Ordering::Relaxed);
        stats.inline_candidates_returned = self
            .inline_completion_stats
            .candidates_returned
            .load(std::sync::atomic::Ordering::Relaxed);
        stats.inline_returned_after_zero_parses = self
            .inline_completion_stats
            .returned_after_zero_parses
            .load(std::sync::atomic::Ordering::Relaxed);
        stats.inline_returned_after_one_parse = self
            .inline_completion_stats
            .returned_after_one_parse
            .load(std::sync::atomic::Ordering::Relaxed);
        stats.inline_returned_after_two_parses = self
            .inline_completion_stats
            .returned_after_two_parses
            .load(std::sync::atomic::Ordering::Relaxed);
        let statistical_scopes = self.inline_statistical_scopes.borrow();
        stats.inline_statistical_scope_files = statistical_scopes.len();
        stats.inline_statistical_scope_bytes = statistical_scope_map_bytes(&statistical_scopes);

        stats
    }

    pub fn fast_diagnostics(&self, path: impl AsRef<Path>) -> Vec<Diagnostic> {
        let path = normalize_path(path.as_ref());
        let Some(source) = self.vfs.source(&path) else {
            return Vec::new();
        };
        let Some(kind) = self.vfs.kind(&path) else {
            return Vec::new();
        };

        fast_diagnostics_for_source(&path, source, analysis_source_kind(kind), &self.config)
    }

    pub fn current_file_hover(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
    ) -> Option<Hover> {
        let file = self.current_analysis_file(path.as_ref())?;
        let parsed = file.parsed.as_ref().ok()?;
        let semantics = file
            .semantics()
            .expect("parsed file must have semantic facts");

        // Shader argument hovers need the snapshot's shared asset metadata.
        if matches!(
            crate::completion::context::completion_context(&file.source, position),
            Some(crate::completion::context::CompletionContext::StringArgument { callee_parts, .. })
                if callee_parts.last().is_some_and(|method| matches!(method.as_str(), "send" | "sendColor"))
        ) {
            return None;
        }

        // Type imports are resolved by the project database. Returning a
        // current-file `any` here would prevent the caller from falling back
        // to the authoritative snapshot query.
        if !parsed.type_imports.is_empty() {
            return None;
        }

        // Required-module fields need the project module graph and its
        // environment-aware export types. The source-only hover fallback may
        // read the module from disk and infer domain-derived exports as `any`,
        // which would prevent the caller from reaching the authoritative
        // snapshot query. This also applies to locals that copy a module field,
        // such as `local flow = Globals.flow`.
        if let Some(chain) = dotted_chain_at_position(&parsed.source, position) {
            let required = require_bindings_for_parsed_source(parsed);
            let root = chain.parts.first().map(|part| part.text.as_str());
            let copied_from_required_module = root
                .and_then(|name| {
                    SemanticModel::new(parsed, &semantics.environment)
                        .local_initializer_field_chain(name, position)
                })
                .and_then(|parts| parts.into_iter().next())
                .is_some_and(|initializer_root| {
                    required
                        .iter()
                        .any(|binding| binding.alias == initializer_root)
                });

            if root.is_some_and(|root| required.iter().any(|binding| binding.alias == root))
                || copied_from_required_module
            {
                return None;
            }
        }

        if !semantics.environment_diagnostics.is_empty() {
            return None;
        }

        if builtin_constructor_call_contains_position(parsed, position)
            || store_watch_key_reference_at_position(parsed, position).is_some()
        {
            return None;
        }

        if let Some(asset) = asset::asset_at_position(&file.assets, position) {
            return Some(Hover {
                contents: asset.hover_markdown(),
                span: Some(asset.span),
                documentation: None,
                context: Vec::new(),
            });
        }

        let signal_index =
            crate::signal::local_event_index(parsed, &semantics.environment, &semantics.scope);
        if let Some(event) = signal_index.event_at(&parsed.path, position) {
            if signal_index.schema_for_event(event).is_some() {
                return signal_event_hover(&signal_index, &parsed.path, position);
            }

            return None;
        }

        hover_parsed_source_with_environment(parsed, position, &semantics.environment)
    }

    pub fn current_file_completion(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
    ) -> Vec<Completion> {
        let path = path.as_ref();
        let Some(file) = self.current_analysis_file(path) else {
            return Vec::new();
        };

        match &file.parsed {
            Ok(parsed) => {
                let environment = file
                    .environment()
                    .expect("parsed file must have a type environment");

                complete_source_with_parsed_environment_and_ranking_inputs(
                    &file.source,
                    parsed,
                    position,
                    environment,
                    CompletionRankingInputs::from_sources(
                        &[],
                        None,
                        CompletionAdaptiveStats::file_scope_hash(path),
                        &self.completion_sequence_ranking,
                    ),
                )
            }
            Err(_) => {
                if let Some(facts) = self.cached_last_good_semantic_facts(path) {
                    let mut completions =
                        complete_source_with_parsed_environment_and_ranking_inputs(
                            &file.source,
                            &facts.parsed,
                            position,
                            &facts.environment,
                            CompletionRankingInputs::from_sources(
                                &[],
                                None,
                                CompletionAdaptiveStats::file_scope_hash(path),
                                &self.completion_sequence_ranking,
                            ),
                        );
                    if let Some((_, lossy)) =
                        complete_lossy_identifier_source(&file.source, position)
                    {
                        let mut labels = completions
                            .iter()
                            .map(|completion| completion.label.clone())
                            .collect::<BTreeSet<_>>();
                        completions.extend(
                            lossy
                                .into_iter()
                                .filter(|completion| labels.insert(completion.label.clone())),
                        );
                    }
                    completions
                } else if let Some(degraded) = &file.degraded_semantics {
                    complete_source_with_parsed_environment_and_ranking_inputs(
                        &file.source,
                        &degraded.parsed,
                        position,
                        &degraded.environment,
                        CompletionRankingInputs::from_sources(
                            &[],
                            None,
                            CompletionAdaptiveStats::file_scope_hash(path),
                            &self.completion_sequence_ranking,
                        ),
                    )
                } else {
                    degraded_completion_fallback(path, &file.source, position)
                }
            }
        }
    }

    pub fn current_file_field_completion(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
    ) -> Option<Vec<Completion>> {
        let path = path.as_ref();
        let file = self.current_analysis_file(path)?;
        let ranking = || {
            CompletionRankingInputs::from_sources(
                &[],
                Some(&self.completion_adaptive_stats),
                CompletionAdaptiveStats::file_scope_hash(path),
                &self.completion_sequence_ranking,
            )
        };

        match &file.parsed {
            Ok(parsed) => {
                if !parsed.type_imports.is_empty() {
                    return None;
                }

                complete_current_file_field_access_with_environment_and_ranking_inputs(
                    &file.source,
                    parsed,
                    position,
                    file.environment()
                        .expect("parsed file must have a type environment"),
                    ranking(),
                )
            }
            Err(_) => self
                .cached_last_good_semantic_facts(path)
                .and_then(|facts| {
                    complete_current_file_field_access_with_environment_and_ranking_inputs(
                        &file.source,
                        &facts.parsed,
                        position,
                        &facts.environment,
                        ranking(),
                    )
                })
                .or_else(|| {
                    let degraded = file.degraded_semantics.as_ref()?;
                    if !degraded.parsed.type_imports.is_empty() {
                        return None;
                    }

                    complete_current_file_field_access_with_environment_and_ranking_inputs(
                        &file.source,
                        &degraded.parsed,
                        position,
                        &degraded.environment,
                        ranking(),
                    )
                })
                .or_else(|| complete_current_file_lua_field_access(&file.source, position)),
        }
    }

    fn cached_last_good_semantic_facts(&self, path: &Path) -> Option<DegradedSemanticFacts> {
        self.snapshot_cache
            .lock()
            .expect("analysis snapshot cache lock poisoned")
            .as_ref()
            .and_then(|cached| cached.snapshot.last_good_semantic_facts(path))
    }

    pub fn current_file_inlay_hints(
        &self,
        path: impl AsRef<Path>,
        range: Option<SourceSpan>,
    ) -> Vec<InlayHint> {
        let Some(file) = self.current_analysis_file(path.as_ref()) else {
            return Vec::new();
        };
        let (parsed, base_environment) = match &file.parsed {
            Ok(parsed) => (
                parsed,
                file.environment()
                    .expect("parsed file must have a type environment")
                    .clone(),
            ),
            Err(_) => {
                let Some(degraded) = &file.degraded_semantics else {
                    return Vec::new();
                };

                (&degraded.parsed, degraded.environment.clone())
            }
        };
        let previous_snapshot = self
            .snapshot_cache
            .lock()
            .expect("analysis snapshot cache lock poisoned")
            .as_ref()
            .map(|cached| cached.snapshot.clone());
        let environment = if let Some(snapshot) = previous_snapshot {
            let project_inference = snapshot.project_local_inference(parsed);

            snapshot.project_type_environment_for_parsed(
                parsed,
                base_environment,
                &project_inference,
            )
        } else {
            base_environment
        };

        inlay_hints_parsed_source_with_environment_in_range(parsed, &environment, range)
    }

    pub fn document_links(&self, path: impl AsRef<Path>) -> Vec<ModuleLink> {
        self.current_analysis_file(path.as_ref())
            .map(|file| file.require_edges.clone())
            .unwrap_or_default()
    }

    pub fn document_highlights(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
    ) -> Vec<DocumentHighlight> {
        self.current_analysis_file(path.as_ref())
            .and_then(|file| {
                document_highlights_parsed_source(file.parsed.as_ref().ok()?, position).ok()
            })
            .unwrap_or_default()
    }

    pub fn document_symbols(&self, path: impl AsRef<Path>) -> Vec<DocumentSymbol> {
        self.current_analysis_file(path.as_ref())
            .and_then(|file| {
                Some(document_symbols_parsed_source_with_hir(
                    file.parsed.as_ref().ok()?,
                    file.hir()?,
                ))
            })
            .unwrap_or_default()
    }

    pub fn selection_ranges(
        &self,
        path: impl AsRef<Path>,
        positions: &[SourcePosition],
    ) -> Vec<Option<SelectionRange>> {
        let Some(file) = self.current_analysis_file(path.as_ref()) else {
            return Vec::new();
        };
        let file_span = file.parsed.as_ref().ok().and_then(|parsed| parsed.span);

        selection_ranges_source(&file.source, file_span, positions)
    }

    pub fn folding_ranges(&self, path: impl AsRef<Path>) -> Vec<FoldingRange> {
        let Some(file) = self.current_analysis_file(path.as_ref()) else {
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

    pub fn semantic_tokens(&self, path: impl AsRef<Path>) -> Vec<SemanticToken> {
        let path = normalize_path(path.as_ref());
        let Some(file) = self.current_analysis_file(&path) else {
            return Vec::new();
        };

        match &file.parsed {
            Ok(parsed) => semantic_tokens_parsed_source(parsed).unwrap_or_default(),
            Err(_) => degraded_semantic_tokens(&path, &file),
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    fn bump_revision(&mut self) {
        self.revision = self.revision.saturating_add(1);
    }

    fn bump_module_path_revision(&mut self) {
        self.module_path_revision = self.module_path_revision.saturating_add(1);
    }

    fn has_module_path(&self, path: &Path) -> bool {
        self.indexed_module_paths.contains(path) || self.vfs.file_id(path).is_some()
    }

    fn module_path_index(&self, vfs: &VfsSnapshot) -> Arc<ModulePathIndex> {
        let mut cache = self
            .module_path_index_cache
            .lock()
            .expect("module path index cache lock poisoned");

        if let Some(cached) = cache
            .as_ref()
            .filter(|cached| cached.revision == self.module_path_revision)
        {
            return Arc::clone(&cached.index);
        }

        let index = Arc::new(ModulePathIndex::build(
            &self.module_path_workspaces,
            self.indexed_module_paths
                .iter()
                .map(PathBuf::as_path)
                .chain(vfs.files().map(|file| file.path())),
            vfs.source_roots(),
            self.require_path_separator,
        ));

        cache.replace(CachedModulePathIndex {
            revision: self.module_path_revision,
            index: Arc::clone(&index),
        });

        index
    }

    /// The cached analysis for `path`, only when it was built from exactly the
    /// current VFS source and kind. Never parses, builds, or records cache
    /// statistics, so inline completion can use parsed facts when a warm-up
    /// already produced them without ever paying for a build itself. Local
    /// evaluator uses it to measure opportunistically available parse facts.
    #[cfg(any(test, feature = "test-fixture"))]
    fn peek_current_analysis_file(&self, path: &Path) -> Option<AnalysisFile> {
        let path = normalize_path(path);
        let source = self.vfs.source(&path)?;
        let kind = analysis_source_kind(self.vfs.kind(&path)?);
        let cache = self.cache.borrow();
        let cached = cache.files.get(&path)?;

        (cached.source == source && cached.kind == kind).then(|| cached.file.clone())
    }

    fn peek_current_parse_errors(&self, path: &Path) -> Option<Vec<Diagnostic>> {
        let path = normalize_path(path);
        let source = self.vfs.source(&path)?;
        let kind = self.vfs.kind(&path).map(analysis_source_kind)?;
        let cache = self.cache.borrow();
        let cached = cache.files.get(&path)?;
        if cached.source != source || cached.kind != kind {
            return None;
        }
        Some(
            cached
                .file
                .parsed
                .as_ref()
                .err()
                .cloned()
                .unwrap_or_default(),
        )
    }

    fn peek_current_statistical_scope(
        &self,
        path: &Path,
    ) -> Option<crate::inline_completion::statistical::StatisticalScopeFacts> {
        if !self.inline_statistical_enabled {
            return None;
        }
        let path = normalize_path(path);
        let source = self.vfs.source(&path)?;
        if analysis_source_kind(self.vfs.kind(&path)?) != AnalysisSourceKind::Tua
            || !crate::inline_completion::within_inline_validation_parse_cap(
                AnalysisSourceKind::Tua,
                source.len(),
            )
        {
            return None;
        }
        // Scope extraction happens only while publishing successful analysis,
        // never on the request path. Facts before the first changed character
        // retain their exact spans. Beyond that boundary the predictor treats
        // bare identifiers as conservative bound slots, so forward typing does
        // not trigger analysis or reuse stale declaration classifications.
        let facts = self.inline_statistical_scopes.borrow();
        Some(if let Some(entry) = facts.get(&path) {
            let common_prefix_bytes = if source == entry.source.as_ref() {
                source.len()
            } else {
                source
                    .chars()
                    .zip(entry.source.chars())
                    .take_while(|(current, previous)| current == previous)
                    .map(|(character, _)| character.len_utf8())
                    .sum()
            };
            entry
                .facts
                .clone()
                .restricted_to_common_prefix(common_prefix_bytes)
        } else {
            // Missing analysis facts are not permission to treat identifiers
            // as globals. The predictor classifies every bare identifier in
            // this cold source as a bound slot and therefore cannot emit a
            // project-local name that is absent from the current context.
            crate::inline_completion::statistical::StatisticalScopeFacts::default()
                .restricted_to_common_prefix(0)
        })
    }

    /// Whether the current source parsed, from the cache only. `None` when the
    /// cache has no entry for the current source.
    #[cfg(any(test, feature = "test-fixture"))]
    pub(crate) fn peek_current_source_parses(&self, path: impl AsRef<Path>) -> Option<bool> {
        self.peek_current_analysis_file(path.as_ref())
            .map(|file| file.parsed.is_ok())
    }

    /// LÖVE callbacks declared in the current file, from cached parsed facts
    /// only. `None` means the cache has no entry for the current source, so
    /// duplicates cannot be ruled out. A file whose current source does not
    /// parse uses its recovered (degraded) parse when one exists.
    #[cfg(any(test, feature = "test-fixture"))]
    pub(crate) fn peek_declared_love_callbacks(
        &self,
        path: impl AsRef<Path>,
    ) -> Option<BTreeSet<String>> {
        let path = normalize_path(path.as_ref());
        let source = self.vfs.source(&path)?;
        let kind = self.vfs.kind(&path).map(analysis_source_kind)?;
        let cache = self.cache.borrow();
        let cached = cache.files.get(&path)?;
        if cached.source != source || cached.kind != kind {
            return None;
        }
        let parsed = match &cached.file.parsed {
            Ok(parsed) => parsed,
            Err(_) => &cached.file.degraded_semantics.as_ref()?.parsed,
        };

        Some(
            crate::love_callback::declared_love_callbacks(parsed)
                .into_iter()
                .map(str::to_owned)
                .collect(),
        )
    }

    /// Capture the immutable, source-local inputs for inline completion. This
    /// never builds analysis or takes a snapshot; cached parse errors are used
    /// only when an exact current-source entry already exists.
    pub fn prepare_local_inline_completion(
        &self,
        path: impl AsRef<Path>,
    ) -> Option<crate::PreparedInlineCompletion> {
        self.prepare_local_inline_completion_with_project(path, false)
    }

    pub fn prepare_local_inline_completion_with_project(
        &self,
        path: impl AsRef<Path>,
        include_project: bool,
    ) -> Option<crate::PreparedInlineCompletion> {
        self.prepare_local_inline_completion_with_sources(path, include_project, false)
    }

    pub fn prepare_local_inline_completion_with_sources(
        &self,
        path: impl AsRef<Path>,
        include_project: bool,
        include_statistical: bool,
    ) -> Option<crate::PreparedInlineCompletion> {
        let path = normalize_path(path.as_ref());
        let source = self.vfs.source(&path)?.to_owned();
        let kind = self.vfs.kind(&path).map(analysis_source_kind)?;
        let baseline_parse_errors = self.peek_current_parse_errors(&path);
        self.inline_completion_stats
            .requests
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let counter = if baseline_parse_errors.is_some() {
            &self.inline_completion_stats.cache_peek_hits
        } else {
            &self.inline_completion_stats.cache_peek_misses
        };
        counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let project = (include_project || include_statistical).then(|| {
            // Capture the live epoch before any immutable project value. A
            // concurrent generation change or clear after this point makes
            // the prepared request stale instead of blessing the old corpus
            // with the new epoch.
            let eligibility_epoch = self.project_pattern_eligibility.epoch();
            crate::inline_completion::PreparedProjectCompletion {
                corpus: Arc::clone(&self.project_pattern_corpus),
                versions: self.vfs.source_versions(),
                eligibility: Arc::clone(&self.project_pattern_eligibility),
                generation: self.project_pattern_eligibility.generation(),
                eligibility_epoch,
                limits: self.project_pattern_limits,
            }
        });
        let statistical_scope = include_statistical
            .then(|| self.peek_current_statistical_scope(&path))
            .flatten();
        Some(crate::PreparedInlineCompletion {
            path,
            source,
            kind,
            include_ecs: self.config.ecs.enabled,
            baseline_parse_errors,
            project,
            statistical_scope,
            stats: std::sync::Arc::clone(&self.inline_completion_stats),
        })
    }

    /// Convenience form of [`Self::prepare_local_inline_completion`] for
    /// callers that already execute directly on the host.
    pub fn local_inline_completion(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
        options: &crate::InlineCompletionOptions,
        budget: &crate::InlineCompletionBudget,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Vec<crate::InlineCompletionCandidate> {
        let Some(prepared) = self.prepare_local_inline_completion_with_sources(
            path,
            options.project,
            options.statistical,
        ) else {
            return Vec::new();
        };
        prepared.complete(position, options, budget, cancelled)
    }

    /// Capture one file's immutable corpus-build input from the effective VFS
    /// source. This copies source text but never parses or loads a project
    /// model, so the returned value can be moved to a background worker.
    pub fn prepare_project_pattern_file(
        &self,
        path: impl AsRef<Path>,
    ) -> Option<crate::PatternCorpusBuildInput> {
        let path = normalize_path(path.as_ref());
        let kind = analysis_source_kind(self.vfs.kind(&path)?);
        let include_exact = self.project_pattern_exact_enabled.unwrap_or(true);
        if !include_exact && (!self.inline_statistical_enabled || kind != AnalysisSourceKind::Tua) {
            return None;
        }
        Some(crate::PatternCorpusBuildInput::new(
            path.clone(),
            self.vfs.source(&path)?.to_owned(),
            kind,
            self.vfs.source_version(&path)?,
            include_exact,
            self.inline_statistical_enabled,
        ))
    }

    /// Source size without cloning the source. Background scheduling uses
    /// this to reject oversized files before preparing owned build input.
    pub fn project_pattern_source_bytes(&self, path: impl AsRef<Path>) -> Option<usize> {
        self.vfs.source(path).map(str::len)
    }

    /// Immediately invalidate project-pattern queries for older eligibility
    /// inputs. Rebuilding remains asynchronous; no table work occurs here.
    pub fn set_project_pattern_eligibility_generation(&mut self, generation: u64) {
        self.project_pattern_eligibility.set_generation(generation);
    }

    pub fn set_project_pattern_limits(&mut self, limits: crate::PatternCorpusLimits) {
        self.project_pattern_limits = limits;
    }

    pub fn project_pattern_corpus_stats(&self) -> crate::PatternCorpusStats {
        self.project_pattern_corpus.stats()
    }

    pub fn project_pattern_corpus(&self) -> Arc<crate::ProjectPatternCorpus> {
        Arc::clone(&self.project_pattern_corpus)
    }

    /// Replace the published corpus with an empty value immediately, while
    /// returning ownership of the retired storage for off-coordinator drop.
    pub fn clear_project_pattern_corpus(&mut self) -> Option<crate::RetiredProjectPatternCorpus> {
        self.project_pattern_eligibility.invalidate();
        if self.project_pattern_corpus.stats().files == 0
            && self.project_pattern_corpus.stats().overlay_files == 0
        {
            return None;
        }
        let empty = Arc::new(crate::ProjectPatternCorpus::empty(
            self.project_pattern_eligibility.generation(),
            Some(self.vfs.lineage()),
        ));
        let retired = std::mem::replace(&mut self.project_pattern_corpus, empty);
        Some(crate::RetiredProjectPatternCorpus(retired))
    }

    pub fn prepare_project_pattern_tombstone(
        &self,
        path: impl AsRef<Path>,
    ) -> crate::PatternCorpusTombstone {
        let path = normalize_path(path.as_ref());
        crate::PatternCorpusTombstone {
            expected_version: self.vfs.source_version(&path),
            path,
            lineage: self.vfs.lineage(),
        }
    }

    /// Adopt independently validated files into a new immutable corpus value.
    /// A generation mismatch rejects the whole batch; a stale file rejects
    /// only that file so useful work from the same batch is retained.
    pub fn adopt_project_pattern_batch(
        &mut self,
        generation: u64,
        entries: impl IntoIterator<Item = crate::PatternCorpusBatchEntry>,
    ) -> crate::CorpusAdoptionResult {
        let _profile = profile::span("inline.index.adopt_core");
        let mut result = crate::CorpusAdoptionResult::default();
        if generation != self.project_pattern_eligibility.generation() {
            result.generation_rejected = true;
            result.entries = entries
                .into_iter()
                .map(|entry| crate::CorpusEntryAdoption {
                    path: normalize_path(match &entry {
                        crate::PatternCorpusBatchEntry::File(outcome) => outcome.path(),
                        crate::PatternCorpusBatchEntry::Tombstone(tombstone) => tombstone.path(),
                    }),
                    status: crate::CorpusEntryAdoptionStatus::GenerationMismatch,
                })
                .collect();
            result
                .entries
                .sort_by(|left, right| left.path.cmp(&right.path));
            return result;
        }
        let lineage = self.vfs.lineage();
        let corpus = if self.project_pattern_corpus.eligibility_generation() == generation
            && self.project_pattern_corpus.lineage() == Some(lineage)
        {
            (*self.project_pattern_corpus).clone()
        } else {
            crate::ProjectPatternCorpus::empty(generation, Some(lineage))
        };
        let mut mutations = Vec::new();
        for entry in entries {
            match entry {
                crate::PatternCorpusBatchEntry::Tombstone(tombstone) => {
                    if tombstone.lineage != lineage {
                        result.lineage_rejected = true;
                        result.stale += 1;
                        result.entries.push(crate::CorpusEntryAdoption {
                            path: tombstone.path,
                            status: crate::CorpusEntryAdoptionStatus::WrongLineage,
                        });
                    } else if self.vfs.source_version(&tombstone.path) != tombstone.expected_version
                    {
                        result.stale += 1;
                        result.entries.push(crate::CorpusEntryAdoption {
                            path: tombstone.path,
                            status: crate::CorpusEntryAdoptionStatus::Stale,
                        });
                    } else {
                        mutations.push(
                            crate::inline_completion::ValidatedCorpusMutation::Tombstone {
                                path: tombstone.path,
                                skipped: false,
                            },
                        );
                    }
                }
                crate::PatternCorpusBatchEntry::File(outcome) => {
                    let path = normalize_path(outcome.path());
                    if outcome.version().lineage() != lineage {
                        result.lineage_rejected = true;
                        result.stale += 1;
                        result.entries.push(crate::CorpusEntryAdoption {
                            path,
                            status: crate::CorpusEntryAdoptionStatus::WrongLineage,
                        });
                        continue;
                    }
                    if self.vfs.source_version(&path) != Some(outcome.version()) {
                        result.stale += 1;
                        result.entries.push(crate::CorpusEntryAdoption {
                            path,
                            status: crate::CorpusEntryAdoptionStatus::Stale,
                        });
                        continue;
                    }
                    match outcome {
                        crate::PatternCorpusBuildOutcome::Built(build) => {
                            mutations.push(
                                crate::inline_completion::ValidatedCorpusMutation::File(build),
                            );
                        }
                        crate::PatternCorpusBuildOutcome::Skipped { .. } => {
                            mutations.push(
                                crate::inline_completion::ValidatedCorpusMutation::Tombstone {
                                    path,
                                    skipped: true,
                                },
                            );
                        }
                    }
                }
            }
        }
        if mutations.is_empty() {
            result
                .entries
                .sort_by(|left, right| left.path.cmp(&right.path));
            result.needs_remerge = corpus.needs_remerge(self.project_pattern_limits);
            return result;
        }
        let (corpus, admissions) = corpus.with_batch(mutations, self.project_pattern_limits);
        for admission in &admissions {
            match admission.status {
                crate::CorpusEntryAdoptionStatus::Adopted => result.adopted += 1,
                crate::CorpusEntryAdoptionStatus::Tombstoned => result.tombstoned += 1,
                crate::CorpusEntryAdoptionStatus::Skipped => result.skipped += 1,
                crate::CorpusEntryAdoptionStatus::OverBudget => result.over_budget += 1,
                crate::CorpusEntryAdoptionStatus::FileLimit => result.file_limit += 1,
                crate::CorpusEntryAdoptionStatus::Stale
                | crate::CorpusEntryAdoptionStatus::GenerationMismatch
                | crate::CorpusEntryAdoptionStatus::WrongLineage => {}
            }
        }
        result.entries.extend(admissions);
        result
            .entries
            .sort_by(|left, right| left.path.cmp(&right.path));
        result.needs_remerge = corpus.needs_remerge(self.project_pattern_limits);
        self.project_pattern_corpus = Arc::new(corpus);
        result
    }

    pub fn tombstone_project_pattern_file(
        &mut self,
        generation: u64,
        path: impl AsRef<Path>,
    ) -> bool {
        if generation != self.project_pattern_eligibility.generation() {
            return false;
        }
        let tombstone = self.prepare_project_pattern_tombstone(path);
        let result = self.adopt_project_pattern_batch(
            generation,
            [crate::PatternCorpusBatchEntry::Tombstone(tombstone)],
        );
        !result.generation_rejected && !result.lineage_rejected && result.stale == 0
    }

    pub fn prepare_project_pattern_remerge(&self) -> Option<crate::PreparedPatternCorpusRemerge> {
        let _profile = profile::span("inline.index.prepare_remerge");
        if self.project_pattern_corpus.eligibility_generation()
            != self.project_pattern_eligibility.generation()
        {
            return None;
        }
        crate::ProjectPatternCorpus::prepare_remerge(
            Arc::clone(&self.project_pattern_corpus),
            self.vfs.lineage(),
        )
    }

    pub fn adopt_project_pattern_remerge(
        &mut self,
        outcome: crate::PatternCorpusRemergeOutcome,
    ) -> bool {
        self.try_adopt_project_pattern_remerge(outcome).is_ok()
    }

    /// Adopt a worker-built remerge without releasing either accepted old
    /// storage or a rejected worker result on the coordinator thread.
    pub fn try_adopt_project_pattern_remerge(
        &mut self,
        outcome: crate::PatternCorpusRemergeOutcome,
    ) -> Result<crate::RetiredProjectPatternCorpus, crate::PatternCorpusRemergeOutcome> {
        let _profile = profile::span("inline.index.adopt_remerge_core");
        if outcome.generation() != self.project_pattern_eligibility.generation()
            || outcome.lineage() != self.vfs.lineage()
            || !Arc::ptr_eq(outcome.source(), &self.project_pattern_corpus)
            || outcome
                .versions()
                .iter()
                .any(|(path, version)| self.vfs.source_version(path) != Some(*version))
            || outcome.merged_bytes() > self.project_pattern_limits.max_bytes
        {
            return Err(outcome);
        }
        let retired = crate::RetiredProjectPatternCorpus(Arc::clone(outcome.source()));
        let merged = outcome.into_merged();
        self.project_pattern_corpus = Arc::new(merged);
        Ok(retired)
    }

    /// Query only immutable corpus state plus effective VFS versions/source.
    /// No analysis, project model, filesystem, or snapshot work is performed.
    pub fn project_pattern_matches(
        &self,
        path: impl AsRef<Path>,
        prefix: &str,
        active_line: usize,
        max_results: usize,
        budget: &crate::InlineCompletionBudget,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Vec<crate::PatternCorpusMatch> {
        let path = normalize_path(path.as_ref());
        let Some(source) = self.vfs.source(&path) else {
            return Vec::new();
        };
        self.project_pattern_corpus.query(
            crate::inline_completion::PatternCorpusQuery {
                generation: self.project_pattern_eligibility.generation(),
                active_path: &path,
                active_source: source,
                prefix,
                active_line,
                max_results,
                max_active_source_bytes: self.project_pattern_limits.max_active_source_bytes,
                max_query_patterns: self.project_pattern_limits.max_query_patterns,
                max_query_prefix_bytes: self.project_pattern_limits.max_query_prefix_bytes,
                max_related_targets: self.project_pattern_limits.max_related_targets,
                rank_same_directory: self.project_pattern_limits.rank_same_directory,
                rest_of_project_only: false,
                budget: *budget,
                cancelled,
            },
            |candidate| self.vfs.source_version(candidate),
        )
    }

    pub fn project_declared_love_callbacks(&self) -> BTreeSet<String> {
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        self.project_pattern_corpus
            .declared_love_callbacks(
                self.project_pattern_eligibility.generation(),
                self.vfs.lineage(),
                self.project_pattern_limits.max_callback_support_checks,
                crate::InlineCompletionBudget::unlimited(),
                &cancelled,
                |path| self.vfs.source_version(path),
            )
            .unwrap_or_default()
    }

    fn current_analysis_file(&self, path: &Path) -> Option<AnalysisFile> {
        let path = normalize_path(path);
        let _profile = profile::span_for_path("analysis.current_file", &path);
        let source = self.vfs.source(&path)?.to_owned();
        let kind = analysis_source_kind(self.vfs.kind(&path)?);
        let source_roots = self.vfs.options().source_roots.clone();
        let output_roots = self.vfs.options().output_roots.clone();
        let known_paths = self.vfs.paths();
        Some(self.cache.borrow_mut().cached_file(
            &path,
            &source,
            kind,
            AnalysisBuildContext {
                config: &self.config,
                source_roots: &source_roots,
                output_roots: &output_roots,
                known_paths: &known_paths,
                require_path_separator: self.require_path_separator,
                asset_roots: &self.asset_roots,
                asset_index: self.asset_index.as_ref(),
            },
        ))
    }

    fn invalidate_all_analysis(&mut self) {
        self.cache.borrow_mut().invalidate_all();
        self.last_good_files.borrow_mut().clear();
        Arc::make_mut(self.inline_statistical_scopes.get_mut()).clear();
        self.clear_query_caches();
    }

    fn clear_query_caches(&self) {
        self.clear_completion_cache();
        self.clear_definition_cache();
    }

    fn clear_completion_cache(&self) {
        self.completion_cache
            .lock()
            .expect("completion cache lock poisoned")
            .clear();
    }

    fn clear_definition_cache(&self) {
        self.definition_cache
            .lock()
            .expect("definition cache lock poisoned")
            .clear();
    }

    fn refresh_last_good_files(&self, files: &BTreeMap<PathBuf, AnalysisFile>) {
        let mut last_good_files = self.last_good_files.borrow_mut();

        last_good_files.retain(|path, _| files.contains_key(path));

        for (path, file) in files {
            let Ok(parsed) = &file.parsed else {
                continue;
            };

            last_good_files.insert(
                path.clone(),
                LastGoodAnalysisFile(Arc::new(LastGoodAnalysisFileData {
                    parsed: parsed.clone(),
                    hir: file.hir().cloned().expect("parsed file must have HIR"),
                    environment: file
                        .environment()
                        .cloned()
                        .expect("parsed file must have a type environment"),
                })),
            );
        }
    }

    fn publish_inline_statistical_scope(&self, path: &Path, file: &AnalysisFile) {
        if !self.inline_statistical_enabled
            || file.kind != AnalysisSourceKind::Tua
            || !crate::inline_completion::within_inline_validation_parse_cap(
                AnalysisSourceKind::Tua,
                file.source.len(),
            )
        {
            return;
        }
        let Ok(parsed) = &file.parsed else {
            return;
        };
        let Some(scope) = file.scope() else {
            return;
        };
        insert_statistical_scope(
            Arc::make_mut(&mut self.inline_statistical_scopes.borrow_mut()),
            path.to_path_buf(),
            InlineStatisticalScopeEntry {
                facts:
                    crate::inline_completion::statistical::StatisticalScopeFacts::from_parsed_scope(
                        parsed, scope,
                    ),
                source: Arc::from(parsed.source.as_str()),
                // Scheduling priority is not an ownership signal: related
                // dependencies are also warmed at High priority. Only an
                // editor overlay is protected from bounded-cache eviction.
                pinned_open_document: self.vfs.is_open(path),
            },
        );
    }
}

fn statistical_scope_map_bytes(scopes: &BTreeMap<PathBuf, InlineStatisticalScopeEntry>) -> usize {
    std::mem::size_of_val(scopes)
        + scopes
            .iter()
            .map(|(path, entry)| {
                64 + path.as_os_str().len() + entry.source.len() + entry.facts.approximate_bytes()
            })
            .sum::<usize>()
}

fn insert_statistical_scope(
    scopes: &mut BTreeMap<PathBuf, InlineStatisticalScopeEntry>,
    path: PathBuf,
    entry: InlineStatisticalScopeEntry,
) {
    let protected = path.clone();
    scopes.insert(path, entry);
    while scopes.len() > crate::inline_completion::statistical::MAX_SCOPE_FACT_FILES
        || statistical_scope_map_bytes(scopes)
            > crate::inline_completion::statistical::MAX_SCOPE_FACT_BYTES
    {
        let Some(last) = scopes
            .iter()
            .rev()
            .find(|(candidate, entry)| **candidate != protected && !entry.pinned_open_document)
            .map(|(candidate, _)| candidate.clone())
            .or_else(|| {
                scopes
                    .keys()
                    .rev()
                    .find(|candidate| **candidate != protected)
                    .cloned()
            })
            .or_else(|| scopes.keys().next_back().cloned())
        else {
            break;
        };
        scopes.remove(&last);
    }
}

use super::*;

impl AnalysisHost {
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
        self.invalidate_all_analysis();
        self.bump_revision();
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

    pub fn refresh_asset_index_from_disk(&mut self) {
        let roots = analysis_asset_roots(self.source_roots(), &self.asset_roots).to_vec();
        let index = asset::AssetIndex::from_roots(&roots);

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
        let vfs_kind = vfs_source_kind(kind);

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
        self.vfs.set_disk_file(path, source, vfs_kind);
        self.bump_revision();

        invalidated_dependents
    }

    pub fn remove_file(&mut self, path: impl AsRef<Path>) {
        self.cache.borrow_mut().invalidate_file(path.as_ref(), true);
        self.clear_query_caches();
        self.last_good_files
            .borrow_mut()
            .remove(&normalize_path(path.as_ref()));
        self.vfs.remove_file(path);
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
        let vfs_kind = vfs_source_kind(kind);

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
        self.vfs.open_file(path, source, vfs_kind);
        self.bump_revision();
    }

    pub fn update_open_document(&mut self, path: impl Into<PathBuf>, source: impl Into<String>) {
        self.open_document(path, source);
    }

    pub fn close_document(&mut self, path: impl AsRef<Path>) {
        self.cache.borrow_mut().invalidate_file(path.as_ref(), true);
        self.clear_query_caches();
        self.vfs.close_file(path);
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
            )
        };
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
            revision: self.revision,
            config: self.config.clone(),
            vfs,
            asset_roots: self.asset_roots.clone(),
            asset_index: self.asset_index.clone(),
            files,
            require_path_separator: self.require_path_separator,
            builtin_catalogs: self.builtin_catalogs.clone(),
            module_graph,
            type_database,
            symbol_index,
            usage_index,
            last_good_files: self.last_good_files.borrow().clone(),
            completion_adaptive_stats: self.completion_adaptive_stats.clone(),
            completion_sequence_ranking: self.completion_sequence_ranking.clone(),
            completion_multi_token: self.completion_multi_token.clone(),
            semantic_facts: Arc::new(Mutex::new(BTreeMap::new())),
            degraded_facts: Arc::new(Mutex::new(BTreeMap::new())),
            completion_cache: Arc::clone(&self.completion_cache),
            completion_cache_stats: Arc::clone(&self.completion_cache_stats),
            definition_cache: Arc::clone(&self.definition_cache),
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

    pub fn warm_file(&self, path: impl AsRef<Path>, _priority: AnalysisWarmPriority) -> bool {
        self.snapshot().semantic_index(path).is_some()
    }

    pub fn cache_stats(&self) -> AnalysisCacheStats {
        let mut stats = self.cache.borrow().stats.clone();
        let completion_stats = self
            .completion_cache_stats
            .lock()
            .expect("completion cache stats lock poisoned");

        stats.completion_context_hits = completion_stats.hits;
        stats.completion_context_misses = completion_stats.misses;

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

        if !semantics.environment_diagnostics.is_empty() {
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
                if let Some(degraded) = &file.degraded_semantics {
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

    pub fn document_links(&self, path: impl AsRef<Path>) -> Vec<ModuleLink> {
        self.current_analysis_file(path.as_ref())
            .map(|file| file.require_edges)
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
                LastGoodAnalysisFile {
                    parsed: parsed.clone(),
                    hir: file.hir().cloned().expect("parsed file must have HIR"),
                    environment: file
                        .environment()
                        .cloned()
                        .expect("parsed file must have a type environment"),
                },
            );
        }
    }
}

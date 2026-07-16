use super::*;

impl AnalysisQueryCache {
    pub(super) fn cached_file(
        &mut self,
        path: &Path,
        source: &str,
        kind: AnalysisSourceKind,
        context: AnalysisBuildContext<'_>,
    ) -> AnalysisFile {
        let path = normalize_path(path);

        if let Some(cached) = self.files.get(&path) {
            if cached.source == source && cached.kind == kind {
                let file = cached.file.clone();

                self.record_file_hit();
                return file;
            }
        }

        let (parsed, parsed_cache_hit) = self.cached_parsed_file(&path, source, kind);

        self.record_file_rebuild(parsed_cache_hit);

        let file = build_analysis_file(&path, source.to_owned(), kind, context, parsed);
        self.update_dependencies(&path, &file.require_edges);
        self.files.insert(
            path,
            CachedAnalysisFile {
                source: source.to_owned(),
                kind,
                file: file.clone(),
            },
        );

        file
    }

    pub(super) fn cached_parsed_file(
        &mut self,
        path: &Path,
        source: &str,
        kind: AnalysisSourceKind,
    ) -> (Result<ParsedSource, Vec<Diagnostic>>, bool) {
        let key = parsed_cache_key(path, source, kind);

        if let Some(parsed) = self.parsed_files.get(&key) {
            return (parsed.clone(), true);
        }

        self.parsed_files
            .retain(|cached_key, _| cached_key.path != key.path);

        let parsed = {
            let _profile = profile::span_for_path("analysis.parse", path);
            let _trace = profile::diagnostics_span(profile::DiagnosticsTraceCategory::Parse);

            parse_analysis_source(path, source.to_owned(), kind)
        };

        self.parsed_files.insert(key, parsed.clone());

        (parsed, false)
    }

    pub(super) fn invalidate_changed_file(
        &mut self,
        path: &Path,
        source: &str,
        kind: AnalysisSourceKind,
        config: &ProjectConfig,
    ) -> bool {
        let invalidation = self.changed_file_dependency_invalidation(path, source, kind, config);
        let invalidate_dependents = invalidation.invalidates_importers();

        self.stats.require_surface_invalidations += usize::from(invalidation.require_graph);
        self.stats.export_surface_invalidations += usize::from(invalidation.exports);
        self.stats.type_surface_invalidations += usize::from(invalidation.types);

        self.invalidate_file(path, invalidate_dependents);
        invalidate_dependents
    }

    pub(super) fn changed_file_dependency_invalidation(
        &mut self,
        path: &Path,
        source: &str,
        kind: AnalysisSourceKind,
        config: &ProjectConfig,
    ) -> DependencyInvalidation {
        let path = normalize_path(path);

        if self
            .importers
            .get(&path)
            .is_none_or(|importers| importers.is_empty())
        {
            return DependencyInvalidation::default();
        }

        let Some(cached) = self.files.get(&path) else {
            return DependencyInvalidation::all();
        };

        if cached.kind != kind {
            return DependencyInvalidation::all();
        }

        let old_surface = DependencySurface::from_analysis_file(&cached.file);
        let new_require_targets = require_targets_for_source(&path, source);

        let (parsed, parsed_cache_hit) = self.cached_parsed_file(&path, source, kind);
        self.record_parse_cache_lookup(parsed_cache_hit);
        let Ok(parsed) = parsed else {
            return DependencyInvalidation::all();
        };

        let new_surface = DependencySurface::from_parsed(&parsed, config, new_require_targets);

        new_surface.invalidation_from(&old_surface)
    }

    pub(super) fn invalidate_file(&mut self, path: &Path, invalidate_dependents: bool) {
        let path = normalize_path(path);
        let mut invalidated = BTreeSet::new();

        self.invalidate_one(&path, &mut invalidated, false);

        if invalidate_dependents {
            self.invalidate_dependents(&path, &mut invalidated);
        }
    }

    pub(super) fn invalidate_dependents(
        &mut self,
        path: &Path,
        invalidated: &mut BTreeSet<PathBuf>,
    ) {
        if let Some(importers) = self.importers.get(path).cloned() {
            for importer in importers {
                if invalidated.contains(&importer) {
                    continue;
                }

                self.invalidate_one(&importer, invalidated, true);
                self.invalidate_dependents(&importer, invalidated);
            }
        }
    }

    pub(super) fn invalidate_all(&mut self) {
        self.stats.invalidations += self.files.len();
        self.files.clear();
        self.dependencies.clear();
        self.importers.clear();
    }

    pub(super) fn retain_paths(&mut self, live_paths: &BTreeSet<PathBuf>) {
        let removed = self
            .files
            .keys()
            .filter(|path| !live_paths.contains(*path))
            .cloned()
            .collect::<Vec<_>>();

        for path in removed {
            self.invalidate_file(&path, true);
        }

        self.parsed_files
            .retain(|key, _| live_paths.contains(&key.path));
    }

    pub(super) fn invalidate_one(
        &mut self,
        path: &Path,
        invalidated: &mut BTreeSet<PathBuf>,
        dependent: bool,
    ) {
        let path = normalize_path(path);

        if !invalidated.insert(path.clone()) {
            return;
        }

        if self.files.remove(&path).is_some() {
            self.stats.invalidations += 1;
            if dependent {
                self.stats.dependent_invalidations += 1;
            }
        }

        if let Some(dependencies) = self.dependencies.remove(&path) {
            for dependency in dependencies {
                if let Some(importers) = self.importers.get_mut(&dependency) {
                    importers.remove(&path);
                    if importers.is_empty() {
                        self.importers.remove(&dependency);
                    }
                }
            }
        }
    }

    pub(super) fn update_dependencies(&mut self, path: &Path, require_edges: &[ModuleLink]) {
        let path = normalize_path(path);

        if let Some(old_dependencies) = self.dependencies.remove(&path) {
            for dependency in old_dependencies {
                if let Some(importers) = self.importers.get_mut(&dependency) {
                    importers.remove(&path);
                    if importers.is_empty() {
                        self.importers.remove(&dependency);
                    }
                }
            }
        }

        let dependencies = require_edges
            .iter()
            .map(|edge| normalize_path(&edge.target))
            .collect::<BTreeSet<_>>();

        for dependency in &dependencies {
            self.importers
                .entry(dependency.clone())
                .or_default()
                .insert(path.clone());
        }

        self.dependencies.insert(path, dependencies);
    }

    pub(super) fn record_file_hit(&mut self) {
        self.stats.parse_hits += 1;
        self.stats.hir_hits += 1;
        self.stats.scope_hits += 1;
        self.stats.type_environment_hits += 1;
        self.stats.diagnostics_hits += 1;
        self.stats.module_export_hits += 1;
        self.stats.require_edge_hits += 1;
        self.stats.symbol_index_hits += 1;
        self.stats.usage_index_hits += 1;
    }

    pub(super) fn record_file_rebuild(&mut self, parsed_cache_hit: bool) {
        if parsed_cache_hit {
            self.stats.parse_hits += 1;
        } else {
            self.stats.parse_misses += 1;
        }

        self.stats.hir_misses += 1;
        self.stats.scope_misses += 1;
        self.stats.type_environment_misses += 1;
        self.stats.diagnostics_misses += 1;
        self.stats.module_export_misses += 1;
        self.stats.require_edge_misses += 1;
        self.stats.symbol_index_misses += 1;
        self.stats.usage_index_misses += 1;
    }

    pub(super) fn record_parse_cache_lookup(&mut self, parsed_cache_hit: bool) {
        if parsed_cache_hit {
            self.stats.parse_hits += 1;
        } else {
            self.stats.parse_misses += 1;
        }
    }
}

fn build_analysis_file(
    path: &Path,
    source: String,
    kind: AnalysisSourceKind,
    context: AnalysisBuildContext<'_>,
    parsed: Result<ParsedSource, Vec<Diagnostic>>,
) -> AnalysisFile {
    let (semantics, mut semantic_diagnostics) = match &parsed {
        Ok(parsed) => {
            let hir = {
                let _profile = profile::span_for_path("analysis.lower.hir", path);

                lower_parsed_source(parsed)
            };
            let scope = {
                let _profile = profile::span_for_path("analysis.lower.scope", path);

                scope_map_parsed_source(parsed)
            };
            let (environment, diagnostics) = {
                let _profile = profile::span_for_path("analysis.types.environment", path);
                let _trace =
                    profile::diagnostics_span(profile::DiagnosticsTraceCategory::TypeEnvironment);

                build_analysis_type_environment_with_diagnostics(parsed, context.config)
            };

            (
                Some(AnalysisFileSemantics {
                    hir,
                    scope,
                    environment,
                    environment_diagnostics: diagnostics.clone(),
                }),
                diagnostics,
            )
        }
        Err(_) => (None, Vec::new()),
    };
    let degraded_semantics = parsed
        .is_err()
        .then(|| build_degraded_file_semantics(path, &source, kind, context.config))
        .flatten();
    let mut diagnostics = {
        let _profile = profile::span_for_path("analysis.check", path);
        let mut diagnostics_config = context.config.diagnostics.clone();

        diagnostics_config.inline_ignores = false;

        match &parsed {
            Ok(parsed) => {
                let asset_roots = analysis_asset_roots(context.source_roots, context.asset_roots);
                let environment = &semantics
                    .as_ref()
                    .expect("parsed file must have semantic facts")
                    .environment;

                if semantic_diagnostics.is_empty() {
                    semantic_diagnostics.extend(
                        check_parsed_source_with_environment_asset_roots_diagnostics_jit_config_and_asset_index(
                            parsed,
                            environment,
                            asset_roots,
                            &diagnostics_config,
                            &context.config.jit,
                            context.asset_index,
                            &context.config.format.options(),
                        )
                        .err()
                        .unwrap_or_default(),
                    );
                }

                std::mem::take(&mut semantic_diagnostics)
            }
            Err(diagnostics) => {
                let mut diagnostics = diagnostics.clone();

                if let Some(degraded) = &degraded_semantics {
                    diagnostics.extend(
                        check_parsed_source_with_asset_roots_diagnostics_jit_and_ecs_config_and_asset_index(
                            &degraded.parsed,
                            analysis_asset_roots(context.source_roots, context.asset_roots),
                            &diagnostics_config,
                            &context.config.jit,
                            &context.config.ecs,
                            context.asset_index,
                            &context.config.format.options(),
                        )
                        .err()
                        .unwrap_or_default(),
                    );
                }

                diagnostics
            }
        }
    };
    if kind == AnalysisSourceKind::Tua {
        if let Ok(parsed) = &parsed {
            let _profile = profile::span_for_path("analysis.emit.check", path);
            let _trace = profile::diagnostics_span(profile::DiagnosticsTraceCategory::Checker);

            let import_resolver = |specifier: &str| {
                module_name_for_import_specifier(
                    &parsed.path,
                    specifier,
                    context.known_paths,
                    context.source_roots,
                    context.output_roots,
                    &context.config.extension,
                    context.require_path_separator,
                )
            };

            if let Err(mut emit_diagnostics) = crate::emit::emit_lua_with_mode_and_import_resolver(
                parsed,
                context.config.emit.annotation_mode,
                Some(&import_resolver),
            ) {
                diagnostics.append(&mut emit_diagnostics);
            }
        }
    }
    let assets = {
        let _profile = profile::span_for_path("analysis.assets.metadata", path);

        parsed
            .as_ref()
            .ok()
            .map(|parsed| {
                let asset_roots = analysis_asset_roots(context.source_roots, context.asset_roots);

                if let Some(asset_index) = context.asset_index {
                    asset::collect_asset_metadata_with_index(parsed, asset_roots, Some(asset_index))
                } else {
                    asset::collect_asset_metadata(parsed, asset_roots)
                }
            })
            .unwrap_or_default()
    };
    if let Ok(parsed) = &parsed {
        let _profile = profile::span_for_path("analysis.sprite.diagnostics", path);

        diagnostics.extend(crate::sprite::diagnostics(
            path,
            &parsed.sprite_declarations,
            &assets,
        ));
    }
    if context.config.ecs.enabled {
        let _profile = profile::span_for_path("analysis.ecs.diagnostics", path);

        diagnostics.extend(crate::ecs::diagnostics(path, &source));
    }
    let diagnostics = deduplicate_diagnostics(diagnostics);
    let exports = {
        let _profile = profile::span_for_path("analysis.index.exports", path);
        let _trace = profile::diagnostics_span(profile::DiagnosticsTraceCategory::ModuleGraph);

        parsed.as_ref().ok().and_then(|parsed| {
            infer_module_exports_with_optional_environment(
                parsed,
                semantics.as_ref().map(|semantics| &semantics.environment),
            )
        })
    };
    let require_edges = {
        let _profile = profile::span_for_path("analysis.index.requires", path);
        let _trace = profile::diagnostics_span(profile::DiagnosticsTraceCategory::ModuleGraph);

        module_links_tua_source(path, source.clone())
    };
    let symbols = {
        let _profile = profile::span_for_path("analysis.index.symbols", path);

        parsed
            .as_ref()
            .ok()
            .map(|parsed| {
                symbol_entries_for_parsed(
                    parsed,
                    semantics.as_ref().map(|semantics| &semantics.hir),
                    context.source_roots,
                )
            })
            .unwrap_or_default()
    };
    let usage_index = {
        let _profile = profile::span_for_path("analysis.index.usage.file", path);
        Arc::new(FileUsageIndex::build(
            &source,
            parsed.as_ref().ok(),
            semantics.as_ref().map(|semantics| &semantics.environment),
            semantics.as_ref().map(|semantics| &semantics.scope),
        ))
    };

    AnalysisFile {
        source,
        kind,
        parsed,
        semantics,
        degraded_semantics,
        diagnostics,
        exports,
        require_edges,
        assets,
        usage_index,
        symbols,
    }
}

pub(super) fn parse_analysis_source(
    path: &Path,
    source: String,
    kind: AnalysisSourceKind,
) -> Result<ParsedSource, Vec<Diagnostic>> {
    match kind {
        AnalysisSourceKind::Lua => parse_lua_source(path, source),
        AnalysisSourceKind::Tua => parse_tua_source(path, source),
    }
}

pub(super) fn fast_diagnostics_for_source(
    path: &Path,
    source: &str,
    kind: AnalysisSourceKind,
    config: &ProjectConfig,
) -> Vec<Diagnostic> {
    let parsed = match parse_analysis_source(path, source.to_owned(), kind) {
        Ok(parsed) => parsed,
        Err(diagnostics) => {
            return apply_inline_ignores(path, source, diagnostics, &config.diagnostics);
        }
    };
    let (environment, mut diagnostics) =
        build_analysis_type_environment_with_diagnostics(&parsed, config);
    let fast_config = fast_diagnostics_config(&config.diagnostics);
    diagnostics.extend(
        check_parsed_source_with_environment_asset_roots_diagnostics_jit_config_and_asset_index(
            &parsed,
            &environment,
            &[],
            &fast_config,
            &crate::config::JitConfig::default(),
            None,
            &config.format.options(),
        )
        .err()
        .unwrap_or_default(),
    );

    let diagnostics = fast_lane_diagnostics(deduplicate_diagnostics(diagnostics));

    apply_inline_ignores(path, source, diagnostics, &config.diagnostics)
}

pub(super) fn build_analysis_type_environment_with_diagnostics(
    parsed: &ParsedSource,
    config: &ProjectConfig,
) -> (TypeEnvironment, Vec<Diagnostic>) {
    if config.ecs.enabled {
        let (environment, diagnostics) =
            build_type_environment_with_builtin_aliases(parsed, crate::ecs::builtin_aliases());

        (
            environment.with_local_types(crate::ecs::builtin_locals()),
            diagnostics,
        )
    } else {
        build_type_environment_with_diagnostics(parsed)
    }
}

pub(super) fn fast_diagnostics_config(
    config: &crate::config::DiagnosticsConfig,
) -> crate::config::DiagnosticsConfig {
    crate::config::DiagnosticsConfig {
        undefined_variables: config.undefined_variables,
        formatting_lints: config.formatting_lints,
        globals: config.globals.clone(),
        ..crate::config::DiagnosticsConfig::default()
    }
}

pub(super) fn fast_lane_diagnostics(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.code.is_some_and(is_fast_lane_diagnostic_code))
        .collect()
}

pub(super) fn is_fast_lane_diagnostic_code(code: crate::DiagnosticCode) -> bool {
    matches!(
        code,
        crate::DiagnosticCode::SYNTAX_PARSE
            | crate::DiagnosticCode::SYNTAX_COMPOUND_ASSIGNMENT
            | crate::DiagnosticCode::TYPE_UNKNOWN_NAME
            | crate::DiagnosticCode::TYPE_DUPLICATE_ALIAS
            | crate::DiagnosticCode::TYPE_MISMATCH
            | crate::DiagnosticCode::TYPE_ARGUMENT_COUNT
            | crate::DiagnosticCode::TYPE_IMPOSSIBLE_COMPARISON
            | crate::DiagnosticCode::TYPE_UNSUPPORTED_RUNTIME_CHECK
            | crate::DiagnosticCode::TYPE_CLASS
            | crate::DiagnosticCode::DOMAIN_SIGNAL_SCHEMA
            | crate::DiagnosticCode::LINT_UNDEFINED_VARIABLE
            | crate::DiagnosticCode::LINT_FORMAT_TRAILING_WHITESPACE
            | crate::DiagnosticCode::LINT_FORMAT_INDENTATION
            | crate::DiagnosticCode::LINT_FORMAT_KEYWORD_SPACING
            | crate::DiagnosticCode::LINT_FORMAT_TOKEN_SPACING
            | crate::DiagnosticCode::LINT_FORMAT_LINE_WIDTH
    )
}

pub(super) fn require_targets_for_source(path: &Path, source: &str) -> BTreeSet<PathBuf> {
    require_targets_for_links(&module_links_tua_source(path, source.to_owned()))
}

pub(super) fn require_targets_for_links(require_edges: &[ModuleLink]) -> BTreeSet<PathBuf> {
    require_edges
        .iter()
        .map(|edge| normalize_path(&edge.target))
        .collect()
}

pub(super) fn parsed_cache_key(
    path: &Path,
    source: &str,
    kind: AnalysisSourceKind,
) -> ParsedCacheKey {
    ParsedCacheKey {
        path: normalize_path(path),
        kind,
        fingerprint: source_fingerprint(source),
    }
}

pub(super) fn source_fingerprint(source: &str) -> SourceFingerprint {
    let mut hasher = DefaultHasher::new();

    source.hash(&mut hasher);

    SourceFingerprint {
        len: source.len(),
        hash: hasher.finish(),
    }
}

pub(super) fn infer_source_kind(path: &Path, config: &ProjectConfig) -> AnalysisSourceKind {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("lua") => AnalysisSourceKind::Lua,
        Some(extension) if extension == config.extension => AnalysisSourceKind::Tua,
        _ => AnalysisSourceKind::Tua,
    }
}

pub(super) fn vfs_source_kind(kind: AnalysisSourceKind) -> VfsSourceKind {
    match kind {
        AnalysisSourceKind::Lua => VfsSourceKind::Lua,
        AnalysisSourceKind::Tua => VfsSourceKind::Tua,
    }
}

pub(super) fn analysis_source_kind(kind: VfsSourceKind) -> AnalysisSourceKind {
    match kind {
        VfsSourceKind::Lua => AnalysisSourceKind::Lua,
        VfsSourceKind::Tua => AnalysisSourceKind::Tua,
    }
}

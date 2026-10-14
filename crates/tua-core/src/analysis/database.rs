use super::*;
use crate::types::build_type_environment_with_domain_registry;
use crate::DiagnosticCode;

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
        self.update_dependencies(&path, &file);
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

        self.invalidate_file_for_change(path, &invalidation);
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

    fn invalidate_file_for_change(&mut self, path: &Path, invalidation: &DependencyInvalidation) {
        let path = normalize_path(path);
        let mut invalidated = BTreeSet::new();
        self.invalidate_one(&path, &mut invalidated, false);
        if invalidation.require_graph && !invalidation.exports && !invalidation.types {
            return;
        }

        let importers = self.importers.get(&path).cloned().unwrap_or_default();
        for importer in importers {
            let dependency = self
                .type_dependency_uses
                .get(&(importer.clone(), path.clone()));
            let runtime_dependency = dependency.is_none_or(Option::is_none);
            let type_dependency_matches = invalidation.types
                && match (&invalidation.changed_aliases, dependency) {
                    (None, _) | (_, None) | (_, Some(None)) => true,
                    (Some(changed), Some(Some(used))) => !changed.is_disjoint(used),
                };
            if invalidation.exports && runtime_dependency || type_dependency_matches {
                self.invalidate_one(&importer, &mut invalidated, true);
                self.invalidate_dependents(&importer, &mut invalidated);
            }
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
        self.type_dependency_uses.clear();
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
        self.type_dependency_uses
            .retain(|(importer, _), _| importer != &path);
    }

    pub(super) fn update_dependencies(&mut self, path: &Path, file: &AnalysisFile) {
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
        self.type_dependency_uses
            .retain(|(importer, _), _| importer != &path);

        let dependencies = file
            .require_edges
            .iter()
            .map(|edge| normalize_path(&edge.target))
            .collect::<BTreeSet<_>>();

        for dependency in &dependencies {
            self.importers
                .entry(dependency.clone())
                .or_default()
                .insert(path.clone());
        }

        let referenced_names = file
            .parsed
            .as_ref()
            .ok()
            .map(referenced_type_names_for_dependencies)
            .unwrap_or_default();
        for edge in &file.require_edges {
            let target = normalize_path(&edge.target);
            let type_import = file.parsed.as_ref().ok().and_then(|parsed| {
                parsed
                    .type_imports
                    .iter()
                    .find(|import| import.module_span == edge.span)
            });
            let Some(import) = type_import else {
                self.type_dependency_uses
                    .insert((path.clone(), target), None);
                continue;
            };
            let used = self
                .type_dependency_uses
                .entry((path.clone(), target))
                .or_insert_with(|| Some(BTreeSet::new()));
            let Some(used) = used else {
                continue;
            };
            if !import.namespace.is_empty() {
                let prefix = format!("{}.", import.namespace);
                used.extend(
                    referenced_names
                        .iter()
                        .filter_map(|name| name.strip_prefix(&prefix).map(str::to_owned)),
                );
            }
            used.extend(
                import
                    .bindings
                    .iter()
                    .filter(|binding| import.reexport || referenced_names.contains(&binding.local))
                    .map(|binding| binding.imported.clone()),
            );
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
            let (environment, mut diagnostics) = {
                let _profile = profile::span_for_path("analysis.types.environment", path);
                let _trace =
                    profile::diagnostics_span(profile::DiagnosticsTraceCategory::TypeEnvironment);

                build_analysis_type_environment_with_diagnostics(parsed, context.config)
            };
            diagnostics.extend(type_import_collision_diagnostics(parsed, &scope));

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
    let object_pool_index = {
        let _profile = profile::span_for_path("analysis.index.object_pool.file", path);

        Arc::new(
            parsed
                .as_ref()
                .ok()
                .map(|parsed| {
                    crate::object_pool::object_pool_file_index(
                        parsed,
                        &semantics
                            .as_ref()
                            .expect("parsed file must have semantic facts")
                            .environment,
                    )
                })
                .unwrap_or_default(),
        )
    };
    if parsed.is_ok() {
        let _profile = profile::span_for_path("analysis.object_pool.diagnostics", path);
        let graph = crate::object_pool::object_pool_graph([object_pool_index.as_ref()]);

        diagnostics.extend(crate::object_pool::diagnostics_for_graph(&graph, path));
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
    let ecs_index = {
        let _profile = profile::span_for_path("analysis.index.ecs.file", path);

        Arc::new(if context.config.ecs.enabled {
            parsed
                .as_ref()
                .ok()
                .map(|parsed| {
                    crate::ecs::ecs_file_index(
                        parsed,
                        &semantics
                            .as_ref()
                            .expect("parsed file must have semantic facts")
                            .environment,
                    )
                })
                .unwrap_or_default()
        } else {
            crate::ecs::EcsFileIndex::default()
        })
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
        ecs_index,
        object_pool_index,
        symbols,
    }
}

fn type_import_collision_diagnostics(parsed: &ParsedSource, scope: &ScopeMap) -> Vec<Diagnostic> {
    let runtime_names = scope
        .declarations()
        .iter()
        .map(|declaration| declaration.name.as_str())
        .collect::<BTreeSet<_>>();
    parsed
        .type_imports
        .iter()
        .filter(|import| !import.reexport)
        .flat_map(|import| {
            let namespace = (!import.namespace.is_empty())
                .then_some((import.namespace.as_str(), import.namespace_span));
            namespace.into_iter().chain(
                import
                    .bindings
                    .iter()
                    .map(|binding| (binding.local.as_str(), binding.local_span)),
            )
        })
        .filter(|(name, _)| runtime_names.contains(name))
        .map(|(name, span)| {
            Diagnostic::error(
                &parsed.path,
                span.start.line,
                span.start.column,
                format!("type import name `{name}` conflicts with a runtime declaration"),
            )
            .with_code(DiagnosticCode::TYPE_IMPORT_COLLISION)
        })
        .collect()
}

fn referenced_type_names_for_dependencies(parsed: &ParsedSource) -> BTreeSet<String> {
    fn collect(ty: &TuaType, names: &mut BTreeSet<String>) {
        match ty {
            TuaType::Named(name) => {
                names.insert(name.clone());
            }
            TuaType::StringLiteral(_) => {}
            TuaType::Union(types) | TuaType::Tuple(types) => {
                for ty in types {
                    collect(ty, names);
                }
            }
            TuaType::Table(fields) => {
                for field in fields {
                    collect(&field.ty, names);
                }
            }
            TuaType::Array(item) => collect(item, names),
            TuaType::Dictionary { key, value } => {
                collect(key, names);
                collect(value, names);
            }
            TuaType::Function {
                parameters,
                returns,
            } => {
                for parameter in parameters {
                    collect(&parameter.ty, names);
                }
                for ty in returns {
                    collect(ty, names);
                }
            }
        }
    }

    let mut names = BTreeSet::new();
    for ty in parsed
        .type_aliases
        .iter()
        .map(|alias| &alias.ty)
        .chain(
            parsed
                .local_annotations
                .iter()
                .map(|annotation| &annotation.ty),
        )
        .chain(parsed.function_annotations.iter().flat_map(|function| {
            function
                .parameters
                .iter()
                .map(|parameter| &parameter.ty)
                .chain(function.return_type.iter())
        }))
    {
        collect(ty, &mut names);
    }
    names
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
        let (environment, diagnostics) = build_type_environment_with_domain_registry(
            parsed,
            crate::semantic_domains::BuiltinDomainRegistry::with_ecs(true),
        );

        (environment, diagnostics)
    } else {
        build_type_environment_with_diagnostics(parsed)
    }
}

pub(super) fn fast_diagnostics_config(
    config: &crate::config::DiagnosticsConfig,
) -> crate::config::DiagnosticsConfig {
    crate::config::DiagnosticsConfig {
        undefined_variables: config.undefined_variables,
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
            | crate::DiagnosticCode::DOMAIN_LOVE_CALLBACK
            | crate::DiagnosticCode::LINT_UNDEFINED_VARIABLE
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

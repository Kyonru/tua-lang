use std::cell::{OnceCell, RefCell};
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use full_moon::node::Node;
use full_moon::tokenizer::{TokenReference, TokenType};

use crate::asset::{self, AssetMetadata};
use crate::completion::{
    complete_field_access_in_source_with_environment, complete_parsed_source_with_environment,
    complete_source_with_parsed_environment,
};
use crate::folding_range::folding_ranges_source;
use crate::hir::{HirExportValue, HirModule, HirRequireBinding, HirSymbol, HirSymbolKind};
use crate::hover::hover_parsed_source_with_environment;
use crate::ide_db::{SymbolIndexEntry, SymbolIndexKind};
use crate::inlay_hint::inlay_hints_parsed_source_with_environment;
use crate::inline_ignore::apply_inline_ignores;
use crate::module_index::infer_module_exports_with_optional_environment;
use crate::module_link::require_module_at_position;
use crate::plugin::{PluginContext, PluginRegistry};
use crate::profile;
use crate::scope::{ScopeDeclarationKind, ScopeGlobalKind, ScopeResolution};
use crate::scope_map_parsed_source;
use crate::selection_range::selection_ranges_source;
use crate::semantic::SemanticModel;
use crate::signature_help::signature_help_parsed_source_with_environment;
use crate::source_text::{byte_for_column, dotted_chain_at_position, line_at};
use crate::types::{
    build_type_environment_with_builtin_aliases, build_type_environment_with_diagnostics,
};
use crate::vfs::{normalize_path, VfsSnapshot, VfsSourceKind, VirtualFileSystem};
use crate::{
    assists_parsed_source, call_hierarchy_calls_parsed_source,
    call_hierarchy_prepare_parsed_source, call_hierarchy_symbols_parsed_source,
    check::{
        check_parsed_source_with_asset_roots_diagnostics_jit_and_ecs_config_and_asset_index,
        check_parsed_source_with_environment_asset_roots_diagnostics_jit_config,
        check_parsed_source_with_environment_asset_roots_diagnostics_jit_config_and_asset_index,
    },
    complete_parsed_source, complete_tua_source, definition_parsed_source_with_hir,
    document_highlights_parsed_source, document_symbols_parsed_source_with_hir, emit_lua_with_mode,
    exported_field_for_fields, global_function_definition_count,
    global_function_name_for_definition, global_references_to_definition_parsed_source_with_hir,
    global_type_alias_name_for_definition,
    global_type_alias_references_to_definition_parsed_source_with_hir, hover_parsed_source,
    inlay_hints_parsed_source, is_valid_rename_identifier, lower_parsed_source,
    module_links_tua_source, parse_lua_source, parse_tua_source, reference_kind_for_rename,
    references_to_definition_parsed_source_with_artifacts, rename_target_for_definition,
    require_bindings_for_parsed_source, semantic_tokens_parsed_source, semantic_tokens_tua_source,
    signature_help_parsed_source, signature_help_tua_source, type_alias_definition_count, Assist,
    AssistRange, CallHierarchyCallSite, CallHierarchyCallTarget, CallHierarchyIncomingCall,
    CallHierarchyOutgoingCall, CallHierarchySymbol, CallHierarchySymbolTarget, Completion,
    CompletionKind, Definition, DefinitionKind, Diagnostic, DocumentHighlight, DocumentSymbol,
    FoldingRange, Hover, InlayHint, ModuleExportField, ModuleExports, ModuleGraph,
    ModuleGraphInput, ModuleGraphOptions, ModuleLink, ParsedSource, ProjectConfig,
    ProjectTypeDatabase, ProjectTypeDatabaseInput, Reference, ReferenceKind, RenameEdit,
    RenameTarget, RequireResolution, ScopeMap, SelectionRange, SemanticIndex, SemanticToken,
    SignatureHelp, SourcePosition, SourceSpan, SymbolIndex, TableField, TextEdit, TuaType, Type,
    TypeEnvironment, TypedFunction, WorkspaceSymbol,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltinCatalogs {
    pub lua: bool,
    pub love: bool,
}

impl Default for BuiltinCatalogs {
    fn default() -> Self {
        Self {
            lua: true,
            love: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum AnalysisSourceKind {
    Tua,
    Lua,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnalysisWarmPriority {
    High,
    Low,
}

#[derive(Clone, Debug, Default)]
pub struct AnalysisHost {
    config: ProjectConfig,
    vfs: VirtualFileSystem,
    asset_roots: Vec<PathBuf>,
    asset_index: Option<asset::AssetIndex>,
    builtin_catalogs: BuiltinCatalogs,
    plugin_registry: PluginRegistry,
    cache: RefCell<AnalysisQueryCache>,
    last_good_files: RefCell<BTreeMap<PathBuf, LastGoodAnalysisFile>>,
    snapshot_cache: RefCell<Option<CachedAnalysisSnapshot>>,
    revision: u64,
}

#[derive(Clone, Debug)]
pub struct AnalysisSnapshot {
    config: ProjectConfig,
    vfs: VfsSnapshot,
    asset_roots: Vec<PathBuf>,
    asset_index: Option<asset::AssetIndex>,
    files: BTreeMap<PathBuf, AnalysisFile>,
    builtin_catalogs: BuiltinCatalogs,
    plugin_registry: PluginRegistry,
    module_graph: ModuleGraph,
    type_database: ProjectTypeDatabase,
    symbol_index: SymbolIndex,
    last_good_files: BTreeMap<PathBuf, LastGoodAnalysisFile>,
    semantic_facts: Rc<RefCell<BTreeMap<PathBuf, Rc<AnalysisSemanticFacts>>>>,
    completion_cache: Rc<RefCell<BTreeMap<CompletionRequestKey, Vec<Completion>>>>,
    definition_cache: Rc<RefCell<BTreeMap<DefinitionRequestKey, Option<Definition>>>>,
    reverse_reference_index: Rc<OnceCell<ReverseReferenceIndex>>,
    call_hierarchy_symbols: BTreeMap<PathBuf, Vec<CallHierarchySymbol>>,
    call_hierarchy_calls: BTreeMap<PathBuf, Vec<CallHierarchyCallSite>>,
}

#[derive(Clone, Copy, Debug)]
pub struct AnalysisSemanticFile<'a> {
    pub parsed: &'a ParsedSource,
    pub hir: Option<&'a HirModule>,
    pub scope: Option<&'a ScopeMap>,
}

#[derive(Clone, Debug)]
struct AnalysisFile {
    source: String,
    kind: AnalysisSourceKind,
    parsed: Result<ParsedSource, Vec<Diagnostic>>,
    hir: Option<HirModule>,
    scope: Option<ScopeMap>,
    environment: Option<TypeEnvironment>,
    diagnostics: Vec<Diagnostic>,
    exports: Option<ModuleExports>,
    require_edges: Vec<ModuleLink>,
    assets: Vec<AssetMetadata>,
    #[allow(dead_code)]
    symbols: Vec<SymbolIndexEntry>,
}

#[derive(Clone, Debug)]
struct AnalysisSemanticFacts {
    environment: TypeEnvironment,
    semantic_index: SemanticIndex,
    inlay_hints: OnceCell<Vec<InlayHint>>,
}

#[derive(Clone, Debug)]
struct LastGoodAnalysisFile {
    parsed: ParsedSource,
    hir: Option<HirModule>,
    environment: Option<TypeEnvironment>,
}

#[derive(Clone, Debug)]
struct RecoveredSemanticFacts {
    parsed: ParsedSource,
    hir: Option<HirModule>,
    environment: TypeEnvironment,
    semantic_index: SemanticIndex,
}

#[derive(Clone, Copy, Debug)]
struct AnalysisBuildContext<'a> {
    config: &'a ProjectConfig,
    source_roots: &'a [PathBuf],
    asset_roots: &'a [PathBuf],
    asset_index: Option<&'a asset::AssetIndex>,
    plugin_registry: &'a PluginRegistry,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AnalysisCacheStats {
    pub parse_hits: usize,
    pub parse_misses: usize,
    pub hir_hits: usize,
    pub hir_misses: usize,
    pub scope_hits: usize,
    pub scope_misses: usize,
    pub type_environment_hits: usize,
    pub type_environment_misses: usize,
    pub diagnostics_hits: usize,
    pub diagnostics_misses: usize,
    pub module_export_hits: usize,
    pub module_export_misses: usize,
    pub require_edge_hits: usize,
    pub require_edge_misses: usize,
    pub symbol_index_hits: usize,
    pub symbol_index_misses: usize,
    pub invalidations: usize,
    pub dependent_invalidations: usize,
}

#[derive(Clone, Debug, Default)]
struct AnalysisQueryCache {
    files: BTreeMap<PathBuf, CachedAnalysisFile>,
    parsed_files: BTreeMap<ParsedCacheKey, Result<ParsedSource, Vec<Diagnostic>>>,
    dependencies: BTreeMap<PathBuf, BTreeSet<PathBuf>>,
    importers: BTreeMap<PathBuf, BTreeSet<PathBuf>>,
    stats: AnalysisCacheStats,
}

#[derive(Clone, Debug)]
struct CachedAnalysisFile {
    source: String,
    kind: AnalysisSourceKind,
    file: AnalysisFile,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DependencySurface {
    exports: Option<ModuleExports>,
    aliases: BTreeMap<String, Type>,
    locals: BTreeMap<String, Type>,
    functions: BTreeMap<String, TypedFunction>,
    require_targets: BTreeSet<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct ParsedCacheKey {
    path: PathBuf,
    kind: AnalysisSourceKind,
    fingerprint: SourceFingerprint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct SourceFingerprint {
    len: usize,
    hash: u64,
}

#[derive(Clone, Debug)]
struct CachedAnalysisSnapshot {
    revision: u64,
    snapshot: AnalysisSnapshot,
}

impl DependencySurface {
    fn from_analysis_file(file: &AnalysisFile) -> Self {
        Self::from_parts(
            file.exports.clone(),
            file.environment.as_ref(),
            require_targets_for_links(&file.require_edges),
        )
    }

    fn from_parsed(
        parsed: &ParsedSource,
        config: &ProjectConfig,
        require_targets: BTreeSet<PathBuf>,
    ) -> Self {
        let environment = build_analysis_type_environment(parsed, config);
        let exports = infer_module_exports_with_optional_environment(parsed, environment.as_ref());

        Self::from_parts(exports, environment.as_ref(), require_targets)
    }

    fn from_parts(
        exports: Option<ModuleExports>,
        environment: Option<&TypeEnvironment>,
        require_targets: BTreeSet<PathBuf>,
    ) -> Self {
        Self {
            exports,
            aliases: environment
                .into_iter()
                .flat_map(TypeEnvironment::aliases)
                .map(|(name, ty)| (name.to_owned(), ty.clone()))
                .collect(),
            locals: environment
                .into_iter()
                .flat_map(TypeEnvironment::locals)
                .map(|(name, ty)| (name.to_owned(), ty.clone()))
                .collect(),
            functions: environment
                .into_iter()
                .flat_map(TypeEnvironment::functions)
                .filter_map(|function| {
                    function
                        .name
                        .as_ref()
                        .map(|name| (name.clone(), function.clone()))
                })
                .collect(),
            require_targets,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct ReverseReferenceIndex {
    references: BTreeMap<ReferenceIndexKey, Vec<Reference>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct ReferenceIndexKey {
    path: PathBuf,
    start_line: usize,
    start_column: usize,
    end_line: usize,
    end_column: usize,
    kind: u8,
}

type ModuleExportCandidates = BTreeMap<String, BTreeSet<String>>;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct CompletionRequestKey {
    path: PathBuf,
    line: usize,
    column: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct DefinitionRequestKey {
    path: PathBuf,
    line: usize,
    column: usize,
}

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

    pub fn plugin_registry(&self) -> &PluginRegistry {
        &self.plugin_registry
    }

    pub fn set_plugin_registry(&mut self, registry: PluginRegistry) {
        self.plugin_registry = registry;
        self.invalidate_all_analysis();
        self.bump_revision();
    }

    fn effective_plugin_registry(&self) -> PluginRegistry {
        let mut registry = self.plugin_registry.clone();

        if self.config.ecs.enabled {
            registry.register(crate::ecs::EcsPlugin);
        }

        registry
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
        let path = path.into();
        let source = source.into();
        let vfs_kind = vfs_source_kind(kind);

        if self.vfs.disk_file_matches(&path, &source, vfs_kind) {
            return;
        }

        self.cache
            .borrow_mut()
            .invalidate_changed_file(&path, &source, kind, &self.config);
        self.vfs.set_disk_file(path, source, vfs_kind);
        self.bump_revision();
    }

    pub fn remove_file(&mut self, path: impl AsRef<Path>) {
        self.cache.borrow_mut().invalidate_file(path.as_ref(), true);
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

        self.cache
            .borrow_mut()
            .invalidate_changed_file(&path, &source, kind, &self.config);
        self.vfs.open_file(path, source, vfs_kind);
        self.bump_revision();
    }

    pub fn update_open_document(&mut self, path: impl Into<PathBuf>, source: impl Into<String>) {
        self.open_document(path, source);
    }

    pub fn close_document(&mut self, path: impl AsRef<Path>) {
        self.cache.borrow_mut().invalidate_file(path.as_ref(), true);
        self.vfs.close_file(path);
        self.bump_revision();
    }

    pub fn snapshot(&self) -> AnalysisSnapshot {
        let _profile = profile::span("analysis.snapshot");
        if let Some(cached) = self.snapshot_cache.borrow().as_ref() {
            if cached.revision == self.revision {
                return cached.snapshot.clone();
            }
        }

        let vfs = self.vfs.snapshot();
        let source_roots = vfs.source_roots().to_vec();
        let plugin_registry = self.effective_plugin_registry();
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
                            asset_roots: &self.asset_roots,
                            asset_index: self.asset_index.as_ref(),
                            plugin_registry: &plugin_registry,
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
                    hir: file.hir.as_ref(),
                    exports: file.exports.as_ref(),
                    source_kind: file.kind,
                }),
                ModuleGraphOptions {
                    source_roots: vfs.source_roots().to_vec(),
                    output_roots: vfs.options().output_roots.clone(),
                    extension: self.config.extension.clone(),
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
                    environment: file.environment.as_ref(),
                    scope: file.scope.as_ref(),
                }),
                &module_graph,
            )
        };
        let symbol_index = {
            let _profile = profile::span("analysis.index.symbol_index");

            SymbolIndex::build(
                files.iter().map(|(path, file)| (path, file.hir.as_ref())),
                &module_graph,
                vfs.source_roots(),
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
            config: self.config.clone(),
            vfs,
            asset_roots: self.asset_roots.clone(),
            asset_index: self.asset_index.clone(),
            files,
            builtin_catalogs: self.builtin_catalogs.clone(),
            plugin_registry,
            module_graph,
            type_database,
            symbol_index,
            last_good_files: self.last_good_files.borrow().clone(),
            semantic_facts: Rc::new(RefCell::new(BTreeMap::new())),
            completion_cache: Rc::new(RefCell::new(BTreeMap::new())),
            definition_cache: Rc::new(RefCell::new(BTreeMap::new())),
            reverse_reference_index: Rc::new(OnceCell::new()),
            call_hierarchy_symbols,
            call_hierarchy_calls,
        };

        self.snapshot_cache
            .borrow_mut()
            .replace(CachedAnalysisSnapshot {
                revision: self.revision,
                snapshot: snapshot.clone(),
            });

        snapshot
    }

    pub fn warm_file(&self, path: impl AsRef<Path>, _priority: AnalysisWarmPriority) -> bool {
        self.snapshot().semantic_index(path).is_some()
    }

    pub fn cache_stats(&self) -> AnalysisCacheStats {
        self.cache.borrow().stats.clone()
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

    pub fn revision(&self) -> u64 {
        self.revision
    }

    fn bump_revision(&mut self) {
        self.revision = self.revision.saturating_add(1);
    }

    fn invalidate_all_analysis(&mut self) {
        self.cache.borrow_mut().invalidate_all();
        self.last_good_files.borrow_mut().clear();
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
                    hir: file.hir.clone(),
                    environment: file.environment.clone(),
                },
            );
        }
    }
}

impl AnalysisSnapshot {
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

    pub fn plugin_registry(&self) -> &PluginRegistry {
        &self.plugin_registry
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
            hir: file.hir.as_ref(),
            scope: file.scope.as_ref(),
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
        let Some(file) = self.file(path.as_ref()) else {
            return Vec::new();
        };
        let mut diagnostics = file.diagnostics.clone();

        diagnostics.extend(
            self.module_graph
                .diagnostics(path.as_ref())
                .iter()
                .filter(|diagnostic| self.project_diagnostic_enabled(diagnostic))
                .cloned(),
        );

        let diagnostics =
            self.project_aware_diagnostics(path.as_ref(), deduplicate_diagnostics(diagnostics));

        apply_inline_ignores(
            path.as_ref(),
            &file.source,
            diagnostics,
            &self.config.diagnostics,
        )
    }

    pub fn hover(&self, path: impl AsRef<Path>, position: SourcePosition) -> Option<Hover> {
        let _profile = profile::span("analysis.query.hover");
        let path = path.as_ref();
        let file = self.file(path)?;

        match &file.parsed {
            Ok(parsed) => {
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
                let facts = self.recovered_semantic_facts(path, file)?;

                self.hover_parsed_snapshot_with_environment(
                    &facts.parsed,
                    position,
                    Some(&facts.environment),
                )
            }
        }
    }

    fn hover_parsed_snapshot(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<Hover> {
        self.hover_parsed_snapshot_with_environment(parsed, position, None)
    }

    fn hover_parsed_snapshot_with_environment(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
        recovered_environment: Option<&TypeEnvironment>,
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

        recovered_environment
            .and_then(|environment| {
                hover_parsed_source_with_environment(parsed, position, environment)
            })
            .or_else(|| {
                self.semantic_facts_for_parsed(parsed).and_then(|facts| {
                    hover_parsed_source_with_environment(parsed, position, &facts.environment)
                })
            })
            .or_else(|| hover_parsed_source(parsed, position).ok().flatten())
            .or_else(|| self.project_symbol_hover(parsed, position))
    }

    pub fn completion(&self, path: impl AsRef<Path>, position: SourcePosition) -> Vec<Completion> {
        let _profile = profile::span("analysis.query.completion");
        let path = path.as_ref();
        let key = completion_request_key(path, position);

        if let Some(completions) = self.completion_cache.borrow().get(&key).cloned() {
            return completions;
        }

        let Some(file) = self.file(path) else {
            return Vec::new();
        };

        let completions = match &file.parsed {
            Ok(parsed) => {
                let completions = self.project_completions(parsed, &file.source, position);

                if !completions.is_empty() {
                    completions
                } else if let Some(facts) = self.semantic_facts_for_parsed(parsed) {
                    complete_parsed_source_with_environment(parsed, position, &facts.environment)
                } else {
                    complete_parsed_source(parsed, position).unwrap_or_default()
                }
            }
            Err(_) => {
                if let Some(facts) = self.recovered_semantic_facts(path, file) {
                    let completions = self.project_completions_with_environment(
                        &facts.parsed,
                        &file.source,
                        position,
                        &facts.environment,
                    );

                    if !completions.is_empty() {
                        completions
                    } else {
                        let completions = complete_source_with_parsed_environment(
                            &file.source,
                            &facts.parsed,
                            position,
                            &facts.environment,
                        );

                        if !completions.is_empty() {
                            completions
                        } else {
                            complete_tua_source(path, file.source.clone(), position)
                                .unwrap_or_default()
                        }
                    }
                } else {
                    complete_tua_source(path, file.source.clone(), position).unwrap_or_default()
                }
            }
        };

        self.completion_cache
            .borrow_mut()
            .insert(key, completions.clone());

        completions
    }

    pub fn definition(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
    ) -> Option<Definition> {
        let _profile = profile::span("analysis.query.definition");
        let path = path.as_ref();
        let key = definition_request_key(path, position);

        if let Some(definition) = self.definition_cache.borrow().get(&key).cloned() {
            return definition;
        }

        let file = self.file(path)?;

        let definition = match &file.parsed {
            Ok(parsed) => self.definition_parsed_snapshot(parsed, file.hir.as_ref(), position),
            Err(_) => {
                let facts = self.recovered_semantic_facts(path, file)?;

                self.definition_parsed_snapshot_with_index(
                    &facts.parsed,
                    facts.hir.as_ref(),
                    position,
                    Some(&facts.semantic_index),
                )
            }
        };

        let mut cache = self.definition_cache.borrow_mut();

        if let Some(span) = definition_lookup_span(&file.source, position) {
            for column in span.start.column..=span.end.column {
                cache.insert(
                    definition_request_key(
                        path,
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

    fn definition_parsed_snapshot(
        &self,
        parsed: &ParsedSource,
        hir: Option<&HirModule>,
        position: SourcePosition,
    ) -> Option<Definition> {
        self.definition_parsed_snapshot_with_index(parsed, hir, position, None)
    }

    fn definition_parsed_snapshot_with_index(
        &self,
        parsed: &ParsedSource,
        hir: Option<&HirModule>,
        position: SourcePosition,
        recovered_index: Option<&SemanticIndex>,
    ) -> Option<Definition> {
        if let Some(definition) = self.module_definition(parsed, position) {
            return Some(definition);
        }

        if self.is_module_reference(parsed, position) {
            return None;
        }

        if let Some(definition) = recovered_index.and_then(|index| index.definition_at(position)) {
            return Some(definition);
        }

        if let Some(definition) = self
            .semantic_facts_for_parsed(parsed)
            .and_then(|facts| facts.semantic_index.definition_at(position))
        {
            return Some(definition);
        }

        definition_parsed_source_with_hir(parsed, hir, position)
            .ok()
            .flatten()
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
            Ok(parsed) => self
                .semantic_facts_for_parsed(parsed)
                .and_then(|facts| {
                    signature_help_parsed_source_with_environment(
                        parsed,
                        position,
                        &facts.environment,
                    )
                })
                .or_else(|| {
                    signature_help_parsed_source(parsed, position)
                        .ok()
                        .flatten()
                }),
            Err(_) => self
                .recovered_semantic_facts(path, file)
                .and_then(|facts| {
                    signature_help_parsed_source_with_environment(
                        &facts.parsed,
                        position,
                        &facts.environment,
                    )
                })
                .or_else(|| {
                    signature_help_tua_source(path, file.source.clone(), position)
                        .ok()
                        .flatten()
                }),
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
                    file.hir.as_ref()?,
                ))
            })
            .unwrap_or_default()
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

        assists_parsed_source(
            parsed,
            diagnostics,
            range,
            &source_dir,
            &self.config.extension,
        )
    }

    pub fn code_actions(
        &self,
        path: impl AsRef<Path>,
        range: AssistRange,
        diagnostics: &[Diagnostic],
    ) -> Vec<Assist> {
        self.assists(path, range, diagnostics)
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

        {
            let _profile = profile::span("analysis.references.reverse_index.lookup");

            if let Some(references) = self
                .reverse_reference_index()
                .references(&target, include_declaration)
            {
                return references;
            }
        }

        self.references_by_scan(&target, include_declaration)
    }

    fn scope_references_at_position(
        &self,
        path: &Path,
        position: SourcePosition,
        include_declaration: bool,
    ) -> Option<Vec<Reference>> {
        let _profile = profile::span("analysis.references.scope_position_fast_path");
        let file = self.file(path)?;
        let parsed = file.parsed.as_ref().ok()?;
        let scope = file.scope.as_ref()?;
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

    fn parameter_scope_references(
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
            file.hir.as_ref(),
            file.scope.as_ref(),
        )
        .ok()
    }

    fn references_by_scan(&self, target: &Definition, include_declaration: bool) -> Vec<Reference> {
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
            let Ok(mut file_references) = references_to_definition_parsed_source_with_artifacts(
                parsed,
                target,
                include_declaration,
                file.hir.as_ref(),
                file.scope.as_ref(),
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
                        file.hir.as_ref(),
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
                        file.hir.as_ref(),
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

    fn build_reverse_reference_index(&self) -> ReverseReferenceIndex {
        let _profile = profile::span("analysis.references.index.build");
        let global_functions = self.global_function_definitions();
        let global_aliases = self.global_type_alias_definitions();
        let type_alias_names = self.symbol_index.names_for_kind(SymbolIndexKind::TypeAlias);
        let mut index = ReverseReferenceIndex::default();

        for file in self.files.values() {
            let Ok(parsed) = &file.parsed else {
                continue;
            };

            if let Some(scope) = file.scope.as_ref() {
                self.index_scope_references(parsed, file.hir.as_ref(), scope, &mut index);
                self.index_global_function_references(parsed, scope, &global_functions, &mut index);
            }

            if let Some(hir) = file.hir.as_ref() {
                self.index_hir_declarations(parsed, hir, &mut index);
            }

            let module_export_candidates =
                self.module_export_candidates_for_file(parsed, file.hir.as_ref());

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

    fn reverse_reference_index(&self) -> &ReverseReferenceIndex {
        self.reverse_reference_index.get_or_init(|| {
            let _profile = profile::span("analysis.index.references");

            self.build_reverse_reference_index()
        })
    }

    fn index_scope_references(
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

    fn index_hir_declarations(
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

    fn index_export_key_references(
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

    fn index_global_function_references(
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

    fn index_token_references(
        &self,
        parsed: &ParsedSource,
        global_aliases: &BTreeMap<String, Vec<Definition>>,
        type_alias_names: &BTreeSet<String>,
        module_export_candidates: &ModuleExportCandidates,
        index: &mut ReverseReferenceIndex,
    ) {
        for token in parsed
            .ast
            .tokens()
            .filter(|token| is_identifier_token(token))
        {
            let Some(span) = token_span(token) else {
                continue;
            };
            let name = token_text(token);
            let is_type_reference = is_type_reference_position(parsed, span.start);

            if is_type_reference && !type_alias_names.contains(&name) {
                continue;
            }

            if !is_type_reference
                && !is_module_export_candidate(parsed, span.start, module_export_candidates)
            {
                continue;
            }

            if is_type_reference {
                self.index_type_alias_token_reference(parsed, token, span, global_aliases, index);
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

    fn index_type_alias_token_reference(
        &self,
        parsed: &ParsedSource,
        token: &TokenReference,
        span: SourceSpan,
        global_aliases: &BTreeMap<String, Vec<Definition>>,
        index: &mut ReverseReferenceIndex,
    ) {
        let name = token_text(token);
        let Some(definitions) = global_aliases.get(&name) else {
            return;
        };

        for definition in definitions {
            index.insert(
                definition,
                Reference {
                    path: parsed.path.clone(),
                    span,
                    kind: ReferenceKind::TypeAlias,
                },
            );
        }
    }

    fn module_export_definition_at_position(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<Definition> {
        let chain = dotted_chain_at_position(&parsed.source, position)?;

        if chain.cursor_index == 0 {
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
            kind: if matches!(entry.field.ty, Type::Function(_)) {
                DefinitionKind::Function
            } else {
                DefinitionKind::TableField
            },
        })
    }

    fn module_export_candidates_for_file(
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

    fn global_function_definitions(&self) -> BTreeMap<String, Vec<Definition>> {
        let mut definitions: BTreeMap<String, Vec<Definition>> = BTreeMap::new();

        for file in self.files.values() {
            let Ok(parsed) = &file.parsed else {
                continue;
            };
            let Some(hir) = file.hir.as_ref() else {
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

    fn global_type_alias_definitions(&self) -> BTreeMap<String, Vec<Definition>> {
        let mut definitions: BTreeMap<String, Vec<Definition>> = BTreeMap::new();

        for file in self.files.values() {
            let Ok(parsed) = &file.parsed else {
                continue;
            };
            let Some(hir) = file.hir.as_ref() else {
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

        (!edits.is_empty()).then_some(RenameEdit { edits })
    }

    pub fn inlay_hints(&self, path: impl AsRef<Path>) -> Vec<InlayHint> {
        let _profile = profile::span("analysis.query.inlay_hints");
        let Some(file) = self.file(path.as_ref()) else {
            return Vec::new();
        };
        let parsed = match &file.parsed {
            Ok(parsed) => parsed,
            Err(_) => {
                return self
                    .recovered_semantic_facts(path.as_ref(), file)
                    .map(|facts| {
                        inlay_hints_parsed_source_with_environment(
                            &facts.parsed,
                            &facts.environment,
                        )
                    })
                    .unwrap_or_default();
            }
        };
        let Some(facts) = self.semantic_facts_for_parsed(parsed) else {
            return inlay_hints_parsed_source(parsed).unwrap_or_default();
        };

        facts
            .inlay_hints
            .get_or_init(|| {
                let _profile = profile::span("analysis.semantic.inlay_hints");

                inlay_hints_parsed_source_with_environment(parsed, &facts.environment)
            })
            .clone()
    }

    pub fn semantic_tokens(&self, path: impl AsRef<Path>) -> Vec<SemanticToken> {
        let path = path.as_ref();
        let Some(file) = self.file(path) else {
            return Vec::new();
        };

        match &file.parsed {
            Ok(parsed) => semantic_tokens_parsed_source(parsed).unwrap_or_default(),
            Err(_) => semantic_tokens_tua_source(path, file.source.clone()).unwrap_or_default(),
        }
    }

    fn file(&self, path: &Path) -> Option<&AnalysisFile> {
        self.files.get(&normalize_path(path))
    }

    fn effective_asset_roots(&self) -> &[PathBuf] {
        analysis_asset_roots(self.source_roots(), &self.asset_roots)
    }

    pub fn type_environment(&self, path: impl AsRef<Path>) -> Option<&TypeEnvironment> {
        self.file(path.as_ref())?.environment.as_ref()
    }

    pub fn semantic_index(&self, path: impl AsRef<Path>) -> Option<SemanticIndex> {
        let file = self.file(path.as_ref())?;
        let parsed = file.parsed.as_ref().ok()?;

        self.semantic_facts_for_parsed(parsed)
            .map(|facts| facts.semantic_index.clone())
    }

    pub fn scope_map(&self, path: impl AsRef<Path>) -> Option<&ScopeMap> {
        self.file(path.as_ref())?.scope.as_ref()
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

    pub fn importable_symbols(&self, prefix: &str) -> Vec<&SymbolIndexEntry> {
        self.symbol_index.importable_symbols(prefix)
    }

    fn call_hierarchy_symbol_for_identity(
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

    fn call_hierarchy_symbol_for_definition(
        &self,
        definition: &Definition,
    ) -> Option<CallHierarchySymbol> {
        self.call_hierarchy_symbols
            .get(&normalize_path(&definition.path))?
            .iter()
            .find(|symbol| definition_matches_call_symbol(definition, symbol))
            .cloned()
    }

    fn resolve_call_hierarchy_target(
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

    fn resolve_local_call_symbol(
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

    fn resolve_qualified_call_symbol(
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

    fn resolve_required_module_call_symbol(
        &self,
        current_path: &Path,
        parts: &[String],
    ) -> Option<CallHierarchySymbol> {
        let (receiver, fields) = parts.split_first()?;
        let field = fields.first()?;
        let file = self.file(current_path)?;
        let binding = file
            .hir
            .as_ref()?
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

    fn exported_call_symbol(&self, module_path: &Path, field: &str) -> Option<CallHierarchySymbol> {
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

    fn receiver_type_name(
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

    fn resolve_method_call_symbol(
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

    fn module_hover(&self, parsed: &ParsedSource, position: SourcePosition) -> Option<Hover> {
        if let Some(module) = require_module_at_position(&parsed.source, position) {
            let path = self.resolved_module_path(&parsed.path, &module)?;

            return Some(Hover {
                contents: format!("(module) {module} -> {}", path.display()),
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
                    "(module) {} = require(\"{}\") -> {}",
                    binding.alias,
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

    fn module_definition(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<Definition> {
        if let Some(module) = require_module_at_position(&parsed.source, position) {
            let path = self.resolved_module_path(&parsed.path, &module)?;

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

        let public_name = chain.parts.get(1)?.text.as_str();
        let entry = self.module_graph.export(path, public_name)?;

        Some(Definition {
            path: path.to_path_buf(),
            span: entry.definition_span,
            kind: if matches!(entry.field.ty, Type::Function(_)) {
                DefinitionKind::Function
            } else {
                DefinitionKind::TableField
            },
        })
    }

    fn module_definition_span(&self, path: &Path) -> SourceSpan {
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

    fn module_export_source_documentation(&self, path: &Path, fields: &[String]) -> Option<String> {
        let public_name = fields.first()?;
        let entry = self.module_graph.export(path, public_name)?;
        let source = &self.file(path)?.source;

        leading_plain_documentation(source, entry.definition_span.start.line)
    }

    fn is_module_reference(&self, parsed: &ParsedSource, position: SourcePosition) -> bool {
        if require_module_at_position(&parsed.source, position).is_some() {
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

    fn require_binding(
        &self,
        parsed: &ParsedSource,
        name: &str,
        position: SourcePosition,
    ) -> Option<HirRequireBinding> {
        self.file(&parsed.path)?
            .hir
            .as_ref()?
            .require_bindings
            .iter()
            .filter(|binding| {
                binding.alias == name
                    && position_key(binding.alias_span.start) <= position_key(position)
            })
            .max_by_key(|binding| position_key(binding.alias_span.start))
            .cloned()
    }

    fn resolved_module_path(&self, source_path: &Path, module: &str) -> Option<&Path> {
        match self.module_graph.resolve_require(source_path, module)? {
            RequireResolution::Resolved { path }
            | RequireResolution::Duplicate { selected: path, .. } => Some(path.as_path()),
            RequireResolution::Missing { .. } => None,
        }
    }

    fn module_export_for_fields(
        &self,
        module_path: &Path,
        fields: &[String],
    ) -> Option<ModuleExportField> {
        let exports = self.module_exports(module_path)?;

        exported_field_for_fields(exports, fields)
    }

    fn project_type_hover(&self, parsed: &ParsedSource, position: SourcePosition) -> Option<Hover> {
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

    fn project_symbol_hover(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
    ) -> Option<Hover> {
        let chain = dotted_chain_at_position(&parsed.source, position)?;

        if chain.cursor_index != 0 || chain.parts.len() != 1 {
            return None;
        }

        let current_part = chain.parts.first()?;
        let environment = self.semantic_environment(parsed)?;
        let semantic = SemanticModel::new(parsed, &environment);
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

    fn project_annotated_local_hover(
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

    fn project_completions(
        &self,
        parsed: &ParsedSource,
        source: &str,
        position: SourcePosition,
    ) -> Vec<Completion> {
        let environment = self.semantic_environment(parsed).unwrap_or_else(|| {
            let (environment, _) = build_type_environment_with_diagnostics(parsed);

            environment.with_local_types(self.project_local_annotation_types(parsed))
        });

        self.project_completions_with_environment(parsed, source, position, &environment)
    }

    fn project_completions_with_environment(
        &self,
        parsed: &ParsedSource,
        source: &str,
        position: SourcePosition,
        environment: &TypeEnvironment,
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

        let field_completions =
            complete_field_access_in_source_with_environment(source, parsed, position, environment);

        if !field_completions.is_empty() {
            return field_completions;
        }

        self.project_table_literal_completions(parsed, source, position)
    }

    fn project_asset_path_completions(
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

        if active_parameter != 0 || callee_parts.len() != 1 {
            return Vec::new();
        }

        let Some(kind) = asset::loader_kind(&callee_parts[0]) else {
            return Vec::new();
        };
        if !kind.has_asset_path() {
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
                insert_text: Some(candidate.path),
                is_snippet: false,
                module_path: None,
                source_module: None,
                additional_text_edits: Vec::new(),
            })
            .collect()
    }

    fn semantic_facts_for_parsed(
        &self,
        parsed: &ParsedSource,
    ) -> Option<Rc<AnalysisSemanticFacts>> {
        let path = normalize_path(&parsed.path);

        if let Some(facts) = self.semantic_facts.borrow().get(&path).cloned() {
            return Some(facts);
        }

        let file = self.file(&path)?;
        let _profile = profile::span_for_path("analysis.semantic.facts.file", &path);
        let environment = self.project_type_environment_for_file(file, parsed)?;
        let facts = Rc::new(AnalysisSemanticFacts {
            semantic_index: SemanticIndex::build(parsed, file.hir.as_ref(), &environment),
            environment,
            inlay_hints: OnceCell::new(),
        });

        self.semantic_facts
            .borrow_mut()
            .insert(path, Rc::clone(&facts));

        Some(facts)
    }

    fn semantic_environment(&self, parsed: &ParsedSource) -> Option<TypeEnvironment> {
        self.semantic_facts_for_parsed(parsed)
            .map(|facts| facts.environment.clone())
    }

    fn recovered_semantic_facts(
        &self,
        path: &Path,
        file: &AnalysisFile,
    ) -> Option<RecoveredSemanticFacts> {
        self.recovered_current_semantic_facts(path, file)
            .or_else(|| self.last_good_semantic_facts(path))
    }

    fn recovered_current_semantic_facts(
        &self,
        path: &Path,
        file: &AnalysisFile,
    ) -> Option<RecoveredSemanticFacts> {
        let parsed = parse_recovered_analysis_source(path, &file.source, file.kind).ok()?;
        let hir = Some(lower_parsed_source(&parsed));
        let environment =
            self.recovered_project_type_environment_for_parsed(&parsed, file.environment.clone());
        let semantic_index = SemanticIndex::build(&parsed, hir.as_ref(), &environment);

        Some(RecoveredSemanticFacts {
            parsed,
            hir,
            environment,
            semantic_index,
        })
    }

    fn last_good_semantic_facts(&self, path: &Path) -> Option<RecoveredSemanticFacts> {
        let last_good = self.last_good_files.get(&normalize_path(path))?.clone();
        let environment = self.recovered_project_type_environment_for_parsed(
            &last_good.parsed,
            last_good.environment,
        );
        let semantic_index =
            SemanticIndex::build(&last_good.parsed, last_good.hir.as_ref(), &environment);

        Some(RecoveredSemanticFacts {
            parsed: last_good.parsed,
            hir: last_good.hir,
            environment,
            semantic_index,
        })
    }

    fn project_type_environment_for_file(
        &self,
        file: &AnalysisFile,
        parsed: &ParsedSource,
    ) -> Option<TypeEnvironment> {
        let _profile = profile::span("analysis.semantic.environment");
        self.project_type_environment_for_parsed(parsed, file.environment.clone())
    }

    fn project_type_environment_for_parsed(
        &self,
        parsed: &ParsedSource,
        base_environment: Option<TypeEnvironment>,
    ) -> Option<TypeEnvironment> {
        let environment = base_environment.or_else(|| {
            let (environment, diagnostics) = if self.config.ecs.enabled {
                build_type_environment_with_builtin_aliases(parsed, crate::ecs::builtin_aliases())
            } else {
                build_type_environment_with_diagnostics(parsed)
            };

            diagnostics.is_empty().then_some(environment)
        })?;

        let environment = environment.with_local_types(self.project_local_annotation_types(parsed));

        if self.config.ecs.enabled {
            Some(
                environment
                    .with_alias_types(crate::ecs::builtin_aliases())
                    .with_local_types(crate::ecs::builtin_locals()),
            )
        } else {
            Some(environment)
        }
    }

    fn recovered_project_type_environment_for_parsed(
        &self,
        parsed: &ParsedSource,
        base_environment: Option<TypeEnvironment>,
    ) -> TypeEnvironment {
        let (fallback, _) = if self.config.ecs.enabled {
            build_type_environment_with_builtin_aliases(parsed, crate::ecs::builtin_aliases())
        } else {
            build_type_environment_with_diagnostics(parsed)
        };
        let environment = base_environment.unwrap_or(fallback);
        let environment = environment.with_local_types(self.project_local_annotation_types(parsed));

        if self.config.ecs.enabled {
            environment
                .with_alias_types(crate::ecs::builtin_aliases())
                .with_local_types(crate::ecs::builtin_locals())
        } else {
            environment
        }
    }

    fn project_local_annotation_types(&self, parsed: &ParsedSource) -> Vec<(String, Type)> {
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

    fn project_type_from_tua_type(&self, parsed: &ParsedSource, ty: &TuaType) -> Option<Type> {
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
            } => Some(Type::function_returns(
                parameters.iter().filter_map(|parameter| {
                    let ty = self.project_type_from_tua_type(parsed, &parameter.ty)?;

                    Some(if parameter.optional {
                        Type::union([ty, Type::nil()])
                    } else {
                        ty
                    })
                }),
                returns
                    .iter()
                    .filter_map(|ty| self.project_type_from_tua_type(parsed, ty)),
            )),
        }
    }

    fn required_module_type(&self, parsed: &ParsedSource, name: &str) -> Option<Type> {
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

    fn indexed_required_module_path(&self, parsed: &ParsedSource, receiver: &str) -> Option<&Path> {
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

    fn indexed_module_candidates(&self, module: &str) -> Vec<PathBuf> {
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

    fn project_table_literal_completions(
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
            .filter(|(name, _)| name.starts_with(&prefix))
            .map(|(name, ty)| Completion {
                label: name,
                kind: CompletionKind::Field,
                detail: Some(format!("field: {ty}")),
                documentation: None,
                insert_text: None,
                is_snippet: false,
                module_path: None,
                source_module: None,
                additional_text_edits: Vec::new(),
            })
            .collect()
    }

    fn table_literal_annotation_type(
        &self,
        parsed: &ParsedSource,
        source: &str,
        position: SourcePosition,
    ) -> Option<(Type, String)> {
        let prefix = identifier_prefix_before(source, position)?;
        let annotation = parsed
            .local_annotations
            .iter()
            .filter(|annotation| {
                let Some(name_span) = annotation.name_span else {
                    return false;
                };

                name_span.start.line <= position.line
                    && typed_initializer_is_open(source, name_span.start.line, position)
            })
            .max_by_key(|annotation| annotation.name_span.map(|span| position_key(span.start)))?;
        let ty = self.project_type_from_tua_type(parsed, &annotation.ty)?;

        Some((ty, prefix))
    }

    fn project_module_field_completions(
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
                .filter(|entry| entry.public_name.starts_with(&prefix))
                .map(|entry| Completion {
                    label: entry.public_name.clone(),
                    kind: completion_kind_for_type(&entry.field.ty),
                    detail: Some(format!("field: {}", entry.field.ty)),
                    documentation: entry.field.documentation.clone(),
                    insert_text: None,
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
            .filter(|(name, _)| name.starts_with(&prefix))
            .map(|(name, ty)| Completion {
                kind: completion_kind_for_type(&ty),
                detail: Some(format!("field: {ty}")),
                label: name,
                documentation: entry.field.documentation.clone(),
                insert_text: None,
                is_snippet: false,
                module_path: Some(module_path.to_path_buf()),
                source_module: None,
                additional_text_edits: Vec::new(),
            })
            .collect()
    }

    fn project_aware_diagnostics(
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

    fn project_diagnostic_enabled(&self, diagnostic: &Diagnostic) -> bool {
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

    fn assist_source_root_for_path(&self, path: &Path) -> PathBuf {
        self.source_roots()
            .iter()
            .filter(|root| path.starts_with(root))
            .max_by_key(|root| root.components().count())
            .cloned()
            .or_else(|| path.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn is_ambiguous_project_rename(
        &self,
        target: &Definition,
        rename_target: &RenameTarget,
    ) -> bool {
        match rename_target.kind {
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
            crate::RenameKind::Local | crate::RenameKind::Parameter => false,
        }
    }
}

fn deduplicate_diagnostics(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();

    for diagnostic in diagnostics {
        let key = diagnostic_key(&diagnostic);

        if seen.insert(key) {
            output.push(diagnostic);
        }
    }

    output
}

fn diagnostic_key(diagnostic: &Diagnostic) -> (PathBuf, usize, usize, u8, Option<u16>) {
    if diagnostic.code == Some(crate::DiagnosticCode::LINT_MISSING_REQUIRE) {
        return (
            normalize_path(&diagnostic.file),
            diagnostic.line,
            0,
            diagnostic_severity_key(diagnostic.severity),
            diagnostic.code.map(crate::DiagnosticCode::as_u16),
        );
    }

    (
        normalize_path(&diagnostic.file),
        diagnostic.line,
        diagnostic.column,
        diagnostic_severity_key(diagnostic.severity),
        diagnostic.code.map(crate::DiagnosticCode::as_u16),
    )
}

fn diagnostic_severity_key(severity: crate::DiagnosticSeverity) -> u8 {
    match severity {
        crate::DiagnosticSeverity::Error => 1,
        crate::DiagnosticSeverity::Warning => 2,
        crate::DiagnosticSeverity::Info => 3,
        crate::DiagnosticSeverity::Hint => 4,
    }
}

fn reference_key(reference: &Reference) -> (PathBuf, usize, usize, usize, usize, u8) {
    (
        reference.path.clone(),
        reference.span.start.line,
        reference.span.start.column,
        reference.span.end.line,
        reference.span.end.column,
        reference_kind_key(reference.kind),
    )
}

fn reference_kind_key(kind: crate::ReferenceKind) -> u8 {
    match kind {
        crate::ReferenceKind::Module => 0,
        crate::ReferenceKind::TypeAlias => 1,
        crate::ReferenceKind::Local => 2,
        crate::ReferenceKind::Function => 3,
        crate::ReferenceKind::Parameter => 4,
        crate::ReferenceKind::TableField => 5,
    }
}

fn text_edit_key(edit: &TextEdit) -> (PathBuf, usize, usize, usize, usize) {
    (
        edit.path.clone(),
        edit.span.start.line,
        edit.span.start.column,
        edit.span.end.line,
        edit.span.end.column,
    )
}

impl ReverseReferenceIndex {
    fn insert(&mut self, definition: &Definition, reference: Reference) {
        let Some(key) = ReferenceIndexKey::from_definition(definition) else {
            return;
        };

        self.references.entry(key).or_default().push(reference);
    }

    fn references(
        &self,
        definition: &Definition,
        include_declaration: bool,
    ) -> Option<Vec<Reference>> {
        let key = ReferenceIndexKey::from_definition(definition)?;
        let mut references = self.references.get(&key)?.clone();

        if !include_declaration {
            let declaration_kind = reference_kind_for_definition(definition.kind);

            references.retain(|reference| {
                reference.span != definition.span || reference.kind != declaration_kind
            });
        }

        sort_and_dedup_references(&mut references);
        Some(references)
    }

    fn finish(&mut self) {
        for references in self.references.values_mut() {
            sort_and_dedup_references(references);
        }
    }
}

impl ReferenceIndexKey {
    fn from_definition(definition: &Definition) -> Option<Self> {
        if definition.kind == DefinitionKind::Builtin {
            return None;
        }

        Some(Self {
            path: definition.path.clone(),
            start_line: definition.span.start.line,
            start_column: definition.span.start.column,
            end_line: definition.span.end.line,
            end_column: definition.span.end.column,
            kind: definition_kind_key(definition.kind),
        })
    }
}

fn sort_and_dedup_references(references: &mut Vec<Reference>) {
    references.sort_by_key(reference_key);
    references.dedup_by_key(|reference| reference_key(reference));
}

fn definition_kind_key(kind: DefinitionKind) -> u8 {
    match kind {
        DefinitionKind::Module => 0,
        DefinitionKind::Builtin => 1,
        DefinitionKind::TypeAlias => 2,
        DefinitionKind::Local => 3,
        DefinitionKind::Function => 4,
        DefinitionKind::Parameter => 5,
        DefinitionKind::TableField => 6,
    }
}

fn reference_kind_for_definition(kind: DefinitionKind) -> ReferenceKind {
    match kind {
        DefinitionKind::Module => ReferenceKind::Module,
        DefinitionKind::Builtin => ReferenceKind::Function,
        DefinitionKind::TypeAlias => ReferenceKind::TypeAlias,
        DefinitionKind::Local => ReferenceKind::Local,
        DefinitionKind::Function => ReferenceKind::Function,
        DefinitionKind::Parameter => ReferenceKind::Parameter,
        DefinitionKind::TableField => ReferenceKind::TableField,
    }
}

fn definition_kind_for_scope_declaration(kind: &ScopeDeclarationKind) -> Option<DefinitionKind> {
    match kind {
        ScopeDeclarationKind::Local => Some(DefinitionKind::Local),
        ScopeDeclarationKind::Parameter => Some(DefinitionKind::Parameter),
        ScopeDeclarationKind::Function => Some(DefinitionKind::Function),
        ScopeDeclarationKind::ModuleAlias { .. } | ScopeDeclarationKind::TableField => None,
    }
}

fn definition_kind_for_hir_symbol(kind: HirSymbolKind) -> Option<DefinitionKind> {
    match kind {
        HirSymbolKind::TypeAlias => Some(DefinitionKind::TypeAlias),
        HirSymbolKind::TableField => None,
        HirSymbolKind::Local => Some(DefinitionKind::Local),
        HirSymbolKind::Function => Some(DefinitionKind::Function),
        HirSymbolKind::Parameter => Some(DefinitionKind::Parameter),
    }
}

fn definition_from_hir_symbol_for_index(path: &Path, symbol: &HirSymbol) -> Definition {
    Definition {
        path: path.to_path_buf(),
        span: symbol.name_span,
        kind: definition_kind_for_hir_symbol(symbol.kind).unwrap_or(DefinitionKind::Local),
    }
}

fn is_project_global_reference(resolution: &ScopeResolution) -> bool {
    match resolution {
        ScopeResolution::Unresolved => true,
        ScopeResolution::Global(global) => global.kind == ScopeGlobalKind::Assigned,
        ScopeResolution::Symbol(_) => false,
    }
}

fn is_identifier_token(token: &TokenReference) -> bool {
    matches!(token.token().token_type(), TokenType::Identifier { .. })
}

fn token_span(token: &TokenReference) -> Option<SourceSpan> {
    token
        .range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn token_text(token: &TokenReference) -> String {
    token.to_string().trim().to_owned()
}

fn is_module_export_candidate(
    parsed: &ParsedSource,
    position: SourcePosition,
    candidates: &ModuleExportCandidates,
) -> bool {
    let Some(chain) = dotted_chain_at_position(&parsed.source, position) else {
        return false;
    };

    if chain.cursor_index == 0 {
        return false;
    }

    let Some(root) = chain.parts.first() else {
        return false;
    };
    let Some(member) = chain.parts.get(1) else {
        return false;
    };

    candidates
        .get(&root.text)
        .is_some_and(|names| names.contains(&member.text))
}

fn is_type_reference_position(parsed: &ParsedSource, position: SourcePosition) -> bool {
    let Some(line) = line_at(&parsed.source, position.line) else {
        return false;
    };
    let cursor = byte_for_column(line, position.column).min(line.len());
    let prefix = &line[..cursor];
    let Some(marker_index) = prefix.rfind([':', '|', '=']) else {
        return false;
    };
    let marker = prefix.as_bytes()[marker_index];
    let before_marker = &prefix[..marker_index];

    match marker {
        b':' => true,
        b'|' => before_marker.contains(':') || before_marker.contains("type "),
        b'=' => before_marker.contains("type "),
        _ => false,
    }
}

fn unknown_type_name(message: &str) -> Option<&str> {
    Some(
        message
            .strip_prefix("unknown type `")?
            .strip_suffix('`')?
            .trim(),
    )
}

fn is_type_annotation_position(parsed: &ParsedSource, position: SourcePosition) -> bool {
    parsed.local_annotations.iter().any(|annotation| {
        annotation
            .span
            .is_some_and(|span| contains_position(span, position))
    }) || parsed.function_annotations.iter().any(|function| {
        function.parameters.iter().any(|parameter| {
            parameter
                .span
                .is_some_and(|span| contains_position(span, position))
        })
    })
}

fn tua_type_uses_qualified_name(ty: &TuaType) -> bool {
    match ty {
        TuaType::Named(name) => name.contains('.'),
        TuaType::StringLiteral(_) => false,
        TuaType::Union(types) => types.iter().any(tua_type_uses_qualified_name),
        TuaType::Table(fields) => fields
            .iter()
            .any(|field| tua_type_uses_qualified_name(&field.ty)),
        TuaType::Array(item) => tua_type_uses_qualified_name(item),
        TuaType::Tuple(items) => items.iter().any(tua_type_uses_qualified_name),
        TuaType::Dictionary { key, value } => {
            tua_type_uses_qualified_name(key) || tua_type_uses_qualified_name(value)
        }
        TuaType::Function {
            parameters,
            returns,
        } => {
            parameters
                .iter()
                .any(|parameter| tua_type_uses_qualified_name(&parameter.ty))
                || returns.iter().any(tua_type_uses_qualified_name)
        }
    }
}

fn contains_position(span: SourceSpan, position: SourcePosition) -> bool {
    let start = position_key(span.start);
    let end = position_key(span.end);
    let position = position_key(position);

    start <= position && position < end
}

fn collect_type_fields(ty: &Type, fields: &mut BTreeMap<String, Type>) {
    match ty {
        Type::Table(table) => {
            fields.extend(
                table
                    .semantic_fields()
                    .into_iter()
                    .map(|(name, field)| (name.to_owned(), field.ty.clone())),
            );
        }
        Type::Union(types) => {
            for ty in types {
                collect_type_fields(ty, fields);
            }
        }
        Type::Any
        | Type::Alias(_)
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::StringLiteral(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. } => {}
    }
}

fn completion_kind_for_type(ty: &Type) -> CompletionKind {
    match ty {
        Type::Function(_) => CompletionKind::Function,
        _ => CompletionKind::Field,
    }
}

fn completion_request_key(path: &Path, position: SourcePosition) -> CompletionRequestKey {
    CompletionRequestKey {
        path: normalize_path(path),
        line: position.line,
        column: position.column,
    }
}

fn definition_request_key(path: &Path, position: SourcePosition) -> DefinitionRequestKey {
    DefinitionRequestKey {
        path: normalize_path(path),
        line: position.line,
        column: position.column,
    }
}

fn definition_lookup_span(source: &str, position: SourcePosition) -> Option<SourceSpan> {
    let chain = dotted_chain_at_position(source, position)?;

    chain.parts.get(chain.cursor_index).map(|part| part.span)
}

fn identifier_prefix_before(source: &str, position: SourcePosition) -> Option<String> {
    let line = line_at(source, position.line)?;
    let cursor = byte_for_position_column(line, position.column);
    let mut start = cursor;

    while start > 0 {
        let byte = line.as_bytes()[start - 1];

        if byte == b'_' || byte.is_ascii_alphanumeric() {
            start -= 1;
        } else {
            break;
        }
    }

    Some(line[start..cursor].to_owned())
}

fn typed_initializer_is_open(source: &str, start_line: usize, position: SourcePosition) -> bool {
    let Some(first_line) = line_at(source, start_line) else {
        return false;
    };

    if !first_line.contains("local ") || !first_line.contains(':') || !first_line.contains("= {") {
        return false;
    }

    let mut depth = 0isize;

    for line_number in start_line..=position.line {
        let Some(line) = line_at(source, line_number) else {
            continue;
        };
        let end = if line_number == position.line {
            byte_for_position_column(line, position.column)
        } else {
            line.len()
        };

        for byte in line[..end].bytes() {
            match byte {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
        }
    }

    depth > 0
}

fn byte_for_position_column(line: &str, column: usize) -> usize {
    let target = column.saturating_sub(1);

    line.char_indices()
        .nth(target)
        .map(|(index, _)| index)
        .unwrap_or(line.len())
}

fn parse_recovered_analysis_source(
    path: &Path,
    source: &str,
    kind: AnalysisSourceKind,
) -> Result<ParsedSource, Vec<Diagnostic>> {
    parse_analysis_source(
        path,
        source_without_recoverable_incomplete_lines(source),
        kind,
    )
}

fn source_without_recoverable_incomplete_lines(source: &str) -> String {
    let mut recovered = String::with_capacity(source.len());

    for line in source.split_inclusive('\n') {
        let content = line.strip_suffix('\n').unwrap_or(line);
        let content = content.strip_suffix('\r').unwrap_or(content);
        let trimmed = code_before_line_comment(content).trim();

        if line_is_recoverable_incomplete_edit(trimmed) {
            recovered.push('\n');
        } else {
            recovered.push_str(line);
        }
    }

    recovered
}

fn line_is_recoverable_incomplete_edit(trimmed_code: &str) -> bool {
    if trimmed_code.is_empty() {
        return false;
    }

    trimmed_code.ends_with('.')
        || trimmed_code.ends_with(':')
        || is_bare_dotted_field_chain(trimmed_code)
}

fn is_bare_dotted_field_chain(value: &str) -> bool {
    let mut saw_dot = false;
    let mut parts = value.split('.');

    let Some(first) = parts.next() else {
        return false;
    };

    if !is_simple_identifier(first) {
        return false;
    }

    for part in parts {
        saw_dot = true;

        if !is_simple_identifier(part) {
            return false;
        }
    }

    saw_dot
}

fn is_simple_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };

    if !(first == b'_' || first.is_ascii_alphabetic()) {
        return false;
    }

    bytes.all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
}

fn code_before_line_comment(line: &str) -> &str {
    line.split_once("--").map(|(code, _)| code).unwrap_or(line)
}

fn push_incoming_call(
    calls: &mut Vec<CallHierarchyIncomingCall>,
    from: CallHierarchySymbol,
    from_range: SourceSpan,
) {
    if let Some(existing) = calls
        .iter_mut()
        .find(|call| same_call_hierarchy_symbol(&call.from, &from))
    {
        existing.from_ranges.push(from_range);
        return;
    }

    calls.push(CallHierarchyIncomingCall {
        from,
        from_ranges: vec![from_range],
    });
}

fn push_outgoing_call(
    calls: &mut Vec<CallHierarchyOutgoingCall>,
    to: CallHierarchySymbol,
    from_range: SourceSpan,
) {
    if let Some(existing) = calls
        .iter_mut()
        .find(|call| same_call_hierarchy_symbol(&call.to, &to))
    {
        existing.from_ranges.push(from_range);
        return;
    }

    calls.push(CallHierarchyOutgoingCall {
        to,
        from_ranges: vec![from_range],
    });
}

fn sort_incoming_calls(calls: &mut [CallHierarchyIncomingCall]) {
    for call in calls.iter_mut() {
        sort_source_ranges(&mut call.from_ranges);
    }

    calls.sort_by_key(|call| call_hierarchy_symbol_key(&call.from));
}

fn sort_outgoing_calls(calls: &mut [CallHierarchyOutgoingCall]) {
    for call in calls.iter_mut() {
        sort_source_ranges(&mut call.from_ranges);
    }

    calls.sort_by_key(|call| call_hierarchy_symbol_key(&call.to));
}

fn sort_source_ranges(ranges: &mut [SourceSpan]) {
    ranges.sort_by_key(source_span_key);
}

fn source_span_key(span: &SourceSpan) -> (usize, usize, usize, usize) {
    (
        span.start.line,
        span.start.column,
        span.end.line,
        span.end.column,
    )
}

fn same_call_hierarchy_symbol(left: &CallHierarchySymbol, right: &CallHierarchySymbol) -> bool {
    left.path == right.path && left.selection_range == right.selection_range
}

fn definition_matches_call_symbol(definition: &Definition, symbol: &CallHierarchySymbol) -> bool {
    definition.path == symbol.path && definition.span == symbol.selection_range
}

fn call_symbol_targets_name(symbol: &CallHierarchySymbol, name: &str) -> bool {
    match &symbol.target {
        CallHierarchySymbolTarget::Local(candidate) => candidate == name,
        CallHierarchySymbolTarget::Qualified(parts) => {
            parts.len() == 1 && parts.first().is_some_and(|candidate| candidate == name)
        }
        CallHierarchySymbolTarget::Method { .. } | CallHierarchySymbolTarget::Export(_) => false,
    }
}

fn type_name_from_tua_type(ty: &TuaType) -> Option<String> {
    match ty {
        TuaType::Named(name) if name == "any" => None,
        TuaType::Named(name) => Some(name.clone()),
        TuaType::StringLiteral(_) => Some("string".to_owned()),
        TuaType::Union(types) => {
            let names = types
                .iter()
                .filter_map(type_name_from_tua_type)
                .filter(|name| name != "nil")
                .collect::<BTreeSet<_>>();

            (names.len() == 1)
                .then(|| names.into_iter().next())
                .flatten()
        }
        TuaType::Table(_)
        | TuaType::Array(_)
        | TuaType::Tuple(_)
        | TuaType::Dictionary { .. }
        | TuaType::Function { .. } => None,
    }
}

fn call_hierarchy_symbol_key(
    symbol: &CallHierarchySymbol,
) -> (PathBuf, usize, usize, usize, usize) {
    (
        symbol.path.clone(),
        symbol.selection_range.start.line,
        symbol.selection_range.start.column,
        symbol.selection_range.end.line,
        symbol.selection_range.end.column,
    )
}

impl AnalysisQueryCache {
    fn cached_file(
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

    fn cached_parsed_file(
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

    fn invalidate_changed_file(
        &mut self,
        path: &Path,
        source: &str,
        kind: AnalysisSourceKind,
        config: &ProjectConfig,
    ) {
        let invalidate_dependents =
            self.changed_file_affects_dependents(path, source, kind, config);

        self.invalidate_file(path, invalidate_dependents);
    }

    fn changed_file_affects_dependents(
        &mut self,
        path: &Path,
        source: &str,
        kind: AnalysisSourceKind,
        config: &ProjectConfig,
    ) -> bool {
        let path = normalize_path(path);

        if !self
            .importers
            .get(&path)
            .is_some_and(|importers| !importers.is_empty())
        {
            return false;
        }

        let Some(cached) = self.files.get(&path) else {
            return true;
        };

        if cached.kind != kind {
            return true;
        }

        let old_surface = DependencySurface::from_analysis_file(&cached.file);
        let new_require_targets = require_targets_for_source(&path, source);

        if old_surface.require_targets != new_require_targets {
            return true;
        }

        let (parsed, parsed_cache_hit) = self.cached_parsed_file(&path, source, kind);
        self.record_parse_cache_lookup(parsed_cache_hit);
        let Ok(parsed) = parsed else {
            return false;
        };

        let new_surface = DependencySurface::from_parsed(&parsed, config, new_require_targets);

        old_surface != new_surface
    }

    fn invalidate_file(&mut self, path: &Path, invalidate_dependents: bool) {
        let path = normalize_path(path);
        let mut invalidated = BTreeSet::new();

        self.invalidate_one(&path, &mut invalidated, false);

        if invalidate_dependents {
            self.invalidate_dependents(&path, &mut invalidated);
        }
    }

    fn invalidate_dependents(&mut self, path: &Path, invalidated: &mut BTreeSet<PathBuf>) {
        if let Some(importers) = self.importers.get(path).cloned() {
            for importer in importers {
                self.invalidate_one(&importer, invalidated, true);
            }
        }
    }

    fn invalidate_all(&mut self) {
        self.stats.invalidations += self.files.len();
        self.files.clear();
        self.dependencies.clear();
        self.importers.clear();
    }

    fn retain_paths(&mut self, live_paths: &BTreeSet<PathBuf>) {
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

    fn invalidate_one(
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

    fn update_dependencies(&mut self, path: &Path, require_edges: &[ModuleLink]) {
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

    fn record_file_hit(&mut self) {
        self.stats.parse_hits += 1;
        self.stats.hir_hits += 1;
        self.stats.scope_hits += 1;
        self.stats.type_environment_hits += 1;
        self.stats.diagnostics_hits += 1;
        self.stats.module_export_hits += 1;
        self.stats.require_edge_hits += 1;
        self.stats.symbol_index_hits += 1;
    }

    fn record_file_rebuild(&mut self, parsed_cache_hit: bool) {
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
    }

    fn record_parse_cache_lookup(&mut self, parsed_cache_hit: bool) {
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
    let hir = {
        let _profile = profile::span_for_path("analysis.lower.hir", path);

        parsed.as_ref().ok().map(lower_parsed_source)
    };
    let scope = {
        let _profile = profile::span_for_path("analysis.lower.scope", path);

        parsed.as_ref().ok().map(scope_map_parsed_source)
    };
    let environment = {
        let _profile = profile::span_for_path("analysis.types.environment", path);
        let _trace = profile::diagnostics_span(profile::DiagnosticsTraceCategory::TypeEnvironment);

        parsed
            .as_ref()
            .ok()
            .and_then(|parsed| build_analysis_type_environment(parsed, context.config))
    };
    let mut diagnostics = {
        let _profile = profile::span_for_path("analysis.check", path);
        let mut diagnostics_config = context.config.diagnostics.clone();

        diagnostics_config.inline_ignores = false;

        match &parsed {
            Ok(parsed) => {
                let asset_roots = analysis_asset_roots(context.source_roots, context.asset_roots);

                if let Some(environment) = environment.as_ref() {
                    check_parsed_source_with_environment_asset_roots_diagnostics_jit_config_and_asset_index(
                        parsed,
                        environment,
                        asset_roots,
                        &diagnostics_config,
                        &context.config.jit,
                        context.asset_index,
                    )
                } else {
                    check_parsed_source_with_asset_roots_diagnostics_jit_and_ecs_config_and_asset_index(
                        parsed,
                        asset_roots,
                        &diagnostics_config,
                        &context.config.jit,
                        &context.config.ecs,
                        context.asset_index,
                    )
                }
                .err()
                .unwrap_or_default()
            }
            Err(diagnostics) => {
                let mut diagnostics = diagnostics.clone();

                if let Ok(recovered) = parse_recovered_analysis_source(path, &source, kind) {
                    diagnostics.extend(
                        check_parsed_source_with_asset_roots_diagnostics_jit_and_ecs_config_and_asset_index(
                            &recovered,
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

            if let Err(mut emit_diagnostics) =
                emit_lua_with_mode(parsed, context.config.emit.annotation_mode)
            {
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
    if !context.plugin_registry.is_empty() {
        let _profile = profile::span_for_path("analysis.plugins.diagnostics", path);
        let parsed = parsed.as_ref().ok();

        diagnostics.extend(context.plugin_registry.diagnostics(PluginContext {
            path,
            source: &source,
            parsed,
            assets: &assets,
        }));
    }
    let diagnostics = deduplicate_diagnostics(diagnostics);
    let exports = {
        let _profile = profile::span_for_path("analysis.index.exports", path);
        let _trace = profile::diagnostics_span(profile::DiagnosticsTraceCategory::ModuleGraph);

        parsed.as_ref().ok().and_then(|parsed| {
            infer_module_exports_with_optional_environment(parsed, environment.as_ref())
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
            .map(|parsed| symbol_entries_for_parsed(parsed, hir.as_ref(), context.source_roots))
            .unwrap_or_default()
    };

    AnalysisFile {
        source,
        kind,
        parsed,
        hir,
        scope,
        environment,
        diagnostics,
        exports,
        require_edges,
        assets,
        symbols,
    }
}

fn analysis_asset_roots<'a>(
    source_roots: &'a [PathBuf],
    asset_roots: &'a [PathBuf],
) -> &'a [PathBuf] {
    if asset_roots.is_empty() {
        source_roots
    } else {
        asset_roots
    }
}

fn module_file_definition(path: &Path, span: SourceSpan) -> Definition {
    Definition {
        path: path.to_path_buf(),
        span,
        kind: DefinitionKind::Module,
    }
}

fn format_module_export_hover(name: &str, ty: &Type) -> String {
    let ty = ty.to_string();

    if let Some(signature) = ty.strip_prefix("function") {
        format!("function {name}{signature}")
    } else {
        format!("(field) {name}: {ty}")
    }
}

fn module_export_documentation(
    export: &ModuleExportField,
    source_documentation: Option<String>,
) -> Option<String> {
    let mut parts = Vec::new();

    if let Some(documentation) = source_documentation.as_deref() {
        push_unique_documentation(&mut parts, documentation);
    }

    if let Some(documentation) = export.documentation.as_deref() {
        push_unique_documentation(&mut parts, documentation);
    }

    for documentation in export
        .parameter_documentation
        .iter()
        .filter_map(|documentation| documentation.as_deref())
    {
        push_unique_documentation(&mut parts, documentation);
    }

    (!parts.is_empty()).then(|| parts.join("\n"))
}

fn push_unique_documentation(parts: &mut Vec<String>, documentation: &str) {
    for line in documentation
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if !parts.iter().any(|part| part == line) {
            parts.push(line.to_owned());
        }
    }
}

fn leading_plain_documentation(source: &str, definition_line: usize) -> Option<String> {
    let mut docs = Vec::new();

    let lines = source
        .lines()
        .take(definition_line.saturating_sub(1))
        .collect::<Vec<_>>();

    for line in lines.into_iter().rev() {
        let trimmed = line.trim();

        if let Some(doc) = trimmed.strip_prefix("---") {
            let doc = doc.trim();

            if !doc.starts_with('@') && !doc.is_empty() {
                docs.push(doc.to_owned());
            }

            continue;
        }

        if trimmed.is_empty() {
            continue;
        }

        break;
    }

    docs.reverse();

    (!docs.is_empty()).then(|| docs.join("\n"))
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

fn symbol_entries_for_parsed(
    parsed: &ParsedSource,
    hir: Option<&HirModule>,
    source_roots: &[PathBuf],
) -> Vec<SymbolIndexEntry> {
    SymbolIndex::build([(&parsed.path, hir)], &ModuleGraph::default(), source_roots)
        .entries()
        .to_vec()
}

fn default_span() -> SourceSpan {
    SourceSpan {
        start: SourcePosition { line: 1, column: 1 },
        end: SourcePosition { line: 1, column: 1 },
    }
}

fn parse_analysis_source(
    path: &Path,
    source: String,
    kind: AnalysisSourceKind,
) -> Result<ParsedSource, Vec<Diagnostic>> {
    match kind {
        AnalysisSourceKind::Lua => parse_lua_source(path, source),
        AnalysisSourceKind::Tua => parse_tua_source(path, source),
    }
}

fn fast_diagnostics_for_source(
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
        check_parsed_source_with_environment_asset_roots_diagnostics_jit_config(
            &parsed,
            &environment,
            &[],
            &fast_config,
            &crate::config::JitConfig::default(),
        )
        .err()
        .unwrap_or_default(),
    );

    let diagnostics = fast_lane_diagnostics(deduplicate_diagnostics(diagnostics));

    apply_inline_ignores(path, source, diagnostics, &config.diagnostics)
}

fn build_analysis_type_environment(
    parsed: &ParsedSource,
    config: &ProjectConfig,
) -> Option<TypeEnvironment> {
    let (environment, diagnostics) =
        build_analysis_type_environment_with_diagnostics(parsed, config);

    diagnostics.is_empty().then_some(environment)
}

fn build_analysis_type_environment_with_diagnostics(
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

fn fast_diagnostics_config(
    config: &crate::config::DiagnosticsConfig,
) -> crate::config::DiagnosticsConfig {
    let mut fast = crate::config::DiagnosticsConfig::default();

    fast.undefined_variables = config.undefined_variables;
    fast.globals = config.globals.clone();

    fast
}

fn fast_lane_diagnostics(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.code.is_some_and(is_fast_lane_diagnostic_code))
        .collect()
}

fn is_fast_lane_diagnostic_code(code: crate::DiagnosticCode) -> bool {
    matches!(
        code,
        crate::DiagnosticCode::SYNTAX_PARSE
            | crate::DiagnosticCode::SYNTAX_COMPOUND_ASSIGNMENT
            | crate::DiagnosticCode::TYPE_UNKNOWN_NAME
            | crate::DiagnosticCode::TYPE_DUPLICATE_ALIAS
            | crate::DiagnosticCode::TYPE_MISMATCH
            | crate::DiagnosticCode::TYPE_ARGUMENT_COUNT
            | crate::DiagnosticCode::TYPE_IMPOSSIBLE_COMPARISON
            | crate::DiagnosticCode::LINT_UNDEFINED_VARIABLE
    )
}

fn require_targets_for_source(path: &Path, source: &str) -> BTreeSet<PathBuf> {
    require_targets_for_links(&module_links_tua_source(path, source.to_owned()))
}

fn require_targets_for_links(require_edges: &[ModuleLink]) -> BTreeSet<PathBuf> {
    require_edges
        .iter()
        .map(|edge| normalize_path(&edge.target))
        .collect()
}

fn parsed_cache_key(path: &Path, source: &str, kind: AnalysisSourceKind) -> ParsedCacheKey {
    ParsedCacheKey {
        path: normalize_path(path),
        kind,
        fingerprint: source_fingerprint(source),
    }
}

fn source_fingerprint(source: &str) -> SourceFingerprint {
    let mut hasher = DefaultHasher::new();

    source.hash(&mut hasher);

    SourceFingerprint {
        len: source.len(),
        hash: hasher.finish(),
    }
}

fn infer_source_kind(path: &Path, config: &ProjectConfig) -> AnalysisSourceKind {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("lua") => AnalysisSourceKind::Lua,
        Some(extension) if extension == config.extension => AnalysisSourceKind::Tua,
        _ => AnalysisSourceKind::Tua,
    }
}

fn vfs_source_kind(kind: AnalysisSourceKind) -> VfsSourceKind {
    match kind {
        AnalysisSourceKind::Lua => VfsSourceKind::Lua,
        AnalysisSourceKind::Tua => VfsSourceKind::Tua,
    }
}

fn analysis_source_kind(kind: VfsSourceKind) -> AnalysisSourceKind {
    match kind {
        VfsSourceKind::Lua => AnalysisSourceKind::Lua,
        VfsSourceKind::Tua => AnalysisSourceKind::Tua,
    }
}

#[cfg(test)]
mod tests;

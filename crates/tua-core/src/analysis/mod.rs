use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use full_moon::ast::{self, Expression};
use full_moon::node::Node;
use full_moon::tokenizer::{TokenReference, TokenType};

use crate::asset::{self, AssetMetadata};
use crate::ast_utils::{
    assignment_expression_for_local, field_chain_before_first_call, plain_field_chain_from_var,
};
use crate::completion::{
    complete_field_access_in_source_with_environment_and_ranking_inputs,
    complete_source_with_parsed_environment_and_ranking_inputs,
    complete_source_with_parsed_environment_and_ranking_options, filter_and_rank_completions,
    multitoken::complete_multi_token_source, CompletionAdaptiveStats, CompletionMultiTokenConfig,
    CompletionRankingInputs, CompletionSequenceRankingConfig,
};
use crate::folding_range::folding_ranges_source;
use crate::hir::{HirExportValue, HirModule, HirRequireBinding, HirSymbol, HirSymbolKind};
use crate::hover::hover_parsed_source_with_environment;
use crate::ide_db::{SymbolIndexEntry, SymbolIndexKind};
use crate::inlay_hint::{
    inlay_hints_parsed_source_with_environment, inlay_hints_parsed_source_with_environment_in_range,
};
use crate::inline_ignore::apply_inline_ignores;
use crate::module_index::infer_module_exports_with_optional_environment;
use crate::module_link::{
    module_specifier_at_position, module_specifier_literals, ModuleSpecifierKind,
};
use crate::plugin::{PluginContext, PluginRegistry};
use crate::profile;
use crate::scope::{ScopeDeclarationKind, ScopeGlobalKind, ScopeResolution};
use crate::scope_map_parsed_source;
use crate::selection_range::selection_ranges_source;
use crate::semantic::SemanticModel;
use crate::signal::{graph_call_from_mention, SignalGraph, SignalGraphEvent};
use crate::signature_help::signature_help_parsed_source_with_environment;
use crate::source_text::{
    byte_for_column, dotted_chain_at_position, function_name_table_field, line_at,
};
use crate::text_match::text_matches;
use crate::types::{
    build_type_environment_with_builtin_aliases, build_type_environment_with_diagnostics,
};
use crate::usage_index::{FileUsageIndex, ProjectUsageIndex};
use crate::vfs::{
    module_name_for_path, normalize_path, ModulePathSeparator, VfsSnapshot, VfsSourceKind,
    VirtualFileSystem,
};
use crate::{
    assists_parsed_source, call_hierarchy_calls_parsed_source,
    call_hierarchy_prepare_parsed_source, call_hierarchy_symbols_parsed_source,
    check::{
        check_parsed_source_with_asset_roots_diagnostics_jit_and_ecs_config_and_asset_index,
        check_parsed_source_with_environment_asset_roots_diagnostics_jit_config_and_asset_index,
    },
    code_actions_parsed_source_with_kinds, complete_tua_source, definition_parsed_source_with_hir,
    document_highlights_parsed_source, document_symbols_parsed_source_with_hir,
    exported_field_for_fields, global_function_definition_count,
    global_function_name_for_definition, global_references_to_definition_parsed_source_with_hir,
    global_type_alias_name_for_definition,
    global_type_alias_references_to_definition_parsed_source_with_hir, hover_parsed_source,
    inlay_hints_parsed_source, is_valid_rename_identifier, lower_parsed_source,
    module_links_tua_source, parse_lua_source, parse_tua_source, reference_kind_for_rename,
    references_to_definition_parsed_source_with_artifacts, rename_target_for_definition,
    require_bindings_for_parsed_source, semantic_tokens_parsed_source, semantic_tokens_tua_source,
    signature_help_parsed_source, signature_help_tua_source, type_alias_definition_count, Assist,
    AssistKind, AssistRange, CallHierarchyCallSite, CallHierarchyCallTarget,
    CallHierarchyIncomingCall, CallHierarchyOutgoingCall, CallHierarchySymbol,
    CallHierarchySymbolTarget, CodeLens, CodeLensReferenceKind, CodeLensReferenceTarget,
    CodeLensTarget, Completion, CompletionKind, Definition, DefinitionKind, Diagnostic,
    DocumentHighlight, DocumentSymbol, FoldingRange, Hover, InlayHint, ModuleExportField,
    ModuleExports, ModuleGraph, ModuleGraphInput, ModuleGraphOptions, ModuleLink, ParsedSource,
    ProjectConfig, ProjectTypeDatabase, ProjectTypeDatabaseInput, Reference, ReferenceKind,
    RenameEdit, RenameTarget, RequireResolution, ScopeMap, SelectionRange, SemanticIndex,
    SemanticToken, SignatureHelp, SourcePosition, SourceSpan, SymbolIndex, TableField, TextEdit,
    TuaType, Type, TypeEnvironment, TypedFunction, WorkspaceSymbol,
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
    require_path_separator: ModulePathSeparator,
    asset_roots: Vec<PathBuf>,
    asset_index: Option<asset::AssetIndex>,
    builtin_catalogs: BuiltinCatalogs,
    plugin_registry: PluginRegistry,
    cache: RefCell<AnalysisQueryCache>,
    last_good_files: RefCell<BTreeMap<PathBuf, LastGoodAnalysisFile>>,
    completion_adaptive_stats: CompletionAdaptiveStats,
    completion_sequence_ranking: CompletionSequenceRankingConfig,
    completion_multi_token: CompletionMultiTokenConfig,
    completion_cache: Arc<Mutex<BTreeMap<CompletionRequestKey, Vec<Completion>>>>,
    completion_cache_stats: Arc<Mutex<CompletionCacheStats>>,
    definition_cache: Arc<Mutex<BTreeMap<DefinitionRequestKey, Option<Definition>>>>,
    snapshot_cache: Arc<Mutex<Option<CachedAnalysisSnapshot>>>,
    revision: u64,
}

#[derive(Clone, Debug)]
pub struct AnalysisSnapshot {
    revision: u64,
    config: ProjectConfig,
    vfs: VfsSnapshot,
    asset_roots: Vec<PathBuf>,
    asset_index: Option<asset::AssetIndex>,
    files: BTreeMap<PathBuf, AnalysisFile>,
    require_path_separator: ModulePathSeparator,
    builtin_catalogs: BuiltinCatalogs,
    plugin_registry: PluginRegistry,
    module_graph: ModuleGraph,
    type_database: ProjectTypeDatabase,
    symbol_index: SymbolIndex,
    usage_index: ProjectUsageIndex,
    last_good_files: BTreeMap<PathBuf, LastGoodAnalysisFile>,
    completion_adaptive_stats: CompletionAdaptiveStats,
    completion_sequence_ranking: CompletionSequenceRankingConfig,
    completion_multi_token: CompletionMultiTokenConfig,
    semantic_facts: Arc<Mutex<BTreeMap<PathBuf, Arc<AnalysisSemanticFacts>>>>,
    completion_cache: Arc<Mutex<BTreeMap<CompletionRequestKey, Vec<Completion>>>>,
    completion_cache_stats: Arc<Mutex<CompletionCacheStats>>,
    definition_cache: Arc<Mutex<BTreeMap<DefinitionRequestKey, Option<Definition>>>>,
    project_diagnostics_cache: Arc<Mutex<BTreeMap<PathBuf, Vec<Diagnostic>>>>,
    reverse_reference_index: Arc<OnceLock<ReverseReferenceIndex>>,
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
    usage_index: Arc<FileUsageIndex>,
    #[allow(dead_code)]
    symbols: Vec<SymbolIndexEntry>,
}

#[derive(Clone, Debug)]
struct AnalysisSemanticFacts {
    environment: TypeEnvironment,
    semantic_index: SemanticIndex,
    project_inference: ProjectLocalInference,
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
    project_inference: ProjectLocalInference,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ModuleFileRenamePlan {
    old_path: PathBuf,
    new_path: PathBuf,
    source_root: PathBuf,
    new_module_name: String,
}

#[derive(Clone, Debug, Default)]
struct ProjectLocalInference {
    local_types: Vec<(String, Type)>,
    field_origins: BTreeMap<String, ProjectAliasFieldOrigin>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProjectAliasFieldOrigin {
    module_path: PathBuf,
    alias_name: String,
}

#[derive(Clone, Copy, Debug)]
struct AnalysisBuildContext<'a> {
    config: &'a ProjectConfig,
    source_roots: &'a [PathBuf],
    output_roots: &'a [PathBuf],
    known_paths: &'a BTreeSet<PathBuf>,
    require_path_separator: ModulePathSeparator,
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
    pub usage_index_hits: usize,
    pub usage_index_misses: usize,
    pub completion_context_hits: usize,
    pub completion_context_misses: usize,
    pub invalidations: usize,
    pub dependent_invalidations: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CompletionCacheStats {
    hits: usize,
    misses: usize,
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
    revision: u64,
    path: PathBuf,
    line: usize,
    column: usize,
    fingerprint: SourceFingerprint,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct DefinitionRequestKey {
    path: PathBuf,
    line: usize,
    column: usize,
    fingerprint: SourceFingerprint,
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

    pub fn plugin_registry(&self) -> &PluginRegistry {
        &self.plugin_registry
    }

    pub fn set_plugin_registry(&mut self, registry: PluginRegistry) {
        self.plugin_registry = registry;
        self.invalidate_all_analysis();
        self.bump_revision();
    }

    fn effective_plugin_registry(&self) -> PluginRegistry {
        let mut registry = PluginRegistry::new();

        let _ = registry.register(crate::sprite::SpritePlugin);

        if self.config.ecs.enabled {
            let _ = registry.register(crate::ecs::EcsPlugin);
        }

        registry.extend_missing(&self.plugin_registry);

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
                            output_roots: &vfs.options().output_roots,
                            known_paths: &live_paths,
                            require_path_separator: self.require_path_separator,
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
            plugin_registry,
            module_graph,
            type_database,
            symbol_index,
            usage_index,
            last_good_files: self.last_good_files.borrow().clone(),
            completion_adaptive_stats: self.completion_adaptive_stats.clone(),
            completion_sequence_ranking: self.completion_sequence_ranking.clone(),
            completion_multi_token: self.completion_multi_token.clone(),
            semantic_facts: Arc::new(Mutex::new(BTreeMap::new())),
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

        if let Some(asset) = asset::asset_at_position(&file.assets, position) {
            return Some(Hover {
                contents: asset.hover_markdown(),
                span: Some(asset.span),
                documentation: None,
                context: Vec::new(),
            });
        }

        file.environment
            .as_ref()
            .and_then(|environment| {
                hover_parsed_source_with_environment(parsed, position, environment)
            })
            .or_else(|| hover_parsed_source(parsed, position).ok().flatten())
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
                let environment = file.environment.clone().unwrap_or_else(|| {
                    let (environment, _) = build_type_environment_with_diagnostics(parsed);

                    environment
                });

                complete_source_with_parsed_environment_and_ranking_options(
                    &file.source,
                    parsed,
                    position,
                    &environment,
                    &[],
                    None,
                    CompletionAdaptiveStats::file_scope_hash(path),
                    &self.completion_sequence_ranking,
                )
            }
            Err(_) => complete_tua_source(path, file.source, position).unwrap_or_default(),
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
            Err(_) => semantic_tokens_tua_source(&path, file.source).unwrap_or_default(),
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
        let plugin_registry = self.effective_plugin_registry();

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
                plugin_registry: &plugin_registry,
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
                    hir: file.hir.clone(),
                    environment: file.environment.clone(),
                },
            );
        }
    }
}

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

        diagnostics.extend(self.project_type_diagnostics(path.as_ref(), file));

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

    fn record_completion_cache_hit(&self) {
        self.completion_cache_stats
            .lock()
            .expect("completion cache stats lock poisoned")
            .hits += 1;
    }

    fn record_completion_cache_miss(&self) {
        self.completion_cache_stats
            .lock()
            .expect("completion cache stats lock poisoned")
            .misses += 1;
    }

    fn completion_uncached(
        &self,
        path: &Path,
        file: &AnalysisFile,
        source_override: Option<&str>,
        position: SourcePosition,
    ) -> Vec<Completion> {
        let source = source_override.unwrap_or(&file.source);
        let file_scope_hash = CompletionAdaptiveStats::file_scope_hash(path);
        let completions = match &file.parsed {
            Ok(parsed) => {
                let completions =
                    self.project_completions(parsed, source, position, file_scope_hash);

                if !completions.is_empty() {
                    completions
                } else if matches!(
                    crate::completion::context::completion_context(source, position),
                    Some(crate::completion::context::CompletionContext::Identifier { .. })
                ) {
                    let environment = file.environment.clone().unwrap_or_else(|| {
                        let (environment, _) = build_type_environment_with_diagnostics(parsed);

                        environment
                    });

                    complete_source_with_parsed_environment_and_ranking_inputs(
                        source,
                        parsed,
                        position,
                        &environment,
                        self.completion_ranking_inputs(file_scope_hash),
                    )
                } else if let Some(facts) = self.semantic_facts_for_parsed(parsed) {
                    complete_source_with_parsed_environment_and_ranking_inputs(
                        source,
                        parsed,
                        position,
                        &facts.environment,
                        self.completion_ranking_inputs(file_scope_hash),
                    )
                } else {
                    let (environment, _) = build_type_environment_with_diagnostics(parsed);

                    complete_source_with_parsed_environment_and_ranking_inputs(
                        source,
                        parsed,
                        position,
                        &environment,
                        self.completion_ranking_inputs(file_scope_hash),
                    )
                }
            }
            Err(_) => {
                if let Some(facts) = self.recovered_semantic_facts(path, file) {
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

                        if !completions.is_empty() {
                            completions
                        } else {
                            complete_tua_source(path, source.to_owned(), position)
                                .unwrap_or_default()
                        }
                    }
                } else {
                    complete_tua_source(path, source.to_owned(), position).unwrap_or_default()
                }
            }
        };

        completions
    }

    fn completion_ranking_inputs(&self, file_scope_hash: u64) -> CompletionRankingInputs<'_> {
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

        let definition = match &file.parsed {
            Ok(parsed) => self.definition_parsed_snapshot(parsed, file.hir.as_ref(), position),
            Err(_) => {
                let facts = self.recovered_semantic_facts(path, file)?;

                self.definition_parsed_snapshot_with_facts(
                    &facts.parsed,
                    facts.hir.as_ref(),
                    position,
                    Some(&facts),
                )
            }
        };

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

    fn definition_parsed_snapshot(
        &self,
        parsed: &ParsedSource,
        hir: Option<&HirModule>,
        position: SourcePosition,
    ) -> Option<Definition> {
        self.definition_parsed_snapshot_with_facts(parsed, hir, position, None)
    }

    fn definition_parsed_snapshot_with_facts(
        &self,
        parsed: &ParsedSource,
        hir: Option<&HirModule>,
        position: SourcePosition,
        recovered_facts: Option<&RecoveredSemanticFacts>,
    ) -> Option<Definition> {
        if let Some(definition) = self.module_definition(parsed, position) {
            return Some(definition);
        }

        if self.is_module_reference(parsed, position) {
            return None;
        }

        if let Some(definition) = self.project_imported_return_alias_field_definition(
            parsed,
            position,
            recovered_facts.map(|facts| &facts.project_inference),
        ) {
            return Some(definition);
        }

        if let Some(definition) =
            recovered_facts.and_then(|facts| facts.semantic_index.definition_at(position))
        {
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

    pub fn type_definition(
        &self,
        path: impl AsRef<Path>,
        position: SourcePosition,
    ) -> Option<Definition> {
        let _profile = profile::span("analysis.query.type_definition");
        let path = path.as_ref();
        let file = self.file(path)?;

        match &file.parsed {
            Ok(parsed) => {
                self.type_definition_parsed_snapshot(parsed, file.hir.as_ref(), position, None)
            }
            Err(_) => {
                let facts = self.recovered_semantic_facts(path, file)?;

                self.type_definition_parsed_snapshot(
                    &facts.parsed,
                    facts.hir.as_ref(),
                    position,
                    Some(&facts),
                )
            }
        }
    }

    fn type_definition_parsed_snapshot(
        &self,
        parsed: &ParsedSource,
        hir: Option<&HirModule>,
        position: SourcePosition,
        recovered_facts: Option<&RecoveredSemanticFacts>,
    ) -> Option<Definition> {
        if is_type_reference_position(parsed, position) {
            if let Some(definition) = self.type_alias_definition_at_position(parsed, hir, position)
            {
                return Some(definition);
            }
        }

        let definition =
            self.definition_parsed_snapshot_with_facts(parsed, hir, position, recovered_facts);

        if definition.as_ref().is_some_and(is_type_definition_kind) {
            return definition;
        }

        let environment = self.semantic_query_environment(parsed, recovered_facts);
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
            Ok(parsed) => {
                self.implementation_parsed_snapshot(parsed, file.hir.as_ref(), position, None)
            }
            Err(_) => {
                let Some(facts) = self.recovered_semantic_facts(path, file) else {
                    return Vec::new();
                };

                self.implementation_parsed_snapshot(
                    &facts.parsed,
                    facts.hir.as_ref(),
                    position,
                    Some(&facts),
                )
            }
        };

        definition.into_iter().collect()
    }

    fn implementation_parsed_snapshot(
        &self,
        parsed: &ParsedSource,
        hir: Option<&HirModule>,
        position: SourcePosition,
        recovered_facts: Option<&RecoveredSemanticFacts>,
    ) -> Option<Definition> {
        let environment = self.semantic_query_environment(parsed, recovered_facts);

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
            self.definition_parsed_snapshot_with_facts(parsed, hir, position, recovered_facts)?;

        if definition.kind != DefinitionKind::Function {
            return None;
        }

        if let Some(name) = global_function_name_for_definition(parsed, &definition) {
            return (global_function_definition_count(parsed, &name) == 1).then_some(definition);
        }

        Some(definition)
    }

    fn type_alias_definition_at_position(
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

    fn type_origin_definition(
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

    fn type_alias_definition_for_name(
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

        type_alias_definition_in_parsed(parsed, file.hir.as_ref(), &binding.name).or_else(|| {
            self.module_graph
                .export(&binding.module_path, &binding.name)
                .map(|entry| Definition {
                    path: binding.module_path.clone(),
                    span: entry.definition_span,
                    kind: DefinitionKind::TableField,
                })
        })
    }

    fn type_at_position(
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

    fn type_for_parts(
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

    fn field_type_for_type(&self, source_path: &Path, ty: &Type, field: &str) -> Option<Type> {
        match self
            .type_database
            .resolve_type(&self.module_graph, source_path, ty)
        {
            Type::Table(table) => table.semantic_field(field).map(|field| field.ty.clone()),
            Type::Union(types) => {
                let mut fields = types
                    .iter()
                    .filter_map(|ty| self.field_type_for_type(source_path, ty, field))
                    .collect::<Vec<_>>();

                match fields.len() {
                    0 => None,
                    1 => fields.pop(),
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

    fn typed_receiver_method_implementation(
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

    fn method_implementation_for_alias(
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

    fn semantic_query_environment(
        &self,
        parsed: &ParsedSource,
        recovered_facts: Option<&RecoveredSemanticFacts>,
    ) -> TypeEnvironment {
        if let Some(facts) = recovered_facts {
            return facts.environment.clone();
        }

        if let Some(facts) = self.semantic_facts_for_parsed(parsed) {
            return facts.environment.clone();
        }

        let project_inference = self.project_local_inference(parsed);
        let base_environment = self
            .file(&parsed.path)
            .and_then(|file| file.environment.clone());

        self.recovered_project_type_environment_for_parsed(
            parsed,
            base_environment,
            &project_inference,
        )
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

    fn module_file_rename_plan(
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

    fn source_root_for_path(&self, path: &Path) -> Option<PathBuf> {
        self.source_roots()
            .iter()
            .filter(|root| path.starts_with(root))
            .max_by_key(|root| root.components().count())
            .cloned()
    }

    fn module_name_for_known_path(&self, path: &Path) -> Option<String> {
        module_name_for_path(
            self.source_root_for_path(path)?,
            path,
            self.require_path_separator,
        )
    }

    fn known_module_paths_for_name(&self, name: &str, source_root: &Path) -> Vec<PathBuf> {
        self.files
            .keys()
            .filter(|path| path.starts_with(source_root))
            .filter(|path| self.module_name_for_known_path(path).as_deref() == Some(name))
            .cloned()
            .collect()
    }

    fn is_generated_path(&self, path: &Path) -> bool {
        self.vfs
            .options()
            .output_roots
            .iter()
            .any(|root| path.starts_with(root))
    }

    fn module_specifier_candidates(
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

    fn project_module_candidates(&self, current_path: &Path, specifier: &str) -> Vec<PathBuf> {
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

    fn child_import_module_candidates(&self, current_path: &Path, specifier: &str) -> Vec<PathBuf> {
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

    fn renamed_module_specifier(
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

    fn import_specifier_for_new_path(
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

    pub fn code_lenses(&self, path: impl AsRef<Path>) -> Vec<CodeLens> {
        let path = normalize_path(path.as_ref());
        let Some(file) = self.file(&path) else {
            return Vec::new();
        };
        let Some(hir) = file.hir.as_ref() else {
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

    fn exported_function_code_lenses(&self, path: &Path, hir: &HirModule) -> Vec<CodeLens> {
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

    fn type_alias_code_lenses(&self, path: &Path) -> Vec<CodeLens> {
        let alias_counts = self.type_alias_name_counts();

        self.symbol_index
            .entries_for_path(path)
            .into_iter()
            .filter(|entry| entry.kind == SymbolIndexKind::TypeAlias)
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

    fn type_alias_name_counts(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();

        for entry in self.symbol_index.entries() {
            if entry.kind != SymbolIndexKind::TypeAlias {
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
        requested_kinds: &[AssistKind],
    ) -> Vec<Assist> {
        let path = path.as_ref();
        let Some(parsed) = self.file(path).and_then(|file| file.parsed.as_ref().ok()) else {
            return Vec::new();
        };
        let source_dir = self.assist_source_root_for_path(path);

        code_actions_parsed_source_with_kinds(
            parsed,
            diagnostics,
            range,
            &source_dir,
            &self.config.extension,
            requested_kinds,
        )
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

        if target.kind != DefinitionKind::TableField {
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
            None,
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
            let semantic_facts = (target.kind == DefinitionKind::TableField)
                .then(|| self.semantic_facts_for_parsed(parsed))
                .flatten();
            let Ok(mut file_references) = references_to_definition_parsed_source_with_artifacts(
                parsed,
                target,
                include_declaration,
                file.hir.as_ref(),
                file.scope.as_ref(),
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
        let path = path.as_ref();
        let Some(file) = self.file(path) else {
            return Vec::new();
        };
        let parsed = match &file.parsed {
            Ok(parsed) => parsed,
            Err(_) => {
                return self
                    .recovered_semantic_facts(path, file)
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
                    .recovered_semantic_facts(path, file)
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
        let Some(facts) = self.semantic_facts_for_parsed(parsed) else {
            return inlay_hints_parsed_source(parsed)
                .map(|hints| {
                    hints
                        .into_iter()
                        .filter(|hint| contains_position(range, hint.position))
                        .collect()
                })
                .unwrap_or_default();
        };

        inlay_hints_parsed_source_with_environment_in_range(parsed, &facts.environment, Some(range))
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

    pub fn signal_graph(&self) -> SignalGraph {
        let mut events = BTreeMap::<String, Vec<_>>::new();

        for event in self.usage_index.signal_events() {
            let call = graph_call_from_mention(event.path.clone(), event.mention.clone());

            events.entry(call.event.clone()).or_default().push(call);
        }

        SignalGraph {
            events: events
                .into_iter()
                .map(|(name, mut calls)| {
                    calls.sort_by(|left, right| {
                        left.path
                            .cmp(&right.path)
                            .then_with(|| left.span.start.line.cmp(&right.span.start.line))
                            .then_with(|| left.span.start.column.cmp(&right.span.start.column))
                            .then_with(|| left.method.cmp(&right.method))
                    });

                    SignalGraphEvent { name, calls }
                })
                .collect(),
        }
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

    fn module_definition(
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
            RequireResolution::Missing { .. } | RequireResolution::Invalid { .. } => None,
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

        let environment = self.semantic_environment(parsed).unwrap_or_else(|| {
            let (environment, _) = build_type_environment_with_diagnostics(parsed);

            environment.with_local_types(self.project_local_annotation_types(parsed))
        });

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

    fn project_completions_with_environment(
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

    fn semantic_facts_for_parsed(
        &self,
        parsed: &ParsedSource,
    ) -> Option<Arc<AnalysisSemanticFacts>> {
        let path = normalize_path(&parsed.path);

        if let Some(facts) = self
            .semantic_facts
            .lock()
            .expect("semantic facts lock poisoned")
            .get(&path)
            .cloned()
        {
            return Some(facts);
        }

        let file = self.file(&path)?;
        let _profile = profile::span_for_path("analysis.semantic.facts.file", &path);
        let project_inference = self.project_local_inference(parsed);
        let environment =
            self.project_type_environment_for_file(file, parsed, &project_inference)?;
        let facts = Arc::new(AnalysisSemanticFacts {
            semantic_index: SemanticIndex::build(parsed, file.hir.as_ref(), &environment),
            environment,
            project_inference,
        });

        self.semantic_facts
            .lock()
            .expect("semantic facts lock poisoned")
            .insert(path, Arc::clone(&facts));

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
        let project_inference = self.project_local_inference(&parsed);
        let environment = self.recovered_project_type_environment_for_parsed(
            &parsed,
            file.environment.clone(),
            &project_inference,
        );
        let semantic_index = SemanticIndex::build(&parsed, hir.as_ref(), &environment);

        Some(RecoveredSemanticFacts {
            parsed,
            hir,
            environment,
            semantic_index,
            project_inference,
        })
    }

    fn last_good_semantic_facts(&self, path: &Path) -> Option<RecoveredSemanticFacts> {
        let last_good = self.last_good_files.get(&normalize_path(path))?.clone();
        let project_inference = self.project_local_inference(&last_good.parsed);
        let environment = self.recovered_project_type_environment_for_parsed(
            &last_good.parsed,
            last_good.environment,
            &project_inference,
        );
        let semantic_index =
            SemanticIndex::build(&last_good.parsed, last_good.hir.as_ref(), &environment);

        Some(RecoveredSemanticFacts {
            parsed: last_good.parsed,
            hir: last_good.hir,
            environment,
            semantic_index,
            project_inference,
        })
    }

    fn project_type_environment_for_file(
        &self,
        file: &AnalysisFile,
        parsed: &ParsedSource,
        project_inference: &ProjectLocalInference,
    ) -> Option<TypeEnvironment> {
        let _profile = profile::span("analysis.semantic.environment");
        self.project_type_environment_for_parsed(
            parsed,
            file.environment.clone(),
            project_inference,
        )
    }

    fn project_type_environment_for_parsed(
        &self,
        parsed: &ParsedSource,
        base_environment: Option<TypeEnvironment>,
        project_inference: &ProjectLocalInference,
    ) -> Option<TypeEnvironment> {
        let environment = base_environment.or_else(|| {
            let (environment, diagnostics) = if self.config.ecs.enabled {
                build_type_environment_with_builtin_aliases(parsed, crate::ecs::builtin_aliases())
            } else {
                build_type_environment_with_diagnostics(parsed)
            };

            diagnostics.is_empty().then_some(environment)
        })?;

        let environment = environment
            .with_local_types(project_inference.local_types.clone())
            .with_local_types(self.project_local_annotation_types(parsed));

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
        project_inference: &ProjectLocalInference,
    ) -> TypeEnvironment {
        let (fallback, _) = if self.config.ecs.enabled {
            build_type_environment_with_builtin_aliases(parsed, crate::ecs::builtin_aliases())
        } else {
            build_type_environment_with_diagnostics(parsed)
        };
        let environment = base_environment.unwrap_or(fallback);
        let environment = environment
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

    fn project_local_inference(&self, parsed: &ParsedSource) -> ProjectLocalInference {
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

    fn project_type_for_expression(
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

    fn project_type_for_function_call_parts(
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

    fn project_type_for_field_chain(
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

    fn project_indexed_module_export_type(
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

    fn project_field_origin_for_expression(
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

    fn project_field_origin_for_call_parts(
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

    fn project_function_return_alias_origin_for_expression(
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

    fn project_module_return_alias_origin(
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

    fn project_imported_return_alias_field_definition(
        &self,
        parsed: &ParsedSource,
        position: SourcePosition,
        recovered_inference: Option<&ProjectLocalInference>,
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

        if let Some(inference) = recovered_inference {
            let origin = inference.field_origins.get(receiver)?;

            return self.project_alias_field_definition(origin, field);
        }

        let facts = self.semantic_facts_for_parsed(parsed)?;
        let origin = facts.project_inference.field_origins.get(receiver)?;

        self.project_alias_field_definition(origin, field)
    }

    fn project_alias_field_definition(
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
            } => {
                let required_parameter_count = parameters
                    .iter()
                    .take_while(|parameter| !parameter.optional)
                    .count();

                Some(Type::function_returns_with_required(
                    parameters.iter().filter_map(|parameter| {
                        let ty = self.project_type_from_tua_type(parsed, &parameter.ty)?;

                        Some(if parameter.optional {
                            Type::union([ty, Type::nil()])
                        } else {
                            ty
                        })
                    }),
                    required_parameter_count,
                    returns
                        .iter()
                        .filter_map(|ty| self.project_type_from_tua_type(parsed, ty)),
                ))
            }
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

    fn project_type_diagnostics(&self, path: &Path, file: &AnalysisFile) -> Vec<Diagnostic> {
        if file.require_edges.is_empty() {
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
            .and_then(|parsed| {
                let facts = self.semantic_facts_for_parsed(parsed)?;
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

                Some(diagnostics)
            })
            .unwrap_or_default();

        self.project_diagnostics_cache
            .lock()
            .expect("project diagnostics cache lock poisoned")
            .insert(path, diagnostics.clone());

        diagnostics
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

fn reference_code_lens(
    range: SourceSpan,
    path: &Path,
    position: SourcePosition,
    name: String,
    kind: CodeLensReferenceKind,
) -> CodeLens {
    CodeLens {
        range,
        target: CodeLensTarget::References(CodeLensReferenceTarget {
            path: path.to_path_buf(),
            position,
            name,
            kind,
        }),
    }
}

fn code_lens_key(lens: &CodeLens) -> (usize, usize, usize, usize, u8, String) {
    let (kind, name) = match &lens.target {
        CodeLensTarget::References(target) => (
            code_lens_reference_kind_key(target.kind),
            target.name.clone(),
        ),
    };

    (
        lens.range.start.line,
        lens.range.start.column,
        lens.range.end.line,
        lens.range.end.column,
        kind,
        name,
    )
}

fn code_lens_reference_kind_key(kind: CodeLensReferenceKind) -> u8 {
    match kind {
        CodeLensReferenceKind::ExportedFunction => 1,
        CodeLensReferenceKind::TypeAlias => 2,
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

fn project_local_field_chain_type(
    local_types: &BTreeMap<String, Type>,
    parts: &[String],
) -> Option<Type> {
    let (root, fields) = parts.split_first()?;
    let mut current = local_types.get(root)?.clone();

    for field in fields {
        current = project_table_field_type(&current, field)?;
    }

    Some(current)
}

fn project_table_field_type(ty: &Type, field: &str) -> Option<Type> {
    match ty {
        Type::Table(table) => table.semantic_field(field).map(|field| field.ty.clone()),
        Type::Union(types) => {
            let fields = types
                .iter()
                .filter_map(|ty| project_table_field_type(ty, field))
                .collect::<Vec<_>>();

            match fields.len() {
                0 => None,
                1 => fields.into_iter().next(),
                _ => Some(Type::union(fields)),
            }
        }
        Type::Any
        | Type::Alias(_)
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::StringLiteral(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. } => None,
    }
}

fn project_alias_field_span(
    parsed: &ParsedSource,
    alias_name: &str,
    field: &str,
    depth: usize,
) -> Option<SourceSpan> {
    if depth >= 64 {
        return None;
    }

    let alias = parsed
        .type_aliases
        .iter()
        .find(|alias| alias.name == alias_name)?;

    match &alias.ty {
        TuaType::Table(fields) => fields
            .iter()
            .find(|candidate| candidate.name == field)
            .and_then(|candidate| candidate.name_span.or(candidate.span)),
        TuaType::Named(name) => project_alias_field_span(parsed, name, field, depth + 1),
        TuaType::Union(types) => types.iter().find_map(|ty| match ty {
            TuaType::Named(name) => project_alias_field_span(parsed, name, field, depth + 1),
            TuaType::Table(fields) => fields
                .iter()
                .find(|candidate| candidate.name == field)
                .and_then(|candidate| candidate.name_span.or(candidate.span)),
            _ => None,
        }),
        TuaType::StringLiteral(_)
        | TuaType::Array(_)
        | TuaType::Tuple(_)
        | TuaType::Dictionary { .. }
        | TuaType::Function { .. } => None,
    }
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

fn is_type_definition_kind(definition: &Definition) -> bool {
    matches!(
        definition.kind,
        DefinitionKind::TypeAlias
            | DefinitionKind::TableField
            | DefinitionKind::Builtin
            | DefinitionKind::Module
    )
}

fn type_alias_definition_in_parsed(
    parsed: &ParsedSource,
    hir: Option<&HirModule>,
    name: &str,
) -> Option<Definition> {
    hir.and_then(|hir| hir.definition_for_type_alias(name))
        .map(|symbol| Definition {
            path: parsed.path.clone(),
            span: symbol.name_span,
            kind: DefinitionKind::TypeAlias,
        })
        .or_else(|| {
            parsed.type_aliases.iter().find_map(|alias| {
                let span = alias.name_span.or(alias.span)?;

                (alias.name == name).then_some(Definition {
                    path: parsed.path.clone(),
                    span,
                    kind: DefinitionKind::TypeAlias,
                })
            })
        })
}

fn parameter_type_before_position(
    parsed: &ParsedSource,
    name: &str,
    position: SourcePosition,
) -> Option<Type> {
    parsed
        .function_annotations
        .iter()
        .flat_map(|function| function.parameters.iter())
        .filter(|parameter| parameter.name == name)
        .filter_map(|parameter| {
            let span = parameter.name_span.or(parameter.span)?;

            (position_key(span.start) <= position_key(position))
                .then(|| crate::types::type_from_tua_type(&parameter.ty))
        })
        .last()
}

fn receiver_alias_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Alias(name) => Some(name.clone()),
        Type::Union(types) => {
            let mut aliases = types
                .iter()
                .filter(|ty| !matches!(ty, Type::Primitive(crate::PrimitiveType::Nil)))
                .filter_map(receiver_alias_name)
                .collect::<Vec<_>>();

            (aliases.len() == 1).then(|| aliases.remove(0))
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

fn method_implementation_in_parsed(
    parsed: &ParsedSource,
    owner: &str,
    method: &str,
) -> Option<Definition> {
    parsed.function_annotations.iter().find_map(|function| {
        let name = function.name.as_deref()?;
        let (candidate_owner, candidate_method, _) = function_name_table_field(name)?;

        if candidate_owner != owner || candidate_method != method {
            return None;
        }

        Some(Definition {
            path: parsed.path.clone(),
            span: function.name_span.or(function.span)?,
            kind: DefinitionKind::Function,
        })
    })
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

#[derive(Clone, Debug)]
struct ReusableCompletionRequest {
    key: CompletionRequestKey,
    prefix: String,
    position: SourcePosition,
    source: String,
}

fn reusable_completion_request(
    revision: u64,
    path: &Path,
    source: &str,
    position: SourcePosition,
) -> Option<ReusableCompletionRequest> {
    let prefix = crate::completion::context::completion_prefix(source, position)?;
    let source = source_without_range(source, prefix.base_position, position)?;
    let key = completion_request_key(revision, path, &source, prefix.base_position);

    Some(ReusableCompletionRequest {
        key,
        prefix: prefix.prefix,
        position: prefix.base_position,
        source,
    })
}

fn completion_request_key(
    revision: u64,
    path: &Path,
    source: &str,
    position: SourcePosition,
) -> CompletionRequestKey {
    CompletionRequestKey {
        revision,
        path: normalize_path(path),
        line: position.line,
        column: position.column,
        fingerprint: source_fingerprint(source),
    }
}

fn definition_request_key(
    path: &Path,
    source: &str,
    position: SourcePosition,
) -> DefinitionRequestKey {
    DefinitionRequestKey {
        path: normalize_path(path),
        line: position.line,
        column: position.column,
        fingerprint: source_fingerprint(source),
    }
}

fn source_without_range(
    source: &str,
    start: SourcePosition,
    end: SourcePosition,
) -> Option<String> {
    let start = byte_offset_for_position(source, start)?;
    let end = byte_offset_for_position(source, end)?;

    if start > end || !source.is_char_boundary(start) || !source.is_char_boundary(end) {
        return None;
    }

    let mut stripped = String::with_capacity(source.len().saturating_sub(end - start));

    stripped.push_str(&source[..start]);
    stripped.push_str(&source[end..]);

    Some(stripped)
}

fn byte_offset_for_position(source: &str, position: SourcePosition) -> Option<usize> {
    let mut offset = 0;

    for (index, raw_line) in source.split('\n').enumerate() {
        let line_number = index + 1;
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);

        if line_number == position.line {
            return Some(offset + byte_for_column(line, position.column));
        }

        offset += raw_line.len() + 1;
    }

    None
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
    ) -> bool {
        let invalidate_dependents =
            self.changed_file_affects_dependents(path, source, kind, config);

        self.invalidate_file(path, invalidate_dependents);
        invalidate_dependents
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
            return true;
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
                if invalidated.contains(&importer) {
                    continue;
                }

                self.invalidate_one(&importer, invalidated, true);
                self.invalidate_dependents(&importer, invalidated);
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
        self.stats.usage_index_hits += 1;
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
        self.stats.usage_index_misses += 1;
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
                        &context.config.format.options(),
                    )
                } else {
                    check_parsed_source_with_asset_roots_diagnostics_jit_and_ecs_config_and_asset_index(
                        parsed,
                        asset_roots,
                        &diagnostics_config,
                        &context.config.jit,
                        &context.config.ecs,
                        context.asset_index,
                        &context.config.format.options(),
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
    let usage_index = {
        let _profile = profile::span_for_path("analysis.index.usage.file", path);
        let fallback_environment = parsed.as_ref().ok().and_then(|parsed| {
            (environment.is_none() && crate::signal::source_may_contain_event_call(&source))
                .then(|| build_analysis_type_environment_with_diagnostics(parsed, context.config).0)
        });

        Arc::new(FileUsageIndex::build(
            &source,
            parsed.as_ref().ok(),
            environment.as_ref().or(fallback_environment.as_ref()),
        ))
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
        usage_index,
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

fn module_name_for_import_specifier(
    current_path: &Path,
    specifier: &str,
    known_paths: &BTreeSet<PathBuf>,
    source_roots: &[PathBuf],
    output_roots: &[PathBuf],
    extension: &str,
    separator: ModulePathSeparator,
) -> Option<String> {
    if !is_child_relative_import_specifier(specifier) {
        if is_parent_relative_import_specifier(specifier) {
            return None;
        }

        return module_name_for_project_import(
            specifier,
            known_paths,
            source_roots,
            output_roots,
            extension,
            separator,
        );
    }

    let parent = current_path.parent()?;
    let base = normalize_path(parent.join(specifier));

    module_name_for_import_base(
        base,
        known_paths,
        source_roots,
        output_roots,
        extension,
        separator,
    )
}

fn module_name_for_project_import(
    specifier: &str,
    known_paths: &BTreeSet<PathBuf>,
    source_roots: &[PathBuf],
    output_roots: &[PathBuf],
    extension: &str,
    separator: ModulePathSeparator,
) -> Option<String> {
    if !is_project_import_specifier(specifier) {
        return None;
    }

    let relative = module_relative_path(specifier, separator);
    let extensions = module_extensions(extension);

    for root in source_roots.iter().chain(output_roots) {
        for candidate in module_candidates(root, &relative, &extensions) {
            let candidate = normalize_path(candidate);

            if !module_candidate_exists(&candidate, known_paths) {
                continue;
            }

            if let Some(module) =
                module_name_for_import_candidate(&candidate, source_roots, output_roots, separator)
            {
                return Some(module);
            }
        }
    }

    None
}

fn module_name_for_import_base(
    base: PathBuf,
    known_paths: &BTreeSet<PathBuf>,
    source_roots: &[PathBuf],
    output_roots: &[PathBuf],
    extension: &str,
    separator: ModulePathSeparator,
) -> Option<String> {
    if let Some(runtime_extension) = base.extension().and_then(|extension| extension.to_str()) {
        if runtime_extension != "lua" && runtime_extension != extension {
            return None;
        }
    }

    let extensions = module_extensions(extension);
    let candidates = if base.extension().is_some() {
        vec![base]
    } else {
        import_module_candidates(&base, &extensions)
    };

    for candidate in candidates {
        let candidate = normalize_path(candidate);

        if !module_candidate_exists(&candidate, known_paths) {
            continue;
        }

        if let Some(module) =
            module_name_for_import_candidate(&candidate, source_roots, output_roots, separator)
        {
            return Some(module);
        }
    }

    None
}

fn is_child_relative_import_specifier(specifier: &str) -> bool {
    specifier.starts_with("./") && !has_parent_path_component(specifier)
}

fn is_project_import_specifier(specifier: &str) -> bool {
    !specifier.is_empty() && !specifier.starts_with('.') && !has_parent_path_component(specifier)
}

fn is_parent_relative_import_specifier(specifier: &str) -> bool {
    specifier.starts_with("../")
        || specifier.starts_with("./") && has_parent_path_component(specifier)
}

fn has_parent_path_component(specifier: &str) -> bool {
    Path::new(specifier)
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
}

fn module_relative_path(module: &str, separator: ModulePathSeparator) -> PathBuf {
    let separator = match separator {
        ModulePathSeparator::Dot => '.',
        ModulePathSeparator::Slash => '/',
    };
    let mut relative = PathBuf::new();

    for part in module.split(separator).filter(|part| !part.is_empty()) {
        relative.push(part);
    }

    relative
}

fn module_candidates(root: &Path, relative: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let base = root.join(relative);
    let mut candidates = Vec::new();

    for extension in extensions {
        candidates.push(base.with_extension(extension));
        candidates.push(base.join(format!("init.{extension}")));
    }

    candidates
}

fn module_name_for_import_candidate(
    path: &Path,
    source_roots: &[PathBuf],
    output_roots: &[PathBuf],
    separator: ModulePathSeparator,
) -> Option<String> {
    source_roots
        .iter()
        .chain(output_roots)
        .filter(|root| path.starts_with(root))
        .max_by_key(|root| root.components().count())
        .and_then(|root| module_name_for_path(root, path, separator))
}

fn module_candidate_exists(candidate: &Path, known_paths: &BTreeSet<PathBuf>) -> bool {
    known_paths.contains(candidate) || candidate.is_file()
}

fn module_extensions(project_extension: &str) -> Vec<&str> {
    let mut extensions = vec![project_extension];

    if project_extension != "lua" {
        extensions.push("lua");
    }

    extensions
}

fn import_module_candidates(base: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    for extension in extensions {
        candidates.push(base.with_extension(extension));
        candidates.push(base.join(format!("init.{extension}")));
    }

    candidates
}

fn module_path_without_runtime_suffix(path: &Path) -> PathBuf {
    let without_extension = path.with_extension("");

    if without_extension
        .file_name()
        .is_some_and(|name| name == "init")
    {
        return without_extension
            .parent()
            .map(normalize_path)
            .unwrap_or(without_extension);
    }

    without_extension
}

fn relative_path_parts(from_dir: &Path, target: &Path) -> Option<Vec<String>> {
    let from = normalized_component_strings(from_dir)?;
    let target = normalized_component_strings(target)?;
    let mut common = 0usize;

    while from.get(common).is_some()
        && target.get(common).is_some()
        && from[common] == target[common]
    {
        common += 1;
    }

    let mut parts = Vec::new();

    for _ in common..from.len() {
        parts.push("..".to_owned());
    }
    parts.extend(target.into_iter().skip(common));

    Some(parts)
}

fn normalized_component_strings(path: &Path) -> Option<Vec<String>> {
    let mut parts = Vec::new();

    for component in normalize_path(path).components() {
        match component {
            std::path::Component::Normal(part) => {
                parts.push(part.to_string_lossy().to_string());
            }
            std::path::Component::Prefix(prefix) => {
                parts.push(prefix.as_os_str().to_string_lossy().to_string());
            }
            std::path::Component::RootDir => {
                parts.push(String::new());
            }
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => return None,
        }
    }

    Some(parts)
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
    fast.formatting_lints = config.formatting_lints;
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
            | crate::DiagnosticCode::TYPE_UNSUPPORTED_RUNTIME_CHECK
            | crate::DiagnosticCode::TYPE_CLASS
            | crate::DiagnosticCode::LINT_UNDEFINED_VARIABLE
            | crate::DiagnosticCode::LINT_FORMAT_TRAILING_WHITESPACE
            | crate::DiagnosticCode::LINT_FORMAT_INDENTATION
            | crate::DiagnosticCode::LINT_FORMAT_KEYWORD_SPACING
            | crate::DiagnosticCode::LINT_FORMAT_TOKEN_SPACING
            | crate::DiagnosticCode::LINT_FORMAT_LINE_WIDTH
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

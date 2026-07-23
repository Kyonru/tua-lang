use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use full_moon::ast::{self, Expression, Field, FunctionCall, TableConstructor};
use full_moon::node::Node;
use full_moon::tokenizer::TokenReference;
use full_moon::visitors::Visitor;

use crate::asset::{
    self, AssetInventory, AssetLoaderKind, AssetMetadata, AssetReference, ProjectAsset,
};
use crate::ast_utils::{
    assignment_expression_for_local, field_chain_before_first_call, plain_field_chain_from_var,
};
use crate::classes::{
    class_member_target as class_member_source_target, nominal_class_identity, ClassMemberKind,
};
use crate::code_action::code_actions_parsed_source_with_kinds_and_diagnostics_config;
use crate::completion::{
    complete_current_file_field_access_with_environment_and_ranking_inputs,
    complete_current_file_lua_field_access,
    complete_field_access_in_source_with_environment_and_ranking_inputs,
    complete_indexed_module_paths, complete_source_with_parsed_environment_and_ranking_inputs,
    filter_and_rank_completions, multitoken::complete_multi_token_source, CompletionAdaptiveStats,
    CompletionMultiTokenConfig, CompletionRankingInputs, CompletionSequenceRankingConfig,
};
use crate::folding_range::folding_ranges_source;
use crate::hir::{HirExportValue, HirModule, HirRequireBinding, HirSymbol, HirSymbolKind};
use crate::hover::hover_parsed_source_with_environment;
use crate::ide_db::{SymbolIndexEntry, SymbolIndexKind};
use crate::inlay_hint::{
    inlay_hints_parsed_source_with_environment, inlay_hints_parsed_source_with_environment_in_range,
};
use crate::inline_ignore::apply_inline_ignores;
use crate::inspector::InspectorItem;
use crate::module_index::infer_module_exports_with_optional_environment;
use crate::module_link::{
    module_specifier_at_position, module_specifier_literals, ModuleSpecifierKind,
};
use crate::module_path_index::{ModulePathIndex, ProjectModuleWorkspace};
use crate::profile;
use crate::scope::{ScopeDeclarationKind, ScopeGlobalKind, ScopeResolution};
use crate::scope_map_parsed_source;
use crate::selection_range::selection_ranges_source;
use crate::semantic::SemanticModel;
use crate::signal::{
    bus_label, graph_call_from_mention, graph_event_id, graph_schema_summary, SignalEventIndex,
    SignalGraph, SignalGraphEvent,
};
use crate::signature_help::signature_help_parsed_source_with_environment_and_signal_index;
use crate::source_text::{
    byte_for_column, dotted_chain_at_position, function_name_table_field, line_at,
    source_identifiers, SourceIdentifier,
};
use crate::text_match::text_matches;
use crate::types::{
    build_type_environment_with_diagnostics, format_named_function_pretty, format_type_with_prefix,
    widen_inferred_type,
};
use crate::usage_index::{FileUsageIndex, ProjectUsageIndex};
use crate::vfs::{
    module_name_for_path, normalize_path, ModulePathSeparator, VfsSnapshot, VfsSourceKind,
    VirtualFileSystem,
};
use crate::{
    call_hierarchy_calls_parsed_source, call_hierarchy_prepare_parsed_source,
    call_hierarchy_symbols_parsed_source,
    check::{
        check_parsed_source_with_asset_roots_diagnostics_jit_and_ecs_config_and_asset_index,
        check_parsed_source_with_environment_asset_roots_diagnostics_jit_config_and_asset_index,
    },
    complete_tua_source, definition_parsed_source_with_hir, document_highlights_parsed_source,
    document_symbols_parsed_source_with_hir, exported_field_for_fields,
    global_function_definition_count, global_function_name_for_definition,
    global_references_to_definition_parsed_source_with_hir, global_type_alias_name_for_definition,
    global_type_alias_references_to_definition_parsed_source_with_hir, is_valid_rename_identifier,
    lower_parsed_source, module_links_tua_source, parse_lua_source, parse_tua_source,
    reference_kind_for_rename, references_to_definition_parsed_source_with_artifacts,
    rename_target_for_definition, require_bindings_for_parsed_source,
    semantic_tokens_parsed_source, semantic_tokens_tua_source, signature_help_tua_source,
    type_alias_definition_count, Assist, AssistKind, AssistRange, CallHierarchyCallSite,
    CallHierarchyCallTarget, CallHierarchyIncomingCall, CallHierarchyOutgoingCall,
    CallHierarchySymbol, CallHierarchySymbolTarget, CodeLens, CodeLensReferenceKind,
    CodeLensReferenceTarget, CodeLensTarget, Completion, CompletionKind, Definition,
    DefinitionKind, Diagnostic, DocumentHighlight, DocumentSymbol, FoldingRange, Hover, InlayHint,
    LuaDocAnnotationKind, ModuleDependencyEdge, ModuleDependencyExport, ModuleDependencyGraph,
    ModuleDependencyModule, ModuleExportField, ModuleExports, ModuleGraph, ModuleGraphInput,
    ModuleGraphOptions, ModuleInclusionPath, ModuleInclusionStep, ModuleLink, ParsedSource,
    ProjectConfig, ProjectTypeDatabase, ProjectTypeDatabaseInput, ProjectTypeLibrary, Reference,
    ReferenceKind, RenameEdit, RenameTarget, RequireResolution, ScopeMap, SelectionRange,
    SemanticIndex, SemanticToken, SignatureHelp, SourcePosition, SourceSpan, SymbolIndex,
    TableField, TextEdit, TuaType, Type, TypeEnvironment, TypeExplanation, TypeExplanationStep,
    TypeExplanationStepKind, TypedFunction, WorkspaceSymbol,
};

mod asset_queries;
mod database;
mod ecs_queries;
mod editor_queries;
mod host;
mod inspector_queries;
mod module_queries;
mod navigation_queries;
mod object_pool_queries;
mod project_queries;
mod recovery;
mod snapshot;
mod type_explanation_queries;

use database::*;
use recovery::*;

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
    cache: RefCell<AnalysisQueryCache>,
    last_good_files: RefCell<BTreeMap<PathBuf, LastGoodAnalysisFile>>,
    completion_adaptive_stats: CompletionAdaptiveStats,
    completion_sequence_ranking: CompletionSequenceRankingConfig,
    completion_multi_token: CompletionMultiTokenConfig,
    completion_cache: Arc<Mutex<BTreeMap<CompletionRequestKey, Vec<Completion>>>>,
    completion_cache_stats: Arc<Mutex<CompletionCacheStats>>,
    definition_cache: Arc<Mutex<BTreeMap<DefinitionRequestKey, Option<Definition>>>>,
    type_libraries: Vec<ProjectTypeLibrary>,
    module_path_workspaces: Vec<ProjectModuleWorkspace>,
    indexed_module_paths: BTreeSet<PathBuf>,
    module_path_revision: u64,
    module_path_index_cache: Arc<Mutex<Option<CachedModulePathIndex>>>,
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
    module_path_index: Arc<ModulePathIndex>,
    builtin_catalogs: BuiltinCatalogs,
    module_graph: ModuleGraph,
    type_database: ProjectTypeDatabase,
    symbol_index: SymbolIndex,
    usage_index: ProjectUsageIndex,
    last_good_files: BTreeMap<PathBuf, LastGoodAnalysisFile>,
    completion_adaptive_stats: CompletionAdaptiveStats,
    completion_sequence_ranking: CompletionSequenceRankingConfig,
    completion_multi_token: CompletionMultiTokenConfig,
    semantic_facts: Arc<Mutex<BTreeMap<PathBuf, Arc<AnalysisSemanticFacts>>>>,
    degraded_facts: Arc<Mutex<BTreeMap<PathBuf, Arc<DegradedSemanticFacts>>>>,
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
    pub hir: &'a HirModule,
    pub scope: &'a ScopeMap,
}

#[derive(Clone, Debug)]
struct AnalysisFile {
    source: String,
    kind: AnalysisSourceKind,
    parsed: Result<ParsedSource, Vec<Diagnostic>>,
    semantics: Option<AnalysisFileSemantics>,
    degraded_semantics: Option<DegradedFileSemantics>,
    diagnostics: Vec<Diagnostic>,
    exports: Option<ModuleExports>,
    require_edges: Vec<ModuleLink>,
    assets: Vec<AssetMetadata>,
    usage_index: Arc<FileUsageIndex>,
    ecs_index: Arc<crate::ecs::EcsFileIndex>,
    object_pool_index: Arc<crate::object_pool::ObjectPoolFileIndex>,
    #[allow(dead_code)]
    symbols: Vec<SymbolIndexEntry>,
}

#[derive(Clone, Debug)]
struct AnalysisFileSemantics {
    hir: HirModule,
    scope: ScopeMap,
    environment: TypeEnvironment,
    environment_diagnostics: Vec<Diagnostic>,
}

impl AnalysisFile {
    fn semantics(&self) -> Option<&AnalysisFileSemantics> {
        debug_assert_eq!(self.parsed.is_ok(), self.semantics.is_some());
        self.semantics.as_ref()
    }

    fn hir(&self) -> Option<&HirModule> {
        self.semantics().map(|semantics| &semantics.hir)
    }

    fn scope(&self) -> Option<&ScopeMap> {
        self.semantics().map(|semantics| &semantics.scope)
    }

    fn environment(&self) -> Option<&TypeEnvironment> {
        self.semantics().map(|semantics| &semantics.environment)
    }
}

#[derive(Clone, Debug)]
struct AnalysisSemanticFacts {
    environment: TypeEnvironment,
    semantic_index: SemanticIndex,
    project_inference: ProjectLocalInference,
}

#[derive(Clone, Debug)]
struct DegradedFileSemantics {
    parsed: ParsedSource,
    hir: HirModule,
    environment: TypeEnvironment,
}

#[derive(Clone, Debug)]
struct LastGoodAnalysisFile {
    parsed: ParsedSource,
    hir: HirModule,
    environment: TypeEnvironment,
}

#[derive(Clone, Debug)]
struct DegradedSemanticFacts {
    parsed: ParsedSource,
    hir: HirModule,
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
    pub require_surface_invalidations: usize,
    pub export_surface_invalidations: usize,
    pub type_surface_invalidations: usize,
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
    module: ModuleDependencySurface,
    types: TypeDependencySurface,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ModuleDependencySurface {
    exports: Option<ModuleExports>,
    require_targets: BTreeSet<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TypeDependencySurface {
    aliases: BTreeMap<String, Type>,
    locals: BTreeMap<String, Type>,
    functions: BTreeMap<String, TypedFunction>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DependencyInvalidation {
    require_graph: bool,
    exports: bool,
    types: bool,
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

#[derive(Clone, Debug)]
struct CachedModulePathIndex {
    revision: u64,
    index: Arc<ModulePathIndex>,
}

impl DependencySurface {
    fn from_analysis_file(file: &AnalysisFile) -> Self {
        Self::from_parts(
            file.exports.clone(),
            file.environment(),
            require_targets_for_links(&file.require_edges),
        )
    }

    fn from_parsed(
        parsed: &ParsedSource,
        config: &ProjectConfig,
        require_targets: BTreeSet<PathBuf>,
    ) -> Self {
        let (environment, _) = build_analysis_type_environment_with_diagnostics(parsed, config);
        let exports = infer_module_exports_with_optional_environment(parsed, Some(&environment));

        Self::from_parts(exports, Some(&environment), require_targets)
    }

    fn from_parts(
        exports: Option<ModuleExports>,
        environment: Option<&TypeEnvironment>,
        require_targets: BTreeSet<PathBuf>,
    ) -> Self {
        Self {
            module: ModuleDependencySurface {
                exports,
                require_targets,
            },
            types: TypeDependencySurface {
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
            },
        }
    }

    fn invalidation_from(&self, previous: &Self) -> DependencyInvalidation {
        DependencyInvalidation {
            require_graph: self.module.require_targets != previous.module.require_targets,
            exports: self.module.exports != previous.module.exports,
            types: self.types != previous.types,
        }
    }
}

impl DependencyInvalidation {
    fn all() -> Self {
        Self {
            require_graph: true,
            exports: true,
            types: true,
        }
    }

    fn invalidates_importers(self) -> bool {
        self.exports || self.types
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

fn diagnostic_key(diagnostic: &Diagnostic) -> (PathBuf, usize, usize, u8, Option<u16>, String) {
    if diagnostic.code == Some(crate::DiagnosticCode::LINT_MISSING_REQUIRE) {
        return (
            normalize_path(&diagnostic.file),
            diagnostic.line,
            0,
            diagnostic_severity_key(diagnostic.severity),
            diagnostic.code.map(crate::DiagnosticCode::as_u16),
            String::new(),
        );
    }

    let message = if diagnostic.code == Some(crate::DiagnosticCode::DOMAIN_SPRITE) {
        diagnostic.message.clone()
    } else {
        String::new()
    };

    (
        normalize_path(&diagnostic.file),
        diagnostic.line,
        diagnostic.column,
        diagnostic_severity_key(diagnostic.severity),
        diagnostic.code.map(crate::DiagnosticCode::as_u16),
        message,
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
        crate::ReferenceKind::Asset => 1,
        crate::ReferenceKind::SignalEvent => 2,
        crate::ReferenceKind::TypeAlias => 3,
        crate::ReferenceKind::Local => 4,
        crate::ReferenceKind::Function => 5,
        crate::ReferenceKind::Parameter => 6,
        crate::ReferenceKind::TableField => 7,
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

        let references = self.references.entry(key).or_default();

        references.push(reference);
        references.push(Reference {
            path: definition.path.clone(),
            span: definition.span,
            kind: reference_kind_for_definition(definition.kind),
        });
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
        if matches!(
            definition.kind,
            DefinitionKind::Builtin | DefinitionKind::Asset | DefinitionKind::SignalEvent
        ) {
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
        DefinitionKind::Asset => 2,
        DefinitionKind::SignalEvent => 3,
        DefinitionKind::TypeAlias => 4,
        DefinitionKind::Local => 5,
        DefinitionKind::Function => 6,
        DefinitionKind::Parameter => 7,
        DefinitionKind::TableField => 8,
    }
}

fn reference_kind_for_definition(kind: DefinitionKind) -> ReferenceKind {
    match kind {
        DefinitionKind::Module => ReferenceKind::Module,
        DefinitionKind::Builtin => ReferenceKind::Function,
        DefinitionKind::Asset => ReferenceKind::Asset,
        DefinitionKind::SignalEvent => ReferenceKind::SignalEvent,
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
        HirSymbolKind::Class | HirSymbolKind::TypeAlias => Some(DefinitionKind::TypeAlias),
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
        .next_back()
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

fn signal_event_hover(
    index: &SignalEventIndex,
    path: &Path,
    position: SourcePosition,
) -> Option<Hover> {
    let event = index.event_at(path, position)?;
    let related = index.related_events(event);
    let schema = index.schema_for_event(event);
    let payload = schema
        .as_ref()
        .map(|schema| {
            schema
                .payload
                .iter()
                .map(|parameter| format!("{}: {}", parameter.name, parameter.ty))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let declarations = related
        .iter()
        .filter(|event| event.mention.method == "declare")
        .count();
    let listeners = related
        .iter()
        .filter(|event| matches!(event.mention.method.as_str(), "on" | "once" | "register"))
        .count();
    let emits = related
        .iter()
        .filter(|event| event.mention.method == "emit")
        .count();

    Some(Hover {
        contents: format!("SignalBus event \"{}\"({payload})", event.mention.name),
        span: Some(event.mention.name_span),
        documentation: Some(format!(
            "Bus: `{}`. {declarations} declarations, {listeners} listeners, {emits} emits.",
            crate::signal::bus_label(&event.identity)
        )),
        context: Vec::new(),
    })
}

fn collect_type_fields(ty: &Type, fields: &mut BTreeMap<String, Type>) {
    match ty {
        Type::Table(table) => {
            for (name, field) in table.semantic_fields() {
                let ty = field.ty.clone();

                fields
                    .entry(name.to_owned())
                    .and_modify(|current| *current = Type::union([current.clone(), ty.clone()]))
                    .or_insert(ty);
            }
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

#[derive(Clone, Debug)]
struct ExpectedTableType {
    ty: Type,
    origin_path: PathBuf,
    table: TableConstructor,
}

#[derive(Clone, Debug)]
enum ExpectedTableStep {
    Field(String),
    Element,
}

#[derive(Default)]
struct ExpectedTableSyntaxCollector {
    tables: Vec<TableConstructor>,
    calls: Vec<FunctionCall>,
    locals: Vec<ast::LocalAssignment>,
}

impl Visitor for ExpectedTableSyntaxCollector {
    fn visit_table_constructor(&mut self, table: &TableConstructor) {
        self.tables.push(table.clone());
    }

    fn visit_function_call(&mut self, call: &FunctionCall) {
        self.calls.push(call.clone());
    }

    fn visit_local_assignment(&mut self, local: &ast::LocalAssignment) {
        self.locals.push(local.clone());
    }
}

fn expected_table_syntax(
    parsed: &ParsedSource,
    position: SourcePosition,
) -> Option<ExpectedTableSyntax> {
    let mut collector = ExpectedTableSyntaxCollector::default();
    collector.visit_ast(&parsed.ast);
    let table = collector
        .tables
        .into_iter()
        .filter(|table| node_span(table).is_some_and(|span| span_contains_position(span, position)))
        .min_by_key(|table| node_span(table).map(table_span_size))?;

    Some(ExpectedTableSyntax {
        table,
        calls: collector.calls,
        locals: collector.locals,
    })
}

struct ExpectedTableSyntax {
    table: TableConstructor,
    calls: Vec<FunctionCall>,
    locals: Vec<ast::LocalAssignment>,
}

fn expression_path_to_table(
    expression: &Expression,
    target: SourceSpan,
) -> Option<Vec<ExpectedTableStep>> {
    let span = node_span(expression)?;

    if !span_contains_span(span, target) {
        return None;
    }

    match expression {
        Expression::TableConstructor(table) => {
            if node_span(table) == Some(target) {
                return Some(Vec::new());
            }

            for field in table.fields() {
                let (value, step) = match field {
                    Field::NameKey { key, value, .. } => {
                        (value, ExpectedTableStep::Field(token_text(key)))
                    }
                    Field::ExpressionKey { key, value, .. } => {
                        let step = string_literal_expression_value(key)
                            .map(ExpectedTableStep::Field)
                            .unwrap_or(ExpectedTableStep::Element);
                        (value, step)
                    }
                    Field::NoKey(value) => (value, ExpectedTableStep::Element),
                    _ => continue,
                };

                if let Some(mut path) = expression_path_to_table(value, target) {
                    path.insert(0, step);
                    return Some(path);
                }
            }

            None
        }
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. }
        | Expression::UnaryOperator { expression, .. } => {
            expression_path_to_table(expression, target)
        }
        _ => None,
    }
}

fn string_literal_expression_value(expression: &Expression) -> Option<String> {
    let Expression::String(token) = expression else {
        return None;
    };

    unquote_string_literal(&token_text(token))
}

fn unquote_string_literal(value: &str) -> Option<String> {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .map(str::to_owned)
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn span_contains_position(span: SourceSpan, position: SourcePosition) -> bool {
    position_key(span.start) <= position_key(position)
        && position_key(position) <= position_key(span.end)
}

fn span_contains_span(parent: SourceSpan, child: SourceSpan) -> bool {
    position_key(parent.start) <= position_key(child.start)
        && position_key(child.end) <= position_key(parent.end)
}

fn table_span_size(span: SourceSpan) -> (usize, usize) {
    (
        span.end.line.saturating_sub(span.start.line),
        span.end.column.saturating_sub(span.start.column),
    )
}

fn call_argument_table_path(
    call: &FunctionCall,
    target: SourceSpan,
) -> Option<(usize, Vec<ExpectedTableStep>)> {
    let arguments = call.suffixes().find_map(|suffix| match suffix {
        ast::Suffix::Call(ast::Call::AnonymousCall(arguments)) => Some(arguments),
        ast::Suffix::Call(ast::Call::MethodCall(method)) => Some(method.args()),
        _ => None,
    })?;

    match arguments {
        ast::FunctionArgs::Parentheses { arguments, .. } => {
            arguments
                .iter()
                .enumerate()
                .find_map(|(index, expression)| {
                    expression_path_to_table(expression, target).map(|path| (index, path))
                })
        }
        ast::FunctionArgs::TableConstructor(table) if node_span(table) == Some(target) => {
            Some((0, Vec::new()))
        }
        ast::FunctionArgs::String(_) | ast::FunctionArgs::TableConstructor(_) => None,
        _ => None,
    }
}

fn typed_function_parameter_type(
    function: &TypedFunction,
    argument_index: usize,
    is_method: bool,
) -> Option<Type> {
    let mut types = Vec::new();

    if let Some(ty) = parameter_type_for_call(
        &function.parameters,
        function.vararg_type.as_ref(),
        argument_index,
        is_method,
    ) {
        types.push(ty);
    }
    for overload in &function.overloads {
        if let Some(ty) = parameter_type_for_call(
            &overload.parameters,
            overload.vararg_type.as_ref(),
            argument_index,
            is_method,
        ) {
            types.push(ty);
        }
    }

    match types.len() {
        0 => None,
        1 => types.pop(),
        _ => Some(Type::union(types)),
    }
}

fn parameter_type_for_call(
    parameters: &[crate::TypedParameter],
    vararg_type: Option<&Type>,
    argument_index: usize,
    is_method: bool,
) -> Option<Type> {
    let parameters = if is_method
        && parameters.first().is_some_and(|parameter| {
            parameter.name == "self" || (parameter.name.is_empty() && parameter.ty == Type::Any)
        }) {
        &parameters[1..]
    } else {
        parameters
    };

    parameters
        .get(argument_index)
        .map(|parameter| parameter.ty.clone())
        .or_else(|| vararg_type.cloned())
}

fn table_string_value_field(table: &TableConstructor, position: SourcePosition) -> Option<String> {
    table.fields().iter().find_map(|field| match field {
        Field::NameKey { key, value, .. }
            if matches!(value, Expression::String(_))
                && node_span(value).is_some_and(|span| span_contains_position(span, position)) =>
        {
            Some(token_text(key))
        }
        Field::ExpressionKey { key, value, .. }
            if matches!(value, Expression::String(_))
                && node_span(value).is_some_and(|span| span_contains_position(span, position)) =>
        {
            string_literal_expression_value(key)
        }
        _ => None,
    })
}

fn table_named_fields(table: &TableConstructor) -> BTreeSet<String> {
    table
        .fields()
        .iter()
        .filter_map(|field| match field {
            Field::NameKey { key, .. } => Some(token_text(key)),
            Field::ExpressionKey { key, .. } => string_literal_expression_value(key),
            Field::NoKey(_) => None,
            _ => None,
        })
        .collect()
}

fn table_field_key_position(table: &TableConstructor, position: SourcePosition) -> bool {
    let Some(field) = table
        .fields()
        .iter()
        .find(|field| node_span(*field).is_some_and(|span| span_contains_position(span, position)))
    else {
        return true;
    };

    match field {
        Field::NameKey { key, .. } => {
            node_span(key).is_some_and(|span| span_contains_position(span, position))
        }
        Field::ExpressionKey { key, .. } => {
            node_span(key).is_some_and(|span| span_contains_position(span, position))
        }
        Field::NoKey(Expression::Var(ast::Var::Name(name))) => {
            node_span(name).is_some_and(|span| span_contains_position(span, position))
        }
        Field::NoKey(_) => false,
        _ => false,
    }
}

fn table_callback_field_at_position(
    table: &TableConstructor,
    position: SourcePosition,
) -> Option<(String, &ast::AnonymousFunction)> {
    table.fields().iter().find_map(|field| {
        let (name, value) = match field {
            Field::NameKey { key, value, .. } => (token_text(key), value),
            Field::ExpressionKey { key, value, .. } => {
                (string_literal_expression_value(key)?, value)
            }
            Field::NoKey(_) => return None,
            _ => return None,
        };
        let Expression::Function(function) = value else {
            return None;
        };

        node_span(function)
            .is_some_and(|span| span_contains_position(span, position))
            .then_some((name, function.as_ref()))
    })
}

fn collect_function_parameter_types(ty: &Type, index: usize, types: &mut Vec<Type>) {
    match ty {
        Type::Function(function) => {
            if let Some(parameter) = function.parameters.get(index) {
                types.push(parameter.ty.clone());
            } else if let Some(vararg) = function.vararg_type.as_deref() {
                types.push(vararg.clone());
            }
        }
        Type::Union(items) => {
            for item in items {
                collect_function_parameter_types(item, index, types);
            }
        }
        Type::Alias(_)
        | Type::Any
        | Type::Primitive(_)
        | Type::StringLiteral(_)
        | Type::Table(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. } => {}
    }
}

fn table_string_literal_fields(table: &TableConstructor) -> Vec<(String, String)> {
    table
        .fields()
        .iter()
        .filter_map(|field| match field {
            Field::NameKey { key, value, .. } => {
                Some((token_text(key), string_literal_expression_value(value)?))
            }
            Field::ExpressionKey { key, value, .. } => Some((
                string_literal_expression_value(key)?,
                string_literal_expression_value(value)?,
            )),
            Field::NoKey(_) => None,
            _ => None,
        })
        .collect()
}

fn string_literal_prefix_before(source: &str, position: SourcePosition) -> Option<String> {
    let line = line_at(source, position.line)?;
    let cursor = byte_for_position_column(line, position.column).min(line.len());
    let prefix = &line[..cursor];
    let quote = prefix
        .char_indices()
        .rev()
        .find(|(_, character)| matches!(character, '"' | '\''))?
        .0;

    Some(prefix[quote + 1..].to_owned())
}

fn collect_string_literal_values(ty: &Type, values: &mut BTreeSet<String>) {
    match ty {
        Type::StringLiteral(value) => {
            values.insert(value.clone());
        }
        Type::Union(types) => {
            for ty in types {
                collect_string_literal_values(ty, values);
            }
        }
        Type::Alias(_)
        | Type::Any
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::Table(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. } => {}
    }
}

fn type_contains_string_literal(ty: &Type, literal: &str) -> bool {
    match ty {
        Type::StringLiteral(value) => value == literal,
        Type::Union(types) => types
            .iter()
            .any(|ty| type_contains_string_literal(ty, literal)),
        Type::Alias(_)
        | Type::Any
        | Type::Function(_)
        | Type::Primitive(_)
        | Type::Table(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::Dictionary { .. } => false,
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

fn byte_for_position_column(line: &str, column: usize) -> usize {
    let target = column.saturating_sub(1);

    line.char_indices()
        .nth(target)
        .map(|(index, _)| index)
        .unwrap_or(line.len())
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
    match ty {
        Type::Function(function) => format_named_function_pretty(
            name,
            &function.parameters,
            function.vararg_type.is_some(),
            function.vararg_type.as_deref(),
            function.return_types_or_any(),
        ),
        _ => format_type_with_prefix(format!("(field) {name}: "), ty),
    }
}

fn module_export_documentation(
    export: &ModuleExportField,
    source_documentation: Option<String>,
) -> Option<String> {
    let mut parts = Vec::new();

    if let Some(deprecation) = &export.deprecation {
        push_unique_documentation(&mut parts, &deprecation.markdown());
    }

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

#[cfg(test)]
mod tests;

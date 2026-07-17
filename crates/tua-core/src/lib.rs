pub mod analysis;
mod asset;
mod ast_utils;
pub mod call_hierarchy;
pub mod check;
mod classes;
pub mod code_action;
pub mod code_lens;
pub mod completion;
pub mod config;
pub mod definition;
pub mod diagnostic;
pub mod document_highlight;
pub mod document_symbol;
mod ecs;
pub mod emit;
#[cfg(any(test, feature = "test-fixture"))]
pub mod fixture;
pub mod folding_range;
pub mod format;
pub mod hir;
pub mod hover;
pub mod ide_db;
pub mod inlay_hint;
mod inline_ignore;
pub mod inspector;
mod lint;
mod love_api;
mod love_api_extra;
mod love_api_signatures;
mod love_callback;
mod love_conf;
mod lua;
pub mod module_graph;
pub mod module_index;
pub mod module_link;
mod narrowing;
mod object_pool;
pub mod parser;
pub mod profile;
mod project;
pub mod project_model;
pub mod references;
pub mod rename;
mod runtime_type;
pub mod scope;
pub mod selection_range;
mod semantic;
pub mod semantic_index;
pub mod semantic_token;
mod signal;
pub mod signature_help;
mod source_text;
mod sprite;
mod text_match;
pub mod type_database;
pub mod type_explanation;
pub mod types;
mod usage_index;
mod vector;
pub mod vfs;
pub mod workspace_symbol;

pub use analysis::{
    AnalysisCacheStats, AnalysisHost, AnalysisSemanticFile, AnalysisSnapshot, AnalysisSourceKind,
    AnalysisWarmPriority, BuiltinCatalogs,
};
pub use asset::{AssetLoaderKind, AssetMetadata, AssetValidationStatus};
pub use call_hierarchy::{
    call_hierarchy_calls_parsed_source, call_hierarchy_exports_parsed_source,
    call_hierarchy_prepare_parsed_source, call_hierarchy_symbols_parsed_source,
    CallHierarchyCallSite, CallHierarchyCallTarget, CallHierarchyExport, CallHierarchyIncomingCall,
    CallHierarchyOutgoingCall, CallHierarchySymbol, CallHierarchySymbolKind,
    CallHierarchySymbolTarget,
};
pub use check::{
    check_parsed_source, check_parsed_source_with_asset_roots_and_diagnostics_config,
    check_parsed_source_with_diagnostics_config, check_tua_source,
    check_tua_source_with_asset_roots_and_diagnostics_config,
    check_tua_source_with_diagnostics_config,
};
pub use code_action::{
    assists_parsed_source, code_actions_parsed_source, code_actions_parsed_source_with_kinds,
    Assist, AssistEdit, AssistKind, AssistRange, CodeAction, CodeActionEdit, CodeActionKind,
    CodeActionRange, CreateFile,
};
pub use code_lens::{CodeLens, CodeLensReferenceKind, CodeLensReferenceTarget, CodeLensTarget};
pub use completion::{
    complete_lossy_identifier_source, complete_parsed_source, complete_tua_source, Completion,
    CompletionAdaptiveKey, CompletionAdaptiveMemberStats, CompletionAdaptiveStats, CompletionKind,
    CompletionMultiTokenConfig, CompletionSequenceRankingConfig,
};
pub use config::{
    init_project, load_config_from_dir, parse_config, AnnotationMode, ConfigError,
    ConfigParseError, DiagnosticsConfig, EcsConfig, EmitConfig, FormatConfig, InitResult,
    JitConfig, LoadedConfig, ProjectConfig, CONFIG_FILE, DEFAULT_CONFIG_TEXT,
};
pub use definition::{
    definition_parsed_source, definition_parsed_source_with_hir, definition_tua_source, Definition,
    DefinitionKind,
};
pub use diagnostic::{
    format_diagnostics_for_cli, Diagnostic, DiagnosticCategory, DiagnosticCode, DiagnosticSeverity,
};
pub use document_highlight::{
    document_highlights_parsed_source, document_highlights_tua_source, DocumentHighlight,
    DocumentHighlightKind,
};
pub use document_symbol::{
    document_symbols_parsed_source, document_symbols_parsed_source_with_hir,
    document_symbols_tua_source, DocumentSymbol, DocumentSymbolKind,
};
pub use emit::{
    emit_lua, emit_lua_artifact, emit_lua_artifact_with_mode, emit_lua_with_mode, emit_tua_source,
    emit_tua_source_artifact, emit_tua_source_artifact_with_mode, emit_tua_source_with_mode,
    emit_tua_source_with_workspace, emit_tua_source_with_workspace_artifact, LoweringKind,
    LuaEmission, LuaLineProvenance,
};
#[cfg(any(test, feature = "test-fixture"))]
pub use fixture::{Fixture, FixtureExpectation, FixtureFile, FixtureMarker, FixtureTempDir};
pub use folding_range::{
    folding_ranges_parsed_source, folding_ranges_tua_source, FoldingRange, FoldingRangeKind,
};
pub use format::{
    format_parsed_source, format_tua_source, format_tua_source_on_type, format_tua_source_range,
    FormatOptions, QuoteStyle,
};
pub use hir::{
    lower_parsed_source, HirDeclaration, HirDeclarationKind, HirExportValue, HirModule,
    HirModuleExport, HirRequireBinding, HirSymbol, HirSymbolId, HirSymbolKind,
};
pub use hover::{hover_parsed_source, hover_tua_source, Hover};
pub use ide_db::{
    SymbolIndex, SymbolIndexEntry, SymbolIndexExport, SymbolIndexId, SymbolIndexKind,
};
pub use inlay_hint::{inlay_hints_parsed_source, inlay_hints_tua_source, InlayHint, InlayHintKind};
pub use inspector::{
    InspectorItem, SpriteAnimationInspection, SpriteAssetInspection, SpriteFrameInspection,
    SpriteSheetInspection,
};
pub use lua::builtin_definition_document;
pub use module_graph::{
    ModuleExportEntry, ModuleExportOrigin, ModuleGraph, ModuleGraphInput, ModuleGraphModule,
    ModuleGraphOptions, RequireGraphEdge, RequireResolution,
};
pub use module_index::{
    exported_field_for_fields, exported_type_for_fields, infer_module_exports,
    infer_module_exports_with_environment, require_bindings_for_parsed_source,
    required_module_export_for_fields, required_module_export_type_for_fields,
    required_module_exports_for_receiver, required_module_for_receiver, resolve_module_specifier,
    resolve_required_module, ModuleExportField, ModuleExports, RequireBinding, RequiredModule,
};
pub use module_link::{module_links_tua_source, ModuleLink, ModuleSpecifierKind};
pub use parser::{
    extract_lua_source_model, extract_tua_source_model, normalize_tua_source,
    parse_lua_doc_annotations, parse_lua_doc_annotations_with_options, parse_lua_source,
    parse_raw_lua_source, parse_raw_tua_source, parse_tua_source, CompoundAssignment,
    CompoundOperator, FunctionAnnotation, FunctionTypeParameter, LocalAnnotation, LuaDocAlias,
    LuaDocAnnotation, LuaDocAnnotationKind, LuaDocAs, LuaDocCast, LuaDocClass, LuaDocDeprecated,
    LuaDocDiagnostic, LuaDocEnum, LuaDocField, LuaDocGeneric, LuaDocGenericParameter, LuaDocMeta,
    LuaDocModule, LuaDocNoDiscard, LuaDocParam, LuaDocParseOptions, LuaDocParseResult,
    LuaDocReturn, LuaDocReturnType, LuaDocTypeAnnotation, LuaDocVersion, NormalizedSource,
    ParameterAnnotation, ParameterDefault, ParameterOptional, ParsedLanguage, ParsedSource,
    RawParsedSource, SourcePosition, SourceSpan, TableFieldType, TuaType, TypeAlias, TypeAliasKind,
    TypedSourceModel,
};
pub use project::project_description;
pub use project_model::{
    LoveVersion, LuaVersion, ProjectFileAssociation, ProjectModel, ProjectModelOptions,
    ProjectTarget, ProjectWorkspace,
};
pub use references::{
    global_function_definition_count, global_function_name_for_definition,
    global_references_to_definition_parsed_source,
    global_references_to_definition_parsed_source_with_hir, global_type_alias_name_for_definition,
    global_type_alias_references_to_definition_parsed_source,
    global_type_alias_references_to_definition_parsed_source_with_hir, references_parsed_source,
    references_to_definition_parsed_source, references_to_definition_parsed_source_with_artifacts,
    references_tua_source, type_alias_definition_count, Reference, ReferenceKind,
};
pub use rename::{
    is_valid_rename_identifier, prepare_rename_parsed_source, prepare_rename_tua_source,
    reference_kind_for_rename, rename_kind_for_definition, rename_parsed_source,
    rename_target_for_definition, rename_tua_source, RenameEdit, RenameKind, RenameTarget,
    TextEdit,
};
pub use scope::{
    scope_map_parsed_source, scope_map_parsed_source_with_globals, ScopeDeclaration,
    ScopeDeclarationKind, ScopeGlobal, ScopeGlobalKind, ScopeMap, ScopeReference, ScopeResolution,
    ScopeSymbolId,
};
pub use selection_range::{
    selection_ranges_parsed_source, selection_ranges_tua_source, SelectionRange,
};
pub use semantic_index::{DefinitionTarget, SemanticIndex};
pub use semantic_token::{
    semantic_tokens_parsed_source, semantic_tokens_tua_source, SemanticToken, SemanticTokenKind,
    SemanticTokenModifier,
};
pub use signal::{
    SignalGraph, SignalGraphCall, SignalGraphCallRole, SignalGraphEvent, SignalGraphParameter,
};
pub use signature_help::{
    signature_help_parsed_source, signature_help_tua_source, ParameterInformation, SignatureHelp,
    SignatureInformation,
};
pub use type_database::{
    ProjectTypeBinding, ProjectTypeDatabase, ProjectTypeDatabaseInput, ProjectTypeModule,
    ProjectTypeOrigin,
};
pub use type_explanation::{TypeExplanation, TypeExplanationStep, TypeExplanationStepKind};
pub use types::{
    build_type_environment, is_assignable, FunctionParameter, FunctionType, PrimitiveType,
    TableField, TableShape, Type, TypeContext, TypeEnvironment, TypedFunction, TypedParameter,
};
pub use vfs::{
    file_uri_from_path, module_name_for_path, normalize_path, path_from_file_uri,
    ModulePathSeparator, VfsFile, VfsFileId, VfsOptions, VfsSnapshot, VfsSourceKind,
    VirtualFileSystem,
};
pub use workspace_symbol::{
    workspace_symbols_from_dir, workspace_symbols_from_files, WorkspaceSymbol, WorkspaceSymbolKind,
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn brinevector_runtime_source() -> &'static str {
    vector::runtime_source()
}

pub fn anim8_runtime_source() -> &'static str {
    sprite::runtime_source()
}

pub fn tuasignal_runtime_source() -> &'static str {
    signal::runtime_source()
}

pub fn tuaecs_runtime_source() -> &'static str {
    ecs::runtime_source()
}

pub fn tuaobjectpool_runtime_source() -> &'static str {
    object_pool::runtime_source()
}

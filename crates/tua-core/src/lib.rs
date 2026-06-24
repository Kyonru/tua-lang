pub mod analysis;
mod asset;
pub mod call_hierarchy;
pub mod check;
pub mod code_action;
pub mod completion;
pub mod config;
pub mod definition;
pub mod diagnostic;
pub mod document_highlight;
pub mod document_symbol;
pub mod emit;
#[cfg(any(test, feature = "test-fixture"))]
pub mod fixture;
pub mod folding_range;
pub mod hir;
pub mod hover;
pub mod ide_db;
pub mod inlay_hint;
mod lint;
mod love_api;
mod lua;
pub mod module_graph;
pub mod module_index;
pub mod module_link;
mod narrowing;
pub mod parser;
pub mod profile;
mod project;
pub mod project_model;
pub mod references;
pub mod rename;
pub mod scope;
pub mod selection_range;
mod semantic;
pub mod semantic_token;
pub mod signature_help;
mod source_text;
pub mod type_database;
pub mod types;
pub mod vfs;
pub mod workspace_symbol;

pub use analysis::{
    AnalysisCacheStats, AnalysisHost, AnalysisSnapshot, AnalysisSourceKind, BuiltinCatalogs,
};
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
    assists_parsed_source, code_actions_parsed_source, Assist, AssistEdit, AssistKind, AssistRange,
    CodeAction, CodeActionEdit, CodeActionKind, CodeActionRange, CreateFile,
};
pub use completion::{complete_parsed_source, complete_tua_source, Completion, CompletionKind};
pub use config::{
    init_project, load_config_from_dir, parse_config, AnnotationMode, ConfigError,
    ConfigParseError, DiagnosticsConfig, EmitConfig, InitResult, LoadedConfig, ProjectConfig,
    CONFIG_FILE, DEFAULT_CONFIG_TEXT,
};
pub use definition::{definition_parsed_source, definition_tua_source, Definition, DefinitionKind};
pub use diagnostic::{
    format_diagnostics_for_cli, Diagnostic, DiagnosticCategory, DiagnosticCode, DiagnosticSeverity,
};
pub use document_highlight::{
    document_highlights_parsed_source, document_highlights_tua_source, DocumentHighlight,
    DocumentHighlightKind,
};
pub use document_symbol::{
    document_symbols_parsed_source, document_symbols_tua_source, DocumentSymbol, DocumentSymbolKind,
};
pub use emit::{emit_lua, emit_lua_with_mode, emit_tua_source, emit_tua_source_with_mode};
#[cfg(any(test, feature = "test-fixture"))]
pub use fixture::{Fixture, FixtureExpectation, FixtureFile, FixtureMarker, FixtureTempDir};
pub use folding_range::{
    folding_ranges_parsed_source, folding_ranges_tua_source, FoldingRange, FoldingRangeKind,
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
pub use lua::builtin_definition_document;
pub use module_graph::{
    ModuleExportEntry, ModuleExportOrigin, ModuleGraph, ModuleGraphInput, ModuleGraphModule,
    ModuleGraphOptions, RequireGraphEdge, RequireResolution,
};
pub use module_index::{
    exported_field_for_fields, exported_type_for_fields, infer_module_exports,
    require_bindings_for_parsed_source, required_module_export_for_fields,
    required_module_export_type_for_fields, required_module_exports_for_receiver,
    required_module_for_receiver, resolve_required_module, ModuleExportField, ModuleExports,
    RequireBinding, RequiredModule,
};
pub use module_link::{module_links_tua_source, ModuleLink};
pub use parser::{
    extract_lua_source_model, extract_tua_source_model, normalize_tua_source,
    parse_lua_doc_annotations, parse_lua_doc_annotations_with_options, parse_lua_source,
    parse_raw_lua_source, parse_raw_tua_source, parse_tua_source, CompoundAssignment,
    CompoundOperator, FunctionAnnotation, FunctionTypeParameter, LocalAnnotation, LuaDocAlias,
    LuaDocAnnotation, LuaDocAnnotationKind, LuaDocAs, LuaDocCast, LuaDocClass, LuaDocDeprecated,
    LuaDocDiagnostic, LuaDocEnum, LuaDocField, LuaDocGeneric, LuaDocGenericParameter, LuaDocMeta,
    LuaDocModule, LuaDocNoDiscard, LuaDocParam, LuaDocParseOptions, LuaDocParseResult,
    LuaDocReturn, LuaDocReturnType, LuaDocTypeAnnotation, LuaDocVersion, NormalizedSource,
    ParameterAnnotation, ParsedLanguage, ParsedSource, RawParsedSource, SourcePosition, SourceSpan,
    TableFieldType, TuaType, TypeAlias, TypedSourceModel,
};
pub use project::project_description;
pub use project_model::{
    LoveVersion, LuaVersion, ProjectFileAssociation, ProjectModel, ProjectModelOptions,
    ProjectTarget, ProjectWorkspace,
};
pub use references::{
    global_function_definition_count, global_function_name_for_definition,
    global_references_to_definition_parsed_source, global_type_alias_name_for_definition,
    global_type_alias_references_to_definition_parsed_source, references_parsed_source,
    references_to_definition_parsed_source, references_tua_source, type_alias_definition_count,
    Reference, ReferenceKind,
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
pub use semantic_token::{
    semantic_tokens_parsed_source, semantic_tokens_tua_source, SemanticToken, SemanticTokenKind,
    SemanticTokenModifier,
};
pub use signature_help::{
    signature_help_parsed_source, signature_help_tua_source, ParameterInformation, SignatureHelp,
    SignatureInformation,
};
pub use type_database::{
    ProjectTypeBinding, ProjectTypeDatabase, ProjectTypeDatabaseInput, ProjectTypeModule,
    ProjectTypeOrigin,
};
pub use types::{
    build_type_environment, is_assignable, FunctionType, PrimitiveType, TableField, TableShape,
    Type, TypeContext, TypeEnvironment, TypedFunction, TypedParameter,
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

pub mod check;
pub mod completion;
pub mod config;
pub mod definition;
pub mod diagnostic;
pub mod document_symbol;
pub mod emit;
pub mod hover;
pub mod inlay_hint;
mod lua;
pub mod module_index;
pub mod parser;
mod project;
pub mod references;
pub mod rename;
mod semantic;
pub mod semantic_token;
mod source_text;
pub mod types;
pub mod workspace_symbol;

pub use check::{check_parsed_source, check_tua_source};
pub use completion::{complete_parsed_source, complete_tua_source, Completion, CompletionKind};
pub use config::{
    init_project, load_config_from_dir, parse_config, ConfigError, ConfigParseError, InitResult,
    LoadedConfig, ProjectConfig, CONFIG_FILE, DEFAULT_CONFIG_TEXT,
};
pub use definition::{definition_parsed_source, definition_tua_source, Definition, DefinitionKind};
pub use diagnostic::{
    format_diagnostics_for_cli, Diagnostic, DiagnosticCategory, DiagnosticCode, DiagnosticSeverity,
};
pub use document_symbol::{
    document_symbols_parsed_source, document_symbols_tua_source, DocumentSymbol, DocumentSymbolKind,
};
pub use emit::{emit_lua, emit_tua_source};
pub use hover::{hover_parsed_source, hover_tua_source, Hover};
pub use inlay_hint::{inlay_hints_parsed_source, inlay_hints_tua_source, InlayHint, InlayHintKind};
pub use module_index::{
    infer_module_exports, required_module_exports_for_receiver, resolve_required_module,
    ModuleExports,
};
pub use parser::{
    parse_lua_source, parse_tua_source, CompoundAssignment, CompoundOperator, FunctionAnnotation,
    LocalAnnotation, ParameterAnnotation, ParsedSource, SourcePosition, SourceSpan, TableFieldType,
    TuaType, TypeAlias,
};
pub use project::project_description;
pub use references::{references_parsed_source, references_tua_source, Reference, ReferenceKind};
pub use rename::{
    prepare_rename_parsed_source, prepare_rename_tua_source, rename_parsed_source,
    rename_tua_source, RenameEdit, RenameKind, RenameTarget, TextEdit,
};
pub use semantic_token::{
    semantic_tokens_parsed_source, semantic_tokens_tua_source, SemanticToken, SemanticTokenKind,
    SemanticTokenModifier,
};
pub use types::{
    build_type_environment, is_assignable, FunctionType, PrimitiveType, TableField, TableShape,
    Type, TypeContext, TypeEnvironment, TypedFunction, TypedParameter,
};
pub use workspace_symbol::{
    workspace_symbols_from_dir, workspace_symbols_from_files, WorkspaceSymbol, WorkspaceSymbolKind,
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

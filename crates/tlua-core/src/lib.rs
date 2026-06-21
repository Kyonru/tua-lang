pub mod check;
pub mod completion;
pub mod config;
pub mod definition;
pub mod diagnostic;
pub mod document_symbol;
pub mod emit;
pub mod hover;
pub mod inlay_hint;
pub mod parser;
mod project;
pub mod references;
pub mod rename;
mod semantic;
mod source_text;
pub mod types;

pub use check::{check_parsed_source, check_tlua_source};
pub use completion::{complete_parsed_source, complete_tlua_source, Completion, CompletionKind};
pub use config::{
    init_project, load_config_from_dir, parse_config, ConfigError, ConfigParseError, InitResult,
    LoadedConfig, ProjectConfig, CONFIG_FILE, DEFAULT_CONFIG_TEXT,
};
pub use definition::{
    definition_parsed_source, definition_tlua_source, Definition, DefinitionKind,
};
pub use diagnostic::{
    format_diagnostics_for_cli, Diagnostic, DiagnosticCategory, DiagnosticCode, DiagnosticSeverity,
};
pub use document_symbol::{
    document_symbols_parsed_source, document_symbols_tlua_source, DocumentSymbol,
    DocumentSymbolKind,
};
pub use emit::{emit_lua, emit_tlua_source};
pub use hover::{hover_parsed_source, hover_tlua_source, Hover};
pub use inlay_hint::{
    inlay_hints_parsed_source, inlay_hints_tlua_source, InlayHint, InlayHintKind,
};
pub use parser::{
    parse_lua_source, parse_tlua_source, CompoundAssignment, CompoundOperator, FunctionAnnotation,
    LocalAnnotation, ParameterAnnotation, ParsedSource, SourcePosition, SourceSpan, TableFieldType,
    TluaType, TypeAlias,
};
pub use project::project_description;
pub use references::{references_parsed_source, references_tlua_source, Reference, ReferenceKind};
pub use rename::{
    prepare_rename_parsed_source, prepare_rename_tlua_source, rename_parsed_source,
    rename_tlua_source, RenameEdit, RenameKind, RenameTarget, TextEdit,
};
pub use types::{
    build_type_environment, is_assignable, FunctionType, PrimitiveType, TableField, TableShape,
    Type, TypeContext, TypeEnvironment, TypedFunction, TypedParameter,
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

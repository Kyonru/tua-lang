pub mod check;
pub mod config;
pub mod diagnostic;
pub mod emit;
pub mod hover;
pub mod parser;
mod project;
pub mod types;

pub use check::{check_parsed_source, check_tlua_source};
pub use config::{
    init_project, load_config_from_dir, parse_config, ConfigError, ConfigParseError, InitResult,
    LoadedConfig, ProjectConfig, CONFIG_FILE, DEFAULT_CONFIG_TEXT,
};
pub use diagnostic::{
    format_diagnostics_for_cli, Diagnostic, DiagnosticCategory, DiagnosticCode, DiagnosticSeverity,
};
pub use emit::{emit_lua, emit_tlua_source};
pub use hover::{hover_parsed_source, hover_tlua_source, Hover};
pub use parser::{
    parse_lua_source, parse_tlua_source, CompoundAssignment, CompoundOperator, FunctionAnnotation,
    LocalAnnotation, ParameterAnnotation, ParsedSource, SourcePosition, SourceSpan, TableFieldType,
    TluaType, TypeAlias,
};
pub use project::project_description;
pub use types::{
    build_type_environment, is_assignable, FunctionType, PrimitiveType, TableField, TableShape,
    Type, TypeContext, TypeEnvironment, TypedFunction, TypedParameter,
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

impl fmt::Display for DiagnosticSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
            Self::Info => write!(f, "info"),
            Self::Hint => write!(f, "hint"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticCategory {
    Syntax,
    Type,
    LoweringEmission,
    ProjectConfig,
    Lint,
    Domain,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DiagnosticCode(u16);

impl DiagnosticCode {
    pub const SYNTAX_PARSE: Self = Self(1);
    pub const SYNTAX_COMPOUND_ASSIGNMENT: Self = Self(2);
    pub const SYNTAX_DOC_ANNOTATION: Self = Self(3);
    pub const SYNTAX_UNSUPPORTED_DIALECT: Self = Self(4);
    pub const TYPE_UNKNOWN_NAME: Self = Self(1001);
    pub const TYPE_DUPLICATE_ALIAS: Self = Self(1002);
    pub const TYPE_MISMATCH: Self = Self(1003);
    pub const TYPE_ARGUMENT_COUNT: Self = Self(1004);
    pub const TYPE_IMPOSSIBLE_COMPARISON: Self = Self(1005);
    pub const TYPE_DOC_CONFLICT: Self = Self(1006);
    pub const TYPE_UNSUPPORTED_RUNTIME_CHECK: Self = Self(1007);
    pub const TYPE_CLASS: Self = Self(1008);
    pub const TYPE_IMPORT_COLLISION: Self = Self(1009);
    pub const TYPE_IMPORT_CYCLE: Self = Self(1010);
    pub const TYPE_UNSAFE_UNION_OPERATION: Self = Self(1011);
    pub const TYPE_UNKNOWN_MODULE_EXPORT: Self = Self(1012);
    pub const TYPE_UNPARSED_MODULE: Self = Self(1013);
    pub const LOWERING_COMPOUND_ASSIGNMENT: Self = Self(2001);
    pub const EMIT_IO: Self = Self(2002);
    pub const EMIT_OUTPUT_COLLISION: Self = Self(2003);
    pub const ASSET_MISSING: Self = Self(2004);
    pub const ASSET_UNSUPPORTED_EXTENSION: Self = Self(2005);
    pub const ASSET_WRONG_KIND: Self = Self(2006);
    pub const LOWERING_OPERATION: Self = Self(2007);
    pub const ASSET_PATH_CASE: Self = Self(2008);
    pub const ASSET_UNUSED: Self = Self(2009);
    pub const EMIT_UNSUPPORTED_SYNTAX: Self = Self(2010);
    pub const ASSET_PIPELINE: Self = Self(2011);
    pub const AUDIO_ANALYSIS: Self = Self(2012);
    pub const ASSET_SHADER_UNIFORM: Self = Self(2013);
    pub const ASSET_SHADER_UNREAD_UNIFORM: Self = Self(2014);
    pub const ASSET_SHADER_VALUE: Self = Self(2015);
    pub const ASSET_SHADER_COMPILE: Self = Self(2016);
    pub const ASSET_SHADER_ENTRY_POINT: Self = Self(2017);
    pub const ASSET_SHADER_UNSENT_UNIFORM: Self = Self(2018);
    pub const CONFIG_PARSE: Self = Self(3001);
    pub const CONFIG_IO: Self = Self(3002);
    pub const LINT_DUPLICATE_TABLE_KEY: Self = Self(4001);
    pub const LINT_MISSING_REQUIRE: Self = Self(4002);
    pub const LINT_SELF_ASSIGNMENT: Self = Self(4003);
    pub const LINT_UNDEFINED_VARIABLE: Self = Self(4004);
    pub const LINT_UNUSED_LOCAL: Self = Self(4005);
    pub const LINT_UNUSED_PARAMETER: Self = Self(4006);
    pub const LINT_USE_BEFORE_DEFINITION: Self = Self(4007);
    // Retained for source compatibility; duplicate parameters now emit LINT_DUPLICATE_LOCAL.
    pub const LINT_DUPLICATE_PARAMETER: Self = Self(4008);
    pub const LINT_BAD_REQUIRE: Self = Self(4010);
    pub const LINT_IMPOSSIBLE_BOOLEAN: Self = Self(4011);
    pub const LINT_DUPLICATE_IF_CONDITION: Self = Self(4012);
    pub const LINT_DUPLICATE_BINARY_OPERAND: Self = Self(4013);
    pub const LINT_FLOAT_EQUALITY: Self = Self(4014);
    pub const LINT_CONST_ASSIGNMENT: Self = Self(4015);
    pub const LINT_DOT_METHOD_CALL: Self = Self(4018);
    pub const LINT_DUPLICATE_REQUIRE_CANDIDATE: Self = Self(4019);
    pub const LINT_REQUIRE_CYCLE: Self = Self(4020);
    pub const LINT_INLINE_IGNORE_INVALID: Self = Self(4025);
    pub const LINT_INLINE_IGNORE_UNUSED: Self = Self(4026);
    pub const LINT_ASSIGNMENT_ARITY: Self = Self(4027);
    pub const LINT_LOCAL_DECLARATION_ARITY: Self = Self(4028);
    pub const LINT_UNUSED_ASSIGNMENT: Self = Self(4029);
    pub const LINT_BINARY_OPERATOR_TYPE: Self = Self(4030);
    pub const LINT_UNCALLED_LOCAL_FUNCTION: Self = Self(4031);
    pub const LINT_JIT_TYPE_DRIFT: Self = Self(4032);
    pub const LINT_JIT_TABLE_SHAPE_DRIFT: Self = Self(4033);
    pub const LINT_JIT_HOT_LOOP_DYNAMIC_KEY: Self = Self(4034);
    pub const LINT_LOCAL_SHADOW: Self = Self(4035);
    pub const LINT_SAME_LINE_STATEMENT: Self = Self(4036);
    pub const LINT_MULTI_LINE_STATEMENT: Self = Self(4037);
    pub const LINT_BUILTIN_GLOBAL_WRITE: Self = Self(4038);
    pub const LINT_PLACEHOLDER_READ: Self = Self(4039);
    pub const LINT_UNREACHABLE_CODE: Self = Self(4040);
    pub const LINT_UNKNOWN_RUNTIME_TYPE: Self = Self(4041);
    pub const LINT_FOR_RANGE: Self = Self(4042);
    pub const LINT_DUPLICATE_LOCAL: Self = Self(4043);
    pub const LINT_FORMAT_STRING: Self = Self(4044);
    pub const LINT_UNINITIALIZED_LOCAL: Self = Self(4045);
    pub const LINT_DUPLICATE_FUNCTION: Self = Self(4046);
    pub const LINT_TABLE_OPERATION: Self = Self(4047);
    pub const LINT_MISLEADING_AND_OR: Self = Self(4048);
    pub const LINT_COMMENT_DIRECTIVE: Self = Self(4049);
    pub const LINT_INTEGER_PRECISION: Self = Self(4050);
    pub const LINT_COMPARISON_PRECEDENCE: Self = Self(4051);
    pub const LINT_GLOBAL_USED_AS_LOCAL: Self = Self(4052);
    pub const LINT_IMPLICIT_RETURN: Self = Self(4053);
    pub const LINT_UNUSED_IMPORT: Self = Self(4054);
    pub const LINT_UNUSED_FUNCTION: Self = Self(4055);
    pub const LINT_DEPRECATED_API: Self = Self(4056);
    pub const LINT_CLASS_PARENT_CONSTRUCTOR: Self = Self(4057);
    pub const LINT_CLASS_VIRTUAL_CALL_IN_INIT: Self = Self(4058);
    pub const LINT_CLASS_REDUNDANT_INHERITED_FIELD: Self = Self(4059);
    pub const LINT_VECTOR_LENGTH: Self = Self(4060);
    pub const LINT_NEWLINE_CALL: Self = Self(4061);
    pub const LINT_NEWFIELD_CALL: Self = Self(4062);
    pub const LINT_UNSCOPED_GLOBAL_WRITE: Self = Self(4063);
    pub const LINT_CONSTANT_IDENTITY: Self = Self(4064);
    pub const LINT_IDENTICAL_BRANCHES: Self = Self(4065);
    pub const LINT_EMPTY_BLOCK: Self = Self(4066);
    pub const LINT_SINGLE_ITERATION_LOOP: Self = Self(4067);
    pub const LINT_CANONICAL_MODULE_SPELLING: Self = Self(4069);
    pub const LINT_MUST_USE_RESULT: Self = Self(4068);
    pub const LINT_HIGH_CYCLOMATIC_COMPLEXITY: Self = Self(4070);
    pub const LINT_DIVIDE_BY_ZERO: Self = Self(4071);
    pub const LINT_ALMOST_SWAPPED: Self = Self(4072);
    pub const LINT_MIXED_TABLE: Self = Self(4073);
    pub const LINT_UNKNOWN_RECORD_FIELD: Self = Self(4074);
    pub const LINT_UNUSED_TYPE: Self = Self(4075);
    pub const DOMAIN_ECS_COMPONENT_VALUE: Self = Self(5002);
    pub const DOMAIN_ECS_QUERY: Self = Self(5003);
    pub const DOMAIN_SPRITE: Self = Self(5004);
    pub const DOMAIN_SIGNAL_SCHEMA: Self = Self(5005);
    pub const DOMAIN_LOVE_CALLBACK: Self = Self(5006);
    pub const DOMAIN_OBJECT_POOL_LIFECYCLE: Self = Self(5007);
    pub const DOMAIN_INPUT_CONFIG: Self = Self(5008);
    pub const DOMAIN_AUDIO_CONFIG: Self = Self(5009);

    pub fn new(number: u16) -> Option<Self> {
        if (1..=5999).contains(&number) {
            Some(Self(number))
        } else {
            None
        }
    }

    pub fn as_u16(self) -> u16 {
        self.0
    }

    pub fn category(self) -> DiagnosticCategory {
        match self.0 {
            1..=999 => DiagnosticCategory::Syntax,
            1000..=1999 => DiagnosticCategory::Type,
            2000..=2999 => DiagnosticCategory::LoweringEmission,
            3000..=3999 => DiagnosticCategory::ProjectConfig,
            4000..=4999 => DiagnosticCategory::Lint,
            5000..=5999 => DiagnosticCategory::Domain,
            _ => unreachable!("diagnostic codes are validated at construction"),
        }
    }

    pub fn is_known(self) -> bool {
        matches!(
            self,
            Self::SYNTAX_PARSE
                | Self::SYNTAX_COMPOUND_ASSIGNMENT
                | Self::SYNTAX_DOC_ANNOTATION
                | Self::SYNTAX_UNSUPPORTED_DIALECT
                | Self::TYPE_UNKNOWN_NAME
                | Self::TYPE_DUPLICATE_ALIAS
                | Self::TYPE_MISMATCH
                | Self::TYPE_ARGUMENT_COUNT
                | Self::TYPE_IMPOSSIBLE_COMPARISON
                | Self::TYPE_DOC_CONFLICT
                | Self::TYPE_UNSUPPORTED_RUNTIME_CHECK
                | Self::TYPE_CLASS
                | Self::TYPE_IMPORT_COLLISION
                | Self::TYPE_IMPORT_CYCLE
                | Self::TYPE_UNSAFE_UNION_OPERATION
                | Self::TYPE_UNKNOWN_MODULE_EXPORT
                | Self::TYPE_UNPARSED_MODULE
                | Self::LOWERING_COMPOUND_ASSIGNMENT
                | Self::EMIT_IO
                | Self::EMIT_OUTPUT_COLLISION
                | Self::ASSET_MISSING
                | Self::ASSET_UNSUPPORTED_EXTENSION
                | Self::ASSET_WRONG_KIND
                | Self::LOWERING_OPERATION
                | Self::ASSET_PATH_CASE
                | Self::ASSET_UNUSED
                | Self::EMIT_UNSUPPORTED_SYNTAX
                | Self::ASSET_PIPELINE
                | Self::AUDIO_ANALYSIS
                | Self::ASSET_SHADER_UNIFORM
                | Self::ASSET_SHADER_UNREAD_UNIFORM
                | Self::ASSET_SHADER_VALUE
                | Self::ASSET_SHADER_COMPILE
                | Self::ASSET_SHADER_ENTRY_POINT
                | Self::ASSET_SHADER_UNSENT_UNIFORM
                | Self::CONFIG_PARSE
                | Self::CONFIG_IO
                | Self::LINT_DUPLICATE_TABLE_KEY
                | Self::LINT_MISSING_REQUIRE
                | Self::LINT_SELF_ASSIGNMENT
                | Self::LINT_UNDEFINED_VARIABLE
                | Self::LINT_UNUSED_LOCAL
                | Self::LINT_UNUSED_PARAMETER
                | Self::LINT_USE_BEFORE_DEFINITION
                | Self::LINT_DUPLICATE_PARAMETER
                | Self::LINT_BAD_REQUIRE
                | Self::LINT_IMPOSSIBLE_BOOLEAN
                | Self::LINT_DUPLICATE_IF_CONDITION
                | Self::LINT_DUPLICATE_BINARY_OPERAND
                | Self::LINT_FLOAT_EQUALITY
                | Self::LINT_CONST_ASSIGNMENT
                | Self::LINT_DOT_METHOD_CALL
                | Self::LINT_DUPLICATE_REQUIRE_CANDIDATE
                | Self::LINT_REQUIRE_CYCLE
                | Self::LINT_INLINE_IGNORE_INVALID
                | Self::LINT_INLINE_IGNORE_UNUSED
                | Self::LINT_ASSIGNMENT_ARITY
                | Self::LINT_LOCAL_DECLARATION_ARITY
                | Self::LINT_UNUSED_ASSIGNMENT
                | Self::LINT_BINARY_OPERATOR_TYPE
                | Self::LINT_UNCALLED_LOCAL_FUNCTION
                | Self::LINT_JIT_TYPE_DRIFT
                | Self::LINT_JIT_TABLE_SHAPE_DRIFT
                | Self::LINT_JIT_HOT_LOOP_DYNAMIC_KEY
                | Self::LINT_LOCAL_SHADOW
                | Self::LINT_SAME_LINE_STATEMENT
                | Self::LINT_MULTI_LINE_STATEMENT
                | Self::LINT_BUILTIN_GLOBAL_WRITE
                | Self::LINT_PLACEHOLDER_READ
                | Self::LINT_UNREACHABLE_CODE
                | Self::LINT_UNKNOWN_RUNTIME_TYPE
                | Self::LINT_FOR_RANGE
                | Self::LINT_DUPLICATE_LOCAL
                | Self::LINT_FORMAT_STRING
                | Self::LINT_UNINITIALIZED_LOCAL
                | Self::LINT_DUPLICATE_FUNCTION
                | Self::LINT_TABLE_OPERATION
                | Self::LINT_MISLEADING_AND_OR
                | Self::LINT_COMMENT_DIRECTIVE
                | Self::LINT_INTEGER_PRECISION
                | Self::LINT_COMPARISON_PRECEDENCE
                | Self::LINT_GLOBAL_USED_AS_LOCAL
                | Self::LINT_IMPLICIT_RETURN
                | Self::LINT_UNUSED_IMPORT
                | Self::LINT_UNUSED_FUNCTION
                | Self::LINT_DEPRECATED_API
                | Self::LINT_CLASS_PARENT_CONSTRUCTOR
                | Self::LINT_CLASS_VIRTUAL_CALL_IN_INIT
                | Self::LINT_CLASS_REDUNDANT_INHERITED_FIELD
                | Self::LINT_VECTOR_LENGTH
                | Self::LINT_NEWLINE_CALL
                | Self::LINT_NEWFIELD_CALL
                | Self::LINT_UNSCOPED_GLOBAL_WRITE
                | Self::LINT_CONSTANT_IDENTITY
                | Self::LINT_IDENTICAL_BRANCHES
                | Self::LINT_EMPTY_BLOCK
                | Self::LINT_SINGLE_ITERATION_LOOP
                | Self::LINT_CANONICAL_MODULE_SPELLING
                | Self::LINT_MUST_USE_RESULT
                | Self::LINT_HIGH_CYCLOMATIC_COMPLEXITY
                | Self::LINT_DIVIDE_BY_ZERO
                | Self::LINT_ALMOST_SWAPPED
                | Self::LINT_MIXED_TABLE
                | Self::LINT_UNKNOWN_RECORD_FIELD
                | Self::LINT_UNUSED_TYPE
                | Self::DOMAIN_ECS_COMPONENT_VALUE
                | Self::DOMAIN_ECS_QUERY
                | Self::DOMAIN_SPRITE
                | Self::DOMAIN_SIGNAL_SCHEMA
                | Self::DOMAIN_LOVE_CALLBACK
                | Self::DOMAIN_OBJECT_POOL_LIFECYCLE
                | Self::DOMAIN_INPUT_CONFIG
                | Self::DOMAIN_AUDIO_CONFIG
        )
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TL{:04}", self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub code: Option<DiagnosticCode>,
}

impl Diagnostic {
    pub fn new(
        file: impl Into<PathBuf>,
        line: usize,
        column: usize,
        severity: DiagnosticSeverity,
        message: impl Into<String>,
    ) -> Self {
        Self {
            file: file.into(),
            line: line.max(1),
            column: column.max(1),
            end_line: line.max(1),
            end_column: column.max(1).saturating_add(1),
            severity,
            message: message.into(),
            code: None,
        }
    }

    pub fn error(
        file: impl Into<PathBuf>,
        line: usize,
        column: usize,
        message: impl Into<String>,
    ) -> Self {
        Self::new(file, line, column, DiagnosticSeverity::Error, message)
    }

    pub fn with_code(mut self, code: DiagnosticCode) -> Self {
        self.code = Some(code);
        self
    }

    pub fn with_range(
        mut self,
        line: usize,
        column: usize,
        end_line: usize,
        end_column: usize,
    ) -> Self {
        self.line = line.max(1);
        self.column = column.max(1);
        self.end_line = end_line.max(self.line);
        self.end_column = if self.end_line == self.line {
            end_column.max(self.column.saturating_add(1))
        } else {
            end_column.max(1)
        };
        self
    }

    pub fn format_cli(&self) -> String {
        let path = display_path(&self.file);

        match self.code {
            Some(code) => format!(
                "{}:{}:{} {} {}: {}",
                path, self.line, self.column, self.severity, code, self.message
            ),
            None => format!(
                "{}:{}:{} {}: {}",
                path, self.line, self.column, self.severity, self.message
            ),
        }
    }
}

pub(crate) fn expand_identifier_ranges(source: &str, diagnostics: &mut [Diagnostic]) {
    if !diagnostics.iter().any(|diagnostic| {
        diagnostic.line == diagnostic.end_line
            && diagnostic.end_column == diagnostic.column.saturating_add(1)
    }) {
        return;
    }

    let identifiers = crate::source_text::source_identifiers(source)
        .into_iter()
        .map(|identifier| {
            (
                (identifier.span.start.line, identifier.span.start.column),
                identifier.span.end,
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    for diagnostic in diagnostics {
        let has_default_range = diagnostic.line == diagnostic.end_line
            && diagnostic.end_column == diagnostic.column.saturating_add(1);
        if !has_default_range {
            continue;
        }

        let start = (diagnostic.line, diagnostic.column);
        let Some(end) = identifiers.get(&start) else {
            continue;
        };

        diagnostic.end_line = end.line;
        diagnostic.end_column = end.column;
    }
}

pub fn format_diagnostics_for_cli(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(Diagnostic::format_cli)
        .collect::<Vec<_>>()
        .join("\n")
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests;

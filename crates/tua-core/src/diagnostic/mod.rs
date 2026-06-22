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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticCode(u16);

impl DiagnosticCode {
    pub const SYNTAX_PARSE: Self = Self(1);
    pub const SYNTAX_COMPOUND_ASSIGNMENT: Self = Self(2);
    pub const SYNTAX_DOC_ANNOTATION: Self = Self(3);
    pub const TYPE_UNKNOWN_NAME: Self = Self(1001);
    pub const TYPE_DUPLICATE_ALIAS: Self = Self(1002);
    pub const TYPE_MISMATCH: Self = Self(1003);
    pub const TYPE_ARGUMENT_COUNT: Self = Self(1004);
    pub const TYPE_IMPOSSIBLE_COMPARISON: Self = Self(1005);
    pub const TYPE_DOC_CONFLICT: Self = Self(1006);
    pub const LOWERING_COMPOUND_ASSIGNMENT: Self = Self(2001);
    pub const EMIT_IO: Self = Self(2002);
    pub const EMIT_OUTPUT_COLLISION: Self = Self(2003);
    pub const CONFIG_PARSE: Self = Self(3001);
    pub const CONFIG_IO: Self = Self(3002);
    pub const LINT_DUPLICATE_TABLE_KEY: Self = Self(4001);
    pub const LINT_MISSING_REQUIRE: Self = Self(4002);
    pub const LINT_SELF_ASSIGNMENT: Self = Self(4003);
    pub const LINT_UNDEFINED_VARIABLE: Self = Self(4004);
    pub const LINT_UNUSED_LOCAL: Self = Self(4005);
    pub const LINT_UNUSED_PARAMETER: Self = Self(4006);
    pub const LINT_USE_BEFORE_DEFINITION: Self = Self(4007);

    pub fn new(number: u16) -> Option<Self> {
        if (1..=4999).contains(&number) {
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
            _ => unreachable!("diagnostic codes are validated at construction"),
        }
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

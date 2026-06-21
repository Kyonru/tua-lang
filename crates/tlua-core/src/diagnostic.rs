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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticCode(u16);

impl DiagnosticCode {
    pub const CONFIG_PARSE: Self = Self(3001);
    pub const CONFIG_IO: Self = Self(3002);

    pub fn new(number: u16) -> Option<Self> {
        if (1..=3999).contains(&number) {
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
mod tests {
    use super::*;

    #[test]
    fn reserves_spec_code_ranges() {
        let cases = [
            (1, DiagnosticCategory::Syntax),
            (999, DiagnosticCategory::Syntax),
            (1000, DiagnosticCategory::Type),
            (1999, DiagnosticCategory::Type),
            (2000, DiagnosticCategory::LoweringEmission),
            (2999, DiagnosticCategory::LoweringEmission),
            (3000, DiagnosticCategory::ProjectConfig),
            (3999, DiagnosticCategory::ProjectConfig),
        ];

        for (number, category) in cases {
            let code = DiagnosticCode::new(number).expect("code should be in reserved range");

            assert_eq!(code.category(), category);
            assert_eq!(code.as_u16(), number);
        }

        assert_eq!(DiagnosticCode::new(0), None);
        assert_eq!(DiagnosticCode::new(4000), None);
    }

    #[test]
    fn formats_cli_diagnostic_with_code_snapshot() {
        let diagnostic = Diagnostic::error(
            "src/player.tlua",
            12,
            15,
            "type mismatch: expected number, found string",
        )
        .with_code(DiagnosticCode::new(1002).expect("valid code"));

        assert_eq!(
            diagnostic.format_cli(),
            "src/player.tlua:12:15 error TL1002: type mismatch: expected number, found string"
        );
    }

    #[test]
    fn formats_cli_diagnostic_without_code_snapshot() {
        let diagnostic = Diagnostic::new(
            "src/main.tlua",
            2,
            3,
            DiagnosticSeverity::Warning,
            "unused local `score`",
        );

        assert_eq!(
            diagnostic.format_cli(),
            "src/main.tlua:2:3 warning: unused local `score`"
        );
    }

    #[test]
    fn formats_multiple_cli_diagnostics_snapshot() {
        let diagnostics = vec![
            Diagnostic::error("tlua.toml", 1, 1, "unknown config key `mode`")
                .with_code(DiagnosticCode::CONFIG_PARSE),
            Diagnostic::new(
                "src/player.tlua",
                4,
                7,
                DiagnosticSeverity::Info,
                "inferred type number",
            ),
        ];

        assert_eq!(
            format_diagnostics_for_cli(&diagnostics),
            "tlua.toml:1:1 error TL3001: unknown config key `mode`\n\
src/player.tlua:4:7 info: inferred type number"
        );
    }
}

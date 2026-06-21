use tlua_core::{format_diagnostics_for_cli, ConfigError, Diagnostic};

#[derive(Debug, Eq, PartialEq)]
pub struct CliError {
    pub message: String,
    pub diagnostics: Vec<Diagnostic>,
}

impl CliError {
    pub(crate) fn usage(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            diagnostics: Vec::new(),
        }
    }

    pub(crate) fn from_config_error(error: ConfigError) -> Self {
        Self {
            message: "project configuration error".to_owned(),
            diagnostics: vec![error.to_diagnostic()],
        }
    }

    pub(crate) fn from_diagnostics(
        message: impl Into<String>,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        Self {
            message: message.into(),
            diagnostics,
        }
    }

    pub fn format_stderr(&self) -> String {
        if self.diagnostics.is_empty() {
            format!("error: {}", self.message)
        } else {
            format_diagnostics_for_cli(&self.diagnostics)
        }
    }
}

#[cfg(test)]
mod tests;

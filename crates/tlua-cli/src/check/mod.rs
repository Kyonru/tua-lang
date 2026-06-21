use std::path::Path;

use crate::error::CliError;
use crate::source::{discover_source_files, read_source_file};
use tlua_core::{check_tlua_source, Diagnostic, LoadedConfig};

pub(crate) fn run_check(cwd: impl AsRef<Path>, loaded: LoadedConfig) -> Result<String, CliError> {
    let cwd = cwd.as_ref();
    let source_dir = cwd.join(&loaded.config.source);
    let source_files =
        discover_source_files(&source_dir, &loaded.config.extension).map_err(|error| {
            CliError::from_diagnostics("failed to discover source files", vec![error])
        })?;
    let checked = source_files.len();
    let mut diagnostics = Vec::new();

    for source_path in source_files {
        let source = match read_source_file(&source_path) {
            Ok(source) => source,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };

        if let Err(mut source_diagnostics) = check_tlua_source(&source_path, source) {
            diagnostics.append(&mut source_diagnostics);
        }
    }

    if diagnostics.is_empty() {
        Ok(format!("checked {checked} file(s)"))
    } else {
        Err(CliError::from_diagnostics(
            "project check failed",
            sorted_diagnostics(diagnostics),
        ))
    }
}

fn sorted_diagnostics(mut diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    diagnostics.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.line.cmp(&right.line))
            .then(left.column.cmp(&right.column))
            .then(left.message.cmp(&right.message))
    });
    diagnostics
}

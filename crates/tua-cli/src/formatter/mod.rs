use std::fs;
use std::path::{Path, PathBuf};

use crate::diagnostics::sorted_diagnostics;
use crate::error::CliError;
use crate::source::{discover_build_files, io_diagnostic, read_source_file, SourceKind};
use tua_core::{format_tua_source, LoadedConfig, ProjectWorkspace};

pub(crate) fn run_fmt(
    cwd: impl AsRef<Path>,
    loaded: LoadedConfig,
    check: bool,
) -> Result<String, CliError> {
    let cwd = cwd.as_ref();
    let workspace = ProjectWorkspace::from_loaded(cwd, loaded);
    let source_dir = workspace.primary_source_root().to_path_buf();
    let source_files = discover_build_files(&source_dir, &workspace.config().extension, None)
        .map_err(|error| {
            CliError::from_diagnostics("failed to discover source files", vec![error])
        })?;
    let options = workspace.config().format.options();
    let mut diagnostics = Vec::new();
    let mut changed = Vec::new();
    let mut formatted_count = 0usize;

    for source_file in source_files {
        if source_file.kind != SourceKind::Tua {
            continue;
        }

        formatted_count += 1;
        let source = match read_source_file(&source_file.path) {
            Ok(source) => source,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };
        let formatted = match format_tua_source(&source_file.path, &source, &options) {
            Ok(formatted) => formatted,
            Err(mut source_diagnostics) => {
                diagnostics.append(&mut source_diagnostics);
                continue;
            }
        };

        if formatted == source {
            continue;
        }

        changed.push(source_file.path.clone());
        if !check {
            if let Err(source) = fs::write(&source_file.path, formatted) {
                diagnostics.push(io_diagnostic(&source_file.path, source));
            }
        }
    }

    if !diagnostics.is_empty() {
        return Err(CliError::from_diagnostics(
            "format failed",
            sorted_diagnostics(diagnostics),
        ));
    }

    if check && !changed.is_empty() {
        return Err(CliError::usage(format_check_message(cwd, &changed)));
    }

    if check {
        Ok(format!("checked formatting for {formatted_count} file(s)"))
    } else {
        Ok(format!("formatted {formatted_count} file(s)"))
    }
}

fn format_check_message(cwd: &Path, changed: &[PathBuf]) -> String {
    let mut lines = vec!["format check failed; run `tua fmt`".to_owned()];

    for path in changed {
        let display = path
            .strip_prefix(cwd)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        lines.push(format!("would reformat {display}"));
    }

    lines.join("\n")
}

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::CliError;
use tlua_core::{emit_tlua_source, Diagnostic, DiagnosticCode, LoadedConfig};

pub(crate) fn run_build(
    cwd: impl AsRef<Path>,
    loaded: LoadedConfig,
    _emit_on_error: bool,
) -> Result<String, CliError> {
    let cwd = cwd.as_ref();
    let source_dir = cwd.join(&loaded.config.source);
    let out_dir = cwd.join(&loaded.config.out);
    let source_files =
        discover_source_files(&source_dir, &loaded.config.extension).map_err(|error| {
            CliError::from_diagnostics("failed to discover source files", vec![error])
        })?;
    let mut emitted = 0usize;

    for source_path in source_files {
        let relative_path = source_path
            .strip_prefix(&source_dir)
            .expect("discovered source file should be under source directory");
        let output_path = out_dir.join(relative_path).with_extension("lua");
        let source = fs::read_to_string(&source_path).map_err(|source| {
            CliError::from_diagnostics(
                "failed to read source file",
                vec![io_diagnostic(&source_path, source)],
            )
        })?;
        let output = emit_tlua_source(&source_path, source)
            .map_err(|diagnostics| CliError::from_diagnostics("failed to emit Lua", diagnostics))?;

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|source| {
                CliError::from_diagnostics(
                    "failed to create output directory",
                    vec![io_diagnostic(parent, source)],
                )
            })?;
        }

        fs::write(&output_path, output).map_err(|source| {
            CliError::from_diagnostics(
                "failed to write output file",
                vec![io_diagnostic(&output_path, source)],
            )
        })?;

        emitted += 1;
    }

    Ok(format!("built {emitted} file(s)"))
}

fn discover_source_files(source_dir: &Path, extension: &str) -> Result<Vec<PathBuf>, Diagnostic> {
    let mut files = Vec::new();

    discover_source_files_into(source_dir, extension, &mut files)?;
    files.sort();

    Ok(files)
}

fn discover_source_files_into(
    dir: &Path,
    extension: &str,
    files: &mut Vec<PathBuf>,
) -> Result<(), Diagnostic> {
    let entries = fs::read_dir(dir).map_err(|source| io_diagnostic(dir, source))?;

    for entry in entries {
        let entry = entry.map_err(|source| io_diagnostic(dir, source))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| io_diagnostic(&path, source))?;

        if file_type.is_dir() {
            discover_source_files_into(&path, extension, files)?;
        } else if file_type.is_file() && has_extension(&path, extension) {
            files.push(path);
        }
    }

    Ok(())
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value == extension)
}

fn io_diagnostic(path: &Path, source: io::Error) -> Diagnostic {
    Diagnostic::error(path, 1, 1, source.to_string()).with_code(DiagnosticCode::EMIT_IO)
}

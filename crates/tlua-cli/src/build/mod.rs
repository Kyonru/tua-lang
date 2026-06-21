use std::fs;
use std::path::Path;

use crate::error::CliError;
use crate::source::{discover_source_files, io_diagnostic, read_source_file};
use tlua_core::{emit_tlua_source, LoadedConfig};

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
        let source = read_source_file(&source_path).map_err(|diagnostic| {
            CliError::from_diagnostics("failed to read source file", vec![diagnostic])
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

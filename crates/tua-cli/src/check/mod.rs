use std::path::Path;

use crate::diagnostics::sorted_diagnostics;
use crate::error::CliError;
use crate::source::{discover_source_files, read_source_file};
use tua_core::{
    check_tua_source_with_asset_roots_and_diagnostics_config, LoadedConfig, ProjectWorkspace,
};

pub(crate) fn run_check(cwd: impl AsRef<Path>, loaded: LoadedConfig) -> Result<String, CliError> {
    let cwd = cwd.as_ref();
    let workspace = ProjectWorkspace::from_loaded(cwd, loaded);
    let source_dir = workspace.primary_source_root().to_path_buf();
    let asset_roots = workspace.asset_roots();
    let source_files =
        discover_source_files(&source_dir, &workspace.config().extension).map_err(|error| {
            CliError::from_diagnostics("failed to discover source files", vec![error])
        })?;
    let diagnostics_config = workspace.config().diagnostics.clone();
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

        if let Err(mut source_diagnostics) =
            check_tua_source_with_asset_roots_and_diagnostics_config(
                &source_path,
                source,
                &asset_roots,
                &diagnostics_config,
            )
        {
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

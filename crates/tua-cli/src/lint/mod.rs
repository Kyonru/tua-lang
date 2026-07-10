use std::path::Path;

use crate::check::run_project_diagnostics;
use crate::error::CliError;
use tua_core::LoadedConfig;

pub(crate) fn run_lint(
    cwd: impl AsRef<Path>,
    mut loaded: LoadedConfig,
    strict: bool,
) -> Result<String, CliError> {
    if strict {
        loaded.config.diagnostics.enable_all_lints();
    }

    run_project_diagnostics(cwd, loaded, "linted", "project lint failed")
}

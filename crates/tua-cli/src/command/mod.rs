use std::path::Path;

use crate::args::CommandName;
use crate::build::run_build;
use crate::check::run_check;
use crate::error::CliError;
use crate::watch::run_watch;

pub fn run_command(
    command: CommandName,
    emit_on_error: bool,
    cwd: impl AsRef<Path>,
) -> Result<String, CliError> {
    match command {
        CommandName::Init => tua_core::init_project(cwd)
            .map(format_init_result)
            .map_err(CliError::from_config_error),
        CommandName::Build => {
            let loaded =
                tua_core::load_config_from_dir(&cwd).map_err(CliError::from_config_error)?;

            run_build(cwd, loaded, emit_on_error)
        }
        CommandName::Check => {
            let loaded =
                tua_core::load_config_from_dir(&cwd).map_err(CliError::from_config_error)?;

            run_check(cwd, loaded)
        }
        CommandName::Watch => {
            run_watch_command(cwd)?;

            Ok("watch stopped".to_owned())
        }
        CommandName::Lsp => {
            run_lsp_command()?;

            Ok("lsp stopped".to_owned())
        }
    }
}

pub fn run_watch_command(cwd: impl AsRef<Path>) -> Result<(), CliError> {
    let loaded = tua_core::load_config_from_dir(&cwd).map_err(CliError::from_config_error)?;

    run_watch(cwd, loaded)
}

pub fn run_lsp_command() -> Result<(), CliError> {
    tua_lsp::run_stdio()
        .map_err(|source| CliError::usage(format!("failed to run tua lsp: {source}")))
}

fn format_init_result(result: tua_core::InitResult) -> String {
    let mut lines = vec!["initialized tua project".to_owned()];

    if result.config_created {
        lines.push("created tua.toml".to_owned());
    } else {
        lines.push("tua.toml already exists; left unchanged".to_owned());
    }

    if result.source_created {
        lines.push("created src/".to_owned());
    } else {
        lines.push("src/ already exists".to_owned());
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests;

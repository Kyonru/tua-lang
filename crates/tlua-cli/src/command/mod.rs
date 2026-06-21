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
        CommandName::Init => tlua_core::init_project(cwd)
            .map(format_init_result)
            .map_err(CliError::from_config_error),
        CommandName::Build => {
            let loaded =
                tlua_core::load_config_from_dir(&cwd).map_err(CliError::from_config_error)?;

            run_build(cwd, loaded, emit_on_error)
        }
        CommandName::Check => {
            let loaded =
                tlua_core::load_config_from_dir(&cwd).map_err(CliError::from_config_error)?;

            run_check(cwd, loaded)
        }
        CommandName::Watch => {
            run_watch_command(cwd)?;

            Ok("watch stopped".to_owned())
        }
        _ => Ok(placeholder_text(command)),
    }
}

pub fn run_watch_command(cwd: impl AsRef<Path>) -> Result<(), CliError> {
    let loaded = tlua_core::load_config_from_dir(&cwd).map_err(CliError::from_config_error)?;

    run_watch(cwd, loaded)
}

fn placeholder_text(command: CommandName) -> String {
    match command {
        CommandName::Init => unreachable!("tlua init is handled before placeholder_text"),
        CommandName::Check => unreachable!("tlua check is handled before placeholder_text"),
        CommandName::Build => unreachable!("tlua build is handled before placeholder_text"),
        CommandName::Watch => unreachable!("tlua watch is handled before placeholder_text"),
        CommandName::Lsp => format!("{} is not implemented yet", tlua_lsp::server_name()),
    }
}

fn format_init_result(result: tlua_core::InitResult) -> String {
    let mut lines = vec!["initialized tlua project".to_owned()];

    if result.config_created {
        lines.push("created tlua.toml".to_owned());
    } else {
        lines.push("tlua.toml already exists; left unchanged".to_owned());
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

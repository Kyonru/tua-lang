use std::path::Path;

use crate::args::{CommandName, CommandOptions};
use crate::build::run_build;
use crate::check::run_check;
use crate::error::CliError;
use crate::formatter::run_fmt;
use crate::linter::run_lint;
use crate::traceback::run_traceback;
use crate::watch::run_watch;

pub fn run_command(
    command: CommandName,
    options: CommandOptions,
    cwd: impl AsRef<Path>,
) -> Result<String, CliError> {
    match command {
        CommandName::Init => tua_core::init_project(cwd)
            .map(format_init_result)
            .map_err(CliError::from_config_error),
        CommandName::Build => {
            let loaded =
                tua_core::load_config_from_dir(&cwd).map_err(CliError::from_config_error)?;

            run_build(cwd, loaded, options.emit_on_error)
        }
        CommandName::Traceback => {
            let loaded =
                tua_core::load_config_from_dir(&cwd).map_err(CliError::from_config_error)?;

            run_traceback(cwd, loaded, options)
        }
        CommandName::Check => {
            let loaded =
                tua_core::load_config_from_dir(&cwd).map_err(CliError::from_config_error)?;

            run_check(cwd, loaded)
        }
        CommandName::Fmt => {
            let loaded =
                tua_core::load_config_from_dir(&cwd).map_err(CliError::from_config_error)?;

            run_fmt(cwd, loaded, options.fmt_check)
        }
        CommandName::Lint => {
            let loaded =
                tua_core::load_config_from_dir(&cwd).map_err(CliError::from_config_error)?;

            run_lint(cwd, loaded, options.lint_strict)
        }
        CommandName::Watch => {
            run_watch_command(cwd)?;

            Ok("watch stopped".to_owned())
        }
        CommandName::Lsp => {
            run_lsp_command()?;

            Ok("lsp stopped".to_owned())
        }
        CommandName::Api | CommandName::Mcp => Err(CliError::usage(
            "stdio protocol commands must be run through the tua executable",
        )),
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

    if result.main_created {
        lines.push("created src/main.tua".to_owned());
    } else {
        lines.push("src/main.tua already exists; left unchanged".to_owned());
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests;

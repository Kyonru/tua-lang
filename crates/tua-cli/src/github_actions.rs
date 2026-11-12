//! Opt-in GitHub workflow scaffolding. Packaging stays in `tua package`;
//! this command only writes a reviewable project workflow.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use tua_core::{path_security::reject_symlink_path, CONFIG_FILE};

use crate::error::CliError;

const WORKFLOW_PATH: &str = ".github/workflows/tua-game.yml";
const GAME_WORKFLOW_TEMPLATE: &str = include_str!("github_actions/tua-game.yml");

pub(crate) fn init_game_workflow(cwd: &Path) -> Result<String, CliError> {
    let root = cwd.canonicalize().map_err(|error| {
        CliError::usage(format!("cannot locate the project directory: {error}"))
    })?;
    let config = root.join(CONFIG_FILE);
    if !config.is_file() {
        return Err(CliError::usage(format!(
            "{} is missing; run `tua init` first",
            CONFIG_FILE
        )));
    }
    tua_core::load_config_from_dir(&root).map_err(CliError::from_config_error)?;

    let destination = root.join(WORKFLOW_PATH);
    reject_symlink_path(&root, &destination).map_err(CliError::usage)?;
    if destination.exists() {
        return Ok(format!("{WORKFLOW_PATH} already exists; left unchanged"));
    }
    let parent = destination.parent().expect("workflow has a parent");
    fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
    reject_symlink_path(&root, &destination).map_err(CliError::usage)?;
    let mut file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return Ok(format!("{WORKFLOW_PATH} already exists; left unchanged"));
        }
        Err(error) => return Err(io_error(&destination, error)),
    };
    let workflow = GAME_WORKFLOW_TEMPLATE.replace("@TUA_VERSION@", env!("CARGO_PKG_VERSION"));
    if let Err(error) = file.write_all(workflow.as_bytes()) {
        let _ = fs::remove_file(&destination);
        return Err(io_error(&destination, error));
    }
    Ok(format!("created {WORKFLOW_PATH}"))
}

fn io_error(path: &Path, error: io::Error) -> CliError {
    CliError::usage(format!("cannot write {}: {error}", path.display()))
}

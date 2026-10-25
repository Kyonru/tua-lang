//! Finding the LOVE executable that `tua dev` runs and `tua check` validates
//! shaders with.

use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
const MACOS_LOVE: &str = "/Applications/love.app/Contents/MacOS/love";

/// The commands to try in order: an explicit `--love` path, then `TUA_LOVE`,
/// then `love` on PATH and the platform's default install.
pub(crate) fn love_commands(explicit: Option<&Path>) -> Vec<PathBuf> {
    if let Some(command) = explicit
        .map(Path::to_path_buf)
        .or_else(|| std::env::var_os("TUA_LOVE").map(PathBuf::from))
    {
        return vec![command];
    }

    #[allow(unused_mut)]
    let mut commands = vec![PathBuf::from("love")];
    #[cfg(target_os = "macos")]
    commands.push(PathBuf::from(MACOS_LOVE));
    commands
}

pub(crate) const LOVE_NOT_FOUND: &str =
    "cannot find LOVE; put `love` on PATH, set TUA_LOVE, or pass --love PATH";

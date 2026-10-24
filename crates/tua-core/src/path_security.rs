use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Reject a target outside `root` or reached through any existing symbolic
/// link below it. Callers should validate immediately before filesystem
/// mutations to keep the check close to the operation it protects.
pub fn reject_symlink_path(root: &Path, target: &Path) -> Result<(), String> {
    let relative = target
        .strip_prefix(root)
        .map_err(|_| "output path escaped the project root".to_owned())?;
    let mut current = PathBuf::from(root);

    for component in relative.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "output path `{}` crosses a symbolic link",
                    display_path(&current)
                ));
            }
            Ok(_) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(format!(
                    "cannot inspect output path `{}`: {source}",
                    display_path(&current)
                ));
            }
        }
    }

    Ok(())
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

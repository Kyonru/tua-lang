use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

/// Reject a target outside `root` or reached through any existing symbolic
/// link below it. Callers should validate immediately before filesystem
/// mutations to keep the check close to the operation it protects.
pub fn reject_symlink_path(root: &Path, target: &Path) -> Result<(), String> {
    let relative = target
        .strip_prefix(root)
        .map_err(|_| "output path escaped the project root".to_owned())?;
    let mut current = PathBuf::from(root);

    for component in relative.components() {
        if !matches!(component, Component::Normal(_) | Component::CurDir) {
            return Err("output path contains traversal or a rooted component".to_owned());
        }
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if is_link_or_reparse_point(&metadata) => {
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

fn is_link_or_reparse_point(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Junctions and other reparse points can redirect path resolution even
        // when Rust does not classify them as symbolic links.
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_parent_components_even_for_missing_paths() {
        let root = std::env::temp_dir().join("tua-path-guard-root");
        assert!(reject_symlink_path(&root, &root.join("missing/../../victim")).is_err());
        assert!(reject_symlink_path(&root, &root.join("safe/file")).is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn rejects_windows_junction_ancestors() {
        let root = std::env::temp_dir().join(format!("tua-junction-{}", std::process::id()));
        let project = root.join("project");
        let outside = root.join("outside");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let link = project.join("build");
        let result = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(reject_symlink_path(&project, &link.join("victim")).is_err());
        fs::remove_dir(&link).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}

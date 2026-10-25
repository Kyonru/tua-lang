//! Prepare replacements beside their destination before publishing them.
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn sibling(path: &Path) -> PathBuf {
    path.with_file_name(format!(
        ".tua-stage-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

pub(crate) fn write(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> io::Result<()> {
    write_with(path.as_ref(), |file| file.write_all(bytes.as_ref()))
}

fn write_with(path: &Path, writer: impl FnOnce(&mut File) -> io::Result<()>) -> io::Result<()> {
    write_with_options(path, writer, true)
}

pub(crate) fn copy(source: &Path, destination: &Path) -> io::Result<()> {
    let mut source = File::open(source)?;
    let permissions = source.metadata()?.permissions();
    write_with_options(
        destination,
        |file| {
            io::copy(&mut source, file)?;
            file.set_permissions(permissions)
        },
        false,
    )
}

fn write_with_options(
    path: &Path,
    writer: impl FnOnce(&mut File) -> io::Result<()>,
    preserve_permissions: bool,
) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("destination has no parent"))?;
    tua_core::path_security::reject_symlink_path(parent, path).map_err(io::Error::other)?;
    let (temp, mut file) = loop {
        let temp = sibling(path);
        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file) => break (temp, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };
    let result = (|| {
        writer(&mut file)?;
        if preserve_permissions {
            if let Ok(metadata) = fs::metadata(path) {
                file.set_permissions(metadata.permissions())?;
            }
        }
        file.sync_all()?;
        drop(file);
        tua_core::path_security::reject_symlink_path(parent, path).map_err(io::Error::other)?;
        fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

pub(crate) struct DirectoryStage {
    path: PathBuf,
}

impl DirectoryStage {
    pub(crate) fn new(destination: &Path) -> io::Result<Self> {
        fs::create_dir_all(
            destination
                .parent()
                .ok_or_else(|| io::Error::other("destination has no parent"))?,
        )?;
        loop {
            let path = sibling(destination);
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn publish(self, destination: &Path) -> io::Result<()> {
        // Reserve the backup name ourselves; do not overwrite an existing path.
        let backup = Self::new(destination)?;
        let previous = destination.try_exists()?;
        if previous {
            let old = backup.path.join("previous");
            fs::rename(destination, &old)?;
            if let Err(error) = fs::rename(&self.path, destination) {
                if let Err(rollback) = fs::rename(&old, destination) {
                    // Preserve the previous bundle for manual recovery.
                    let recovery = backup.path.clone();
                    std::mem::forget(backup);
                    return Err(io::Error::other(format!(
                        "{error}; rollback failed: {rollback}; previous bundle retained at {}",
                        recovery.display()
                    )));
                }
                return Err(error);
            }
        } else {
            fs::rename(&self.path, destination)?;
        }
        Ok(())
    }
}

impl Drop for DirectoryStage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_write_preserves_previous_bytes_and_cleans_temporary_file() {
        let base = std::env::temp_dir().join("tua-atomic-test");
        let stage = DirectoryStage::new(&base).unwrap();
        let target = stage.path().join("file");
        fs::write(&target, "original").unwrap();
        assert!(write_with(&target, |file| {
            file.write_all(b"partial")?;
            Err(io::Error::other("injected write failure"))
        })
        .is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "original");
        assert_eq!(fs::read_dir(stage.path()).unwrap().count(), 1);
        write(&target, "replacement").unwrap();
        assert_eq!(fs::read_to_string(target).unwrap(), "replacement");
    }

    #[test]
    fn copy_failure_preserves_previous_asset() {
        let stage = DirectoryStage::new(&std::env::temp_dir().join("tua-atomic-copy")).unwrap();
        let target = stage.path().join("asset");
        fs::write(&target, b"original").unwrap();
        assert!(copy(&stage.path().join("missing"), &target).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"original");
        let source = stage.path().join("source");
        fs::write(&source, b"replacement").unwrap();
        copy(&source, &target).unwrap();
        assert_eq!(fs::read(target).unwrap(), b"replacement");
    }
}

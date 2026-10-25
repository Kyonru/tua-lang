//! Persist only outputs owned by successful, non-overlapping builds.
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use super::{io_diagnostic, OutputGuard};
use tua_core::Diagnostic;

pub(crate) const MANIFEST: &str = ".tua-build-manifest.json";
// A null fingerprint includes a user-provided runtime in packages without
// claiming ownership or making it eligible for stale-file deletion.
type Manifest = BTreeMap<String, Option<u64>>;

pub(crate) fn load(root: &Path) -> io::Result<Manifest> {
    let path = root.join(MANIFEST);
    tua_core::path_security::reject_symlink_path(root, &path).map_err(io::Error::other)?;
    let file = match fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Manifest::new()),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.take(4 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err(io::Error::other("build manifest too large"));
    }
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    for name in manifest.keys() {
        owned_path(root, name)?;
    }
    Ok(manifest)
}

fn owned_path(root: &Path, name: &str) -> io::Result<PathBuf> {
    if name.is_empty()
        || name.contains('\\')
        || name == MANIFEST
        || Path::new(name)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(io::Error::other("unsafe build manifest path"));
    }
    let path = root.join(name);
    tua_core::path_security::reject_symlink_path(root, &path).map_err(io::Error::other)?;
    Ok(path)
}

fn fingerprint(path: &Path) -> io::Result<u64> {
    let mut file = fs::File::open(path)?;
    let mut hash = 0xcbf29ce484222325u64;
    let mut buffer = [0u8; 8192];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok(hash);
        }
        for byte in &buffer[..read] {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
        }
    }
}

pub(super) fn reconcile(
    root: &Path,
    outputs: &BTreeSet<PathBuf>,
    existing_runtimes: &BTreeSet<PathBuf>,
) -> Result<(), Diagnostic> {
    let run = || -> io::Result<()> {
        let previous = load(root)?;
        fs::create_dir_all(root)?;
        let mut current = Manifest::new();
        for path in outputs {
            let name = path
                .strip_prefix(root)
                .map_err(io::Error::other)?
                .to_string_lossy()
                .replace('\\', "/");
            let path = owned_path(root, &name)?;
            let actual = fingerprint(&path)?;
            let owned =
                !existing_runtimes.contains(&path) || previous.get(&name) == Some(&Some(actual));
            current.insert(name, owned.then_some(actual));
        }
        // Validate the entire previous manifest before deleting any files.
        for (name, expected) in previous {
            let Some(expected) = expected else { continue };
            if current.contains_key(&name) {
                continue;
            }
            let path = owned_path(root, &name)?;
            match fingerprint(&path) {
                Ok(actual) if actual == expected => fs::remove_file(path)?,
                Ok(_) => {} // Preserve outputs modified by the user.
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        let path = root.join(MANIFEST);
        OutputGuard::for_root(root)
            .check(&path)
            .map_err(|error| io::Error::other(error.message))?;
        crate::atomic_fs::write(
            path,
            serde_json::to_vec(&current).map_err(io::Error::other)?,
        )
    };
    run().map_err(|error| io_diagnostic(&root.join(MANIFEST), error))
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_runtime_is_packaged_but_never_deleted_as_owned_output() {
        let directory = std::env::temp_dir().join(format!(
            "tua-ownership-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        let root = directory.as_path();
        let runtime = root.join("custom.lua");
        fs::write(&runtime, "return {}").unwrap();
        let paths = BTreeSet::from([runtime.clone()]);
        reconcile(root, &paths, &paths).unwrap();
        assert_eq!(load(root).unwrap().get("custom.lua"), Some(&None));
        reconcile(root, &BTreeSet::new(), &BTreeSet::new()).unwrap();
        assert!(runtime.is_file());
        fs::remove_dir_all(directory).unwrap();
    }
}

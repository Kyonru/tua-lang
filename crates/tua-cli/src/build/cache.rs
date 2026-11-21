//! A conservative persistent fast path for successful, unchanged builds.
//!
//! Snapshot content, not mtimes: output files can also participate in module and
//! asset resolution. Publish only when pre/post-build snapshots agree, so freshly
//! generated outputs are checked once and repaired output corruption cannot be
//! mistaken for the output of a successful build.
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tua_core::{Diagnostic, ProjectWorkspace};

use super::OutputGuard;

const VERSION: u32 = 1;
const MAX_RECORD_BYTES: u64 = 8 * 1024 * 1024;
const MAX_FILES: usize = 50_000;
const DIRECTORY: &str = ".tua/cache/build";

#[derive(Deserialize, Serialize)]
struct Record {
    version: u32,
    identity: String,
    files: BTreeMap<PathBuf, String>,
    diagnostics: String,
    emitted: usize,
}

pub(super) struct BuildCache {
    workspace: ProjectWorkspace,
    path: PathBuf,
    record: Record,
}

impl BuildCache {
    pub(super) fn prepare(
        workspace: &ProjectWorkspace,
        pipeline_diagnostics: &[Diagnostic],
    ) -> io::Result<Self> {
        Self::try_prepare(workspace, pipeline_diagnostics)
    }

    fn try_prepare(
        workspace: &ProjectWorkspace,
        pipeline_diagnostics: &[Diagnostic],
    ) -> io::Result<Self> {
        let guard = OutputGuard::new(workspace).map_err(|e| io::Error::other(e.message))?;
        // Never place a cache inside a source/output tree that overlaps the
        // project root or the cache itself. In-place builds retain their path.
        if guard.cleanup_root.is_none()
            || workspace
                .root()
                .starts_with(workspace.primary_output_root())
            || workspace
                .source_roots()
                .iter()
                .any(|source| workspace.root().starts_with(source))
        {
            return Err(io::Error::other("overlapping build layout"));
        }
        let path = workspace.root().join(DIRECTORY).join("last-success.json");
        guard
            .check(&path)
            .map_err(|e| io::Error::other(e.message))?;
        if workspace.source_roots().iter().any(|source| {
            source.starts_with(workspace.root().join(DIRECTORY)) || path.starts_with(source)
        }) || path.starts_with(workspace.primary_output_root())
        {
            return Err(io::Error::other("cache overlaps build inputs or outputs"));
        }
        // Create only cache directories before snapshotting, so their presence
        // cannot invalidate the successful build we are about to record.
        fs::create_dir_all(path.parent().unwrap())?;
        guard
            .check(&path)
            .map_err(|e| io::Error::other(e.message))?;
        let mut identity = Sha256::new();
        // Hash the executable itself: local development binaries can have the
        // same package version and still contain different compiler/runtimes.
        identity.update(fingerprint(&std::env::current_exe()?)?);
        identity.update(format!("{workspace:?}\n{pipeline_diagnostics:?}"));
        let record = Record {
            version: VERSION,
            identity: format!("{:x}", identity.finalize()),
            files: snapshot(workspace)?,
            diagnostics: String::new(),
            emitted: 0,
        };
        Ok(Self {
            workspace: workspace.clone(),
            path,
            record,
        })
    }

    pub(super) fn hit(&self) -> Result<String, String> {
        let file = fs::File::open(&self.path).map_err(|e| format!("record unavailable: {e}"))?;
        let mut bytes = Vec::new();
        file.take(MAX_RECORD_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("record unreadable: {e}"))?;
        if bytes.len() as u64 > MAX_RECORD_BYTES {
            return Err("record size limit".into());
        }
        let saved: Record =
            serde_json::from_slice(&bytes).map_err(|_| "invalid cache record".to_owned())?;
        if saved.version != VERSION {
            return Err("cache format changed".into());
        }
        if saved.identity != self.record.identity {
            return Err("compiler, target, configuration or pipeline identity changed".into());
        }
        let changed = saved
            .files
            .keys()
            .chain(self.record.files.keys())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .filter(|path| saved.files.get(*path) != self.record.files.get(*path))
            .map(|path| {
                let kind = if !saved.files.contains_key(path) {
                    "added"
                } else if !self.record.files.contains_key(path) {
                    "removed"
                } else {
                    "changed"
                };
                format!(
                    "{kind} {:?}",
                    path.strip_prefix(self.workspace.root()).unwrap_or(path)
                )
            })
            .collect::<Vec<_>>();
        if !changed.is_empty() {
            return Err(format!(
                "{} file fingerprint(s): {}{}",
                changed.len(),
                changed
                    .iter()
                    .take(8)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", "),
                if changed.len() > 8 { ", ..." } else { "" }
            ));
        }
        Ok(format!(
            "{}up to date ({} cached file(s))",
            saved.diagnostics, saved.emitted
        ))
    }

    pub(super) fn save(mut self, emitted: usize, diagnostics: String) -> String {
        match self.try_save(emitted, diagnostics) {
            Ok(true) => "saved".into(),
            Ok(false) => "not saved: inputs/outputs changed during build".into(),
            Err(error) => format!("not saved: {error}"),
        }
    }

    fn try_save(&mut self, emitted: usize, diagnostics: String) -> io::Result<bool> {
        let after = snapshot(&self.workspace)?;
        // The saved snapshot must describe both the inputs checked and the
        // successful outputs. Saving pre-build corruption would accept that
        // same corruption as a hit if a user later reintroduced it.
        if after != self.record.files {
            return Ok(false);
        }
        self.record.emitted = emitted;
        self.record.diagnostics = diagnostics;
        let bytes = serde_json::to_vec(&self.record).map_err(io::Error::other)?;
        if bytes.len() as u64 > MAX_RECORD_BYTES {
            return Ok(false);
        }
        let guard = OutputGuard::for_root(self.workspace.root());
        guard
            .check(&self.path)
            .map_err(|e| io::Error::other(e.message))?;
        fs::create_dir_all(self.path.parent().unwrap())?;
        guard
            .check(&self.path)
            .map_err(|e| io::Error::other(e.message))?;
        crate::atomic_fs::write(&self.path, bytes)?;
        Ok(true)
    }
}

fn snapshot(workspace: &ProjectWorkspace) -> io::Result<BTreeMap<PathBuf, String>> {
    let mut files = BTreeMap::new();
    let ignored = [
        workspace.root().join(DIRECTORY).join("last-success.json"),
        // Live editor ranking statistics are not compiler inputs. They may be
        // rewritten while a build runs; do not invalidate builds on typing.
        workspace.root().join(".tua/completion-stats.json"),
    ];
    // Besides production source, project-relative assets, declaration libraries,
    // test-root imports, and output-root fallback modules affect diagnostics.
    let mut roots = vec![workspace.root().to_path_buf()];
    roots.extend(workspace.source_roots().iter().cloned());
    roots.extend(workspace.output_roots().iter().cloned());
    roots.extend(
        workspace
            .type_libraries()
            .iter()
            .map(|lib| lib.root().to_path_buf()),
    );
    roots.sort();
    roots.dedup();
    let mut scanned: Vec<PathBuf> = Vec::new();
    for root in roots {
        if scanned.iter().any(|parent| root.starts_with(parent)) {
            continue;
        }
        if root.exists() {
            walk(&root, &ignored, &mut files)?;
        } else {
            files.insert(root.clone(), "missing directory".into());
        }
        scanned.push(root);
    }
    Ok(files)
}

fn walk(path: &Path, ignored: &[PathBuf], files: &mut BTreeMap<PathBuf, String>) -> io::Result<()> {
    if ignored.iter().any(|ignored| path == ignored) {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        // The normal build owns symlink diagnostics. A cache must not bypass
        // them or silently omit a module reachable through a link.
        return Err(io::Error::other("symlink in build snapshot"));
    }
    if metadata.is_dir() {
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| matches!(name, ".git" | ".hg" | ".svn"))
        {
            return Ok(());
        }
        if files.len() >= MAX_FILES {
            return Err(io::Error::other("build snapshot too large"));
        }
        files.insert(
            path.to_path_buf(),
            format!("directory:{:?}", metadata.permissions()),
        );
        for entry in fs::read_dir(path)? {
            walk(&entry?.path(), ignored, files)?;
        }
    } else if metadata.is_file() {
        if files.len() >= MAX_FILES {
            return Err(io::Error::other("build snapshot too large"));
        }
        files.insert(
            path.to_path_buf(),
            format!("{}:{:?}", fingerprint(path)?, metadata.permissions()),
        );
    } else {
        return Err(io::Error::other("unsupported build input kind"));
    }
    Ok(())
}

fn fingerprint(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            return Ok(format!("{:x}", hash.finalize()));
        }
        hash.update(&buffer[..count]);
    }
}

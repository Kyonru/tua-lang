use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VfsSourceKind {
    Tua,
    Lua,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ModulePathSeparator {
    #[default]
    Dot,
    Slash,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VfsOptions {
    pub source_roots: Vec<PathBuf>,
    pub output_roots: Vec<PathBuf>,
    pub ignored_paths: Vec<String>,
}

#[derive(Debug)]
pub struct VirtualFileSystem {
    options: VfsOptions,
    disk_files: BTreeMap<PathBuf, VfsFileSource>,
    open_files: BTreeMap<PathBuf, VfsFileSource>,
    effective_versions: Arc<VfsSourceVersionIndex>,
    version_allocator: Arc<AtomicU64>,
    lineage: VfsLineageId,
}

impl Clone for VirtualFileSystem {
    fn clone(&self) -> Self {
        Self {
            options: self.options.clone(),
            disk_files: self.disk_files.clone(),
            open_files: self.open_files.clone(),
            effective_versions: Arc::new(VfsSourceVersionIndex::from_shared(
                self.effective_versions.share(),
            )),
            version_allocator: Arc::clone(&self.version_allocator),
            lineage: self.lineage,
        }
    }
}

#[derive(Clone, Debug)]
pub struct VfsSnapshot {
    options: VfsOptions,
    files: BTreeMap<PathBuf, VfsFile>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct VfsFileId(PathBuf);

#[derive(Clone, Debug)]
pub struct VfsFile {
    id: VfsFileId,
    path: PathBuf,
    source: String,
    kind: VfsSourceKind,
    version: VfsSourceVersion,
    open: bool,
}

/// A monotonic identity for one path's effective source and kind inside a VFS
/// lineage. Versions from independent VFS instances are not comparable.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VfsSourceVersion {
    lineage: VfsLineageId,
    revision: u64,
}

/// Opaque identity shared only by clones of one VFS lineage.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VfsLineageId(u64);

/// Live validity registry for effective VFS sources.
///
/// Readers take only short per-path read locks. The epoch makes a multi-path
/// query fail closed if a writer changes the registry between any two reads,
/// without holding a read guard across the query itself.
///
/// The map is copy-on-write: cloning a VFS for a worker job shares it, and
/// only the first mutation while a clone still holds it copies the map.
#[derive(Debug, Default)]
pub(crate) struct VfsSourceVersionIndex {
    versions: RwLock<Arc<BTreeMap<PathBuf, VfsSourceVersion>>>,
    epoch: AtomicU64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct VfsSourceVersionView<'a> {
    index: &'a VfsSourceVersionIndex,
    epoch: u64,
}

impl VfsSourceVersionIndex {
    fn from_shared(versions: Arc<BTreeMap<PathBuf, VfsSourceVersion>>) -> Self {
        Self {
            versions: RwLock::new(versions),
            epoch: AtomicU64::new(0),
        }
    }

    pub(crate) fn begin_read(&self) -> Option<VfsSourceVersionView<'_>> {
        let epoch = self.epoch.load(Ordering::Acquire);
        epoch
            .is_multiple_of(2)
            .then_some(VfsSourceVersionView { index: self, epoch })
    }

    fn get_blocking(&self, path: &Path) -> Option<VfsSourceVersion> {
        self.versions
            .read()
            .expect("VFS version index poisoned")
            .get(path)
            .copied()
    }

    fn share(&self) -> Arc<BTreeMap<PathBuf, VfsSourceVersion>> {
        Arc::clone(&self.versions.read().expect("VFS version index poisoned"))
    }

    fn mutate<R>(&self, mutation: impl FnOnce(&mut BTreeMap<PathBuf, VfsSourceVersion>) -> R) -> R {
        self.epoch.fetch_add(1, Ordering::AcqRel);
        let result = mutation(Arc::make_mut(
            &mut self.versions.write().expect("VFS version index poisoned"),
        ));
        self.epoch.fetch_add(1, Ordering::Release);
        result
    }

    fn insert(&self, path: PathBuf, version: VfsSourceVersion) {
        self.mutate(|versions| {
            versions.insert(path, version);
        });
    }

    fn remove(&self, path: &Path) {
        self.mutate(|versions| {
            versions.remove(path);
        });
    }

    fn retain(&self, mut keep: impl FnMut(&PathBuf, &mut VfsSourceVersion) -> bool) {
        self.mutate(|versions| versions.retain(|path, version| keep(path, version)));
    }
}

impl VfsSourceVersionView<'_> {
    pub(crate) fn get(&self, path: &Path) -> Option<VfsSourceVersion> {
        if self.index.epoch.load(Ordering::Acquire) != self.epoch {
            return None;
        }
        let versions = self.index.versions.try_read().ok()?;
        let version = versions.get(path).copied();
        drop(versions);
        (self.index.epoch.load(Ordering::Acquire) == self.epoch)
            .then_some(version)
            .flatten()
    }

    pub(crate) fn is_current(self) -> bool {
        self.index.epoch.load(Ordering::Acquire) == self.epoch
    }
}

static NEXT_VFS_LINEAGE: AtomicU64 = AtomicU64::new(1);

impl Default for VirtualFileSystem {
    fn default() -> Self {
        #[allow(
            deprecated,
            reason = "AtomicU64::try_update is unavailable on supported pre-1.99 Rust toolchains"
        )]
        let lineage = NEXT_VFS_LINEAGE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .expect("VFS lineage allocator exhausted");
        Self {
            options: VfsOptions::default(),
            disk_files: BTreeMap::new(),
            open_files: BTreeMap::new(),
            effective_versions: Arc::new(VfsSourceVersionIndex::default()),
            version_allocator: Arc::new(AtomicU64::new(0)),
            lineage: VfsLineageId(lineage),
        }
    }
}

impl VfsSourceVersion {
    pub fn get(self) -> u64 {
        self.revision
    }

    pub fn lineage(self) -> VfsLineageId {
        self.lineage
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VfsFileSource {
    source: String,
    kind: VfsSourceKind,
}

impl VirtualFileSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn options(&self) -> &VfsOptions {
        &self.options
    }

    pub fn lineage(&self) -> VfsLineageId {
        self.lineage
    }

    pub fn set_source_roots(&mut self, roots: impl IntoIterator<Item = PathBuf>) {
        self.options.source_roots = roots.into_iter().map(normalize_path).collect();
        self.retain_included_files();
    }

    pub fn set_output_roots(&mut self, roots: impl IntoIterator<Item = PathBuf>) {
        self.options.output_roots = roots.into_iter().map(normalize_path).collect();
        self.retain_included_files();
    }

    pub fn set_ignored_paths(&mut self, patterns: impl IntoIterator<Item = String>) {
        self.options.ignored_paths = patterns.into_iter().collect();
        self.retain_included_files();
    }

    pub fn set_disk_file(
        &mut self,
        path: impl AsRef<Path>,
        source: impl Into<String>,
        kind: VfsSourceKind,
    ) -> Option<VfsFileId> {
        let path = normalize_path(path);
        let source = VfsFileSource {
            source: source.into(),
            kind,
        };

        if self.is_excluded(&path) {
            self.disk_files.remove(&path);
            self.open_files.remove(&path);
            self.effective_versions.remove(&path);
            return None;
        }

        let effective_changed =
            !self.open_files.contains_key(&path) && self.disk_files.get(&path) != Some(&source);
        self.disk_files.insert(path.clone(), source);
        if effective_changed {
            self.assign_effective_version(&path);
        }

        Some(VfsFileId(path))
    }

    pub fn disk_file_matches(
        &self,
        path: impl AsRef<Path>,
        source: &str,
        kind: VfsSourceKind,
    ) -> bool {
        let path = normalize_path(path);

        !self.is_excluded(&path)
            && self
                .disk_files
                .get(&path)
                .is_some_and(|file| file.source == source && file.kind == kind)
    }

    pub fn open_file(
        &mut self,
        path: impl AsRef<Path>,
        source: impl Into<String>,
        kind: VfsSourceKind,
    ) -> Option<VfsFileId> {
        let path = normalize_path(path);
        let source = VfsFileSource {
            source: source.into(),
            kind,
        };

        if self.is_excluded(&path) {
            self.open_files.remove(&path);
            if !self.disk_files.contains_key(&path) {
                self.effective_versions.remove(&path);
            }
            return None;
        }

        let effective_changed = self.effective_source(&path) != Some(&source);
        self.open_files.insert(path.clone(), source);
        if effective_changed {
            self.assign_effective_version(&path);
        }

        Some(VfsFileId(path))
    }

    pub fn open_file_matches(
        &self,
        path: impl AsRef<Path>,
        source: &str,
        kind: VfsSourceKind,
    ) -> bool {
        let path = normalize_path(path);

        !self.is_excluded(&path)
            && self
                .open_files
                .get(&path)
                .is_some_and(|file| file.source == source && file.kind == kind)
    }

    pub fn close_file(&mut self, path: impl AsRef<Path>) {
        let path = normalize_path(path);
        let effective_changed = self
            .open_files
            .get(&path)
            .is_some_and(|open| self.disk_files.get(&path).is_none_or(|disk| disk != open));
        self.open_files.remove(&path);
        if effective_changed {
            if self.disk_files.contains_key(&path) {
                self.assign_effective_version(&path);
            } else {
                self.effective_versions.remove(&path);
            }
        }
    }

    pub fn remove_file(&mut self, path: impl AsRef<Path>) {
        let path = normalize_path(path);

        self.disk_files.remove(&path);
        self.open_files.remove(&path);
        self.effective_versions.remove(&path);
    }

    pub fn file_id(&self, path: impl AsRef<Path>) -> Option<VfsFileId> {
        let path = normalize_path(path);

        (self.disk_files.contains_key(&path) || self.open_files.contains_key(&path))
            .then_some(VfsFileId(path))
    }

    pub fn source(&self, path: impl AsRef<Path>) -> Option<&str> {
        let path = normalize_path(path);

        self.open_files
            .get(&path)
            .or_else(|| self.disk_files.get(&path))
            .map(|file| file.source.as_str())
    }

    pub(crate) fn is_open(&self, path: impl AsRef<Path>) -> bool {
        self.open_files.contains_key(&normalize_path(path))
    }

    pub fn kind(&self, path: impl AsRef<Path>) -> Option<VfsSourceKind> {
        let path = normalize_path(path);

        self.open_files
            .get(&path)
            .or_else(|| self.disk_files.get(&path))
            .map(|file| file.kind)
    }

    /// Version of the effective open-overlay-or-disk source and kind.
    pub fn source_version(&self, path: impl AsRef<Path>) -> Option<VfsSourceVersion> {
        self.effective_versions.get_blocking(&normalize_path(path))
    }

    pub(crate) fn source_versions(&self) -> Arc<VfsSourceVersionIndex> {
        Arc::clone(&self.effective_versions)
    }

    pub fn paths(&self) -> BTreeSet<PathBuf> {
        let mut paths = self
            .disk_files
            .keys()
            .filter(|path| !self.is_excluded(path))
            .cloned()
            .collect::<BTreeSet<_>>();

        paths.extend(
            self.open_files
                .keys()
                .filter(|path| !self.is_excluded(path))
                .cloned(),
        );

        paths
    }

    pub fn snapshot(&self) -> VfsSnapshot {
        let mut files = BTreeMap::new();

        for (path, source) in &self.disk_files {
            if self.is_excluded(path) || self.open_files.contains_key(path) {
                continue;
            }
            let version = self
                .effective_versions
                .get_blocking(path)
                .expect("visible VFS path should have an effective source version");
            files.insert(path.clone(), vfs_file(path, source, version, false));
        }

        for (path, source) in &self.open_files {
            if self.is_excluded(path) {
                continue;
            }
            let version = self
                .effective_versions
                .get_blocking(path)
                .expect("visible VFS path should have an effective source version");
            files.insert(path.clone(), vfs_file(path, source, version, true));
        }

        VfsSnapshot {
            options: self.options.clone(),
            files,
        }
    }

    fn retain_included_files(&mut self) {
        let options = self.options.clone();

        self.disk_files
            .retain(|path, _| !is_excluded_with_options(path, &options));
        self.open_files
            .retain(|path, _| !is_excluded_with_options(path, &options));
        let disk_files = &self.disk_files;
        let open_files = &self.open_files;
        self.effective_versions
            .retain(|path, _| disk_files.contains_key(path) || open_files.contains_key(path));
    }

    fn is_excluded(&self, path: &Path) -> bool {
        is_excluded_with_options(path, &self.options)
    }

    fn effective_source(&self, path: &Path) -> Option<&VfsFileSource> {
        self.open_files
            .get(path)
            .or_else(|| self.disk_files.get(path))
    }

    fn assign_effective_version(&mut self, path: &Path) {
        #[allow(
            deprecated,
            reason = "AtomicU64::try_update is unavailable on supported pre-1.99 Rust toolchains"
        )]
        let previous = self
            .version_allocator
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |version| {
                version.checked_add(1)
            })
            .expect("VFS source version allocator exhausted");
        self.effective_versions.insert(
            path.to_owned(),
            VfsSourceVersion {
                lineage: self.lineage,
                revision: previous + 1,
            },
        );
    }
}

impl VfsSnapshot {
    pub fn options(&self) -> &VfsOptions {
        &self.options
    }

    pub fn source_roots(&self) -> &[PathBuf] {
        &self.options.source_roots
    }

    pub fn files(&self) -> impl Iterator<Item = &VfsFile> {
        self.files.values()
    }

    pub fn file(&self, path: impl AsRef<Path>) -> Option<&VfsFile> {
        self.files.get(&normalize_path(path))
    }

    pub fn source(&self, path: impl AsRef<Path>) -> Option<&str> {
        self.file(path).map(VfsFile::source)
    }

    /// Frozen effective-source version captured by this snapshot.
    pub fn source_version(&self, path: impl AsRef<Path>) -> Option<VfsSourceVersion> {
        self.file(path).map(VfsFile::version)
    }

    pub fn is_open(&self, path: impl AsRef<Path>) -> bool {
        self.file(path).is_some_and(VfsFile::is_open)
    }

    pub fn file_id(&self, path: impl AsRef<Path>) -> Option<&VfsFileId> {
        self.file(path).map(VfsFile::id)
    }

    pub fn is_excluded(&self, path: impl AsRef<Path>) -> bool {
        is_excluded_with_options(&normalize_path(path), &self.options)
    }
}

impl VfsFile {
    pub fn id(&self) -> &VfsFileId {
        &self.id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn kind(&self) -> VfsSourceKind {
        self.kind
    }

    /// Frozen effective-source version captured for this file.
    pub fn version(&self) -> VfsSourceVersion {
        self.version
    }

    pub fn is_open(&self) -> bool {
        self.open
    }
}

impl VfsFileId {
    pub fn path(&self) -> &Path {
        &self.0
    }
}

pub fn normalize_path(path: impl AsRef<Path>) -> PathBuf {
    let raw = path.as_ref().to_string_lossy().replace('\\', "/");
    let path = Path::new(&raw);
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push("..");
                }
            }
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Normal(part) => normalized.push(part),
        }
    }

    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

pub fn file_uri_from_path(path: impl AsRef<Path>) -> String {
    let raw = path.as_ref().to_string_lossy().replace('\\', "/");

    if raw.starts_with("tua:/") {
        return raw;
    }

    let path = normalize_path(path);
    let encoded = percent_encode_path(&path);

    if encoded.starts_with("//") {
        format!("file:{encoded}")
    } else {
        format!("file://{encoded}")
    }
}

pub fn path_from_file_uri(uri: &str) -> PathBuf {
    let Some(raw_path) = uri.strip_prefix("file://") else {
        return normalize_path(uri);
    };
    let raw_path = raw_path.strip_prefix("localhost").unwrap_or(raw_path);
    let decoded = percent_decode(raw_path);

    #[cfg(windows)]
    let decoded = windows_path_text_from_file_uri(raw_path, decoded);

    normalize_path(decoded)
}

pub fn module_name_for_path(
    source_root: impl AsRef<Path>,
    path: impl AsRef<Path>,
    separator: ModulePathSeparator,
) -> Option<String> {
    let source_root = normalize_path(source_root);
    let path = normalize_path(path);
    let relative = path.strip_prefix(source_root).ok()?;
    let mut without_extension = relative.with_extension("");
    if without_extension
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".d"))
    {
        without_extension.set_extension("");
    }
    let mut parts = without_extension
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();

    if parts.last().is_some_and(|part| part == "init") {
        parts.pop();
    }

    if parts.is_empty() {
        return None;
    }

    Some(match separator {
        ModulePathSeparator::Dot => parts.join("."),
        ModulePathSeparator::Slash => parts.join("/"),
    })
}

pub(crate) fn module_path_without_runtime_suffix(path: &Path) -> PathBuf {
    let mut without_extension = path.with_extension("");
    if without_extension
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".d"))
    {
        without_extension.set_extension("");
    }

    if without_extension
        .file_name()
        .is_some_and(|name| name == "init")
    {
        return without_extension
            .parent()
            .map(normalize_path)
            .unwrap_or(without_extension);
    }

    without_extension
}

fn vfs_file(path: &Path, source: &VfsFileSource, version: VfsSourceVersion, open: bool) -> VfsFile {
    let path = normalize_path(path);

    VfsFile {
        id: VfsFileId(path.clone()),
        path,
        source: source.source.clone(),
        kind: source.kind,
        version,
        open,
    }
}

fn is_excluded_with_options(path: &Path, options: &VfsOptions) -> bool {
    options
        .output_roots
        .iter()
        .any(|root| path.starts_with(root))
        || options
            .ignored_paths
            .iter()
            .any(|pattern| path_matches_any_root(path, pattern, &options.source_roots))
}

fn path_matches_any_root(path: &Path, pattern: &str, roots: &[PathBuf]) -> bool {
    if path_matches(path, pattern) {
        return true;
    }

    roots.iter().any(|root| {
        path.strip_prefix(root)
            .ok()
            .is_some_and(|relative| path_matches(relative, pattern))
    })
}

fn path_matches(path: &Path, pattern: &str) -> bool {
    let path = normalized_path_text(path);
    let pattern = pattern.replace('\\', "/");

    if let Some(extension) = pattern.strip_prefix("*.") {
        return path
            .rsplit('/')
            .next()
            .is_some_and(|name| name.ends_with(&format!(".{extension}")));
    }

    if let Some(suffix) = pattern.strip_prefix("**/*") {
        return path.ends_with(suffix);
    }

    if let Some(prefix) = pattern.strip_suffix("/**") {
        return path == prefix || path.starts_with(&format!("{prefix}/"));
    }

    path == pattern || path.ends_with(&format!("/{pattern}"))
}

fn normalized_path_text(path: &Path) -> String {
    normalize_path(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn percent_encode_path(path: &Path) -> String {
    let mut path = path.to_string_lossy().replace('\\', "/");

    if has_windows_drive_prefix(&path) && !path.starts_with('/') {
        path.insert(0, '/');
    }

    let mut encoded = String::new();

    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }

    encoded
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();

    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

#[cfg(any(windows, test))]
fn windows_path_text_from_file_uri(raw_path: &str, decoded: String) -> String {
    if decoded
        .strip_prefix('/')
        .is_some_and(has_windows_drive_prefix)
    {
        return decoded[1..].to_owned();
    }
    if has_windows_drive_prefix(&decoded) || raw_path.starts_with('/') {
        return decoded;
    }

    format!("//{decoded}")
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = &input[index + 1..index + 3];

            if let Ok(value) = u8::from_str_radix(hex, 16) {
                output.push(value);
                index += 3;
                continue;
            }
        }

        output.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&output).into_owned()
}

#[cfg(test)]
mod tests;

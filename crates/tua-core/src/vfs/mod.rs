use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

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

#[derive(Clone, Debug, Default)]
pub struct VirtualFileSystem {
    options: VfsOptions,
    disk_files: BTreeMap<PathBuf, VfsFileSource>,
    open_files: BTreeMap<PathBuf, VfsFileSource>,
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
    open: bool,
}

#[derive(Clone, Debug)]
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

        if self.is_excluded(&path) {
            self.disk_files.remove(&path);
            self.open_files.remove(&path);
            return None;
        }

        self.disk_files.insert(
            path.clone(),
            VfsFileSource {
                source: source.into(),
                kind,
            },
        );

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

        if self.is_excluded(&path) {
            self.open_files.remove(&path);
            return None;
        }

        self.open_files.insert(
            path.clone(),
            VfsFileSource {
                source: source.into(),
                kind,
            },
        );

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
        self.open_files.remove(&normalize_path(path));
    }

    pub fn remove_file(&mut self, path: impl AsRef<Path>) {
        let path = normalize_path(path);

        self.disk_files.remove(&path);
        self.open_files.remove(&path);
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

    pub fn kind(&self, path: impl AsRef<Path>) -> Option<VfsSourceKind> {
        let path = normalize_path(path);

        self.open_files
            .get(&path)
            .or_else(|| self.disk_files.get(&path))
            .map(|file| file.kind)
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
            if !self.is_excluded(path) {
                files.insert(path.clone(), vfs_file(path, source, false));
            }
        }

        for (path, source) in &self.open_files {
            if !self.is_excluded(path) {
                files.insert(path.clone(), vfs_file(path, source, true));
            }
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
    }

    fn is_excluded(&self, path: &Path) -> bool {
        is_excluded_with_options(path, &self.options)
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
    let without_extension = relative.with_extension("");
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

fn vfs_file(path: &Path, source: &VfsFileSource, open: bool) -> VfsFile {
    let path = normalize_path(path);

    VfsFile {
        id: VfsFileId(path.clone()),
        path,
        source: source.source.clone(),
        kind: source.kind,
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

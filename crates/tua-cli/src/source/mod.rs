use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use tua_core::{Diagnostic, DiagnosticCode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceKind {
    Tua,
    Lua,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceFile {
    pub(crate) path: PathBuf,
    pub(crate) kind: SourceKind,
}

pub(crate) fn discover_source_files(
    source_dir: &Path,
    extension: &str,
) -> Result<Vec<PathBuf>, Diagnostic> {
    let mut files = Vec::new();

    discover_files_into(
        source_dir,
        &|path| has_extension(path, extension).then(|| path.to_path_buf()),
        &mut files,
    )?;
    files.sort();

    Ok(files)
}

pub(crate) fn discover_build_files(
    source_dir: &Path,
    extension: &str,
) -> Result<Vec<SourceFile>, Diagnostic> {
    let mut files = Vec::new();

    discover_files_into(
        source_dir,
        &|path| {
            SourceKind::for_build_path(path, extension).map(|kind| SourceFile {
                path: path.to_path_buf(),
                kind,
            })
        },
        &mut files,
    )?;
    files.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(files)
}

pub(crate) fn read_source_file(path: &Path) -> Result<String, Diagnostic> {
    fs::read_to_string(path).map_err(|source| io_diagnostic(path, source))
}

fn discover_files_into<T>(
    dir: &Path,
    classify_file: &impl Fn(&Path) -> Option<T>,
    files: &mut Vec<T>,
) -> Result<(), Diagnostic> {
    let entries = fs::read_dir(dir).map_err(|source| io_diagnostic(dir, source))?;

    for entry in entries {
        let entry = entry.map_err(|source| io_diagnostic(dir, source))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| io_diagnostic(&path, source))?;

        if file_type.is_dir() {
            discover_files_into(&path, classify_file, files)?;
        } else if file_type.is_file() {
            if let Some(file) = classify_file(&path) {
                files.push(file);
            }
        }
    }

    Ok(())
}

impl SourceKind {
    pub(crate) fn for_build_path(path: &Path, extension: &str) -> Option<Self> {
        if has_extension(path, extension) {
            Some(Self::Tua)
        } else if has_extension(path, "lua") {
            Some(Self::Lua)
        } else {
            None
        }
    }
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value == extension)
}

pub(crate) fn io_diagnostic(path: &Path, source: io::Error) -> Diagnostic {
    Diagnostic::error(path, 1, 1, source.to_string()).with_code(DiagnosticCode::EMIT_IO)
}

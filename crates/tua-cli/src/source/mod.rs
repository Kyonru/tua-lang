use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use tua_core::{AnalysisHost, AnalysisSourceKind, Diagnostic, DiagnosticCode, ProjectWorkspace};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceKind {
    Tua,
    Asset,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceFile {
    pub(crate) path: PathBuf,
    pub(crate) kind: SourceKind,
}

pub(crate) fn discover_build_files(
    source_dir: &Path,
    extension: &str,
    excluded_dir: Option<&Path>,
) -> Result<Vec<SourceFile>, Diagnostic> {
    discover_build_files_with_exclusions(
        source_dir,
        extension,
        &excluded_dir.into_iter().collect::<Vec<_>>(),
    )
}

pub(crate) fn discover_build_files_with_exclusions(
    source_dir: &Path,
    extension: &str,
    excluded_dirs: &[&Path],
) -> Result<Vec<SourceFile>, Diagnostic> {
    let mut files = Vec::new();

    discover_files_into(
        source_dir,
        excluded_dirs,
        &|path| {
            Some(SourceFile {
                path: path.to_path_buf(),
                kind: SourceKind::for_path(path, extension),
            })
        },
        &mut files,
    )?;
    files.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(files)
}

/// Discover project inputs with the same test-root policy for check/build/watch.
pub(crate) fn discover_project_files(
    workspace: &ProjectWorkspace,
    include_tests: bool,
    excluded: &[&Path],
) -> Result<Vec<SourceFile>, Diagnostic> {
    let test_root = workspace.test_source_root();
    let mut excluded = excluded.to_vec();
    if !include_tests {
        if let Some(root) = &test_root {
            excluded.push(root);
        }
    }
    let mut files = discover_build_files_with_exclusions(
        workspace.primary_source_root(),
        &workspace.config().extension,
        &excluded,
    )?;
    if include_tests {
        if let Some(root) = &test_root {
            if !root.starts_with(workspace.primary_source_root()) {
                files.extend(discover_build_files_with_exclusions(
                    root,
                    &workspace.config().extension,
                    &excluded,
                )?);
            }
        }
    }
    files.retain(|file| {
        !workspace.is_type_library_path(&file.path)
            && (include_tests || workspace.is_production_path(&file.path))
    });
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files.dedup_by(|a, b| a.path == b.path);
    Ok(files)
}

pub(crate) fn read_source_file(path: &Path) -> Result<String, Diagnostic> {
    tua_core::input_limits::read_source(path).map_err(|source| io_diagnostic(path, source))
}

pub(crate) fn load_type_libraries(
    host: &mut AnalysisHost,
    workspace: &ProjectWorkspace,
) -> Vec<Diagnostic> {
    host.set_type_libraries(workspace.type_libraries().iter().cloned());

    let mut diagnostics = Vec::new();

    for library in workspace.type_libraries() {
        let paths = match discover_lua_files(library.root()) {
            Ok(paths) => paths,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };

        for path in paths {
            match read_source_file(&path) {
                Ok(source) => host.set_file_with_kind(path, source, AnalysisSourceKind::Lua),
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }
    }

    diagnostics
}

pub(crate) fn discover_lua_files(root: &Path) -> Result<Vec<PathBuf>, Diagnostic> {
    let mut paths = Vec::new();

    discover_files_into(
        root,
        &[],
        &|path| has_extension(path, "lua").then(|| path.to_path_buf()),
        &mut paths,
    )?;
    paths.sort();

    Ok(paths)
}

fn discover_files_into<T>(
    dir: &Path,
    excluded_dirs: &[&Path],
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
            if is_vcs_metadata_dir(&path) {
                continue;
            }

            if excluded_dirs
                .iter()
                .any(|excluded| paths_same_location(&path, excluded))
            {
                continue;
            }

            discover_files_into(&path, excluded_dirs, classify_file, files)?;
        } else if file_type.is_file() {
            if let Some(file) = classify_file(&path) {
                files.push(file);
            }
        }
    }

    Ok(())
}

fn is_vcs_metadata_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| matches!(value, ".git" | ".hg" | ".svn"))
}

impl SourceKind {
    pub(crate) fn for_path(path: &Path, extension: &str) -> Self {
        if has_extension(path, extension) {
            Self::Tua
        } else {
            Self::Asset
        }
    }
}

pub(crate) fn paths_same_location(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => normalize_path(left) == normalize_path(right),
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }

    normalized
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value == extension)
}

pub(crate) fn io_diagnostic(path: &Path, source: io::Error) -> Diagnostic {
    Diagnostic::error(path, 1, 1, source.to_string()).with_code(DiagnosticCode::EMIT_IO)
}

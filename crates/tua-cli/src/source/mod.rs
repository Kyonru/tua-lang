use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use tua_core::{Diagnostic, DiagnosticCode};

pub(crate) fn discover_source_files(
    source_dir: &Path,
    extension: &str,
) -> Result<Vec<PathBuf>, Diagnostic> {
    let mut files = Vec::new();

    discover_source_files_into(source_dir, extension, &mut files)?;
    files.sort();

    Ok(files)
}

pub(crate) fn read_source_file(path: &Path) -> Result<String, Diagnostic> {
    fs::read_to_string(path).map_err(|source| io_diagnostic(path, source))
}

fn discover_source_files_into(
    dir: &Path,
    extension: &str,
    files: &mut Vec<PathBuf>,
) -> Result<(), Diagnostic> {
    let entries = fs::read_dir(dir).map_err(|source| io_diagnostic(dir, source))?;

    for entry in entries {
        let entry = entry.map_err(|source| io_diagnostic(dir, source))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| io_diagnostic(&path, source))?;

        if file_type.is_dir() {
            discover_source_files_into(&path, extension, files)?;
        } else if file_type.is_file() && has_extension(&path, extension) {
            files.push(path);
        }
    }

    Ok(())
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value == extension)
}

pub(crate) fn io_diagnostic(path: &Path, source: io::Error) -> Diagnostic {
    Diagnostic::error(path, 1, 1, source.to_string()).with_code(DiagnosticCode::EMIT_IO)
}

use std::collections::BTreeSet;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::diagnostics::sorted_diagnostics;
use crate::error::CliError;
use crate::source::{
    discover_project_files, io_diagnostic, read_source_file, SourceFile, SourceKind,
};
use tua_core::{
    format_tua_source, path_security::reject_symlink_path, LoadedConfig, ProjectWorkspace,
};

pub(crate) fn run_fmt(
    cwd: impl AsRef<Path>,
    loaded: LoadedConfig,
    check: bool,
    paths: &[PathBuf],
    stdin_filepath: Option<&Path>,
) -> Result<String, CliError> {
    let cwd = cwd.as_ref();
    let workspace = ProjectWorkspace::from_loaded(cwd, loaded);
    let options = workspace.config().format.options();
    if let Some(path) = stdin_filepath {
        if !paths.is_empty() {
            return Err(CliError::usage(
                "stdin formatting cannot be combined with file paths",
            ));
        }
        let path = explicit_path(cwd, &workspace, path)?;
        let mut source = String::new();
        let limit = tua_core::input_limits::MAX_SOURCE_BYTES;
        io::stdin()
            .take(limit as u64 + 1)
            .read_to_string(&mut source)
            .map_err(|error| CliError::usage(format!("cannot read formatter stdin: {error}")))?;
        if source.len() > limit {
            return Err(CliError::usage("formatter stdin exceeds source byte limit"));
        }
        let formatted = format_tua_source(&path, &source, &options).map_err(|errors| {
            CliError::from_diagnostics("format failed", sorted_diagnostics(errors))
        })?;
        if check {
            if formatted != source {
                return Err(CliError::usage(format_check_message(cwd, &[path])));
            }
            return Ok(String::new());
        }
        return Ok(formatted);
    }
    let source_files = if paths.is_empty() {
        discover_project_files(&workspace, true, &[]).map_err(|error| {
            CliError::from_diagnostics("failed to discover source files", vec![error])
        })?
    } else {
        let mut selected = BTreeSet::new();
        for path in paths {
            let path = explicit_path(cwd, &workspace, path)?;
            if !path.is_file() {
                return Err(CliError::usage(format!(
                    "formatter path is not a file: {}",
                    path.display()
                )));
            }
            selected.insert(path);
        }
        selected
            .into_iter()
            .map(|path| SourceFile {
                path,
                kind: SourceKind::Tua,
            })
            .collect()
    };
    let mut diagnostics = Vec::new();
    let mut changed = Vec::new();
    let mut formatted_count = 0usize;

    for source_file in source_files {
        if source_file.kind != SourceKind::Tua {
            continue;
        }

        formatted_count += 1;
        let source = match read_source_file(&source_file.path) {
            Ok(source) => source,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };
        let formatted = match format_tua_source(&source_file.path, &source, &options) {
            Ok(formatted) => formatted,
            Err(mut source_diagnostics) => {
                diagnostics.append(&mut source_diagnostics);
                continue;
            }
        };

        if formatted == source {
            continue;
        }

        changed.push(source_file.path.clone());
        if !check {
            if let Err(message) = reject_symlink_path(workspace.root(), &source_file.path) {
                diagnostics.push(io_diagnostic(
                    &source_file.path,
                    io::Error::new(io::ErrorKind::InvalidInput, message),
                ));
                continue;
            }
            if let Err(source) = crate::atomic_fs::write(&source_file.path, formatted) {
                diagnostics.push(io_diagnostic(&source_file.path, source));
            }
        }
    }

    if !diagnostics.is_empty() {
        return Err(CliError::from_diagnostics(
            "format failed",
            sorted_diagnostics(diagnostics),
        ));
    }

    if check && !changed.is_empty() {
        return Err(CliError::usage(format_check_message(cwd, &changed)));
    }

    if check {
        Ok(format!("checked formatting for {formatted_count} file(s)"))
    } else {
        Ok(format!("formatted {formatted_count} file(s)"))
    }
}

fn format_check_message(cwd: &Path, changed: &[PathBuf]) -> String {
    let mut lines = vec!["format check failed; run `tua fmt`".to_owned()];

    for path in changed {
        let display = path
            .strip_prefix(cwd)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        lines.push(format!("would reformat {display}"));
    }

    lines.join("\n")
}

// Explicit Lua harnesses use the same parser/layout and project settings as Tua.
// Validate the lexical path before reading: canonicalizing first would hide a symlink.
fn explicit_path(
    cwd: &Path,
    workspace: &ProjectWorkspace,
    path: &Path,
) -> Result<PathBuf, CliError> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    reject_symlink_path(workspace.root(), &path).map_err(CliError::usage)?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if extension != "lua" && extension != workspace.config().extension && extension != "tua" {
        return Err(CliError::usage(
            "formatter paths must name a .tua, .lua, or configured source file",
        ));
    }
    Ok(path)
}

use std::fs;
use std::path::{Path, PathBuf};

use crate::diagnostics::sorted_diagnostics;
use crate::error::CliError;
use crate::source::{discover_source_files, io_diagnostic, read_source_file};
use tua_core::{check_tua_source, emit_tua_source, Diagnostic, DiagnosticCategory, LoadedConfig};

#[derive(Debug, Default)]
pub(crate) struct BuildOutcome {
    pub(crate) emitted: usize,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

struct SourceUnit {
    source_path: PathBuf,
    output_path: PathBuf,
    source: String,
    emit_candidate: bool,
}

pub(crate) fn run_build(
    cwd: impl AsRef<Path>,
    loaded: LoadedConfig,
    emit_on_error: bool,
) -> Result<String, CliError> {
    let outcome = build_project(cwd, loaded, emit_on_error)?;

    if outcome.diagnostics.is_empty() {
        Ok(format!("built {} file(s)", outcome.emitted))
    } else {
        Err(CliError::from_diagnostics(
            "project build failed",
            sorted_diagnostics(outcome.diagnostics),
        ))
    }
}

pub(crate) fn build_project(
    cwd: impl AsRef<Path>,
    loaded: LoadedConfig,
    emit_on_error: bool,
) -> Result<BuildOutcome, CliError> {
    let cwd = cwd.as_ref();
    let source_dir = cwd.join(&loaded.config.source);
    let source_files =
        discover_source_files(&source_dir, &loaded.config.extension).map_err(|error| {
            CliError::from_diagnostics("failed to discover source files", vec![error])
        })?;

    Ok(build_project_files(
        cwd,
        loaded,
        source_files,
        emit_on_error,
    ))
}

pub(crate) fn build_changed_files(
    cwd: impl AsRef<Path>,
    loaded: LoadedConfig,
    source_files: Vec<PathBuf>,
) -> BuildOutcome {
    let cwd = cwd.as_ref();
    let source_dir = cwd.join(&loaded.config.source);
    let out_dir = cwd.join(&loaded.config.out);
    let mut outcome = BuildOutcome::default();

    for source_path in source_files {
        let output_path = output_path_for(&source_dir, &out_dir, &source_path);
        let source = match read_source_file(&source_path) {
            Ok(source) => source,
            Err(diagnostic) => {
                outcome.diagnostics.push(diagnostic);
                continue;
            }
        };

        if let Err(mut source_diagnostics) = check_tua_source(&source_path, source.clone()) {
            outcome.diagnostics.append(&mut source_diagnostics);
            continue;
        }

        let output = match emit_tua_source(&source_path, source) {
            Ok(output) => output,
            Err(mut source_diagnostics) => {
                outcome.diagnostics.append(&mut source_diagnostics);
                continue;
            }
        };

        match write_output(&output_path, output) {
            Ok(()) => outcome.emitted += 1,
            Err(diagnostic) => outcome.diagnostics.push(diagnostic),
        }
    }

    outcome
}

fn build_project_files(
    cwd: &Path,
    loaded: LoadedConfig,
    source_files: Vec<PathBuf>,
    emit_on_error: bool,
) -> BuildOutcome {
    let source_dir = cwd.join(&loaded.config.source);
    let out_dir = cwd.join(&loaded.config.out);
    let mut diagnostics = Vec::new();
    let mut units = Vec::new();

    for source_path in source_files {
        let output_path = output_path_for(&source_dir, &out_dir, &source_path);
        let source = match read_source_file(&source_path) {
            Ok(source) => source,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };

        units.push(SourceUnit {
            source_path,
            output_path,
            source,
            emit_candidate: true,
        });
    }

    for unit in &mut units {
        if let Err(mut source_diagnostics) =
            check_tua_source(&unit.source_path, unit.source.clone())
        {
            if source_diagnostics
                .iter()
                .any(|diagnostic| !can_emit_after_diagnostic(diagnostic))
            {
                unit.emit_candidate = false;
            }

            diagnostics.append(&mut source_diagnostics);
        }
    }

    if !diagnostics.is_empty() && !emit_on_error {
        return BuildOutcome {
            emitted: 0,
            diagnostics,
        };
    }

    let mut emitted = 0usize;

    for unit in units {
        if emit_on_error && !unit.emit_candidate {
            continue;
        }

        let output = match emit_tua_source(&unit.source_path, unit.source) {
            Ok(output) => output,
            Err(mut source_diagnostics) => {
                diagnostics.append(&mut source_diagnostics);
                continue;
            }
        };

        match write_output(&unit.output_path, output) {
            Ok(()) => emitted += 1,
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }

    BuildOutcome {
        emitted,
        diagnostics,
    }
}

pub(crate) fn output_path_for(source_dir: &Path, out_dir: &Path, source_path: &Path) -> PathBuf {
    let relative_path = source_path
        .strip_prefix(source_dir)
        .expect("discovered source file should be under source directory");

    out_dir.join(relative_path).with_extension("lua")
}

fn can_emit_after_diagnostic(diagnostic: &Diagnostic) -> bool {
    !matches!(
        diagnostic.code.map(|code| code.category()),
        Some(DiagnosticCategory::Syntax | DiagnosticCategory::LoweringEmission)
    )
}

fn write_output(output_path: &Path, output: String) -> Result<(), Diagnostic> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|source| io_diagnostic(parent, source))?;
    }

    fs::write(output_path, output).map_err(|source| io_diagnostic(output_path, source))
}

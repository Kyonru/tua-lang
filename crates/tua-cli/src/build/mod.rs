use std::fs;
use std::path::{Path, PathBuf};

use crate::diagnostics::sorted_diagnostics;
use crate::error::CliError;
use crate::source::{
    discover_build_files, io_diagnostic, read_source_file, SourceFile, SourceKind,
};
use tua_core::{
    check_tua_source_with_diagnostics_config, emit_tua_source, Diagnostic, DiagnosticCategory,
    LoadedConfig,
};

#[derive(Debug, Default)]
pub(crate) struct BuildOutcome {
    pub(crate) emitted: usize,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

enum SourceUnit {
    Tua {
        source_path: PathBuf,
        output_path: PathBuf,
        source: String,
        emit_candidate: bool,
    },
    Lua {
        source_path: PathBuf,
        output_path: PathBuf,
    },
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
        discover_build_files(&source_dir, &loaded.config.extension).map_err(|error| {
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
    let diagnostics_config = loaded.config.diagnostics;
    let mut outcome = BuildOutcome::default();

    for source_path in source_files {
        let Some(source_kind) = SourceKind::for_build_path(&source_path, &loaded.config.extension)
        else {
            continue;
        };

        let output_path = output_path_for(&source_dir, &out_dir, &source_path);
        if source_kind == SourceKind::Lua {
            match copy_output(&source_path, &output_path) {
                Ok(()) => outcome.emitted += 1,
                Err(diagnostic) => outcome.diagnostics.push(diagnostic),
            }
            continue;
        }

        let source = match read_source_file(&source_path) {
            Ok(source) => source,
            Err(diagnostic) => {
                outcome.diagnostics.push(diagnostic);
                continue;
            }
        };

        if let Err(mut source_diagnostics) = check_tua_source_with_diagnostics_config(
            &source_path,
            source.clone(),
            &diagnostics_config,
        ) {
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
    source_files: Vec<SourceFile>,
    emit_on_error: bool,
) -> BuildOutcome {
    let source_dir = cwd.join(&loaded.config.source);
    let out_dir = cwd.join(&loaded.config.out);
    let diagnostics_config = loaded.config.diagnostics;
    let mut diagnostics = Vec::new();
    let mut units = Vec::new();

    for source_file in source_files {
        let source_path = source_file.path;
        let output_path = output_path_for(&source_dir, &out_dir, &source_path);

        if source_file.kind == SourceKind::Lua {
            units.push(SourceUnit::Lua {
                source_path,
                output_path,
            });
            continue;
        }

        let source = match read_source_file(&source_path) {
            Ok(source) => source,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };

        units.push(SourceUnit::Tua {
            source_path,
            output_path,
            source,
            emit_candidate: true,
        });
    }

    for unit in &mut units {
        let SourceUnit::Tua {
            source_path,
            source,
            emit_candidate,
            ..
        } = unit
        else {
            continue;
        };

        if let Err(mut source_diagnostics) = check_tua_source_with_diagnostics_config(
            source_path,
            source.clone(),
            &diagnostics_config,
        ) {
            if source_diagnostics
                .iter()
                .any(|diagnostic| !can_emit_after_diagnostic(diagnostic))
            {
                *emit_candidate = false;
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
        match unit {
            SourceUnit::Tua {
                source_path,
                output_path,
                source,
                emit_candidate,
            } => {
                if emit_on_error && !emit_candidate {
                    continue;
                }

                let output = match emit_tua_source(&source_path, source) {
                    Ok(output) => output,
                    Err(mut source_diagnostics) => {
                        diagnostics.append(&mut source_diagnostics);
                        continue;
                    }
                };

                match write_output(&output_path, output) {
                    Ok(()) => emitted += 1,
                    Err(diagnostic) => diagnostics.push(diagnostic),
                }
            }
            SourceUnit::Lua {
                source_path,
                output_path,
            } => match copy_output(&source_path, &output_path) {
                Ok(()) => emitted += 1,
                Err(diagnostic) => diagnostics.push(diagnostic),
            },
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

fn copy_output(source_path: &Path, output_path: &Path) -> Result<(), Diagnostic> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|source| io_diagnostic(parent, source))?;
    }

    fs::copy(source_path, output_path)
        .map(|_| ())
        .map_err(|source| io_diagnostic(output_path, source))
}

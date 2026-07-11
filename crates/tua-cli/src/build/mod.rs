use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::diagnostics::sorted_diagnostics;
use crate::error::CliError;
use crate::source::{
    discover_build_files, io_diagnostic, paths_same_location, read_source_file, SourceFile,
    SourceKind,
};
use tua_core::{
    brinevector_runtime_source, emit_tua_source_with_workspace, format_diagnostics_for_cli,
    AnalysisHost, AnalysisSourceKind, AnnotationMode, Diagnostic, DiagnosticCategory,
    DiagnosticCode, DiagnosticSeverity, LoadedConfig, ProjectWorkspace,
};

const BRINEVECTOR_RUNTIME_FILE: &str = "brinevector.lua";
const BRINEVECTOR_RUNTIME_REQUIRE: &str = "require(\"brinevector\")";

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
    Asset {
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

    if has_errors(&outcome.diagnostics) {
        Err(CliError::from_diagnostics(
            "project build failed",
            sorted_diagnostics(outcome.diagnostics),
        ))
    } else {
        Ok(format_success(
            "built",
            outcome.emitted,
            outcome.diagnostics,
        ))
    }
}

pub(crate) fn build_project(
    cwd: impl AsRef<Path>,
    loaded: LoadedConfig,
    emit_on_error: bool,
) -> Result<BuildOutcome, CliError> {
    let cwd = cwd.as_ref();
    let workspace = ProjectWorkspace::from_loaded(cwd, loaded.clone());
    let source_dir = workspace.primary_source_root().to_path_buf();
    let out_dir = workspace.primary_output_root().to_path_buf();
    let in_place = paths_same_location(&source_dir, &out_dir);
    let source_files = discover_build_files(
        &source_dir,
        &workspace.config().extension,
        (!in_place).then_some(&out_dir),
    )
    .map_err(|error| CliError::from_diagnostics("failed to discover source files", vec![error]))?;
    let annotation_mode = workspace.config().emit.annotation_mode;

    Ok(build_project_files(
        cwd,
        loaded,
        annotation_mode,
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
    let workspace = ProjectWorkspace::from_loaded(cwd, loaded);
    let source_dir = workspace.primary_source_root().to_path_buf();
    let out_dir = workspace.primary_output_root().to_path_buf();
    let in_place = paths_same_location(&source_dir, &out_dir);
    let mut outcome = BuildOutcome::default();
    let mut units = Vec::new();
    let mut host = AnalysisHost::with_config(workspace.config().clone());
    let mut needs_brinevector_runtime = false;

    host.set_source_roots(workspace.source_roots().iter().cloned());
    host.set_output_roots(workspace.output_roots().iter().cloned());
    host.set_asset_roots(workspace.asset_roots());

    for source_path in source_files {
        let source_kind = SourceKind::for_path(&source_path, &workspace.config().extension);
        let output_path = output_path_for(&source_dir, &out_dir, &source_path, source_kind);

        if source_kind == SourceKind::Asset {
            if in_place {
                continue;
            }

            if let Some(tua_source_path) =
                colliding_tua_source_for_asset(&source_path, &workspace.config().extension)
            {
                outcome.diagnostics.push(output_collision_diagnostic(
                    &source_path,
                    &tua_source_path,
                    in_place,
                ));
                continue;
            }

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

        host.set_file_with_kind(&source_path, source.clone(), AnalysisSourceKind::Tua);
        units.push((source_path, output_path, source));
    }

    let snapshot = host.snapshot();

    let annotation_mode = workspace.config().emit.annotation_mode;

    for (source_path, output_path, source) in units {
        let mut source_diagnostics = snapshot.pipeline_diagnostics(&source_path);
        let has_source_errors = has_errors(&source_diagnostics);

        outcome.diagnostics.append(&mut source_diagnostics);

        if has_source_errors {
            continue;
        }

        push_sibling_collision_warning(&mut outcome.diagnostics, &source_path, in_place);

        let output =
            match emit_tua_source_with_workspace(&source_path, source, annotation_mode, &workspace)
            {
                Ok(output) => output,
                Err(mut source_diagnostics) => {
                    outcome.diagnostics.append(&mut source_diagnostics);
                    continue;
                }
            };
        needs_brinevector_runtime |= requires_brinevector_runtime(&output);

        match write_output(&output_path, output) {
            Ok(()) => outcome.emitted += 1,
            Err(diagnostic) => outcome.diagnostics.push(diagnostic),
        }
    }

    if needs_brinevector_runtime {
        write_brinevector_runtime(&out_dir, &mut outcome.emitted, &mut outcome.diagnostics);
    }

    outcome
}

fn build_project_files(
    cwd: &Path,
    loaded: LoadedConfig,
    annotation_mode: AnnotationMode,
    source_files: Vec<SourceFile>,
    emit_on_error: bool,
) -> BuildOutcome {
    let workspace = ProjectWorkspace::from_loaded(cwd, loaded);
    let source_dir = workspace.primary_source_root().to_path_buf();
    let out_dir = workspace.primary_output_root().to_path_buf();
    let in_place = paths_same_location(&source_dir, &out_dir);
    let mut diagnostics = Vec::new();
    let mut units = Vec::new();
    let tua_outputs = tua_outputs_by_path(&source_dir, &out_dir, &source_files);

    for source_file in source_files {
        let source_path = source_file.path;
        let output_path = output_path_for(&source_dir, &out_dir, &source_path, source_file.kind);

        if source_file.kind == SourceKind::Asset {
            if let Some(tua_source_path) = tua_outputs.get(&output_path) {
                diagnostics.push(output_collision_diagnostic(
                    &source_path,
                    tua_source_path,
                    in_place,
                ));
                continue;
            }

            if in_place {
                continue;
            }

            units.push(SourceUnit::Asset {
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

    let mut host = AnalysisHost::with_config(workspace.config().clone());
    host.set_source_roots(workspace.source_roots().iter().cloned());
    host.set_output_roots(workspace.output_roots().iter().cloned());
    host.set_asset_roots(workspace.asset_roots());

    for unit in &units {
        match unit {
            SourceUnit::Tua {
                source_path,
                source,
                ..
            } => {
                host.set_file_with_kind(source_path, source.clone(), AnalysisSourceKind::Tua);
            }
            SourceUnit::Asset { source_path, .. } => {
                if source_path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    != Some("lua")
                {
                    continue;
                }

                match read_source_file(source_path) {
                    Ok(source) => {
                        host.set_file_with_kind(source_path, source, AnalysisSourceKind::Lua);
                    }
                    Err(diagnostic) => diagnostics.push(diagnostic),
                }
            }
        }
    }

    let snapshot = host.snapshot();

    for unit in &mut units {
        let SourceUnit::Tua {
            source_path,
            emit_candidate,
            ..
        } = unit
        else {
            continue;
        };

        let mut source_diagnostics = snapshot.pipeline_diagnostics(source_path);
        if !source_diagnostics.is_empty() {
            let has_non_emittable_error = source_diagnostics
                .iter()
                .any(|diagnostic| !can_emit_after_diagnostic(diagnostic));

            if has_non_emittable_error {
                *emit_candidate = false;
            }

            diagnostics.append(&mut source_diagnostics);
        }
    }

    if has_errors(&diagnostics) && !emit_on_error {
        return BuildOutcome {
            emitted: 0,
            diagnostics,
        };
    }

    let mut emitted = 0usize;
    let mut needs_brinevector_runtime = false;

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

                let output = match emit_tua_source_with_workspace(
                    &source_path,
                    source,
                    annotation_mode,
                    &workspace,
                ) {
                    Ok(output) => output,
                    Err(mut source_diagnostics) => {
                        diagnostics.append(&mut source_diagnostics);
                        continue;
                    }
                };
                needs_brinevector_runtime |= requires_brinevector_runtime(&output);

                match write_output(&output_path, output) {
                    Ok(()) => emitted += 1,
                    Err(diagnostic) => diagnostics.push(diagnostic),
                }
            }
            SourceUnit::Asset {
                source_path,
                output_path,
            } => match copy_output(&source_path, &output_path) {
                Ok(()) => emitted += 1,
                Err(diagnostic) => diagnostics.push(diagnostic),
            },
        }
    }

    if needs_brinevector_runtime {
        write_brinevector_runtime(&out_dir, &mut emitted, &mut diagnostics);
    }

    BuildOutcome {
        emitted,
        diagnostics,
    }
}

pub(crate) fn output_path_for(
    source_dir: &Path,
    out_dir: &Path,
    source_path: &Path,
    source_kind: SourceKind,
) -> PathBuf {
    let relative_path = source_path
        .strip_prefix(source_dir)
        .expect("discovered source file should be under source directory");

    let output_path = out_dir.join(relative_path);

    match source_kind {
        SourceKind::Tua => output_path.with_extension("lua"),
        SourceKind::Asset => output_path,
    }
}

fn can_emit_after_diagnostic(diagnostic: &Diagnostic) -> bool {
    diagnostic.severity != DiagnosticSeverity::Error
        || !matches!(
            diagnostic.code.map(|code| code.category()),
            Some(DiagnosticCategory::Syntax | DiagnosticCategory::LoweringEmission)
        )
}

pub(crate) fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
}

fn write_output(output_path: &Path, output: String) -> Result<(), Diagnostic> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|source| io_diagnostic(parent, source))?;
    }

    fs::write(output_path, output).map_err(|source| io_diagnostic(output_path, source))
}

fn requires_brinevector_runtime(output: &str) -> bool {
    output.contains(BRINEVECTOR_RUNTIME_REQUIRE)
}

fn write_brinevector_runtime(
    out_dir: &Path,
    emitted: &mut usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let output_path = out_dir.join(BRINEVECTOR_RUNTIME_FILE);

    if output_path.exists() {
        return;
    }

    match write_output(&output_path, brinevector_runtime_source().to_owned()) {
        Ok(()) => *emitted += 1,
        Err(diagnostic) => diagnostics.push(diagnostic),
    }
}

fn copy_output(source_path: &Path, output_path: &Path) -> Result<(), Diagnostic> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|source| io_diagnostic(parent, source))?;
    }

    fs::copy(source_path, output_path)
        .map(|_| ())
        .map_err(|source| io_diagnostic(output_path, source))
}

fn tua_outputs_by_path(
    source_dir: &Path,
    out_dir: &Path,
    source_files: &[SourceFile],
) -> BTreeMap<PathBuf, PathBuf> {
    source_files
        .iter()
        .filter(|source_file| source_file.kind == SourceKind::Tua)
        .map(|source_file| {
            (
                output_path_for(source_dir, out_dir, &source_file.path, source_file.kind),
                source_file.path.clone(),
            )
        })
        .collect()
}

fn colliding_tua_source_for_asset(asset_path: &Path, extension: &str) -> Option<PathBuf> {
    let tua_path = asset_path.with_extension(extension);

    (tua_path != asset_path && tua_path.exists()).then_some(tua_path)
}

fn push_sibling_collision_warning(
    diagnostics: &mut Vec<Diagnostic>,
    tua_source_path: &Path,
    in_place: bool,
) {
    let lua_path = tua_source_path.with_extension("lua");

    if lua_path != tua_source_path && lua_path.exists() {
        diagnostics.push(output_collision_diagnostic(
            &lua_path,
            tua_source_path,
            in_place,
        ));
    }
}

fn output_collision_diagnostic(
    lua_source_path: &Path,
    tua_source_path: &Path,
    in_place: bool,
) -> Diagnostic {
    let message = if in_place {
        format!(
            "`{}` will be overwritten by generated Lua from `{}`",
            display_path(lua_source_path),
            display_path(tua_source_path)
        )
    } else {
        format!(
            "`{}` would overwrite generated Lua from `{}`; skipping asset copy",
            display_path(lua_source_path),
            display_path(tua_source_path)
        )
    };

    Diagnostic::new(lua_source_path, 1, 1, DiagnosticSeverity::Warning, message)
        .with_code(DiagnosticCode::EMIT_OUTPUT_COLLISION)
}

fn format_success(verb: &str, emitted: usize, diagnostics: Vec<Diagnostic>) -> String {
    if diagnostics.is_empty() {
        return format!("{verb} {emitted} file(s)");
    }

    format!(
        "{}\n{verb} {emitted} file(s)",
        format_diagnostics_for_cli(&sorted_diagnostics(diagnostics))
    )
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

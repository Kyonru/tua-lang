use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::diagnostics::sorted_diagnostics;
use crate::error::CliError;
use crate::source::{
    discover_build_files, io_diagnostic, paths_same_location, read_source_file, SourceFile,
    SourceKind,
};
use crate::source_map::write_source_map;
use tua_core::{
    anim8_runtime_source, brinevector_runtime_source, emit_tua_source_with_workspace,
    format_diagnostics_for_cli, tuasignal_runtime_source, AnalysisHost, AnalysisSourceKind,
    AnnotationMode, Diagnostic, DiagnosticCategory, DiagnosticCode, DiagnosticSeverity,
    LoadedConfig, ProjectWorkspace,
};

struct BundledRuntime {
    file_name: &'static str,
    require_literal: &'static str,
    source: fn() -> &'static str,
}

const BUNDLED_RUNTIMES: &[BundledRuntime] = &[
    BundledRuntime {
        file_name: "brinevector.lua",
        require_literal: "require(\"brinevector\")",
        source: brinevector_runtime_source,
    },
    BundledRuntime {
        file_name: "anim8.lua",
        require_literal: "require(\"anim8\")",
        source: anim8_runtime_source,
    },
    BundledRuntime {
        file_name: "tuasignal.lua",
        require_literal: "require(\"tuasignal\")",
        source: tuasignal_runtime_source,
    },
];

#[derive(Debug, Default)]
pub(crate) struct BuildOutcome {
    pub(crate) emitted: usize,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

mod watch;

pub(crate) use watch::WatchBuildSession;

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
    let mut required_bundled_runtimes = BTreeSet::new();

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
                    source.as_str(),
                    annotation_mode,
                    &workspace,
                ) {
                    Ok(output) => output,
                    Err(mut source_diagnostics) => {
                        diagnostics.append(&mut source_diagnostics);
                        continue;
                    }
                };
                collect_required_bundled_runtimes(&output, &mut required_bundled_runtimes);

                match write_output(&output_path, output.clone()) {
                    Ok(()) => {
                        emitted += 1;

                        if workspace.config().emit_maps {
                            if let Err(diagnostic) =
                                write_source_map(cwd, &source_path, &output_path, &source, &output)
                            {
                                diagnostics.push(diagnostic);
                            }
                        }
                    }
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

    write_required_bundled_runtimes(
        &out_dir,
        &required_bundled_runtimes,
        &mut emitted,
        &mut diagnostics,
    );

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

fn collect_required_bundled_runtimes(output: &str, required: &mut BTreeSet<&'static str>) {
    for runtime in BUNDLED_RUNTIMES {
        if output.contains(runtime.require_literal) {
            required.insert(runtime.file_name);
        }
    }
}

fn write_required_bundled_runtimes(
    out_dir: &Path,
    required: &BTreeSet<&'static str>,
    emitted: &mut usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for runtime in BUNDLED_RUNTIMES {
        if required.contains(runtime.file_name) {
            write_bundled_runtime(runtime, out_dir, emitted, diagnostics);
        }
    }
}

fn write_bundled_runtime(
    runtime: &BundledRuntime,
    out_dir: &Path,
    emitted: &mut usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let output_path = out_dir.join(runtime.file_name);

    if output_path.exists() {
        return;
    }

    match write_output(&output_path, (runtime.source)().to_owned()) {
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

fn analysis_kind_for_path(path: &Path, extension: &str) -> Option<AnalysisSourceKind> {
    match SourceKind::for_path(path, extension) {
        SourceKind::Tua => Some(AnalysisSourceKind::Tua),
        SourceKind::Asset
            if path.extension().and_then(|extension| extension.to_str()) == Some("lua") =>
        {
            Some(AnalysisSourceKind::Lua)
        }
        SourceKind::Asset => None,
    }
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

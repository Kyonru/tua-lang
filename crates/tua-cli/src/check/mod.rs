use std::path::Path;

use crate::asset_pipeline::run_asset_pipelines;
use crate::diagnostics::sorted_diagnostics;
use crate::error::CliError;
use crate::source::{discover_build_files, load_type_libraries, read_source_file, SourceKind};
use tua_core::{format_diagnostics_for_cli, DiagnosticCategory, DiagnosticSeverity};
use tua_core::{profile, AnalysisHost, AnalysisSourceKind, LoadedConfig, ProjectWorkspace};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProjectDiagnosticMode {
    Check,
    Lint,
}

pub(crate) fn run_check(
    cwd: impl AsRef<Path>,
    loaded: LoadedConfig,
    deny_warnings: bool,
    love: Option<&Path>,
) -> Result<String, CliError> {
    run_project_diagnostics(
        cwd,
        loaded,
        ProjectDiagnosticMode::Check,
        "checked",
        "project check failed",
        deny_warnings,
        love,
    )
}

pub(crate) fn run_project_diagnostics(
    cwd: impl AsRef<Path>,
    loaded: LoadedConfig,
    mode: ProjectDiagnosticMode,
    success_verb: &str,
    error_message: &str,
    deny_warnings: bool,
    love: Option<&Path>,
) -> Result<String, CliError> {
    let cwd = cwd.as_ref();
    let pipeline_outcome = if mode == ProjectDiagnosticMode::Check {
        run_asset_pipelines(cwd, &loaded.config)
    } else {
        Default::default()
    };
    let workspace = ProjectWorkspace::from_loaded(cwd, loaded);
    let source_dir = workspace.primary_source_root().to_path_buf();
    let mut source_files = {
        let _trace = profile::diagnostics_span(profile::DiagnosticsTraceCategory::DiscoverFiles);

        discover_build_files(&source_dir, &workspace.config().extension, None).map_err(|error| {
            CliError::from_diagnostics("failed to discover source files", vec![error])
        })?
    };
    source_files.retain(|source_file| !workspace.is_type_library_path(&source_file.path));
    let checked = source_files
        .iter()
        .filter(|source_file| source_file.kind == SourceKind::Tua)
        .count();
    let mut diagnostics = pipeline_outcome.diagnostics;
    let mut host = AnalysisHost::with_config(workspace.config().clone());

    host.set_source_roots(workspace.source_roots().iter().cloned());
    host.set_output_roots(workspace.output_roots().iter().cloned());
    host.set_asset_roots(workspace.asset_roots());
    diagnostics.extend(load_type_libraries(&mut host, &workspace));

    for source_file in &source_files {
        let _trace = profile::diagnostics_span(profile::DiagnosticsTraceCategory::ReadVfsUpdate);

        let Some(kind) =
            analysis_source_kind_for_path(&source_file.path, source_file.kind, &workspace)
        else {
            continue;
        };
        let source = match read_source_file(&source_file.path) {
            Ok(source) => source,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };

        host.set_file_with_kind(&source_file.path, source, kind);
    }

    if mode == ProjectDiagnosticMode::Check {
        host.refresh_asset_index_from_disk();
    }
    let snapshot = host.snapshot();

    for source_file in source_files {
        if source_file.kind != SourceKind::Tua {
            continue;
        }

        diagnostics.extend(snapshot.pipeline_diagnostics(&source_file.path));
    }

    let mut note = String::new();
    if mode == ProjectDiagnosticMode::Check {
        use tua_core::shader_validation::{
            shader_validation_requests, validation_diagnostics, ShaderValidator,
        };
        let requests = shader_validation_requests(
            &snapshot.asset_inventory(),
            &workspace.config().shaders,
            |path| {
                tua_core::input_limits::read_source(path)
                    .map_err(|error| format!("{}: {error}", path.display()))
            },
        )
        .map_err(CliError::usage)?;
        match crate::shaders::ProjectShaderValidator::new(love).validate(&requests) {
            Ok(outcomes) => diagnostics.extend(validation_diagnostics(&requests, &outcomes)),
            Err(error)
                if [
                    crate::love::LOVE_NOT_FOUND,
                    crate::shaders::NO_GLSLANG,
                    crate::shaders::NO_VALIDATOR,
                ]
                .contains(&error.as_str()) =>
            {
                note = format!("note: shader compilation skipped: {error}\n");
            }
            Err(error) => {
                return Err(CliError::usage(format!(
                    "shader validation failed: {error}"
                )))
            }
        }
    }

    // Snapshot diagnostics have already passed through this policy. Applying
    // it once more is idempotent and also covers CLI-owned diagnostics such as
    // source I/O and asset-pipeline results.
    diagnostics = workspace
        .config()
        .diagnostics
        .apply_severity_policy(diagnostics);

    if mode == ProjectDiagnosticMode::Lint {
        diagnostics.retain(|diagnostic| {
            diagnostic.code.is_none_or(|code| {
                matches!(
                    code.category(),
                    DiagnosticCategory::Syntax | DiagnosticCategory::Lint
                )
            })
        });
    }

    profile::report_diagnostics_trace(format!(
        "cli {success_verb} files={checked} diagnostics={}",
        diagnostics.len()
    ));

    let fails = diagnostics.iter().any(|diagnostic| match mode {
        ProjectDiagnosticMode::Check => {
            diagnostic.severity == DiagnosticSeverity::Error
                || (deny_warnings && diagnostic.severity == DiagnosticSeverity::Warning)
        }
        ProjectDiagnosticMode::Lint => matches!(
            diagnostic.severity,
            DiagnosticSeverity::Error | DiagnosticSeverity::Warning
        ),
    });

    if fails {
        Err(CliError::from_diagnostics(
            error_message,
            sorted_diagnostics(diagnostics),
        ))
    } else if diagnostics.is_empty() {
        Ok(format!("{note}{success_verb} {checked} file(s)"))
    } else {
        let diagnostics = sorted_diagnostics(diagnostics);
        Ok(format!(
            "{note}{}\n{success_verb} {checked} file(s)",
            format_diagnostics_for_cli(&diagnostics)
        ))
    }
}

fn analysis_source_kind_for_path(
    path: &Path,
    source_kind: SourceKind,
    workspace: &ProjectWorkspace,
) -> Option<AnalysisSourceKind> {
    match source_kind {
        SourceKind::Tua => Some(AnalysisSourceKind::Tua),
        SourceKind::Asset => (path.extension().and_then(|extension| extension.to_str())
            == Some("lua"))
        .then_some(AnalysisSourceKind::Lua),
    }
    .filter(|_| {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                extension == "lua" || extension == workspace.config().extension
            })
    })
}

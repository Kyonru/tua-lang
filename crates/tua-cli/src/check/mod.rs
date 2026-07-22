use std::path::Path;

use crate::diagnostics::sorted_diagnostics;
use crate::error::CliError;
use crate::source::{discover_build_files, load_type_libraries, read_source_file, SourceKind};
use tua_core::DiagnosticCategory;
use tua_core::{profile, AnalysisHost, AnalysisSourceKind, LoadedConfig, ProjectWorkspace};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProjectDiagnosticMode {
    Check,
    Lint,
}

pub(crate) fn run_check(cwd: impl AsRef<Path>, loaded: LoadedConfig) -> Result<String, CliError> {
    run_project_diagnostics(
        cwd,
        loaded,
        ProjectDiagnosticMode::Check,
        "checked",
        "project check failed",
    )
}

pub(crate) fn run_project_diagnostics(
    cwd: impl AsRef<Path>,
    loaded: LoadedConfig,
    mode: ProjectDiagnosticMode,
    success_verb: &str,
    error_message: &str,
) -> Result<String, CliError> {
    let cwd = cwd.as_ref();
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
    let mut diagnostics = Vec::new();
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

    let snapshot = host.snapshot();

    for source_file in source_files {
        if source_file.kind != SourceKind::Tua {
            continue;
        }

        diagnostics.extend(snapshot.pipeline_diagnostics(&source_file.path));
    }

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

    if diagnostics.is_empty() {
        Ok(format!("{success_verb} {checked} file(s)"))
    } else {
        Err(CliError::from_diagnostics(
            error_message,
            sorted_diagnostics(diagnostics),
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

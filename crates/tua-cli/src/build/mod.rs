use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::asset_pipeline::run_asset_pipelines;
use crate::diagnostics::sorted_diagnostics;
use crate::error::CliError;
use crate::source::{
    io_diagnostic, load_type_libraries, paths_same_location, read_source_file, SourceFile,
    SourceKind,
};
use crate::source_map::{debugger_source_map_path_for, source_map_path_for, write_source_map};
use tua_core::path_security::reject_symlink_path;
use tua_core::{
    bundled_runtime_requirements, emit_tua_source_with_workspace_artifact,
    emit_tua_source_with_workspace_environment_artifact, format_diagnostics_for_cli, AnalysisHost,
    AnalysisSourceKind, AnnotationMode, BundledRuntime, Diagnostic, DiagnosticCategory,
    DiagnosticCode, DiagnosticSeverity, LoadedConfig, ProjectWorkspace, BUNDLED_RUNTIMES,
};

#[derive(Debug, Default)]
pub(crate) struct BuildOutcome {
    pub(crate) emitted: usize,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) shader_dependencies: Vec<ShaderDependency>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ShaderDependency {
    pub(crate) shader: String,
    pub(crate) module: String,
}

mod cache;
pub(crate) mod ownership;
mod watch;

pub(crate) use watch::WatchBuildSession;

enum SourceUnit {
    Tua {
        source_path: PathBuf,
        source: String,
        emit_candidate: bool,
    },
    Asset {
        source_path: PathBuf,
        output_path: PathBuf,
        lua_source: Option<String>,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct OutputGuard {
    root: PathBuf,
    cleanup_root: Option<PathBuf>,
}

impl OutputGuard {
    pub(crate) fn new(workspace: &ProjectWorkspace) -> Result<Self, Diagnostic> {
        let mut guard = Self::for_root(workspace.root());
        let output = workspace.primary_output_root();
        if !workspace
            .source_roots()
            .iter()
            .any(|source| output.starts_with(source) || source.starts_with(output))
        {
            guard.cleanup_root = Some(output.to_path_buf());
        }
        guard.check(workspace.primary_output_root())?;
        Ok(guard)
    }

    pub(crate) fn for_root(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            cleanup_root: None,
        }
    }

    pub(crate) fn check(&self, path: &Path) -> Result<(), Diagnostic> {
        reject_symlink_path(&self.root, path).map_err(|message| {
            Diagnostic::error(path, 1, 1, message).with_code(DiagnosticCode::EMIT_IO)
        })
    }
}

pub(crate) fn run_build(
    cwd: impl AsRef<Path>,
    loaded: LoadedConfig,
    emit_on_error: bool,
) -> Result<String, CliError> {
    run_build_with_cache(cwd, loaded, emit_on_error, true, false)
}

pub(crate) fn run_build_with_cache(
    cwd: impl AsRef<Path>,
    loaded: LoadedConfig,
    emit_on_error: bool,
    use_cache: bool,
    report: bool,
) -> Result<String, CliError> {
    let trace = BuildReport::new(report);
    if report {
        trace.note(&format!(
            "compiler: {}",
            serde_json::to_string(&crate::build_info()).expect("serializable build info")
        ));
    }
    let cwd = cwd.as_ref();
    let workspace = ProjectWorkspace::from_loaded(cwd, loaded.clone());
    OutputGuard::new(&workspace).map_err(|diagnostic| {
        CliError::from_diagnostics("unsafe build output path", vec![diagnostic])
    })?;
    // Importers retain their existing content cache and always run before the
    // build snapshot, so changed/generated inputs cannot be hidden by a hit.
    let phase = std::time::Instant::now();
    let pipeline = run_asset_pipelines(cwd, &loaded.config);
    trace.phase("asset pipelines", phase);
    let phase = std::time::Instant::now();
    let cache = (use_cache && !emit_on_error && !has_errors(&pipeline.diagnostics))
        .then(|| cache::BuildCache::prepare(&workspace, &pipeline.diagnostics));
    let cache = match cache {
        Some(Ok(cache)) => Some(cache),
        Some(Err(error)) => {
            trace.note(&format!("cache unavailable: {error}"));
            None
        }
        None => {
            trace.note("cache bypassed: disabled, emit-on-error or pipeline errors");
            None
        }
    };
    if let Some(cache) = &cache {
        match cache.hit() {
            Ok(message) => {
                trace.note("cache hit");
                trace.phase("cache probe", phase);
                return Ok(message);
            }
            Err(reason) => trace.note(&format!("cache miss: {reason}")),
        }
    }
    trace.phase("cache probe", phase);
    let phase = std::time::Instant::now();
    let outcome = build_project_after_pipelines(
        cwd,
        loaded,
        emit_on_error,
        &[],
        false,
        pipeline.diagnostics,
    )?;
    trace.phase("discovery, analysis and emission", phase);
    if has_errors(&outcome.diagnostics) {
        Err(CliError::from_diagnostics(
            "project build failed",
            sorted_diagnostics(outcome.diagnostics),
        ))
    } else {
        if let Some(cache) = cache {
            let diagnostics =
                format_diagnostics_for_cli(&sorted_diagnostics(outcome.diagnostics.clone()));
            let diagnostics = if diagnostics.is_empty() {
                diagnostics
            } else {
                format!("{diagnostics}\n")
            };
            let phase = std::time::Instant::now();
            trace.note(&format!(
                "cache {}",
                cache.save(outcome.emitted, diagnostics)
            ));
            trace.phase("cache publication", phase);
        }
        Ok(format_success(
            "built",
            outcome.emitted,
            outcome.diagnostics,
        ))
    }
}

pub(crate) fn build_project_excluding(
    cwd: &Path,
    loaded: LoadedConfig,
    emit_on_error: bool,
    additional_excluded_outputs: &[&Path],
    include_tests: bool,
) -> Result<BuildOutcome, CliError> {
    let workspace = ProjectWorkspace::from_loaded(cwd, loaded.clone());
    OutputGuard::new(&workspace).map_err(|diagnostic| {
        CliError::from_diagnostics("unsafe build output path", vec![diagnostic])
    })?;
    let pipeline = run_asset_pipelines(cwd, &loaded.config);
    build_project_after_pipelines(
        cwd,
        loaded,
        emit_on_error,
        additional_excluded_outputs,
        include_tests,
        pipeline.diagnostics,
    )
}

fn build_project_after_pipelines(
    cwd: &Path,
    loaded: LoadedConfig,
    emit_on_error: bool,
    additional_excluded_outputs: &[&Path],
    include_tests: bool,
    pipeline_diagnostics: Vec<Diagnostic>,
) -> Result<BuildOutcome, CliError> {
    let workspace = ProjectWorkspace::from_loaded(cwd, loaded.clone());
    OutputGuard::new(&workspace).map_err(|diagnostic| {
        CliError::from_diagnostics("unsafe build output path", vec![diagnostic])
    })?;
    let source_dir = workspace.primary_source_root().to_path_buf();
    let out_dir = workspace.primary_output_root().to_path_buf();
    let in_place = paths_same_location(&source_dir, &out_dir);
    let mut excluded = additional_excluded_outputs.to_vec();
    if !in_place {
        excluded.push(&out_dir);
    }
    let source_files = crate::source::discover_project_files(&workspace, include_tests, &excluded)
        .map_err(|error| {
            CliError::from_diagnostics("failed to discover source files", vec![error])
        })?;
    let annotation_mode = workspace.config().emit.annotation_mode;

    Ok(build_project_files(
        cwd,
        loaded,
        annotation_mode,
        source_files,
        emit_on_error,
        pipeline_diagnostics,
    ))
}

fn build_project_files(
    cwd: &Path,
    loaded: LoadedConfig,
    annotation_mode: AnnotationMode,
    source_files: Vec<SourceFile>,
    emit_on_error: bool,
    initial_diagnostics: Vec<Diagnostic>,
) -> BuildOutcome {
    let workspace = ProjectWorkspace::from_loaded(cwd, loaded);
    let output_guard = match OutputGuard::new(&workspace) {
        Ok(guard) => guard,
        Err(diagnostic) => {
            return BuildOutcome {
                emitted: 0,
                diagnostics: vec![diagnostic],
                ..BuildOutcome::default()
            };
        }
    };
    let source_dir = workspace.primary_source_root().to_path_buf();
    let out_dir = workspace.primary_output_root().to_path_buf();
    let in_place = paths_same_location(&source_dir, &out_dir);
    let mut diagnostics = initial_diagnostics;
    let mut units = Vec::new();
    let tua_output_owners = tua_output_owners_for_paths(
        &workspace,
        source_files
            .iter()
            .filter(|source_file| source_file.kind.is_compiled())
            .map(|source_file| source_file.path.clone()),
    );
    let tua_output_collisions = colliding_tua_outputs(&tua_output_owners);

    push_tua_output_collision_diagnostics(&mut diagnostics, &tua_output_collisions);

    if workspace.config().i18n.enabled {
        for catalog in &workspace.config().i18n.catalogs {
            let path = source_dir.join(&catalog.path);
            if !path.is_file() {
                diagnostics.push(
                    Diagnostic::error(
                        &path,
                        1,
                        1,
                        format!("i18n catalog for `{}` does not exist", catalog.locale),
                    )
                    .with_code(DiagnosticCode::CONFIG_IO),
                );
            } else if let Err(message) = reject_symlink_path(workspace.root(), &path) {
                diagnostics.push(
                    Diagnostic::error(&path, 1, 1, message).with_code(DiagnosticCode::CONFIG_IO),
                );
            }
        }
    }

    for source_file in source_files {
        let source_path = source_file.path;
        let output_path = output_path_for_asset(&workspace, &source_path);

        if source_file.kind == SourceKind::Asset {
            if is_pipeline_development_output(cwd, workspace.config(), &source_path) {
                continue;
            }
            if let Some(tua_source_path) = tua_output_owners
                .get(&output_path)
                .and_then(|owners| owners.first())
            {
                diagnostics.push(output_collision_diagnostic(
                    &source_path,
                    tua_source_path,
                    in_place,
                ));
                continue;
            }

            let lua_source = (source_path
                .extension()
                .and_then(|extension| extension.to_str())
                == Some("lua"))
            .then(|| read_source_file(&source_path))
            .transpose()
            .unwrap_or_else(|diagnostic| {
                diagnostics.push(diagnostic);
                None
            });
            units.push(SourceUnit::Asset {
                source_path,
                output_path,
                lua_source,
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
            source,
            emit_candidate: true,
        });
    }

    let mut host = AnalysisHost::with_config(workspace.config().clone());
    host.set_source_roots(workspace.source_roots().iter().cloned());
    host.set_output_roots(workspace.output_roots().iter().cloned());
    host.set_asset_roots(workspace.asset_roots());
    diagnostics.extend(load_type_libraries(&mut host, &workspace));

    for unit in &units {
        match unit {
            SourceUnit::Tua {
                source_path,
                source,
                ..
            } => {
                host.set_file_with_kind(
                    source_path,
                    source.clone(),
                    analysis_kind_for_path(source_path, &workspace.config().extension)
                        .expect("compiled source"),
                );
            }
            SourceUnit::Asset {
                source_path,
                lua_source,
                ..
            } => {
                if let Some(source) = lua_source {
                    host.set_file_with_kind(source_path, source.clone(), AnalysisSourceKind::Lua);
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

    remove_colliding_outputs(&output_guard, &tua_output_collisions, &mut diagnostics);

    if has_errors(&diagnostics) && !emit_on_error {
        return BuildOutcome {
            emitted: 0,
            diagnostics,
            ..BuildOutcome::default()
        };
    }

    let mut emitted = 0usize;
    let mut owned_outputs = BTreeSet::new();
    let mut required_bundled_runtimes = BTreeSet::new();

    for unit in units {
        match unit {
            SourceUnit::Tua {
                source_path,
                source,
                emit_candidate,
            } => {
                if emit_on_error && !emit_candidate {
                    continue;
                }
                if tua_core::is_tua_declaration_path(&source_path, &workspace.config().extension) {
                    remove_declaration_outputs(
                        &output_guard,
                        &workspace,
                        &source_path,
                        &mut diagnostics,
                    );
                    continue;
                }

                let environment = snapshot.project_type_environment(&source_path);
                let emission = if source_path
                    .extension()
                    .is_some_and(|extension| extension == "tdialogue")
                {
                    tua_core::emit::emit_dialogue_source(&source_path, &source)
                } else {
                    match environment.as_ref() {
                        Some(environment) => emit_tua_source_with_workspace_environment_artifact(
                            &source_path,
                            source.as_str(),
                            annotation_mode,
                            &workspace,
                            environment,
                        ),
                        None => emit_tua_source_with_workspace_artifact(
                            &source_path,
                            source.as_str(),
                            annotation_mode,
                            &workspace,
                        ),
                    }
                };
                let emission = match emission {
                    Ok(output) => output,
                    Err(mut source_diagnostics) => {
                        diagnostics.append(&mut source_diagnostics);
                        continue;
                    }
                };
                let output = emission.code;
                collect_required_bundled_runtimes(&output, &mut required_bundled_runtimes);

                for output_path in output_paths_for_tua_source(&workspace, &source_path) {
                    if tua_output_collisions.contains_key(&output_path) {
                        continue;
                    }

                    match write_output(&output_guard, &output_path, output.clone()) {
                        Ok(()) => {
                            emitted += 1;
                            owned_outputs.insert(output_path.clone());

                            if workspace.config().emit_maps {
                                if let Err(diagnostic) = write_source_map(
                                    output_guard.root(),
                                    cwd,
                                    &source_path,
                                    &output_path,
                                    &emission.provenance,
                                ) {
                                    diagnostics.push(diagnostic);
                                } else {
                                    owned_outputs.insert(source_map_path_for(&output_path));
                                    owned_outputs
                                        .insert(debugger_source_map_path_for(&output_path));
                                }
                            } else {
                                remove_source_maps(&output_guard, &output_path, &mut diagnostics);
                            }
                        }
                        Err(diagnostic) => diagnostics.push(diagnostic),
                    }
                }
            }
            SourceUnit::Asset {
                source_path,
                output_path,
                lua_source,
            } => {
                if let Some(source) = lua_source {
                    collect_required_bundled_runtimes(&source, &mut required_bundled_runtimes);
                }
                if in_place {
                    continue;
                }
                match copy_output(&output_guard, &source_path, &output_path) {
                    Ok(()) => {
                        emitted += 1;
                        owned_outputs.insert(output_path);
                    }
                    Err(diagnostic) => diagnostics.push(diagnostic),
                }
            }
        }
    }

    if !workspace.config().i18n.enabled {
        required_bundled_runtimes.remove("tuai18n");
    }
    if !required_bundled_runtimes.contains("tuai18n") {
        required_bundled_runtimes.retain(|module| !module.starts_with("smiti18n"));
    }

    let existing_runtimes = write_required_bundled_runtimes(
        &output_guard,
        &workspace,
        &out_dir,
        &required_bundled_runtimes,
        &mut emitted,
        &mut diagnostics,
    );

    if required_bundled_runtimes.contains("tuai18n") && workspace.config().i18n.enabled {
        let path = out_dir.join("tuai18n_config.lua");
        if source_dir.join("tuai18n_config.lua").exists() || tua_output_owners.contains_key(&path) {
            diagnostics.push(Diagnostic::error(&path, 1, 1, "configured localization reserves `tuai18n_config.lua`; rename the project file").with_code(DiagnosticCode::EMIT_OUTPUT_COLLISION));
        } else {
            match write_output(&output_guard, &path, i18n_config_lua(workspace.config())) {
                Ok(()) => {
                    emitted += 1;
                    owned_outputs.insert(path);
                }
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }
    }

    for runtime in BUNDLED_RUNTIMES {
        if required_bundled_runtimes.contains(runtime.module) {
            owned_outputs.insert(out_dir.join(runtime.file_name));
        }
    }
    // Never reconcile a tree containing sources, or an in-place build.
    let overlaps_source = workspace
        .source_roots()
        .iter()
        .any(|source| out_dir.starts_with(source) || source.starts_with(&out_dir));
    if !overlaps_source && !has_errors(&diagnostics) {
        if let Err(error) = ownership::reconcile(&out_dir, &owned_outputs, &existing_runtimes) {
            diagnostics.push(error);
        }
    }

    BuildOutcome {
        emitted,
        diagnostics,
        ..BuildOutcome::default()
    }
}

fn i18n_config_lua(config: &tua_core::ProjectConfig) -> String {
    let mut output = format!(
        "return {{\n  default_locale = {},\n  fallback_locale = {},\n  catalogs = {{\n",
        lua_quoted(&config.i18n.default_locale),
        lua_quoted(&config.i18n.fallback_locale)
    );
    for catalog in &config.i18n.catalogs {
        let module = catalog
            .path
            .with_extension("")
            .components()
            .filter_map(|part| part.as_os_str().to_str())
            .collect::<Vec<_>>()
            .join(".");
        output.push_str(&format!(
            "    {{ locale = {}, module = {} }},\n",
            lua_quoted(&catalog.locale),
            lua_quoted(&module)
        ));
    }
    output.push_str("  },\n}\n");
    output
}

fn lua_quoted(value: &str) -> String {
    let mut output = String::from("\"");
    for byte in value.bytes() {
        match byte {
            b'"' => output.push_str("\\\""),
            b'\\' => output.push_str("\\\\"),
            32..=126 => output.push(char::from(byte)),
            _ => output.push_str(&format!("\\{byte:03}")),
        }
    }
    output.push('"');
    output
}

fn is_pipeline_development_output(
    cwd: &Path,
    config: &tua_core::ProjectConfig,
    source_path: &Path,
) -> bool {
    config.asset_pipelines.iter().any(|pipeline| {
        pipeline.builtin.as_deref() == Some("audio-analysis")
            && paths_same_location(
                source_path,
                &cwd.join(&pipeline.output).join("audio-index.json"),
            )
    })
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
        SourceKind::Tua | SourceKind::Dialogue => output_path.with_extension("lua"),
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

fn write_output(guard: &OutputGuard, output_path: &Path, output: String) -> Result<(), Diagnostic> {
    guard.check(output_path)?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|source| io_diagnostic(parent, source))?;
    }

    crate::atomic_fs::write(output_path, output)
        .map_err(|source| io_diagnostic(output_path, source))
}

fn collect_required_bundled_runtimes(output: &str, required: &mut BTreeSet<&'static str>) {
    required.extend(bundled_runtime_requirements(output));
}

fn write_required_bundled_runtimes(
    guard: &OutputGuard,
    workspace: &ProjectWorkspace,
    out_dir: &Path,
    required: &BTreeSet<&'static str>,
    emitted: &mut usize,
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeSet<PathBuf> {
    let mut preserved = BTreeSet::new();
    if required.is_empty() {
        return preserved;
    }
    let previous = match ownership::load(out_dir) {
        Ok(previous) => previous,
        Err(error) => {
            diagnostics.push(io_diagnostic(&out_dir.join(ownership::MANIFEST), error));
            return preserved;
        }
    };
    for runtime in BUNDLED_RUNTIMES {
        if required.contains(runtime.module)
            && write_bundled_runtime(
                guard,
                workspace,
                runtime,
                out_dir,
                &previous,
                emitted,
                diagnostics,
            )
        {
            preserved.insert(out_dir.join(runtime.file_name));
        }
    }
    preserved
}

fn write_bundled_runtime(
    guard: &OutputGuard,
    workspace: &ProjectWorkspace,
    runtime: &BundledRuntime,
    out_dir: &Path,
    previous: &ownership::Manifest,
    emitted: &mut usize,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let output_path = out_dir.join(runtime.file_name);

    let project_provides_runtime = workspace.source_roots().iter().any(|root| {
        root.join(runtime.file_name).is_file()
            || root
                .join(format!(
                    "{}.{}",
                    runtime.module,
                    workspace.config().extension
                ))
                .is_file()
    });
    if project_provides_runtime {
        return output_path.exists();
    }

    if output_path.exists() {
        match ownership::is_owned_unchanged(out_dir, &output_path, previous) {
            Ok(true) => {
                if fs::read_to_string(&output_path).ok().as_deref() == Some((runtime.source)()) {
                    return false;
                }
            }
            Ok(false) => return true,
            Err(error) => {
                diagnostics.push(io_diagnostic(&output_path, error));
                return true;
            }
        }
    }

    match write_output(guard, &output_path, (runtime.source)().to_owned()) {
        Ok(()) => *emitted += 1,
        Err(diagnostic) => diagnostics.push(diagnostic),
    }
    false
}

fn copy_output(
    guard: &OutputGuard,
    source_path: &Path,
    output_path: &Path,
) -> Result<(), Diagnostic> {
    guard.check(output_path)?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|source| io_diagnostic(parent, source))?;
    }

    crate::atomic_fs::copy(source_path, output_path)
        .map_err(|source| io_diagnostic(output_path, source))
}

pub(crate) fn tua_output_owners_for_paths(
    workspace: &ProjectWorkspace,
    source_paths: impl IntoIterator<Item = PathBuf>,
) -> BTreeMap<PathBuf, Vec<PathBuf>> {
    let mut owners = BTreeMap::new();

    for source_path in source_paths {
        for output_path in output_paths_for_tua_source(workspace, &source_path) {
            owners
                .entry(output_path)
                .or_insert_with(Vec::new)
                .push(source_path.clone());
        }
    }

    for sources in owners.values_mut() {
        sources.sort();
        sources.dedup();
    }

    owners
}

pub(crate) fn colliding_tua_outputs(
    owners: &BTreeMap<PathBuf, Vec<PathBuf>>,
) -> BTreeMap<PathBuf, Vec<PathBuf>> {
    owners
        .iter()
        .filter(|(_, sources)| sources.len() > 1)
        .map(|(output_path, sources)| (output_path.clone(), sources.clone()))
        .collect()
}

pub(crate) fn filter_tua_output_collisions_for_sources(
    collisions: &BTreeMap<PathBuf, Vec<PathBuf>>,
    relevant_sources: &BTreeSet<PathBuf>,
) -> BTreeMap<PathBuf, Vec<PathBuf>> {
    collisions
        .iter()
        .filter(|(_, sources)| {
            sources
                .iter()
                .any(|source_path| relevant_sources.contains(source_path))
        })
        .map(|(output_path, sources)| (output_path.clone(), sources.clone()))
        .collect()
}

pub(crate) fn output_paths_for_tua_source(
    workspace: &ProjectWorkspace,
    source_path: &Path,
) -> Vec<PathBuf> {
    workspace
        .lua_output_paths_for_tua_source(source_path)
        .unwrap_or_else(|| {
            vec![output_path_for(
                workspace.primary_source_root(),
                workspace.primary_output_root(),
                source_path,
                SourceKind::Tua,
            )]
        })
}

pub(crate) fn remove_declaration_outputs(
    guard: &OutputGuard,
    workspace: &ProjectWorkspace,
    source_path: &Path,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(mirrored) = workspace.mirrored_output_path_for_source(source_path) else {
        return;
    };
    let output = mirrored.with_extension("lua");
    remove_output_if_exists(guard, &output, diagnostics);
    remove_source_maps(guard, &output, diagnostics);
}

pub(crate) fn output_path_for_asset(workspace: &ProjectWorkspace, source_path: &Path) -> PathBuf {
    workspace
        .mirrored_output_path_for_source(source_path)
        .unwrap_or_else(|| {
            output_path_for(
                workspace.primary_source_root(),
                workspace.primary_output_root(),
                source_path,
                SourceKind::Asset,
            )
        })
}

fn analysis_kind_for_path(path: &Path, extension: &str) -> Option<AnalysisSourceKind> {
    match SourceKind::for_path(path, extension) {
        SourceKind::Tua => Some(AnalysisSourceKind::Tua),
        SourceKind::Dialogue => Some(AnalysisSourceKind::Dialogue),
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

    if tua_path != asset_path && tua_path.exists() {
        return Some(tua_path);
    }
    let dialogue_path = asset_path.with_extension("tdialogue");
    (dialogue_path != asset_path && dialogue_path.exists()).then_some(dialogue_path)
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

    let severity = if !in_place
        && tua_source_path
            .extension()
            .is_some_and(|ext| ext == "tdialogue")
    {
        DiagnosticSeverity::Error
    } else {
        DiagnosticSeverity::Warning
    };
    Diagnostic::new(lua_source_path, 1, 1, severity, message)
        .with_code(DiagnosticCode::EMIT_OUTPUT_COLLISION)
}

pub(crate) fn push_tua_output_collision_diagnostics(
    diagnostics: &mut Vec<Diagnostic>,
    collisions: &BTreeMap<PathBuf, Vec<PathBuf>>,
) {
    for (output_path, sources) in collisions {
        for (index, source_path) in sources.iter().enumerate() {
            let other_sources = sources
                .iter()
                .enumerate()
                .filter(|(other_index, _)| *other_index != index)
                .map(|(_, other_source)| display_path(other_source))
                .collect::<Vec<_>>()
                .join(", ");
            let message = format!(
                "generated output `{}` also comes from {}; only one compiled source can emit this path",
                display_path(output_path),
                other_sources,
            );

            diagnostics.push(
                Diagnostic::new(source_path, 1, 1, DiagnosticSeverity::Error, message)
                    .with_code(DiagnosticCode::EMIT_OUTPUT_COLLISION),
            );
        }
    }
}

pub(crate) fn remove_colliding_outputs(
    guard: &OutputGuard,
    collisions: &BTreeMap<PathBuf, Vec<PathBuf>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for output_path in collisions.keys() {
        remove_output_if_exists(guard, output_path, diagnostics);
        remove_source_maps(guard, output_path, diagnostics);
    }
}

fn remove_source_maps(guard: &OutputGuard, output_path: &Path, diagnostics: &mut Vec<Diagnostic>) {
    remove_output_if_exists(guard, &source_map_path_for(output_path), diagnostics);
    remove_output_if_exists(
        guard,
        &debugger_source_map_path_for(output_path),
        diagnostics,
    );
}

fn remove_output_if_exists(guard: &OutputGuard, path: &Path, diagnostics: &mut Vec<Diagnostic>) {
    if let Err(diagnostic) = guard.check(path) {
        diagnostics.push(diagnostic);
        return;
    }
    let Some(root) = &guard.cleanup_root else {
        return;
    };
    if let Err(source) = ownership::remove_owned(root, path) {
        diagnostics.push(io_diagnostic(path, source));
    }
}

impl OutputGuard {
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
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

// Opt-in wall durations, not a CPU profiler. Drop also reports failed builds.
struct BuildReport {
    enabled: bool,
    start: std::time::Instant,
}
impl BuildReport {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            start: std::time::Instant::now(),
        }
    }
    fn note(&self, message: &str) {
        if self.enabled {
            eprintln!("build report: {message}");
        }
    }
    fn phase(&self, name: &str, start: std::time::Instant) {
        self.note(&format!(
            "{name}: {:.3} ms wall",
            start.elapsed().as_secs_f64() * 1000.0
        ));
    }
}
impl Drop for BuildReport {
    fn drop(&mut self) {
        self.phase("total", self.start);
    }
}

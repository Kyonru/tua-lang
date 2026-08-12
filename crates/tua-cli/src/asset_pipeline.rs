use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tua_core::{
    AssetPipelineConfig, Diagnostic, DiagnosticCode, DiagnosticSeverity, ProjectConfig,
};

use crate::source::io_diagnostic;

const MANIFEST_VERSION: u32 = 1;
const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

#[derive(Debug, Default)]
pub(crate) struct AssetPipelineOutcome {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) executed: usize,
    pub(crate) cached: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ImportManifest {
    version: u32,
    #[serde(default)]
    outputs: Vec<String>,
    #[serde(default)]
    diagnostics: Vec<ImportDiagnostic>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ImportDiagnostic {
    severity: String,
    message: String,
    #[serde(default)]
    path: String,
    #[serde(default = "one")]
    line: usize,
    #[serde(default = "one")]
    column: usize,
    #[serde(default)]
    end_line: Option<usize>,
    #[serde(default)]
    end_column: Option<usize>,
    #[serde(default)]
    code: Option<u16>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MaterializedState {
    #[serde(default)]
    outputs: Vec<String>,
}

fn one() -> usize {
    1
}

pub(crate) fn run_asset_pipelines(cwd: &Path, config: &ProjectConfig) -> AssetPipelineOutcome {
    let mut outcome = AssetPipelineOutcome::default();
    let mut output_roots = Vec::<(String, PathBuf)>::new();
    let mut names = BTreeSet::new();

    for pipeline in &config.asset_pipelines {
        if !names.insert(pipeline.name.clone()) {
            outcome.diagnostics.push(pipeline_error(
                cwd,
                pipeline,
                "pipeline names must be unique",
            ));
            continue;
        }
        match validated_output_root(cwd, config, pipeline) {
            Ok(root) => {
                if let Some((owner, _)) = output_roots
                    .iter()
                    .find(|(_, existing)| root.starts_with(existing) || existing.starts_with(&root))
                {
                    outcome.diagnostics.push(pipeline_error(
                        cwd,
                        pipeline,
                        format!(
                            "output `{}` overlaps asset pipeline `{owner}`; each pipeline must own a separate generated directory",
                            display_path(&pipeline.output)
                        ),
                    ));
                    continue;
                }
                output_roots.push((pipeline.name.clone(), root));
            }
            Err(diagnostic) => {
                outcome.diagnostics.push(diagnostic);
                continue;
            }
        }

        run_one_pipeline(cwd, pipeline, &mut outcome);
    }

    outcome
}

pub(crate) fn asset_pipeline_input_files(
    cwd: &Path,
    config: &ProjectConfig,
) -> Result<Vec<PathBuf>, Diagnostic> {
    let mut files = BTreeSet::new();

    for pipeline in &config.asset_pipelines {
        for pattern in &pipeline.inputs {
            let paths = matching_input_files(cwd, pattern).map_err(|message| {
                pipeline_error(cwd, pipeline, format!("input `{pattern}`: {message}"))
            })?;
            files.extend(paths);
        }
    }

    Ok(files.into_iter().collect())
}

pub(crate) fn asset_pipeline_watch_roots(cwd: &Path, config: &ProjectConfig) -> Vec<PathBuf> {
    let mut roots = BTreeSet::new();

    for pipeline in &config.asset_pipelines {
        for input in &pipeline.inputs {
            if let Ok(relative) = safe_relative_pattern(input) {
                let text = display_path(&relative);
                let wildcard = text.contains('*') || text.contains('?');
                let absolute = cwd.join(&relative);
                roots.insert(if !wildcard && absolute.is_dir() {
                    absolute
                } else if !wildcard {
                    absolute.parent().unwrap_or(cwd).to_path_buf()
                } else {
                    cwd.join(pattern_anchor(&relative))
                });
            }
        }
    }

    roots.into_iter().collect()
}

fn run_one_pipeline(
    cwd: &Path,
    pipeline: &AssetPipelineConfig,
    outcome: &mut AssetPipelineOutcome,
) {
    if !valid_pipeline_name(&pipeline.name) {
        outcome.diagnostics.push(pipeline_error(
            cwd,
            pipeline,
            "name must contain only letters, digits, `.`, `-`, or `_`",
        ));
        return;
    }
    if pipeline.command.is_empty() {
        outcome
            .diagnostics
            .push(pipeline_error(cwd, pipeline, "command must not be empty"));
        return;
    }

    let input_files = match pipeline_inputs(cwd, pipeline) {
        Ok(files) => files,
        Err(diagnostics) => {
            outcome.diagnostics.extend(diagnostics);
            return;
        }
    };
    let cache_key = match pipeline_cache_key(cwd, pipeline, &input_files) {
        Ok(key) => key,
        Err(diagnostic) => {
            outcome.diagnostics.push(diagnostic);
            return;
        }
    };
    let pipeline_cache = cwd
        .join(".tua")
        .join("cache")
        .join("asset-pipelines")
        .join(&pipeline.name);
    let cache_entry = pipeline_cache.join(&cache_key);

    let manifest = match read_valid_cached_manifest(&cache_entry) {
        Ok(Some(manifest)) => {
            outcome.cached += 1;
            manifest
        }
        Ok(None) => {
            if let Err(diagnostic) = clear_cache_entry(&cache_entry) {
                outcome.diagnostics.push(diagnostic);
                return;
            }
            match execute_importer(cwd, pipeline, &input_files, &cache_key, &cache_entry) {
                Ok(manifest) => {
                    outcome.executed += 1;
                    manifest
                }
                Err(mut diagnostics) => {
                    outcome.diagnostics.append(&mut diagnostics);
                    return;
                }
            }
        }
        Err(diagnostic) => {
            if let Err(clear_diagnostic) = clear_cache_entry(&cache_entry) {
                outcome.diagnostics.push(diagnostic);
                outcome.diagnostics.push(clear_diagnostic);
                return;
            }
            match execute_importer(cwd, pipeline, &input_files, &cache_key, &cache_entry) {
                Ok(manifest) => {
                    outcome.executed += 1;
                    manifest
                }
                Err(mut diagnostics) => {
                    outcome.diagnostics.append(&mut diagnostics);
                    return;
                }
            }
        }
    };

    outcome
        .diagnostics
        .extend(manifest_diagnostics(cwd, pipeline, &manifest));

    if let Err(mut diagnostics) = materialize_outputs(cwd, pipeline, &cache_entry, &manifest) {
        outcome.diagnostics.append(&mut diagnostics);
    }
}

fn pipeline_inputs(
    cwd: &Path,
    pipeline: &AssetPipelineConfig,
) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
    let mut files = BTreeSet::new();
    let mut diagnostics = Vec::new();

    if pipeline.inputs.is_empty() {
        diagnostics.push(pipeline_error(
            cwd,
            pipeline,
            "inputs must contain at least one file, directory, or glob",
        ));
    }

    for pattern in &pipeline.inputs {
        match matching_input_files(cwd, pattern) {
            Ok(matches) if matches.is_empty() => diagnostics.push(pipeline_error(
                cwd,
                pipeline,
                format!("input pattern `{pattern}` matched no files"),
            )),
            Ok(matches) => files.extend(matches),
            Err(message) => diagnostics.push(pipeline_error(
                cwd,
                pipeline,
                format!("input `{pattern}`: {message}"),
            )),
        }
    }

    if diagnostics.is_empty() {
        Ok(files.into_iter().collect())
    } else {
        Err(diagnostics)
    }
}

fn pipeline_cache_key(
    cwd: &Path,
    pipeline: &AssetPipelineConfig,
    inputs: &[PathBuf],
) -> Result<String, Diagnostic> {
    let mut hash = StableHash::new();
    hash.add(b"tua-asset-pipeline-v1\0");
    hash.add(pipeline.name.as_bytes());
    hash.add(b"\0");
    hash.add(pipeline.version.as_bytes());
    hash.add(b"\0");
    hash.add(display_path(&pipeline.output).as_bytes());

    for argument in &pipeline.command {
        hash.add(b"\0command\0");
        hash.add(argument.as_bytes());
    }

    if let Some(program) = pipeline.command.first() {
        let program_path = cwd.join(program);
        if program_path.is_file() {
            hash_file_into(&program_path, &mut hash)
                .map_err(|source| io_diagnostic(&program_path, source))?;
        }
    }

    for path in inputs {
        hash.add(b"\0input\0");
        hash.add(
            path.strip_prefix(cwd)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/")
                .as_bytes(),
        );
        hash_file_into(path, &mut hash).map_err(|source| io_diagnostic(path, source))?;
    }

    Ok(hash.finish())
}

fn execute_importer(
    cwd: &Path,
    pipeline: &AssetPipelineConfig,
    inputs: &[PathBuf],
    cache_key: &str,
    cache_entry: &Path,
) -> Result<ImportManifest, Vec<Diagnostic>> {
    let parent = cache_entry
        .parent()
        .expect("cache entry should have a parent");
    if let Err(source) = fs::create_dir_all(parent) {
        return Err(vec![io_diagnostic(parent, source)]);
    }

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let staging = parent.join(format!(".{cache_key}.tmp-{}-{nonce}", std::process::id()));
    let files_dir = staging.join("files");
    let manifest_path = staging.join("manifest.json");
    if let Err(source) = fs::create_dir_all(&files_dir) {
        return Err(vec![io_diagnostic(&files_dir, source)]);
    }

    let input_strings = inputs
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let manifest_text = manifest_path.to_string_lossy().into_owned();
    let output_text = files_dir.to_string_lossy().into_owned();
    let project_text = cwd.to_string_lossy().into_owned();
    let mut arguments = Vec::new();

    for argument in pipeline.command.iter().skip(1) {
        if argument == "{inputs}" {
            arguments.extend(input_strings.iter().cloned());
        } else {
            arguments.push(
                argument
                    .replace("{manifest}", &manifest_text)
                    .replace("{output}", &output_text)
                    .replace("{project}", &project_text)
                    .replace("{cache_key}", cache_key),
            );
        }
    }

    let configured_program = &pipeline.command[0];
    let local_program = cwd.join(configured_program);
    let program = if configured_program.contains('/') || configured_program.contains('\\') {
        local_program
    } else {
        PathBuf::from(configured_program)
    };
    let command_output = Command::new(&program)
        .args(arguments)
        .current_dir(cwd)
        .env("TUA_ASSET_PIPELINE_NAME", &pipeline.name)
        .env("TUA_ASSET_PIPELINE_PROJECT_ROOT", cwd)
        .env("TUA_ASSET_PIPELINE_OUTPUT_DIR", &files_dir)
        .env("TUA_ASSET_PIPELINE_MANIFEST", &manifest_path)
        .env("TUA_ASSET_PIPELINE_CACHE_KEY", cache_key)
        .env(
            "TUA_ASSET_PIPELINE_INPUTS",
            serde_json::to_string(&input_strings).unwrap_or_else(|_| "[]".to_owned()),
        )
        .output();

    let command_output = match command_output {
        Ok(output) => output,
        Err(source) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(vec![pipeline_error(
                cwd,
                pipeline,
                format!("failed to start `{}`: {source}", display_path(&program)),
            )]);
        }
    };

    let mut diagnostics = match read_manifest(&manifest_path) {
        Ok(manifest) => manifest_diagnostics(cwd, pipeline, &manifest),
        Err(_) => Vec::new(),
    };

    if !command_output.status.success() {
        let stderr = String::from_utf8_lossy(&command_output.stderr)
            .trim()
            .to_owned();
        let suffix = if stderr.is_empty() {
            String::new()
        } else {
            format!(": {stderr}")
        };
        diagnostics.push(pipeline_error(
            cwd,
            pipeline,
            format!("importer exited with {}{suffix}", command_output.status),
        ));
        let _ = fs::remove_dir_all(&staging);
        return Err(diagnostics);
    }

    let manifest = match read_manifest(&manifest_path) {
        Ok(manifest) => manifest,
        Err(diagnostic) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(vec![diagnostic]);
        }
    };
    if let Err(diagnostic) = validate_manifest(cwd, pipeline, &staging, &manifest) {
        let _ = fs::remove_dir_all(&staging);
        return Err(vec![diagnostic]);
    }

    if cache_entry.exists() {
        let _ = fs::remove_dir_all(&staging);
        return read_manifest(&cache_entry.join("manifest.json")).map_err(|error| vec![error]);
    }
    if let Err(source) = fs::rename(&staging, cache_entry) {
        let diagnostic = io_diagnostic(cache_entry, source);
        let _ = fs::remove_dir_all(&staging);
        return Err(vec![diagnostic]);
    }

    Ok(manifest)
}

fn read_valid_cached_manifest(cache_entry: &Path) -> Result<Option<ImportManifest>, Diagnostic> {
    if !cache_entry.is_dir() {
        return Ok(None);
    }
    let manifest = read_manifest(&cache_entry.join("manifest.json"))?;
    if manifest.version != MANIFEST_VERSION {
        return Ok(None);
    }
    for output in &manifest.outputs {
        let Ok(relative) = safe_relative_path(output) else {
            return Ok(None);
        };
        let path = cache_entry.join("files").join(relative);
        if !fs::symlink_metadata(path)
            .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
        {
            return Ok(None);
        }
    }
    Ok(Some(manifest))
}

fn clear_cache_entry(cache_entry: &Path) -> Result<(), Diagnostic> {
    match fs::symlink_metadata(cache_entry) {
        Ok(metadata) if metadata.file_type().is_symlink() || metadata.is_file() => {
            fs::remove_file(cache_entry).map_err(|source| io_diagnostic(cache_entry, source))
        }
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir_all(cache_entry).map_err(|source| io_diagnostic(cache_entry, source))
        }
        Ok(_) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(io_diagnostic(cache_entry, source)),
    }
}

fn read_manifest(path: &Path) -> Result<ImportManifest, Diagnostic> {
    let source = fs::read_to_string(path).map_err(|source| io_diagnostic(path, source))?;
    serde_json::from_str(&source).map_err(|source| {
        Diagnostic::error(path, source.line(), source.column(), source.to_string())
            .with_code(DiagnosticCode::ASSET_PIPELINE)
    })
}

fn validate_manifest(
    cwd: &Path,
    pipeline: &AssetPipelineConfig,
    staging: &Path,
    manifest: &ImportManifest,
) -> Result<(), Diagnostic> {
    if manifest.version != MANIFEST_VERSION {
        return Err(pipeline_error(
            cwd,
            pipeline,
            format!(
                "manifest version {} is unsupported; expected {MANIFEST_VERSION}",
                manifest.version
            ),
        ));
    }

    let mut outputs = BTreeSet::new();
    for output in &manifest.outputs {
        let relative = safe_relative_path(output).map_err(|message| {
            pipeline_error(
                cwd,
                pipeline,
                format!("manifest output `{output}` {message}"),
            )
        })?;
        if !outputs.insert(relative.clone()) {
            return Err(pipeline_error(
                cwd,
                pipeline,
                format!("manifest output `{output}` is duplicated"),
            ));
        }
        let generated_path = staging.join("files").join(&relative);
        let generated_is_regular_file = fs::symlink_metadata(&generated_path)
            .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink());
        if !generated_is_regular_file {
            return Err(pipeline_error(
                cwd,
                pipeline,
                format!("manifest output `{output}` was not written by the importer"),
            ));
        }
    }
    Ok(())
}

fn materialize_outputs(
    cwd: &Path,
    pipeline: &AssetPipelineConfig,
    cache_entry: &Path,
    manifest: &ImportManifest,
) -> Result<(), Vec<Diagnostic>> {
    let output_root = cwd.join(&pipeline.output);
    let state_path = cache_entry
        .parent()
        .expect("cache entry should have parent")
        .join("materialized.json");
    let previous = fs::read_to_string(&state_path)
        .ok()
        .and_then(|source| serde_json::from_str::<MaterializedState>(&source).ok())
        .unwrap_or_default();
    let current = manifest.outputs.iter().cloned().collect::<BTreeSet<_>>();
    let previously_owned = previous.outputs.iter().cloned().collect::<BTreeSet<_>>();
    let mut diagnostics = Vec::new();

    for stale in previous
        .outputs
        .iter()
        .filter(|output| !current.contains(*output))
    {
        let Ok(relative) = safe_relative_path(stale) else {
            continue;
        };
        let path = output_root.join(relative);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) => diagnostics.push(io_diagnostic(&path, source)),
        }
    }

    for output in &manifest.outputs {
        let relative = match safe_relative_path(output) {
            Ok(path) => path,
            Err(message) => {
                diagnostics.push(pipeline_error(
                    cwd,
                    pipeline,
                    format!("manifest output `{output}` {message}"),
                ));
                continue;
            }
        };
        let source = cache_entry.join("files").join(&relative);
        let destination = output_root.join(&relative);
        if let Err(message) = reject_symlink_path(cwd, &destination) {
            diagnostics.push(pipeline_error(cwd, pipeline, message));
            continue;
        }
        if destination.is_file() && !previously_owned.contains(output) {
            let same_content = fs::read(&source)
                .ok()
                .zip(fs::read(&destination).ok())
                .is_some_and(|(left, right)| left == right);
            if !same_content {
                diagnostics.push(pipeline_error(
                    cwd,
                    pipeline,
                    format!(
                        "generated output `{}` would overwrite an unowned source file",
                        display_path(&destination)
                    ),
                ));
                continue;
            }
        }
        if let Some(parent) = destination.parent() {
            if let Err(source) = fs::create_dir_all(parent) {
                diagnostics.push(io_diagnostic(parent, source));
                continue;
            }
        }
        if let Err(source) = fs::copy(&source, &destination) {
            diagnostics.push(io_diagnostic(&destination, source));
        }
    }

    if diagnostics.is_empty() {
        let state = MaterializedState {
            outputs: manifest.outputs.clone(),
        };
        if let Some(parent) = state_path.parent() {
            if let Err(source) = fs::create_dir_all(parent) {
                return Err(vec![io_diagnostic(parent, source)]);
            }
        }
        let source = serde_json::to_string_pretty(&state).expect("state should serialize");
        if let Err(error) = fs::write(&state_path, format!("{source}\n")) {
            diagnostics.push(io_diagnostic(&state_path, error));
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn manifest_diagnostics(
    cwd: &Path,
    pipeline: &AssetPipelineConfig,
    manifest: &ImportManifest,
) -> Vec<Diagnostic> {
    manifest
        .diagnostics
        .iter()
        .map(|item| {
            let severity = match item.severity.as_str() {
                "error" => DiagnosticSeverity::Error,
                "warning" => DiagnosticSeverity::Warning,
                "info" => DiagnosticSeverity::Info,
                "hint" => DiagnosticSeverity::Hint,
                _ => DiagnosticSeverity::Error,
            };
            let path = safe_relative_path(&item.path)
                .ok()
                .map(|path| cwd.join(path))
                .unwrap_or_else(|| cwd.join("tua.toml"));
            let mut diagnostic = Diagnostic::new(
                path,
                item.line,
                item.column,
                severity,
                format!("asset pipeline `{}`: {}", pipeline.name, item.message),
            )
            .with_code(
                item.code
                    .and_then(DiagnosticCode::new)
                    .unwrap_or(DiagnosticCode::ASSET_PIPELINE),
            );
            if let (Some(end_line), Some(end_column)) = (item.end_line, item.end_column) {
                diagnostic = diagnostic.with_range(item.line, item.column, end_line, end_column);
            }
            diagnostic
        })
        .collect()
}

fn validated_output_root(
    cwd: &Path,
    config: &ProjectConfig,
    pipeline: &AssetPipelineConfig,
) -> Result<PathBuf, Diagnostic> {
    let output = safe_relative_path(&pipeline.output.to_string_lossy()).map_err(|message| {
        pipeline_error(
            cwd,
            pipeline,
            format!("output `{}` {message}", display_path(&pipeline.output)),
        )
    })?;
    let source = safe_relative_path(&config.source.to_string_lossy())
        .map_err(|message| pipeline_error(cwd, pipeline, format!("source root {message}")))?;
    let output = cwd.join(output);
    let source = cwd.join(source);
    if !output.starts_with(&source) || output == source {
        return Err(pipeline_error(
            cwd,
            pipeline,
            format!(
                "output `{}` must be a dedicated directory below source root `{}`",
                display_path(&pipeline.output),
                display_path(&config.source)
            ),
        ));
    }
    Ok(output)
}

fn matching_input_files(cwd: &Path, pattern: &str) -> Result<Vec<PathBuf>, String> {
    let relative = safe_relative_pattern(pattern)?;
    let pattern_text = display_path(&relative);
    let has_wildcard = pattern_text.contains('*') || pattern_text.contains('?');
    let absolute = cwd.join(&relative);
    let mut files = Vec::new();

    if !has_wildcard {
        if absolute.is_file() {
            return Ok(vec![absolute]);
        }
        if absolute.is_dir() {
            discover_regular_files(&absolute, &mut files).map_err(|error| error.to_string())?;
            files.sort();
            return Ok(files);
        }
        return Ok(Vec::new());
    }

    let anchor = cwd.join(pattern_anchor(&relative));
    if !anchor.exists() {
        return Ok(Vec::new());
    }
    discover_regular_files(&anchor, &mut files).map_err(|error| error.to_string())?;
    files.retain(|path| {
        let candidate = path
            .strip_prefix(cwd)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        glob_matches(&pattern_text, &candidate)
    });
    files.sort();
    Ok(files)
}

fn discover_regular_files(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| matches!(name, ".git" | ".hg" | ".svn"))
            {
                continue;
            }
            discover_regular_files(&path, files)?;
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn glob_matches(pattern: &str, value: &str) -> bool {
    fn matches_from(
        pattern: &[u8],
        value: &[u8],
        pi: usize,
        vi: usize,
        memo: &mut BTreeMap<(usize, usize), bool>,
    ) -> bool {
        if let Some(result) = memo.get(&(pi, vi)) {
            return *result;
        }
        let result = if pi == pattern.len() {
            vi == value.len()
        } else if pattern[pi] == b'*' {
            let double = pattern.get(pi + 1) == Some(&b'*');
            let next = pi + if double { 2 } else { 1 };
            matches_from(pattern, value, next, vi, memo)
                || (double
                    && pattern.get(next) == Some(&b'/')
                    && matches_from(pattern, value, next + 1, vi, memo))
                || (vi < value.len()
                    && (double || value[vi] != b'/')
                    && matches_from(pattern, value, pi, vi + 1, memo))
        } else if vi < value.len()
            && (pattern[pi] == b'?' && value[vi] != b'/' || pattern[pi] == value[vi])
        {
            matches_from(pattern, value, pi + 1, vi + 1, memo)
        } else {
            false
        };
        memo.insert((pi, vi), result);
        result
    }

    matches_from(
        pattern.as_bytes(),
        value.as_bytes(),
        0,
        0,
        &mut BTreeMap::new(),
    )
}

fn pattern_anchor(pattern: &Path) -> PathBuf {
    let mut anchor = PathBuf::new();
    for component in pattern.components() {
        let text = component.as_os_str().to_string_lossy();
        if text.contains('*') || text.contains('?') {
            break;
        }
        anchor.push(component.as_os_str());
    }
    if anchor.as_os_str().is_empty() {
        PathBuf::from(".")
    } else if pattern == anchor {
        anchor.parent().unwrap_or(Path::new(".")).to_path_buf()
    } else {
        anchor
    }
}

fn safe_relative_pattern(value: &str) -> Result<PathBuf, String> {
    safe_relative(value, true)
}

fn safe_relative_path(value: &str) -> Result<PathBuf, String> {
    safe_relative(value, false)
}

fn safe_relative(value: &str, allow_wildcards: bool) -> Result<PathBuf, String> {
    if value.trim().is_empty() {
        return Err("must not be empty".to_owned());
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return Err("must be relative to the project root".to_owned());
    }
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                let text = value.to_string_lossy();
                if !allow_wildcards && (text.contains('*') || text.contains('?')) {
                    return Err("must not contain wildcards".to_owned());
                }
                result.push(value);
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err("must not escape the project root".to_owned());
            }
        }
    }
    if result.as_os_str().is_empty() {
        Err("must not resolve to the project root".to_owned())
    } else {
        Ok(result)
    }
}

fn reject_symlink_path(root: &Path, target: &Path) -> Result<(), String> {
    let relative = target
        .strip_prefix(root)
        .map_err(|_| "generated output escaped the project root".to_owned())?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "generated output path `{}` crosses a symbolic link",
                    display_path(&current)
                ));
            }
            Ok(_) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(format!(
                    "cannot inspect generated output path `{}`: {source}",
                    display_path(&current)
                ));
            }
        }
    }
    Ok(())
}

fn hash_file_into(path: &Path, hash: &mut StableHash) -> io::Result<()> {
    let mut file = File::open(path)?;
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok(());
        }
        hash.add(&buffer[..read]);
    }
}

struct StableHash {
    left: u64,
    right: u64,
}

impl StableHash {
    fn new() -> Self {
        Self {
            left: FNV_OFFSET_BASIS,
            right: FNV_OFFSET_BASIS ^ 0x9e3779b97f4a7c15,
        }
    }

    fn add(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.left ^= u64::from(*byte);
            self.left = self.left.wrapping_mul(FNV_PRIME);
            self.right ^= u64::from(*byte).rotate_left(1);
            self.right = self.right.wrapping_mul(FNV_PRIME ^ 0x100);
        }
    }

    fn finish(&self) -> String {
        format!("{:016x}{:016x}", self.left, self.right)
    }
}

fn valid_pipeline_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn pipeline_error(
    cwd: &Path,
    pipeline: &AssetPipelineConfig,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic::error(
        cwd.join("tua.toml"),
        1,
        1,
        format!("asset pipeline `{}`: {}", pipeline.name, message.into()),
    )
    .with_code(DiagnosticCode::ASSET_PIPELINE)
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_matching_supports_recursive_and_component_wildcards() {
        assert!(glob_matches("maps/**/*.json", "maps/world/level.json"));
        assert!(glob_matches("maps/**/*.json", "maps/level.json"));
        assert!(glob_matches("assets/?.png", "assets/a.png"));
        assert!(!glob_matches("assets/*.png", "assets/ui/a.png"));
    }

    #[test]
    fn generated_paths_cannot_escape_their_owned_root() {
        assert!(safe_relative_path("types/game.d.tua").is_ok());
        assert!(safe_relative_path("../main.tua").is_err());
        assert!(safe_relative_path("/tmp/main.tua").is_err());
    }
}

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tua_core::{Diagnostic, LuaLineProvenance};

use crate::error::CliError;
use crate::source::io_diagnostic;

const SOURCE_MAP_VERSION: u64 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TracebackSourceMap {
    pub(crate) generated: String,
    pub(crate) source: String,
    pub(crate) lines: BTreeMap<usize, usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LineMapping {
    generated_line: usize,
    source_line: usize,
}

pub(crate) fn source_map_path_for(output_path: &Path) -> PathBuf {
    output_path.with_extension("lua.map.json")
}

pub(crate) fn write_source_map(
    cwd: &Path,
    source_path: &Path,
    output_path: &Path,
    provenance: &[LuaLineProvenance],
) -> Result<(), Diagnostic> {
    let map = source_map_json(cwd, source_path, output_path, line_mappings(provenance));
    let map_path = source_map_path_for(output_path);
    let serialized = serde_json::to_string_pretty(&map)
        .expect("source map JSON should serialize from literal values");

    if let Some(parent) = map_path.parent() {
        fs::create_dir_all(parent).map_err(|source| io_diagnostic(parent, source))?;
    }

    fs::write(&map_path, format!("{serialized}\n"))
        .map_err(|source| io_diagnostic(&map_path, source))
}

pub(crate) fn load_traceback_maps(path: &Path) -> Result<Vec<TracebackSourceMap>, CliError> {
    let mut paths = Vec::new();

    collect_source_map_paths(path, &mut paths)?;
    paths.sort();

    paths
        .into_iter()
        .map(|path| load_traceback_map(&path))
        .collect()
}

fn collect_source_map_paths(path: &Path, paths: &mut Vec<PathBuf>) -> Result<(), CliError> {
    if path.is_file() {
        if is_source_map_path(path) {
            paths.push(path.to_path_buf());
        }

        return Ok(());
    }

    if !path.exists() {
        return Err(CliError::usage(format!(
            "source map path not found: {}",
            display_path(path)
        )));
    }

    let entries = fs::read_dir(path).map_err(|source| {
        CliError::usage(format!(
            "failed to read source map directory `{}`: {source}",
            display_path(path)
        ))
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| {
            CliError::usage(format!(
                "failed to read source map directory `{}`: {source}",
                display_path(path)
            ))
        })?;
        let entry_path = entry.path();
        let file_type = entry.file_type().map_err(|source| {
            CliError::usage(format!(
                "failed to inspect source map path `{}`: {source}",
                display_path(&entry_path)
            ))
        })?;

        if file_type.is_dir() {
            collect_source_map_paths(&entry_path, paths)?;
        } else if file_type.is_file() && is_source_map_path(&entry_path) {
            paths.push(entry_path);
        }
    }

    Ok(())
}

fn load_traceback_map(path: &Path) -> Result<TracebackSourceMap, CliError> {
    let contents = fs::read_to_string(path).map_err(|source| {
        CliError::usage(format!(
            "failed to read source map `{}`: {source}",
            display_path(path)
        ))
    })?;
    let value = serde_json::from_str::<Value>(&contents).map_err(|source| {
        CliError::usage(format!(
            "failed to parse source map `{}`: {source}",
            display_path(path)
        ))
    })?;

    parse_traceback_map(path, value)
}

fn parse_traceback_map(path: &Path, value: Value) -> Result<TracebackSourceMap, CliError> {
    if value.get("version").and_then(Value::as_u64) != Some(SOURCE_MAP_VERSION) {
        return Err(invalid_source_map(
            path,
            "unsupported or missing source map version",
        ));
    }

    let generated = value
        .get("generated")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_source_map(path, "missing generated path"))?
        .to_owned();
    let source = value
        .get("source")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_source_map(path, "missing source path"))?
        .to_owned();
    let mappings = value
        .get("lineMappings")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_source_map(path, "missing lineMappings"))?;
    let mut lines = BTreeMap::new();

    for mapping in mappings {
        let generated_line = mapping
            .get("generatedLine")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid_source_map(path, "line mapping is missing generatedLine"))?
            as usize;
        let source_line = mapping
            .get("sourceLine")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid_source_map(path, "line mapping is missing sourceLine"))?
            as usize;

        lines.insert(generated_line, source_line);
    }

    Ok(TracebackSourceMap {
        generated,
        source,
        lines,
    })
}

fn invalid_source_map(path: &Path, message: &str) -> CliError {
    CliError::usage(format!(
        "invalid source map `{}`: {message}",
        display_path(path)
    ))
}

fn source_map_json(
    cwd: &Path,
    source_path: &Path,
    output_path: &Path,
    mappings: Vec<LineMapping>,
) -> Value {
    let line_mappings = mappings
        .into_iter()
        .map(|mapping| {
            json!({
                "generatedLine": mapping.generated_line,
                "sourceLine": mapping.source_line,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "version": SOURCE_MAP_VERSION,
        "generated": project_relative_path(cwd, output_path),
        "source": project_relative_path(cwd, source_path),
        "lineMappings": line_mappings,
    })
}

fn line_mappings(provenance: &[LuaLineProvenance]) -> Vec<LineMapping> {
    provenance
        .iter()
        .map(|line| LineMapping {
            generated_line: line.generated_line,
            source_line: line.source_span.start.line,
        })
        .collect()
}

fn project_relative_path(cwd: &Path, path: &Path) -> String {
    path.strip_prefix(cwd)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn is_source_map_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".lua.map.json"))
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests;

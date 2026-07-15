use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tua_core::{parse_tua_source, Diagnostic, ParsedSource, SourceSpan};

use crate::error::CliError;
use crate::source::io_diagnostic;

const SOURCE_MAP_VERSION: u64 = 1;
const VECTOR_RUNTIME_REQUIRE: &str = "local __tua_vec2 = require(\"brinevector\")";
const SIGNAL_RUNTIME_REQUIRE: &str = "local __tua_signal = require(\"tuasignal\")";

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

#[derive(Clone, Debug, Eq, PartialEq)]
struct DefaultGuard {
    name: String,
    source_line: usize,
}

pub(crate) fn source_map_path_for(output_path: &Path) -> PathBuf {
    output_path.with_extension("lua.map.json")
}

pub(crate) fn write_source_map(
    cwd: &Path,
    source_path: &Path,
    output_path: &Path,
    source: &str,
    output: &str,
) -> Result<(), Diagnostic> {
    let parsed = parse_tua_source(source_path, source).ok();
    let map = source_map_json(
        cwd,
        source_path,
        output_path,
        line_mappings(source, output, parsed.as_ref()),
    );
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

fn line_mappings(source: &str, output: &str, parsed: Option<&ParsedSource>) -> Vec<LineMapping> {
    let source_lines = source.lines().collect::<Vec<_>>();
    let skipped_source_lines = parsed.map(skipped_lines).unwrap_or_default();
    let mut default_guards = parsed.map(default_guards).unwrap_or_default();
    let vector_source_line = first_source_line_containing(&source_lines, "vec2(").unwrap_or(1);
    let signal_source_line = first_source_line_containing(&source_lines, "signal(").unwrap_or(1);
    let mut active_default_guard = None::<(usize, usize)>;
    let mut source_line = 1usize;
    let mut mappings = Vec::new();

    for (index, generated_line) in output.lines().enumerate() {
        let generated_line_number = index + 1;
        let trimmed = generated_line.trim_start();

        if trimmed.starts_with("---@") {
            mappings.push(LineMapping {
                generated_line: generated_line_number,
                source_line: nearest_source_line(source_line, source_lines.len()),
            });
            continue;
        }

        if trimmed == VECTOR_RUNTIME_REQUIRE {
            mappings.push(LineMapping {
                generated_line: generated_line_number,
                source_line: vector_source_line,
            });
            continue;
        }

        if trimmed == SIGNAL_RUNTIME_REQUIRE {
            mappings.push(LineMapping {
                generated_line: generated_line_number,
                source_line: signal_source_line,
            });
            continue;
        }

        if let Some((guard_source_line, remaining)) = active_default_guard.take() {
            mappings.push(LineMapping {
                generated_line: generated_line_number,
                source_line: guard_source_line,
            });

            if remaining > 1 {
                active_default_guard = Some((guard_source_line, remaining - 1));
            }

            continue;
        }

        if let Some(guard) = pop_matching_default_guard(trimmed, &mut default_guards) {
            mappings.push(LineMapping {
                generated_line: generated_line_number,
                source_line: guard.source_line,
            });
            active_default_guard = Some((guard.source_line, 2));
            continue;
        }

        source_line = next_source_line_for_generated(
            &source_lines,
            &skipped_source_lines,
            source_line,
            generated_line,
        );
        mappings.push(LineMapping {
            generated_line: generated_line_number,
            source_line: nearest_source_line(source_line, source_lines.len()),
        });
        source_line += 1;
    }

    mappings
}

fn skipped_lines(parsed: &ParsedSource) -> BTreeSet<usize> {
    let mut lines = erased_type_syntax_lines(&parsed.source);

    for alias in &parsed.type_aliases {
        if let Some(span) = alias.span {
            push_span_lines(&mut lines, span);
        }
    }

    lines
}

fn erased_type_syntax_lines(source: &str) -> BTreeSet<usize> {
    let mut lines = BTreeSet::new();
    let mut type_brace_depth = 0isize;
    let mut in_enum = false;

    for (index, line) in source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();

        if in_enum {
            lines.insert(line_number);

            if trimmed == "end" {
                in_enum = false;
            }

            continue;
        }

        if type_brace_depth > 0 {
            lines.insert(line_number);
            type_brace_depth += brace_delta(trimmed);

            if type_brace_depth <= 0 {
                type_brace_depth = 0;
            }

            continue;
        }

        if trimmed.starts_with("enum ") {
            lines.insert(line_number);
            in_enum = true;
            continue;
        }

        if trimmed.starts_with("type ") || trimmed.starts_with("export type ") {
            lines.insert(line_number);
            type_brace_depth = brace_delta(trimmed).max(0);
        }
    }

    lines
}

fn brace_delta(line: &str) -> isize {
    line.chars()
        .map(|character| match character {
            '{' => 1,
            '}' => -1,
            _ => 0,
        })
        .sum()
}

fn push_span_lines(lines: &mut BTreeSet<usize>, span: SourceSpan) {
    for line in span.start.line..=span.end.line.max(span.start.line) {
        lines.insert(line);
    }
}

fn default_guards(parsed: &ParsedSource) -> VecDeque<DefaultGuard> {
    parsed
        .function_annotations
        .iter()
        .flat_map(|function| {
            function
                .default_parameters
                .iter()
                .map(|default| DefaultGuard {
                    name: default.name.clone(),
                    source_line: default
                        .span
                        .map(|span| span.start.line)
                        .or(function.span.map(|span| span.start.line))
                        .unwrap_or(1),
                })
        })
        .collect()
}

fn pop_matching_default_guard(
    generated_line: &str,
    guards: &mut VecDeque<DefaultGuard>,
) -> Option<DefaultGuard> {
    let guard = guards.front()?;
    let expected = format!("if {} == nil then", guard.name);

    if generated_line.trim() == expected {
        guards.pop_front()
    } else {
        None
    }
}

fn next_source_line_for_generated(
    source_lines: &[&str],
    skipped_source_lines: &BTreeSet<usize>,
    mut source_line: usize,
    generated_line: &str,
) -> usize {
    while source_line <= source_lines.len() {
        let current = source_lines[source_line - 1];

        if skipped_source_lines.contains(&source_line) {
            source_line += 1;
            continue;
        }

        if !generated_line.trim().is_empty() && current.trim().is_empty() {
            source_line += 1;
            continue;
        }

        return source_line;
    }

    nearest_source_line(source_line, source_lines.len())
}

fn nearest_source_line(source_line: usize, source_line_count: usize) -> usize {
    source_line.clamp(1, source_line_count.max(1))
}

fn first_source_line_containing(source_lines: &[&str], needle: &str) -> Option<usize> {
    source_lines
        .iter()
        .position(|line| line.contains(needle))
        .map(|index| index + 1)
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

//! Source-mapped LuaCov line coverage. Generated helper lines have no source entry.
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde_json::json;
use tua_core::ProjectWorkspace;

use crate::error::CliError;
use crate::source_map::load_traceback_maps;
use crate::traceback::lookup_source_location;

pub(super) fn initialize(root: &Path, destination: &Path) -> Result<(), CliError> {
    write(
        root,
        destination,
        &json!({"version": 1, "completed": false, "files": []}),
    )
}

pub(super) fn save(
    workspace: &ProjectWorkspace,
    output: &Path,
    raw: &Path,
    destination: &Path,
) -> Result<(), CliError> {
    let bytes = fs::read(raw).map_err(|error| CliError::usage(format!(
        "LuaCov did not produce coverage: {error}. Install LuaCov for the Busted interpreter with `luarocks install luacov`"
    )))?;
    let report: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| CliError::usage(format!("invalid LuaCov report: {error}")))?;
    if let Some(errors) = report["errors"]
        .as_array()
        .filter(|errors| !errors.is_empty())
    {
        return Err(CliError::usage(format!("LuaCov report errors: {errors:?}")));
    }
    if !report["files"].is_array() || !report["errors"].is_array() {
        return Err(CliError::usage(
            "invalid LuaCov report: expected files and errors arrays",
        ));
    }
    let maps = load_traceback_maps(output)?;
    let mut files = BTreeMap::<_, BTreeMap<usize, u64>>::new();
    for file in report["files"].as_array().into_iter().flatten() {
        let Some(path) = file["path"].as_str() else {
            continue;
        };
        let generated = tua_core::normalize_path(output.join(path));
        let Ok(relative) = generated.strip_prefix(output) else {
            continue;
        };
        for row in file["lines"].as_array().into_iter().flatten() {
            let (Some(line), Some(hits)) = (row["line"].as_u64(), row["hits"].as_u64()) else {
                continue;
            };
            let mapped = lookup_source_location(&maps, &generated.to_string_lossy(), line as usize);
            let (source, line) = if let Some((source, line)) = mapped {
                (workspace.root().join(source), line)
            } else {
                let source = if let (Some(root), Ok(relative)) = (
                    workspace.test_source_root(),
                    relative.strip_prefix("__tua_tests__"),
                ) {
                    root.join(relative)
                } else {
                    workspace.primary_source_root().join(relative)
                };
                // Only copied Lua can fall back to identity mapping. A generated
                // native helper line must never become a fabricated source line.
                if !source.is_file() || source.extension().is_none_or(|ext| ext != "lua") {
                    continue;
                }
                (source, line as usize)
            };
            let value = files.entry(source).or_default().entry(line).or_default();
            // Several generated lines may represent the same native line.
            *value = (*value).max(hits);
        }
    }
    let files = files
        .into_iter()
        .map(|(path, lines)| {
            let covered = lines.values().filter(|hits| **hits > 0).count();
            let total = lines.len();
            let lines = lines
                .into_iter()
                .map(|(line, hits)| json!({"line": line, "hits": hits}))
                .collect::<Vec<_>>();
            json!({"path": path, "covered": covered, "total": total, "lines": lines})
        })
        .collect::<Vec<_>>();
    write(
        workspace.root(),
        destination,
        &json!({"version": 1, "completed": true, "files": files}),
    )
}

fn write(root: &Path, destination: &Path, report: &serde_json::Value) -> Result<(), CliError> {
    let path = root.join(destination);
    let bytes =
        serde_json::to_vec_pretty(report).map_err(|error| CliError::usage(error.to_string()))?;
    fs::write(&path, bytes).map_err(|error| {
        CliError::usage(format!(
            "cannot write coverage report {}: {error}",
            path.display()
        ))
    })
}

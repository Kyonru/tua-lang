use crate::error::CliError;
use crate::source_map::load_traceback_maps;
use crate::traceback::{lookup_source_location, remap_traceback_text};
use std::fs;
use std::path::Path;

pub(super) fn save(
    workspace: &tua_core::ProjectWorkspace,
    output: &Path,
    raw_report: &Path,
    destination: &Path,
    success: bool,
    log: &str,
) -> Result<(), CliError> {
    let cwd = workspace.root();
    let raw = fs::read(raw_report).map_err(|error| {
        CliError::usage(format!(
            "Busted did not produce a test report: {error}\n{log}"
        ))
    })?;
    let mut report: serde_json::Value = serde_json::from_slice(&raw)
        .map_err(|error| CliError::usage(format!("invalid Busted report: {error}\n{log}")))?;
    let maps = load_traceback_maps(output)?;
    if let Some(tests) = report["tests"].as_array_mut() {
        for test in tests {
            if let (Some(file), Some(line)) = (test["file"].as_str(), test["line"].as_u64()) {
                if let Some((source, line)) =
                    lookup_source_location(&maps, file.trim_start_matches('@'), line as usize)
                {
                    test["file"] = serde_json::json!(cwd.join(source));
                    test["line"] = serde_json::json!(line);
                } else {
                    let generated = Path::new(file.trim_start_matches('@'));
                    let relative = if generated.is_absolute() {
                        generated.strip_prefix(output).ok()
                    } else {
                        Some(generated)
                    };
                    if let Some(relative) = relative {
                        let source = if let (Some(root), Ok(relative)) = (
                            workspace.test_source_root(),
                            relative.strip_prefix("__tua_tests__"),
                        ) {
                            root.join(relative)
                        } else {
                            workspace.primary_source_root().join(relative)
                        };
                        if source.is_file() {
                            test["file"] = serde_json::json!(source);
                        }
                    }
                }
            }
            if let Some(message) = test["message"].as_str() {
                test["message"] = serde_json::json!(remap_traceback_text(message, &maps));
            }
        }
    }
    if report["tests"]
        .as_object()
        .is_some_and(|object| object.is_empty())
    {
        report["tests"] = serde_json::json!([]);
    }
    report["completed"] = serde_json::json!(true);
    report["success"] = serde_json::json!(success);
    report["output"] = serde_json::json!(log);
    let path = cwd.join(destination);
    let bytes = serde_json::to_vec_pretty(&report).map_err(|e| CliError::usage(e.to_string()))?;
    fs::write(&path, bytes).map_err(|error| {
        CliError::usage(format!(
            "cannot write test report {}: {error}",
            path.display()
        ))
    })
}

pub(super) fn initialize(cwd: &Path, destination: &Path) -> Result<(), CliError> {
    let path = cwd.join(destination);
    fs::write(
        &path,
        r#"{"version":1,"completed":false,"success":false,"tests":[]}"#,
    )
    .map_err(|error| {
        CliError::usage(format!(
            "cannot initialize test report {}: {error}",
            path.display()
        ))
    })
}

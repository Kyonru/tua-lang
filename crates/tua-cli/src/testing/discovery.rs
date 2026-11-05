use std::path::Path;
use tua_core::ProjectWorkspace;

pub(super) fn list(workspace: &ProjectWorkspace, paths: &[&Path]) -> serde_json::Value {
    let root = workspace
        .test_source_root()
        .unwrap_or_else(|| workspace.primary_source_root().to_owned());
    let files = paths.iter().map(|path| {
        let parsed = crate::source::read_source_file(path).map_err(|error| vec![error]).and_then(|source| {
            if path.extension().is_some_and(|ext| ext == "lua") {
                tua_core::parse_lua_source(path, source)
            } else { tua_core::parse_tua_source(path, source) }
        });
        let (cases, error) = match parsed {
            Ok(parsed) => (tua_core::testing::discover_test_cases(&parsed), None),
            Err(diagnostics) => (Vec::new(), Some(tua_core::format_diagnostics_for_cli(&diagnostics))),
        };
        serde_json::json!({"path": path, "selector": path.strip_prefix(&root).unwrap_or(path), "cases": cases, "error": error})
    }).collect::<Vec<_>>();
    serde_json::json!({"version": 1, "root": workspace.root(), "test_root": root, "files": files})
}

use std::path::{Path, PathBuf};

use serde_json::Value;
use tua_core::{file_uri_from_path, path_from_file_uri};

pub(super) fn workspace_folders_from_initialize(message: &Value) -> Vec<PathBuf> {
    let Some(params) = message.get("params") else {
        return Vec::new();
    };
    let folders = workspace_folders(params);

    if !folders.is_empty() {
        return folders;
    }

    params
        .get("rootUri")
        .and_then(Value::as_str)
        .map(path_from_uri)
        .or_else(|| {
            params
                .get("rootPath")
                .and_then(Value::as_str)
                .map(PathBuf::from)
        })
        .into_iter()
        .collect()
}

pub(super) fn workspace_folders_from_change(message: &Value, key: &str) -> Vec<PathBuf> {
    message
        .get("params")
        .and_then(|params| params.get("event"))
        .and_then(|event| event.get(key))
        .and_then(workspace_folders_from_value)
        .unwrap_or_default()
}

fn workspace_folders(params: &Value) -> Vec<PathBuf> {
    params
        .get("workspaceFolders")
        .and_then(workspace_folders_from_value)
        .unwrap_or_default()
}

fn workspace_folders_from_value(value: &Value) -> Option<Vec<PathBuf>> {
    Some(
        value
            .as_array()?
            .iter()
            .filter_map(|folder| folder.get("uri").and_then(Value::as_str).map(path_from_uri))
            .collect(),
    )
}

pub(super) fn uri_from_path(path: &Path) -> String {
    file_uri_from_path(path)
}

pub(super) fn path_from_uri(uri: &str) -> PathBuf {
    path_from_file_uri(uri)
}

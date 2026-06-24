use std::path::{Path, PathBuf};

use serde_json::Value;

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
    if let Some(uri) = path.to_str().filter(|path| path.starts_with("tua:/")) {
        return uri.to_owned();
    }

    format!("file://{}", percent_encode_path(path))
}

pub(super) fn path_from_uri(uri: &str) -> PathBuf {
    let Some(raw_path) = uri.strip_prefix("file://") else {
        return PathBuf::from(uri);
    };
    let raw_path = raw_path.strip_prefix("localhost").unwrap_or(raw_path);

    PathBuf::from(percent_decode(raw_path))
}

fn percent_encode_path(path: &Path) -> String {
    let mut encoded = String::new();

    for byte in path.to_string_lossy().bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }

    encoded
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = &input[index + 1..index + 3];

            if let Ok(value) = u8::from_str_radix(hex, 16) {
                output.push(value);
                index += 3;
                continue;
            }
        }

        output.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&output).into_owned()
}

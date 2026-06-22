use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::{Completion, CompletionKind};

pub(super) fn complete_require_paths(path: &Path, prefix: &str) -> Vec<Completion> {
    let Some(base_dir) = path.parent() else {
        return Vec::new();
    };
    let mut completions = BTreeMap::new();
    let current_path = normalize_path(path);

    collect_require_paths(base_dir, base_dir, &current_path, &mut completions);

    completions
        .into_values()
        .filter(|completion| completion.label.starts_with(prefix))
        .collect()
}

fn collect_require_paths(
    base_dir: &Path,
    dir: &Path,
    current_path: &Path,
    completions: &mut BTreeMap<String, Completion>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries = entries.filter_map(Result::ok).collect::<Vec<_>>();

    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            collect_require_paths(base_dir, &path, current_path, completions);
            continue;
        }

        if !file_type.is_file() || normalize_path(&path) == current_path {
            continue;
        }

        if !matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("lua" | "tua")
        ) {
            continue;
        }

        let Some(module_name) = module_name(base_dir, &path) else {
            continue;
        };

        completions.insert(
            module_name.clone(),
            Completion {
                label: module_name,
                kind: CompletionKind::Module,
                detail: Some("module".to_owned()),
                documentation: None,
                insert_text: None,
                is_snippet: false,
                module_path: Some(path),
            },
        );
    }
}

fn module_name(base_dir: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(base_dir).ok()?;
    let without_extension = relative.with_extension("");
    let mut parts = without_extension
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();

    if parts.last().is_some_and(|part| part == "init") {
        parts.pop();
    }

    (!parts.is_empty()).then(|| parts.join("."))
}

fn normalize_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

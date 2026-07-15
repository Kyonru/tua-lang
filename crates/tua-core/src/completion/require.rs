use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::{completion_text_matches, Completion, CompletionKind};
use crate::project_model::ProjectModel;
use crate::vfs::{module_name_for_path, ModulePathSeparator};

pub(super) fn complete_require_paths(path: &Path, prefix: &str) -> Vec<Completion> {
    let Some(base_dir) = path.parent() else {
        return Vec::new();
    };
    let mut completions = BTreeMap::new();
    let current_path = normalize_path(path);

    collect_require_paths(base_dir, base_dir, &current_path, &mut completions);

    completions
        .into_values()
        .filter(|completion| completion_text_matches(&completion.label, prefix))
        .collect()
}

pub(super) fn complete_import_paths(path: &Path, prefix: &str) -> Vec<Completion> {
    let Some(current_dir) = path.parent() else {
        return Vec::new();
    };
    let current_path = normalize_path(path);
    let mut completions = BTreeMap::new();

    if is_parent_import_prefix(prefix) || prefix.starts_with('.') && !prefix.starts_with("./") {
        return Vec::new();
    }

    if prefix.starts_with("./") {
        collect_import_paths(current_dir, current_dir, &current_path, &mut completions);
    } else {
        collect_project_import_paths(path, &current_path, &mut completions);
    }

    completions
        .into_values()
        .filter(|completion| completion_text_matches(&completion.label, prefix))
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
                filter_text: None,
                is_snippet: false,
                module_path: Some(path),
                source_module: None,
                additional_text_edits: Vec::new(),
            },
        );
    }
}

fn collect_import_paths(
    current_dir: &Path,
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
            collect_import_paths(current_dir, &path, current_path, completions);
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

        let Some(specifier) = import_specifier(current_dir, &path) else {
            continue;
        };

        completions.insert(
            specifier.clone(),
            Completion {
                label: specifier,
                kind: CompletionKind::Module,
                detail: Some("relative module".to_owned()),
                documentation: None,
                insert_text: None,
                filter_text: None,
                is_snippet: false,
                module_path: Some(path),
                source_module: None,
                additional_text_edits: Vec::new(),
            },
        );
    }
}

fn collect_project_import_paths(
    path: &Path,
    current_path: &Path,
    completions: &mut BTreeMap<String, Completion>,
) {
    let Ok(model) = ProjectModel::load_for_path(path) else {
        return;
    };
    let Some(workspace) = model.workspace_for_path(path) else {
        return;
    };
    let separator = workspace.require_path_separator();

    for root in workspace.require_roots() {
        collect_project_module_paths(root, root, current_path, separator, completions);
    }
}

fn collect_project_module_paths(
    root: &Path,
    dir: &Path,
    current_path: &Path,
    separator: ModulePathSeparator,
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
            collect_project_module_paths(root, &path, current_path, separator, completions);
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

        let Some(module_name) = module_name_for_path(root, &path, separator) else {
            continue;
        };

        completions.insert(
            module_name.clone(),
            Completion {
                label: module_name,
                kind: CompletionKind::Module,
                detail: Some("project module".to_owned()),
                documentation: None,
                insert_text: None,
                filter_text: None,
                is_snippet: false,
                module_path: Some(path),
                source_module: None,
                additional_text_edits: Vec::new(),
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

fn import_specifier(current_dir: &Path, path: &Path) -> Option<String> {
    let module_path = module_path_without_runtime_suffix(path);
    let mut parts = relative_path_parts(current_dir, &module_path)?;

    if parts.is_empty() {
        return None;
    }

    if parts.first().is_none_or(|part| part != "..") {
        parts.insert(0, ".".to_owned());
    }

    Some(parts.join("/"))
}

fn is_parent_import_prefix(prefix: &str) -> bool {
    prefix.starts_with("../")
        || (prefix.starts_with("./") && has_parent_path_component(prefix))
        || has_parent_path_component(prefix)
}

fn has_parent_path_component(specifier: &str) -> bool {
    Path::new(specifier)
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
}

fn module_path_without_runtime_suffix(path: &Path) -> PathBuf {
    let without_extension = path.with_extension("");

    if without_extension
        .file_name()
        .is_some_and(|name| name == "init")
    {
        return without_extension
            .parent()
            .map(normalize_path)
            .unwrap_or(without_extension);
    }

    without_extension
}

fn relative_path_parts(from_dir: &Path, target: &Path) -> Option<Vec<String>> {
    let from = normalized_component_strings(from_dir)?;
    let target = normalized_component_strings(target)?;
    let mut common = 0usize;

    while from.get(common).is_some()
        && target.get(common).is_some()
        && from[common] == target[common]
    {
        common += 1;
    }

    let mut parts = Vec::new();

    for _ in common..from.len() {
        parts.push("..".to_owned());
    }
    parts.extend(target.into_iter().skip(common));

    Some(parts)
}

fn normalized_component_strings(path: &Path) -> Option<Vec<String>> {
    let mut parts = Vec::new();

    for component in normalize_path(path).components() {
        match component {
            std::path::Component::Normal(part) => {
                parts.push(part.to_string_lossy().to_string());
            }
            std::path::Component::Prefix(prefix) => {
                parts.push(prefix.as_os_str().to_string_lossy().to_string());
            }
            std::path::Component::RootDir => {
                parts.push(String::new());
            }
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => return None,
        }
    }

    Some(parts)
}

fn normalize_path(path: &Path) -> PathBuf {
    crate::vfs::normalize_path(path)
}

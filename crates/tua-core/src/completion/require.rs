use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::{completion_text_matches, Completion, CompletionKind};
use crate::module_path_index::ModulePathIndex;
use crate::vfs::module_path_without_runtime_suffix;

pub(crate) fn complete_require_paths(
    path: &Path,
    prefix: &str,
    module_paths: &ModulePathIndex,
) -> Vec<Completion> {
    let mut completions = BTreeMap::new();
    let current_path = normalize_path(path);

    collect_project_module_paths(&current_path, module_paths, "module", &mut completions);

    completions
        .into_values()
        .filter(|completion| completion_text_matches(&completion.label, prefix))
        .collect()
}

pub(crate) fn complete_import_paths(
    path: &Path,
    prefix: &str,
    module_paths: &ModulePathIndex,
) -> Vec<Completion> {
    let Some(parent) = path.parent() else {
        return Vec::new();
    };
    let current_dir = normalize_path(parent);
    let current_path = normalize_path(path);
    let mut completions = BTreeMap::new();

    if is_parent_import_prefix(prefix) || prefix.starts_with('.') && !prefix.starts_with("./") {
        return Vec::new();
    }

    if prefix.starts_with("./") {
        collect_import_paths(&current_dir, &current_path, module_paths, &mut completions);
    } else {
        collect_project_module_paths(
            &current_path,
            module_paths,
            "project module",
            &mut completions,
        );
    }

    completions
        .into_values()
        .filter(|completion| completion_text_matches(&completion.label, prefix))
        .collect()
}

fn collect_project_module_paths(
    current_path: &Path,
    module_paths: &ModulePathIndex,
    detail: &str,
    completions: &mut BTreeMap<String, Completion>,
) {
    let Some(modules) = module_paths.project_modules_for_path(current_path) else {
        return;
    };

    for (module_name, path) in modules {
        if path == current_path {
            continue;
        }
        completions.insert(
            module_name.clone(),
            Completion {
                label: module_name.clone(),
                kind: CompletionKind::Module,
                detail: Some(detail.to_owned()),
                documentation: None,
                deprecation: None,
                insert_text: None,
                filter_text: None,
                is_snippet: false,
                module_path: Some(path.clone()),
                source_module: None,
                additional_text_edits: Vec::new(),
            },
        );
    }
}

fn collect_import_paths(
    current_dir: &Path,
    current_path: &Path,
    module_paths: &ModulePathIndex,
    completions: &mut BTreeMap<String, Completion>,
) {
    for path in module_paths.paths_under(current_dir) {
        let path = normalize_path(path);

        if !path.starts_with(current_dir) || path == current_path {
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
                deprecation: None,
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

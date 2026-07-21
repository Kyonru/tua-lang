use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound::{Included, Unbounded};
use std::path::{Path, PathBuf};

use crate::vfs::{module_name_for_path, normalize_path, ModulePathSeparator};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectModuleWorkspace {
    root: PathBuf,
    require_roots: Vec<PathBuf>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ModulePathIndex {
    paths: BTreeSet<PathBuf>,
    workspaces: Vec<ModulePathWorkspace>,
}

#[derive(Clone, Debug)]
struct ModulePathWorkspace {
    root: PathBuf,
    modules: BTreeMap<String, PathBuf>,
}

impl ProjectModuleWorkspace {
    pub fn new(root: impl Into<PathBuf>, require_roots: impl IntoIterator<Item = PathBuf>) -> Self {
        let root = normalize_path(root.into());
        let mut normalized_require_roots = Vec::new();

        for require_root in require_roots.into_iter().map(normalize_path) {
            if !normalized_require_roots.contains(&require_root) {
                normalized_require_roots.push(require_root);
            }
        }

        Self {
            root,
            require_roots: normalized_require_roots,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn require_roots(&self) -> &[PathBuf] {
        &self.require_roots
    }
}

impl ModulePathIndex {
    pub(crate) fn build<'a>(
        configured_workspaces: &[ProjectModuleWorkspace],
        indexed_paths: impl IntoIterator<Item = &'a Path>,
        source_roots: &[PathBuf],
        separator: ModulePathSeparator,
    ) -> Self {
        let paths = indexed_paths
            .into_iter()
            .map(normalize_path)
            .collect::<BTreeSet<_>>();
        let fallback_workspaces;
        let configured_workspaces = if configured_workspaces.is_empty() {
            fallback_workspaces = source_roots
                .iter()
                .map(|root| ProjectModuleWorkspace::new(root.clone(), [root.clone()]))
                .collect::<Vec<_>>();
            fallback_workspaces.as_slice()
        } else {
            configured_workspaces
        };
        let mut workspaces = configured_workspaces
            .iter()
            .map(|workspace| ModulePathWorkspace {
                root: workspace.root.clone(),
                modules: project_modules(&paths, &workspace.require_roots, separator),
            })
            .collect::<Vec<_>>();

        workspaces.sort_by(|left, right| left.root.cmp(&right.root));

        Self { paths, workspaces }
    }

    pub(crate) fn paths_under<'a>(&'a self, directory: &'a Path) -> impl Iterator<Item = &'a Path> {
        self.paths
            .range::<Path, _>((Included(directory), Unbounded))
            .map(PathBuf::as_path)
            .take_while(move |path| path.starts_with(directory))
    }

    pub(crate) fn project_modules_for_path(
        &self,
        path: &Path,
    ) -> Option<&BTreeMap<String, PathBuf>> {
        let path = normalize_path(path);

        self.workspaces
            .iter()
            .filter(|workspace| path.starts_with(&workspace.root))
            .max_by_key(|workspace| workspace.root.components().count())
            .or_else(|| self.workspaces.first())
            .map(|workspace| &workspace.modules)
    }
}

fn project_modules(
    paths: &BTreeSet<PathBuf>,
    require_roots: &[PathBuf],
    separator: ModulePathSeparator,
) -> BTreeMap<String, PathBuf> {
    let mut modules = BTreeMap::new();

    for root in require_roots {
        for path in paths
            .range::<Path, _>((Included(root.as_path()), Unbounded))
            .take_while(|path| path.starts_with(root))
        {
            let selected_root = require_roots
                .iter()
                .filter(|candidate| path.starts_with(candidate))
                .max_by_key(|candidate| candidate.components().count());
            if selected_root != Some(root) {
                continue;
            }
            let Some(name) = module_name_for_path(root, path, separator) else {
                continue;
            };

            modules.entry(name).or_insert_with(|| path.clone());
        }
    }

    modules
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_module_names_to_the_nearest_workspace_and_all_of_its_require_roots() {
        let workspaces = [
            ProjectModuleWorkspace::new("game-a", ["game-a/src".into(), "game-a/vendor".into()]),
            ProjectModuleWorkspace::new("game-b", ["game-b/src".into()]),
        ];
        let paths = [
            PathBuf::from("game-a/src/main.tua"),
            PathBuf::from("game-a/vendor/feel/init.lua"),
            PathBuf::from("game-b/src/player.tua"),
        ];
        let index = ModulePathIndex::build(
            &workspaces,
            paths.iter().map(PathBuf::as_path),
            &[],
            ModulePathSeparator::Dot,
        );
        let game_a = index
            .project_modules_for_path(Path::new("game-a/src/main.tua"))
            .expect("game-a workspace should be indexed");

        assert_eq!(
            game_a.get("feel").map(PathBuf::as_path),
            Some(Path::new("game-a/vendor/feel/init.lua"))
        );
        assert!(!game_a.contains_key("player"));
    }

    #[test]
    fn preserves_require_root_precedence_for_duplicate_module_names() {
        let workspace = ProjectModuleWorkspace::new(
            "game",
            ["game/src".into(), "game/build".into(), "game".into()],
        );
        let paths = [
            PathBuf::from("game/build/player.lua"),
            PathBuf::from("game/src/player.tua"),
            PathBuf::from("game/vendor/feel/init.lua"),
        ];
        let index = ModulePathIndex::build(
            &[workspace],
            paths.iter().map(PathBuf::as_path),
            &[],
            ModulePathSeparator::Dot,
        );
        let modules = index
            .project_modules_for_path(Path::new("game/src/main.tua"))
            .expect("game workspace should be indexed");

        assert_eq!(
            modules.get("player").map(PathBuf::as_path),
            Some(Path::new("game/src/player.tua"))
        );
        assert_eq!(
            modules.get("vendor.feel").map(PathBuf::as_path),
            Some(Path::new("game/vendor/feel/init.lua"))
        );
    }
}

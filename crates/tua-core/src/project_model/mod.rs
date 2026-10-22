use std::path::{Path, PathBuf};

use crate::config::{load_config_from_dir, ConfigError, LoadedConfig, ProjectConfig, CONFIG_FILE};
use crate::module_path_index::ProjectModuleWorkspace;
use crate::vfs::{normalize_path, ModulePathSeparator, VfsSourceKind};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectModel {
    workspaces: Vec<ProjectWorkspace>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectWorkspace {
    root: PathBuf,
    config_path: PathBuf,
    config_found: bool,
    config: ProjectConfig,
    source_roots: Vec<PathBuf>,
    output_roots: Vec<PathBuf>,
    ignored_paths: Vec<String>,
    file_associations: Vec<ProjectFileAssociation>,
    require_roots: Vec<PathBuf>,
    type_libraries: Vec<ProjectTypeLibrary>,
    require_path_separator: ModulePathSeparator,
    target: ProjectTarget,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectModelOptions {
    pub workspace_folders: Vec<PathBuf>,
    pub extra_source_roots: Vec<PathBuf>,
    pub extra_require_roots: Vec<PathBuf>,
    pub ignored_paths: Vec<String>,
    pub file_associations: Vec<ProjectFileAssociation>,
    pub require_path_separator: ModulePathSeparator,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectFileAssociation {
    pub pattern: String,
    pub kind: VfsSourceKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTarget {
    pub raw: String,
    pub lua_version: LuaVersion,
    pub love_version: Option<LoveVersion>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTypeLibrary {
    root: PathBuf,
    declaration_prefix: String,
    runtime_prefix: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LuaVersion {
    Lua51,
    LuaJit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoveVersion {
    pub major: u16,
    pub minor: Option<u16>,
}

impl ProjectModel {
    pub fn load(workspace_folders: impl IntoIterator<Item = PathBuf>) -> Result<Self, ConfigError> {
        Self::load_with_options(ProjectModelOptions {
            workspace_folders: workspace_folders.into_iter().collect(),
            ..ProjectModelOptions::default()
        })
    }

    pub fn load_with_options(options: ProjectModelOptions) -> Result<Self, ConfigError> {
        let folders = if options.workspace_folders.is_empty() {
            vec![std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))]
        } else {
            options.workspace_folders.clone()
        };
        let mut workspaces = Vec::new();

        for folder in folders {
            let loaded = load_config_from_dir(&folder)?;

            workspaces.push(ProjectWorkspace::from_loaded_with_options(
                folder, loaded, &options,
            ));
        }

        workspaces.sort_by(|left, right| left.root.cmp(&right.root));

        Ok(Self { workspaces })
    }

    pub fn load_for_path(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let start = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let root = start
            .ancestors()
            .find(|dir| dir.join(CONFIG_FILE).is_file())
            .unwrap_or(start);

        Self::load([root.to_path_buf()])
    }

    pub fn workspaces(&self) -> &[ProjectWorkspace] {
        &self.workspaces
    }

    pub fn primary_workspace(&self) -> Option<&ProjectWorkspace> {
        self.workspaces.first()
    }

    pub fn workspace_for_path(&self, path: impl AsRef<Path>) -> Option<&ProjectWorkspace> {
        let path = normalize_path(path);

        self.workspaces
            .iter()
            .filter(|workspace| path.starts_with(&workspace.root))
            .max_by_key(|workspace| workspace.root.components().count())
            .or_else(|| self.primary_workspace())
    }

    pub fn resolve_required_module(
        &self,
        current_path: impl AsRef<Path>,
        module: &str,
    ) -> Option<PathBuf> {
        self.workspace_for_path(current_path)
            .and_then(|workspace| workspace.resolve_required_module(module))
    }

    pub fn resolve_imported_module(
        &self,
        current_path: impl AsRef<Path>,
        specifier: &str,
    ) -> Option<PathBuf> {
        let current_path = current_path.as_ref();

        self.workspace_for_path(current_path)
            .and_then(|workspace| workspace.resolve_imported_module(current_path, specifier))
    }

    pub fn resolve_type_imported_module(
        &self,
        current_path: impl AsRef<Path>,
        specifier: &str,
    ) -> Option<PathBuf> {
        let current_path = current_path.as_ref();
        self.workspace_for_path(current_path)
            .and_then(|workspace| workspace.resolve_type_imported_module(current_path, specifier))
    }

    pub fn module_name_for_import(
        &self,
        current_path: impl AsRef<Path>,
        specifier: &str,
    ) -> Option<String> {
        let current_path = current_path.as_ref();

        self.workspace_for_path(current_path)
            .and_then(|workspace| workspace.module_name_for_import(current_path, specifier))
    }

    pub fn import_specifier_for_path(
        &self,
        current_path: impl AsRef<Path>,
        target_path: impl AsRef<Path>,
    ) -> Option<String> {
        let current_path = current_path.as_ref();
        let target_path = target_path.as_ref();

        self.workspace_for_path(current_path)
            .and_then(|workspace| workspace.import_specifier_for_path(current_path, target_path))
    }
}

impl ProjectWorkspace {
    pub fn from_loaded(root: impl AsRef<Path>, loaded: LoadedConfig) -> Self {
        Self::from_loaded_with_options(root, loaded, &ProjectModelOptions::default())
    }

    fn from_loaded_with_options(
        root: impl AsRef<Path>,
        loaded: LoadedConfig,
        options: &ProjectModelOptions,
    ) -> Self {
        let root = normalize_path(root);
        let config = loaded.config;
        let mut source_roots = vec![workspace_path(&root, &config.source)];
        let output_roots = vec![workspace_path(&root, &config.out)];

        for source_root in &options.extra_source_roots {
            push_unique_path(&mut source_roots, workspace_path(&root, source_root));
        }

        let mut require_roots = source_roots.clone();
        for output_root in &output_roots {
            push_unique_path(&mut require_roots, output_root.clone());
        }
        if !loaded.found {
            push_unique_path(&mut require_roots, root.clone());
        }
        for require_root in &options.extra_require_roots {
            push_unique_path(&mut require_roots, workspace_path(&root, require_root));
        }

        let target = ProjectTarget::parse(&config.target);
        let type_libraries = config
            .type_libraries
            .iter()
            .map(|library| ProjectTypeLibrary {
                root: workspace_path(&root, &library.path),
                declaration_prefix: library.declaration_prefix.clone(),
                runtime_prefix: library.runtime_prefix.clone(),
            })
            .collect();

        Self {
            root,
            config_path: normalize_path(loaded.path),
            config_found: loaded.found,
            config,
            source_roots,
            output_roots,
            ignored_paths: options.ignored_paths.clone(),
            file_associations: options.file_associations.clone(),
            require_roots,
            type_libraries,
            require_path_separator: options.require_path_separator,
            target,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn config_found(&self) -> bool {
        self.config_found
    }

    pub fn config(&self) -> &ProjectConfig {
        &self.config
    }

    pub fn source_roots(&self) -> &[PathBuf] {
        &self.source_roots
    }

    pub fn primary_source_root(&self) -> &Path {
        &self.source_roots[0]
    }

    pub fn output_roots(&self) -> &[PathBuf] {
        &self.output_roots
    }

    pub fn primary_output_root(&self) -> &Path {
        &self.output_roots[0]
    }

    pub fn configured_main(&self) -> &Path {
        &self.config.main
    }

    pub fn configured_main_source_path(&self) -> PathBuf {
        normalize_path(self.primary_source_root().join(&self.config.main))
    }

    pub fn mirrored_output_path_for_source(
        &self,
        source_path: impl AsRef<Path>,
    ) -> Option<PathBuf> {
        let source_path = source_path.as_ref();
        let relative_path = source_path.strip_prefix(self.primary_source_root()).ok()?;

        Some(normalize_path(
            self.primary_output_root().join(relative_path),
        ))
    }

    pub fn lua_output_paths_for_tua_source(
        &self,
        source_path: impl AsRef<Path>,
    ) -> Option<Vec<PathBuf>> {
        let source_path = normalize_path(source_path);
        if is_tua_declaration_path(&source_path, &self.config.extension) {
            return Some(Vec::new());
        }
        let mirrored = self.mirrored_output_path_for_source(&source_path)?;
        let mut paths = vec![mirrored.with_extension("lua")];
        let main_output = normalize_path(self.primary_output_root().join("main.lua"));

        if source_path == self.configured_main_source_path() && !paths.contains(&main_output) {
            paths.push(main_output);
        }

        Some(paths)
    }

    pub fn ignored_paths(&self) -> &[String] {
        &self.ignored_paths
    }

    pub fn file_associations(&self) -> &[ProjectFileAssociation] {
        &self.file_associations
    }

    pub fn require_roots(&self) -> &[PathBuf] {
        &self.require_roots
    }

    pub fn module_path_workspace(&self) -> ProjectModuleWorkspace {
        ProjectModuleWorkspace::new(self.root.clone(), self.require_roots.iter().cloned())
    }

    pub fn type_libraries(&self) -> &[ProjectTypeLibrary] {
        &self.type_libraries
    }

    pub fn is_type_library_path(&self, path: impl AsRef<Path>) -> bool {
        let path = normalize_path(path);

        self.type_libraries
            .iter()
            .any(|library| path.starts_with(library.root()))
    }

    pub fn require_path_separator(&self) -> ModulePathSeparator {
        self.require_path_separator
    }

    pub fn target(&self) -> &ProjectTarget {
        &self.target
    }

    pub fn asset_roots(&self) -> Vec<PathBuf> {
        let mut roots = vec![self.root.clone()];

        for source_root in &self.source_roots {
            push_unique_path(&mut roots, source_root.clone());
        }
        for output_root in &self.output_roots {
            push_unique_path(&mut roots, output_root.clone());
        }

        roots
    }

    pub fn source_kind_for_path(&self, path: impl AsRef<Path>) -> Option<VfsSourceKind> {
        let path = path.as_ref();
        let extension = path.extension().and_then(|extension| extension.to_str());

        if extension.is_some_and(|extension| extension == self.config.extension) {
            return Some(VfsSourceKind::Tua);
        }

        if extension == Some("lua") {
            return Some(VfsSourceKind::Lua);
        }

        self.file_associations
            .iter()
            .find(|association| path_matches_any_root(path, &association.pattern, &self.root))
            .map(|association| association.kind)
    }

    pub fn is_ignored(&self, path: impl AsRef<Path>) -> bool {
        let path = path.as_ref();

        self.ignored_paths
            .iter()
            .any(|pattern| path_matches_any_root(path, pattern, &self.root))
    }

    pub fn module_name_for_path(&self, path: impl AsRef<Path>) -> Option<String> {
        let path = path.as_ref();
        crate::module_graph::runtime_module_key_for_path(
            path,
            &self.require_roots,
            self.require_path_separator,
        )
    }

    pub fn resolve_required_module(&self, module: &str) -> Option<PathBuf> {
        let relative = module_relative_path(module, self.require_path_separator);
        let extensions = module_extensions(&self.config.extension);

        for root in &self.require_roots {
            for candidate in module_candidates(root, &relative, &extensions) {
                if candidate.is_file() {
                    return Some(normalize_path(candidate));
                }
            }
        }

        None
    }

    pub fn resolve_imported_module(
        &self,
        current_path: impl AsRef<Path>,
        specifier: &str,
    ) -> Option<PathBuf> {
        if is_child_relative_import_specifier(specifier) {
            return self.resolve_child_imported_module(current_path, specifier);
        }

        if is_parent_relative_import_specifier(specifier) {
            return None;
        }

        if is_project_import_specifier(specifier) {
            self.resolve_required_module(specifier)
        } else {
            None
        }
    }

    pub fn resolve_type_imported_module(
        &self,
        current_path: impl AsRef<Path>,
        specifier: &str,
    ) -> Option<PathBuf> {
        if is_child_relative_import_specifier(specifier) {
            return self.resolve_child_imported_module_with_declarations(
                current_path,
                specifier,
                true,
            );
        }
        if !is_project_import_specifier(specifier) {
            return None;
        }
        let relative = module_relative_path(specifier, self.require_path_separator);
        let extensions = module_extensions(&self.config.extension);
        for root in &self.require_roots {
            for candidate in module_candidates_with_declarations(root, &relative, &extensions, true)
            {
                if candidate.is_file() {
                    return Some(normalize_path(candidate));
                }
            }
        }
        None
    }

    fn resolve_child_imported_module(
        &self,
        current_path: impl AsRef<Path>,
        specifier: &str,
    ) -> Option<PathBuf> {
        self.resolve_child_imported_module_with_declarations(current_path, specifier, false)
    }

    fn resolve_child_imported_module_with_declarations(
        &self,
        current_path: impl AsRef<Path>,
        specifier: &str,
        include_declarations: bool,
    ) -> Option<PathBuf> {
        let base = child_relative_import_base(current_path.as_ref(), specifier)?;
        let extensions = module_extensions(&self.config.extension);

        if let Some(extension) = base.extension().and_then(|extension| extension.to_str()) {
            if extension != "lua" && extension != self.config.extension {
                return None;
            }
        }

        let candidates = if base.extension().is_some() {
            vec![base]
        } else {
            import_module_candidates_with_declarations(&base, &extensions, include_declarations)
        };

        for candidate in candidates {
            let candidate = normalize_path(candidate);

            if candidate.is_file() && self.module_name_for_path(&candidate).is_some() {
                return Some(candidate);
            }
        }

        None
    }

    pub fn module_name_for_import(
        &self,
        current_path: impl AsRef<Path>,
        specifier: &str,
    ) -> Option<String> {
        let path = self.resolve_imported_module(current_path, specifier)?;

        self.module_name_for_path(path)
    }

    pub fn import_specifier_for_path(
        &self,
        current_path: impl AsRef<Path>,
        target_path: impl AsRef<Path>,
    ) -> Option<String> {
        let target_path = normalize_path(target_path);

        self.module_name_for_path(&target_path)?;

        let current_dir = normalize_path(current_path.as_ref().parent()?);
        let target_module_path = module_path_without_runtime_suffix(&target_path);
        let mut parts = relative_path_parts(&current_dir, &target_module_path)?;

        if parts.is_empty() {
            return None;
        }

        if parts.first().is_some_and(|part| part == "..") {
            return self.module_name_for_path(&target_path);
        }

        parts.insert(0, ".".to_owned());

        Some(parts.join("/"))
    }
}

pub fn is_tua_declaration_path(path: &Path, extension: &str) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(&format!(".d.{extension}")))
}

impl ProjectTypeLibrary {
    pub fn new(
        root: impl Into<PathBuf>,
        declaration_prefix: impl Into<String>,
        runtime_prefix: impl Into<String>,
    ) -> Self {
        Self {
            root: normalize_path(root.into()),
            declaration_prefix: declaration_prefix.into(),
            runtime_prefix: runtime_prefix.into(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn declaration_prefix(&self) -> &str {
        &self.declaration_prefix
    }

    pub fn runtime_prefix(&self) -> &str {
        &self.runtime_prefix
    }
}

impl ProjectTarget {
    pub fn parse(raw: &str) -> Self {
        let normalized = raw.to_ascii_lowercase();

        Self {
            raw: raw.to_owned(),
            lua_version: if normalized.contains("luajit") {
                LuaVersion::LuaJit
            } else {
                LuaVersion::Lua51
            },
            love_version: parse_love_version(&normalized),
        }
    }
}

fn workspace_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        normalize_path(path)
    } else {
        normalize_path(root.join(path))
    }
}

fn module_relative_path(module: &str, separator: ModulePathSeparator) -> PathBuf {
    let separator = match separator {
        ModulePathSeparator::Dot => '.',
        ModulePathSeparator::Slash => '/',
    };
    let mut relative = PathBuf::new();

    for part in module.split(separator).filter(|part| !part.is_empty()) {
        relative.push(part);
    }

    relative
}

fn module_extensions(project_extension: &str) -> Vec<&str> {
    let mut extensions = vec![project_extension];

    if project_extension != "lua" {
        extensions.push("lua");
    }

    extensions
}

fn module_candidates(root: &Path, relative: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    module_candidates_with_declarations(root, relative, extensions, false)
}

fn module_candidates_with_declarations(
    root: &Path,
    relative: &Path,
    extensions: &[&str],
    include_declarations: bool,
) -> Vec<PathBuf> {
    let base = root.join(relative);
    let mut candidates = Vec::new();

    for extension in extensions {
        if include_declarations && *extension != "lua" {
            candidates.push(base.with_extension(format!("d.{extension}")));
            candidates.push(base.join(format!("init.d.{extension}")));
        }
        candidates.push(base.with_extension(extension));
        candidates.push(base.join(format!("init.{extension}")));
    }

    candidates
}

fn child_relative_import_base(current_path: &Path, specifier: &str) -> Option<PathBuf> {
    if !is_child_relative_import_specifier(specifier) {
        return None;
    }

    Some(normalize_path(current_path.parent()?.join(specifier)))
}

fn is_child_relative_import_specifier(specifier: &str) -> bool {
    specifier.starts_with("./") && !has_parent_path_component(specifier)
}

fn is_project_import_specifier(specifier: &str) -> bool {
    !specifier.is_empty() && !specifier.starts_with('.') && !has_parent_path_component(specifier)
}

fn is_parent_relative_import_specifier(specifier: &str) -> bool {
    specifier.starts_with("../")
        || specifier.starts_with("./") && has_parent_path_component(specifier)
}

fn has_parent_path_component(specifier: &str) -> bool {
    Path::new(specifier)
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
}

fn import_module_candidates_with_declarations(
    base: &Path,
    extensions: &[&str],
    include_declarations: bool,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    for extension in extensions {
        if include_declarations && *extension != "lua" {
            candidates.push(base.with_extension(format!("d.{extension}")));
            candidates.push(base.join(format!("init.d.{extension}")));
        }
        candidates.push(base.with_extension(extension));
        candidates.push(base.join(format!("init.{extension}")));
    }

    candidates
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

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn parse_love_version(target: &str) -> Option<LoveVersion> {
    let version = target.strip_prefix("love")?;
    let version = version.split('-').next().unwrap_or(version);
    let (major, minor) = version.split_once('.').unwrap_or((version, ""));
    let major = major.parse().ok()?;
    let minor = if minor.is_empty() {
        None
    } else {
        minor.parse().ok()
    };

    Some(LoveVersion { major, minor })
}

fn path_matches_any_root(path: &Path, pattern: &str, root: &Path) -> bool {
    path_matches(path, pattern)
        || path
            .strip_prefix(root)
            .ok()
            .is_some_and(|relative| path_matches(relative, pattern))
}

fn path_matches(path: &Path, pattern: &str) -> bool {
    let path = normalize_path(path).to_string_lossy().replace('\\', "/");
    let pattern = pattern.replace('\\', "/");

    if let Some(extension) = pattern.strip_prefix("*.") {
        return path
            .rsplit('/')
            .next()
            .is_some_and(|name| name.ends_with(&format!(".{extension}")));
    }

    if let Some(suffix) = pattern.strip_prefix("**/*") {
        return path.ends_with(suffix);
    }

    if let Some(prefix) = pattern.strip_suffix("/**") {
        return path == prefix || path.starts_with(&format!("{prefix}/"));
    }

    path == pattern || path.ends_with(&format!("/{pattern}"))
}

#[cfg(test)]
mod tests;

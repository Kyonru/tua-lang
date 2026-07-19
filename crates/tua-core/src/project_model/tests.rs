use super::*;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

struct TempProject {
    path: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!(
            "tua-project-model-{name}-{}-{unique}",
            process::id()
        ));

        fs::create_dir_all(&path).expect("failed to create temp project");

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write(&self, path: &str, contents: &str) {
        let path = self.path.join(path);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("failed to create parent directory");
        }

        fs::write(path, contents).expect("failed to write file");
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn loads_tua_config_into_project_workspace() {
    let project = TempProject::new("load");

    project.write(
        "tua.toml",
        r#"
source = "game"
out = "dist"
main = "boot/start.tu"
target = "love12-luajit"
extension = "tu"
"#,
    );

    let model = ProjectModel::load([project.path().to_path_buf()]).expect("model should load");
    let workspace = model.primary_workspace().expect("workspace should exist");

    assert_eq!(workspace.root(), project.path());
    assert_eq!(workspace.config_path(), project.path().join("tua.toml"));
    assert!(workspace.config_found());
    assert_eq!(workspace.primary_source_root(), project.path().join("game"));
    assert_eq!(workspace.primary_output_root(), project.path().join("dist"));
    assert_eq!(workspace.configured_main(), Path::new("boot/start.tu"));
    assert_eq!(
        workspace.configured_main_source_path(),
        project.path().join("game/boot/start.tu")
    );
    assert_eq!(workspace.config().extension, "tu");
    assert_eq!(workspace.target().lua_version, LuaVersion::LuaJit);
    assert_eq!(
        workspace.target().love_version,
        Some(LoveVersion {
            major: 12,
            minor: None
        })
    );
}

#[test]
fn resolves_type_library_roots_without_adding_runtime_require_roots() {
    let project = TempProject::new("type-libraries");

    project.write(
        "tua.toml",
        r#"
[[type_libraries]]
path = "lib/feel/types"
declaration_prefix = "feel"
runtime_prefix = "lib.feel"
"#,
    );

    let model = ProjectModel::load([project.path().to_path_buf()]).expect("model should load");
    let workspace = model.primary_workspace().expect("workspace should exist");
    let library = workspace
        .type_libraries()
        .first()
        .expect("type library should be resolved");

    assert_eq!(library.root(), project.path().join("lib/feel/types"));
    assert_eq!(library.declaration_prefix(), "feel");
    assert_eq!(library.runtime_prefix(), "lib.feel");
    assert!(!workspace
        .require_roots()
        .contains(&library.root().to_path_buf()));
}

#[test]
fn duplicates_configured_main_source_to_root_main_lua() {
    let project = TempProject::new("main-output");

    project.write(
        "tua.toml",
        "source = \"src\"\nout = \"build\"\nmain = \"love/start.tua\"\n",
    );

    let model = ProjectModel::load([project.path().to_path_buf()]).expect("model should load");
    let workspace = model.primary_workspace().expect("workspace should exist");
    let start = project.path().join("src/love/start.tua");
    let helper = project.path().join("src/shared/helper.tua");

    assert_eq!(
        workspace.lua_output_paths_for_tua_source(&start),
        Some(vec![
            project.path().join("build/love/start.lua"),
            project.path().join("build/main.lua"),
        ])
    );
    assert_eq!(
        workspace.lua_output_paths_for_tua_source(&helper),
        Some(vec![project.path().join("build/shared/helper.lua")])
    );
}

#[test]
fn keeps_multiple_workspace_configs_separate() {
    let left = TempProject::new("left");
    let right = TempProject::new("right");

    left.write("tua.toml", "source = \"left-src\"\nout = \"left-build\"\n");
    right.write(
        "tua.toml",
        "source = \"right-src\"\nout = \"right-build\"\n",
    );

    let model = ProjectModel::load([left.path().to_path_buf(), right.path().to_path_buf()])
        .expect("model should load");

    assert_eq!(model.workspaces().len(), 2);

    let left_workspace = model
        .workspace_for_path(left.path().join("left-src/main.tua"))
        .expect("left workspace should own left file");
    let right_workspace = model
        .workspace_for_path(right.path().join("right-src/main.tua"))
        .expect("right workspace should own right file");

    assert_eq!(
        left_workspace.primary_source_root(),
        left.path().join("left-src")
    );
    assert_eq!(
        right_workspace.primary_source_root(),
        right.path().join("right-src")
    );
}

#[test]
fn supports_multiple_source_roots_in_model_options() {
    let project = TempProject::new("extra-source-root");

    project.write("tua.toml", "source = \"src\"\n");

    let model = ProjectModel::load_with_options(ProjectModelOptions {
        workspace_folders: vec![project.path().to_path_buf()],
        extra_source_roots: vec![PathBuf::from("generated")],
        ..ProjectModelOptions::default()
    })
    .expect("model should load");
    let workspace = model.primary_workspace().expect("workspace should exist");

    assert_eq!(
        workspace.source_roots(),
        &[project.path().join("src"), project.path().join("generated"),]
    );
}

#[test]
fn resolves_required_modules_from_project_require_roots() {
    let project = TempProject::new("require-roots");

    project.write("tua.toml", "source = \"src\"\nout = \"build\"\n");
    project.write("src/main.tua", "local Player = require(\"player\")\n");
    project.write("src/player.tua", "return {}\n");
    project.write("build/generated.lua", "return {}\n");
    project.write("outside/player.tua", "return {}\n");

    let model = ProjectModel::load([project.path().to_path_buf()]).expect("model should load");
    let main = project.path().join("src/main.tua");

    assert_eq!(
        model.resolve_required_module(&main, "player"),
        Some(project.path().join("src/player.tua"))
    );
    assert_eq!(
        model.resolve_required_module(&main, "generated"),
        Some(project.path().join("build/generated.lua"))
    );
}

#[test]
fn resolves_static_imports_from_child_and_project_modules() {
    let project = TempProject::new("relative-imports");

    project.write("tua.toml", "source = \"src\"\n");
    project.write("src/main.tua", "local Player = import(\"./player\")\n");
    project.write("src/player.tua", "return {}\n");
    project.write("src/entities/enemy.lua", "return {}\n");
    project.write("src/shared/init.tua", "return {}\n");
    project.write(
        "src/systems/combat.tua",
        "local Shared = import(\"shared\")\nlocal Enemy = import(\"./enemy\")\n",
    );
    project.write("src/systems/enemy.tua", "return {}\n");

    let model = ProjectModel::load([project.path().to_path_buf()]).expect("model should load");
    let main = project.path().join("src/main.tua");
    let combat = project.path().join("src/systems/combat.tua");

    assert_eq!(
        model.resolve_imported_module(&main, "./player"),
        Some(project.path().join("src/player.tua"))
    );
    assert_eq!(
        model.module_name_for_import(&main, "./entities/enemy.lua"),
        Some("entities.enemy".to_owned())
    );
    assert_eq!(
        model.resolve_imported_module(&combat, "./enemy"),
        Some(project.path().join("src/systems/enemy.tua"))
    );
    assert_eq!(
        model.resolve_imported_module(&combat, "shared"),
        Some(project.path().join("src/shared/init.tua"))
    );
    assert_eq!(
        model.resolve_imported_module(&combat, "entities.enemy"),
        Some(project.path().join("src/entities/enemy.lua"))
    );
    assert_eq!(model.resolve_imported_module(&combat, "../shared"), None);
    assert_eq!(model.resolve_imported_module(&combat, "./../shared"), None);
}

#[test]
fn maps_module_paths_back_to_child_import_specifiers() {
    let project = TempProject::new("relative-import-specifier");

    project.write("tua.toml", "source = \"src\"\n");
    project.write("src/main.tua", "");
    project.write("src/player.tua", "return {}\n");
    project.write("src/shared/init.lua", "return {}\n");

    let model = ProjectModel::load([project.path().to_path_buf()]).expect("model should load");
    let main = project.path().join("src/main.tua");

    assert_eq!(
        model.import_specifier_for_path(&main, project.path().join("src/player.tua")),
        Some("./player".to_owned())
    );
    assert_eq!(
        model.import_specifier_for_path(&main, project.path().join("src/shared/init.lua")),
        Some("./shared".to_owned())
    );
}

#[test]
fn maps_parent_directory_auto_imports_to_project_module_specifiers() {
    let project = TempProject::new("parent-import-specifier");

    project.write("tua.toml", "source = \"src\"\n");
    project.write("src/systems/main.tua", "");
    project.write("src/shared/init.lua", "return {}\n");

    let model = ProjectModel::load([project.path().to_path_buf()]).expect("model should load");
    let main = project.path().join("src/systems/main.tua");

    assert_eq!(
        model.import_specifier_for_path(&main, project.path().join("src/shared/init.lua")),
        Some("shared".to_owned())
    );
}

#[test]
fn missing_config_falls_back_to_workspace_root_for_require_resolution() {
    let project = TempProject::new("missing-config-require");

    project.write("main.tua", "local Legacy = require(\"legacy\")\n");
    project.write("legacy.lua", "return {}\n");

    let model = ProjectModel::load_for_path(project.path().join("main.tua"))
        .expect("model should load without config");
    let main = project.path().join("main.tua");
    let workspace = model.primary_workspace().expect("workspace should exist");

    assert!(!workspace.config_found());
    assert_eq!(
        model.resolve_required_module(&main, "legacy"),
        Some(project.path().join("legacy.lua"))
    );
}

#[test]
fn uses_configured_require_path_separator() {
    let project = TempProject::new("slash-separator");

    project.write("tua.toml", "source = \"src\"\n");
    project.write("src/entities/player.tua", "return {}\n");

    let model = ProjectModel::load_with_options(ProjectModelOptions {
        workspace_folders: vec![project.path().to_path_buf()],
        require_path_separator: ModulePathSeparator::Slash,
        ..ProjectModelOptions::default()
    })
    .expect("model should load");
    let main = project.path().join("src/main.tua");
    let workspace = model.primary_workspace().expect("workspace should exist");

    assert_eq!(
        model.resolve_required_module(&main, "entities/player"),
        Some(project.path().join("src/entities/player.tua"))
    );
    assert_eq!(
        workspace.module_name_for_path(project.path().join("src/entities/player.tua")),
        Some("entities/player".to_owned())
    );
}

#[test]
fn represents_ignored_paths_and_file_associations() {
    let project = TempProject::new("settings");

    project.write("tua.toml", "source = \"src\"\n");

    let model = ProjectModel::load_with_options(ProjectModelOptions {
        workspace_folders: vec![project.path().to_path_buf()],
        ignored_paths: vec!["vendor/**".to_owned()],
        file_associations: vec![ProjectFileAssociation {
            pattern: "*.script".to_owned(),
            kind: VfsSourceKind::Lua,
        }],
        ..ProjectModelOptions::default()
    })
    .expect("model should load");
    let workspace = model.primary_workspace().expect("workspace should exist");

    assert!(workspace.is_ignored(project.path().join("vendor/lib.lua")));
    assert_eq!(
        workspace.source_kind_for_path(project.path().join("src/hooks.script")),
        Some(VfsSourceKind::Lua)
    );
}

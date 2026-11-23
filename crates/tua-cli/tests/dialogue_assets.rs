use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const SCENE: &str = "@dialogue 1\n[start]\n// @id intro.hello\n: Hello (end)\n";
const HOST: &str = r#"local Dialogue = require("tuadialogue")
local document = require("scenes.intro")
local player = Dialogue.new(document)
player:start()
local node = player:snapshot().node
if node then assert(node.text == "Hello") end
player:advance()
assert(player:snapshot().state == "completed")
"#;

struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let project = Self(std::env::temp_dir().join(format!(
            "tua-dialogue-assets-{}-{nonce}",
            std::process::id()
        )));
        project.write("tua.toml", "source = \"src\"\nout = \"build\"\n");
        project.write("src/main.tua", HOST);
        project.write("src/scenes/intro.tdialogue", SCENE);
        project
    }
    fn write(&self, path: &str, source: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_tua"))
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
    fn success(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn builds_typed_dialogue_imports_with_runtime_and_source_maps() {
    let project = Project::new();
    project.success(&["check"]);
    project.success(&["build"]);
    assert!(project.0.join("build/scenes/intro.lua").is_file());
    assert!(!project.0.join("build/scenes/intro.tdialogue").exists());
    assert!(project.0.join("build/tuadialogue.lua").is_file());
    let maps = fs::read_dir(project.0.join("build/scenes"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(maps.iter().any(|name| name.contains("map")), "{maps:?}");
    if Command::new("lua").arg("-v").output().is_ok() {
        let output = Command::new("lua")
            .arg("main.lua")
            .current_dir(project.0.join("build"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    project.write(
        "src/main.tua",
        "local document = require(\"scenes.intro\")\ndocument.entry = 42\n",
    );
    assert!(!project.run(&["check"]).status.success());
}

#[test]
fn cached_assets_rebuild_and_invalid_edits_preserve_last_valid_output() {
    let project = Project::new();
    project.success(&["build"]);
    project.success(&["build"]);
    assert!(project.success(&["build"]).contains("up to date"));
    let output = project.0.join("build/scenes/intro.lua");
    let valid = fs::read_to_string(&output).unwrap();
    project.write("src/scenes/intro.tdialogue", &SCENE.replace("(end)", ""));
    let failure = project.run(&["build"]);
    assert!(!failure.status.success());
    let errors = String::from_utf8_lossy(&failure.stderr);
    assert!(
        errors.contains("intro.tdialogue:2:") && errors.contains("TL5012"),
        "{errors}"
    );
    assert_eq!(fs::read_to_string(&output).unwrap(), valid);
    project.write(
        "src/scenes/intro.tdialogue",
        &SCENE.replace("Hello", "Welcome"),
    );
    project.success(&["build"]);
    assert!(fs::read_to_string(&output).unwrap().contains("Welcome"));
    project.write("src/main.tua", "print(\"no dialogue\")\n");
    fs::remove_file(project.0.join("src/scenes/intro.tdialogue")).unwrap();
    project.success(&["build"]);
    assert!(!output.exists(), "deleted dialogue output must be pruned");
}

#[test]
fn rejects_output_collisions_and_does_not_bundle_unused_runtime() {
    let project = Project::new();
    project.write("src/scenes/intro.lua", "return {}\n");
    assert!(!project.run(&["build"]).status.success());
    fs::remove_file(project.0.join("src/scenes/intro.lua")).unwrap();
    project.write("src/scenes/intro.tua", "return {}\n");
    assert!(!project.run(&["build"]).status.success());
    fs::remove_file(project.0.join("src/scenes/intro.tua")).unwrap();
    project.write(
        "src/main.tua",
        "local document = require(\"scenes.intro\")\nprint(document.entry)\n",
    );
    project.success(&["build"]);
    assert!(!project.0.join("build/tuadialogue.lua").exists());
}

#[test]
fn duplicate_text_ids_are_project_errors_and_in_place_builds_still_work() {
    let project = Project::new();
    project.write("src/other.tdialogue", SCENE);
    let invalid = project.run(&["check"]);
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("duplicated across project"));
    fs::remove_file(project.0.join("src/other.tdialogue")).unwrap();
    project.write("tua.toml", "source = \"src\"\nout = \"src\"\n");
    project.success(&["build"]);
    project.success(&["build"]);
    assert!(project.0.join("src/scenes/intro.lua").is_file());
}

#[test]
fn shared_character_modules_build_once_and_validate_each_dialogue_reference() {
    let project = Project::new();
    project.write(
        "src/characters.tua",
        "return { Guide = { name = 'Shared guide' } }\n",
    );
    let source = "@dialogue 1\n@characters Cast \"characters\"\n[start]\n// @id intro.hello\nCast.Guide: Hello (end)\n";
    project.write("src/scenes/intro.tdialogue", source);
    project.write(
        "src/scenes/again.tdialogue",
        &source.replace("intro.hello", "again.hello"),
    );
    project.success(&["check"]);
    project.success(&["build"]);
    let emitted = fs::read_to_string(project.0.join("build/scenes/intro.lua")).unwrap();
    assert!(emitted.contains("require(\"characters\")"), "{emitted}");
    if Command::new("lua").arg("-v").output().is_ok() {
        let output = Command::new("lua").args(["-e", "local a = require('scenes.intro'); local b = require('scenes.again'); assert(a.nodes.start.speaker == 'Shared guide'); assert(b.nodes.start.speaker == a.nodes.start.speaker)"]).current_dir(project.0.join("build")).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    project.write(
        "src/characters.tua",
        "error('analysis must not run this')\nreturn { Guide = { name = 3 } }\n",
    );
    let output = project.run(&["check"]);
    assert!(!output.status.success());
    let errors = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(errors.contains("required string `name`"), "{errors}");
    assert!(!project.run(&["build"]).status.success());
    assert_eq!(
        fs::read_to_string(project.0.join("build/scenes/intro.lua")).unwrap(),
        emitted
    );
}

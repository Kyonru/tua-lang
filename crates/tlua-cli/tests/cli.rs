use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use tlua_core::{CONFIG_FILE, DEFAULT_CONFIG_TEXT};

fn tlua() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tlua"))
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("tlua-cli-{name}-{}-{unique}", process::id()));

        fs::create_dir_all(&path).expect("failed to create temp dir");

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn help_prints_command_list() {
    let output = tlua()
        .arg("--help")
        .output()
        .expect("failed to run tlua --help");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    for command in ["init", "check", "build", "watch", "lsp"] {
        assert!(stdout.contains(command), "missing command `{command}`");
    }

    assert!(stdout.contains("--emit-on-error"));
}

#[test]
fn unknown_command_exits_with_useful_error() {
    let output = tlua()
        .arg("compile")
        .output()
        .expect("failed to run tlua compile");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("unknown command `compile`"));
    assert!(stderr.contains("tlua --help"));
}

#[test]
fn invalid_flag_exits_with_useful_error() {
    let output = tlua()
        .args(["check", "--emit-on-error"])
        .output()
        .expect("failed to run tlua check --emit-on-error");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("only valid with `tlua build`"));
    assert!(stderr.contains("tlua --help"));
}

#[test]
fn init_creates_default_project_files() {
    let temp = TempDir::new("init");
    let output = tlua()
        .arg("init")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tlua init");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    assert!(stdout.contains("initialized tlua project"));
    assert!(stdout.contains("created tlua.toml"));
    assert!(stdout.contains("created src/"));
    assert_eq!(
        fs::read_to_string(temp.path().join(CONFIG_FILE)).expect("failed to read config"),
        DEFAULT_CONFIG_TEXT
    );
    assert!(temp.path().join("src").is_dir());
}

#[test]
fn init_does_not_overwrite_existing_config() {
    let temp = TempDir::new("init-existing");
    let config_path = temp.path().join(CONFIG_FILE);
    let existing = "source = \"custom\"\n";

    fs::write(&config_path, existing).expect("failed to write existing config");

    let output = tlua()
        .arg("init")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tlua init");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    assert!(stdout.contains("left unchanged"));
    assert_eq!(
        fs::read_to_string(config_path).expect("failed to read config"),
        existing
    );
    assert!(temp.path().join("src").is_dir());
}

#[test]
fn check_prints_config_diagnostics() {
    let temp = TempDir::new("check-invalid-config");

    fs::write(temp.path().join(CONFIG_FILE), "emit_maps = yes\n")
        .expect("failed to write invalid config");

    let output = tlua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tlua check");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("tlua.toml:1:1 error TL3001: expected boolean value"));
    assert!(!stderr.contains("panicked"));
}

#[test]
fn check_exits_successfully_for_clean_project() {
    let temp = TempDir::new("check-clean");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tlua"),
        r#"
type Player = {
  hp: number,
}

local player: Player = { hp = 10 }
player.hp = 9
"#,
    )
    .expect("failed to write source");

    let output = tlua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tlua check");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    assert!(stdout.contains("checked 1 file(s)"));
}

#[test]
fn check_exits_nonzero_for_parse_errors() {
    let temp = TempDir::new("check-parse-error");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(temp.path().join("src/main.tlua"), "local hp = \n").expect("failed to write source");

    let output = tlua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tlua check");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("main.tlua:1:10 error TL0001"));
    assert!(!stderr.contains("panicked"));
}

#[test]
fn check_exits_nonzero_for_type_errors() {
    let temp = TempDir::new("check-type-error");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tlua"),
        r#"
local hp: number = "full"
"#,
    )
    .expect("failed to write source");

    let output = tlua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tlua check");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("main.tlua:2:20 error TL1003"));
    assert!(stderr.contains("type mismatch: expected number, found string"));
    assert!(!stderr.contains("panicked"));
}

#[test]
fn build_compiles_nested_source_tree_to_matching_output_tree() {
    let temp = TempDir::new("build-nested");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src/systems/player")).expect("failed to create source");
    fs::create_dir_all(temp.path().join("src/assets")).expect("failed to create ignored source");
    fs::write(
        temp.path().join("src/main.tlua"),
        r#"
local Player = require("systems.player.core")
local current: Player | nil = nil

return Player
"#,
    )
    .expect("failed to write main.tlua");
    fs::write(
        temp.path().join("src/systems/player/core.tlua"),
        r#"
type Player = {
  hp: number,
}

local function damage(player: Player, amount: number): Player
  player.hp -= amount
  return player
end

return {
  damage = damage,
}
"#,
    )
    .expect("failed to write nested source");
    fs::write(
        temp.path().join("src/assets/ignored.lua"),
        "return { ignored = true }\n",
    )
    .expect("failed to write ignored lua");

    let output = tlua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tlua build");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    assert!(stdout.contains("built 2 file(s)"));

    let main_output =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read main output");
    let nested_output = fs::read_to_string(temp.path().join("build/systems/player/core.lua"))
        .expect("failed to read nested output");

    assert!(main_output.contains(r#"require("systems.player.core")"#));
    assert!(!main_output.contains(": Player"));
    assert!(nested_output.contains("player.hp = player.hp - amount"));
    assert!(!nested_output.contains("type Player"));
    assert!(!temp.path().join("build/assets/ignored.lua").exists());
}

#[test]
fn build_preserves_custom_relative_paths_with_only_extension_changed() {
    let temp = TempDir::new("build-custom-layout");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"
source = "game"
out = "dist/lua"
target = "love11-luajit"
extension = "tlua"
emit_maps = true
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("game/client/ui")).expect("failed to create source");
    fs::write(
        temp.path().join("game/client/ui/menu.tlua"),
        r#"
local Menu = {}
return Menu
"#,
    )
    .expect("failed to write source");

    let output = tlua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tlua build");

    assert!(output.status.success());
    assert!(temp.path().join("dist/lua/client/ui/menu.lua").is_file());
    assert!(!temp.path().join("dist/lua/client/ui/menu.tlua").exists());
}

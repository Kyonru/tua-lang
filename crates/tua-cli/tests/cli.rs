use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tua_core::{CONFIG_FILE, DEFAULT_CONFIG_TEXT};

fn tua() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tua"))
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
        let path = std::env::temp_dir().join(format!("tua-cli-{name}-{}-{unique}", process::id()));

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
    let output = tua()
        .arg("--help")
        .output()
        .expect("failed to run tua --help");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    for command in ["init", "check", "build", "watch", "lsp"] {
        assert!(stdout.contains(command), "missing command `{command}`");
    }

    assert!(stdout.contains("--emit-on-error"));
}

#[test]
fn unknown_command_exits_with_useful_error() {
    let output = tua()
        .arg("compile")
        .output()
        .expect("failed to run tua compile");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("unknown command `compile`"));
    assert!(stderr.contains("tua --help"));
}

#[test]
fn invalid_flag_exits_with_useful_error() {
    let output = tua()
        .args(["check", "--emit-on-error"])
        .output()
        .expect("failed to run tua check --emit-on-error");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("only valid with `tua build`"));
    assert!(stderr.contains("tua --help"));
}

#[test]
fn init_creates_default_project_files() {
    let temp = TempDir::new("init");
    let output = tua()
        .arg("init")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua init");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    assert!(stdout.contains("initialized tua project"));
    assert!(stdout.contains("created tua.toml"));
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

    let output = tua()
        .arg("init")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua init");

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

    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua check");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("tua.toml:1:1 error TL3001: expected boolean value"));
    assert!(!stderr.contains("panicked"));
}

#[test]
fn check_exits_successfully_for_clean_project() {
    let temp = TempDir::new("check-clean");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
type Player = {
  hp: number,
}

local player: Player = { hp = 10 }
player.hp = 9
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua check");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    assert!(stdout.contains("checked 1 file(s)"));
}

#[test]
fn check_accepts_project_aware_types_from_tua_and_lua_modules() {
    let temp = TempDir::new("check-project-types");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local Types = require("types")
local LuaDocTypes = require("lua_doc_types")

local player: Types.Player = nil
local enemy: LuaDocTypes.Enemy = nil

return {
  player = player,
  enemy = enemy,
}
"#,
    )
    .expect("failed to write main source");
    fs::write(
        temp.path().join("src/types.tua"),
        r#"
type Player = { hp: number }

return {}
"#,
    )
    .expect("failed to write tua type provider");
    fs::write(
        temp.path().join("src/lua_doc_types.lua"),
        r#"
---@class Enemy
---@field hp number
local Enemy = {}

return {
  Enemy = Enemy,
}
"#,
    )
    .expect("failed to write lua type provider");

    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua check");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    assert!(stdout.contains("checked 2 file(s)"));
}

#[test]
fn check_exits_nonzero_for_parse_errors() {
    let temp = TempDir::new("check-parse-error");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(temp.path().join("src/main.tua"), "local hp = \n").expect("failed to write source");

    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua check");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("main.tua:1:10 error TL0001"));
    assert!(!stderr.contains("panicked"));
}

#[test]
fn check_exits_nonzero_for_type_errors() {
    let temp = TempDir::new("check-type-error");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local hp: number = "full"
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua check");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("main.tua:2:20 error TL1003"));
    assert!(stderr.contains("type mismatch: expected number, found string"));
    assert!(!stderr.contains("panicked"));
}

#[test]
fn check_applies_enabled_inline_ignores() {
    let temp = TempDir::new("check-inline-ignores");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"source = "src"
out = "build"
target = "love11-luajit"
extension = "tua"
emit_maps = true

[diagnostics]
inline_ignores = true
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
-- tua-ignore-next-line TL1003
local hp: number = "full"
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("check")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua check");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    assert!(stdout.contains("checked 1 file(s)"));
}

#[test]
fn build_compiles_nested_source_tree_to_matching_output_tree() {
    let temp = TempDir::new("build-nested");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src/systems/player")).expect("failed to create source");
    fs::create_dir_all(temp.path().join("src/assets")).expect("failed to create ignored source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local Player = require("systems.player.core")
local current: any = nil

return Player
"#,
    )
    .expect("failed to write main.tua");
    fs::write(
        temp.path().join("src/systems/player/core.tua"),
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

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    assert!(stdout.contains("built 3 file(s)"));

    let main_output =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read main output");
    let nested_output = fs::read_to_string(temp.path().join("build/systems/player/core.lua"))
        .expect("failed to read nested output");
    let lua_output = fs::read_to_string(temp.path().join("build/assets/ignored.lua"))
        .expect("failed to read copied lua output");

    assert!(main_output.contains(r#"require("systems.player.core")"#));
    assert!(!main_output.contains(": Player"));
    assert!(nested_output.contains("player.hp = player.hp - amount"));
    assert!(!nested_output.contains("type Player"));
    assert_eq!(lua_output, "return { ignored = true }\n");
}

#[test]
fn build_emits_luacats_annotations_when_configured() {
    let temp = TempDir::new("build-luacats");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"
source = "src"
out = "build"
target = "love11-luajit"
extension = "tua"
emit_maps = true

[emit]
annotation_mode = "luacats"
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
type Player = {
  hp: number,
}

local player: Player = { hp = 10 }

local function make_player(name: string): Player
  player.hp = #name
  return player
end

return {
  player = player,
  make_player = make_player,
}
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");

    assert!(emitted.contains("---@alias Player { hp: number }"));
    assert!(emitted.contains("---@type Player\nlocal player = { hp = 10 }"));
    assert!(emitted.contains("---@param name string\n---@return Player"));
    assert!(emitted.contains("---@field player Player"));
    assert!(emitted.contains("---@field make_player fun(name: string): Player"));
    assert!(!emitted.contains("local player: Player"));
}

#[test]
fn build_accepts_project_aware_types_from_tua_and_lua_modules() {
    let temp = TempDir::new("build-project-types");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local Types = require("types")
local LuaDocTypes = require("lua_doc_types")

local player: Types.Player = nil
local enemy: LuaDocTypes.Enemy = nil

return {
  player = player,
  enemy = enemy,
}
"#,
    )
    .expect("failed to write main source");
    fs::write(
        temp.path().join("src/types.tua"),
        r#"
type Player = { hp: number }

return {}
"#,
    )
    .expect("failed to write tua type provider");
    fs::write(
        temp.path().join("src/lua_doc_types.lua"),
        r#"
---@class Enemy
---@field hp number
local Enemy = {}

return {
  Enemy = Enemy,
}
"#,
    )
    .expect("failed to write lua type provider");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");

    assert!(stdout.contains("built 3 file(s)"));
    assert!(!emitted.contains("Types.Player"));
    assert!(!emitted.contains("LuaDocTypes.Enemy"));
    assert!(temp.path().join("build/lua_doc_types.lua").is_file());
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
extension = "tua"
emit_maps = true
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("game/client/ui")).expect("failed to create source");
    fs::write(
        temp.path().join("game/client/ui/menu.tua"),
        r#"
local Menu = {}
return Menu
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());
    assert!(temp.path().join("dist/lua/client/ui/menu.lua").is_file());
    assert!(!temp.path().join("dist/lua/client/ui/menu.tua").exists());
}

#[test]
fn build_copies_non_tua_files_without_compiling_them() {
    let temp = TempDir::new("build-copies-assets");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src/lib")).expect("failed to create source");
    fs::create_dir_all(temp.path().join("src/assets")).expect("failed to create assets");
    fs::write(
        temp.path().join("src/lib/legacy.lua"),
        "local value = 1\nvalue = value + 1\nreturn value\n",
    )
    .expect("failed to write lua source");
    fs::write(
        temp.path().join("src/assets/player.png"),
        [0x89, b'P', b'N', b'G'],
    )
    .expect("failed to write asset");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let copied = fs::read_to_string(temp.path().join("build/lib/legacy.lua"))
        .expect("failed to read copied lua output");
    let copied_asset =
        fs::read(temp.path().join("build/assets/player.png")).expect("failed to read copied asset");

    assert!(stdout.contains("built 2 file(s)"));
    assert_eq!(copied, "local value = 1\nvalue = value + 1\nreturn value\n");
    assert_eq!(copied_asset, [0x89, b'P', b'N', b'G']);
}

#[test]
fn build_excludes_output_directory_when_source_is_project_root() {
    let temp = TempDir::new("build-root-excludes-dist");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"
source = "."
out = "dist"
target = "love11-luajit"
extension = "tua"
emit_maps = true
"#,
    )
    .expect("failed to write config");
    fs::write(temp.path().join("main.tua"), "return 1\n").expect("failed to write source");
    fs::create_dir_all(temp.path().join("dist")).expect("failed to create previous dist");
    fs::write(temp.path().join("dist/old.lua"), "return 'old'\n")
        .expect("failed to write previous dist file");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());
    assert!(temp.path().join("dist/main.lua").is_file());
    assert!(!temp.path().join("dist/dist/old.lua").exists());
}

#[test]
fn build_warns_and_keeps_generated_lua_when_lua_asset_collides_with_tua_output() {
    let temp = TempDir::new("build-collision");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/player.tua"),
        "local hp: number = 1\nreturn hp\n",
    )
    .expect("failed to write tua source");
    fs::write(temp.path().join("src/player.lua"), "return 'handwritten'\n")
        .expect("failed to write lua source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let emitted =
        fs::read_to_string(temp.path().join("build/player.lua")).expect("failed to read output");

    assert!(stdout.contains("warning TL2003"));
    assert!(stdout.contains("skipping asset copy"));
    assert!(stdout.contains("built 1 file(s)"));
    assert!(emitted.contains("local hp = 1"));
    assert!(!emitted.contains("handwritten"));
}

#[test]
fn build_can_transpile_tua_in_place_without_copying_assets() {
    let temp = TempDir::new("build-in-place");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"
source = "game"
out = "game"
target = "love11-luajit"
extension = "tua"
emit_maps = true
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("game/assets")).expect("failed to create source");
    fs::write(
        temp.path().join("game/main.tua"),
        "local hp: number = 2\nhp += 3\nreturn hp\n",
    )
    .expect("failed to write tua source");
    fs::write(temp.path().join("game/main.lua"), "return 'old'\n")
        .expect("failed to write colliding lua source");
    fs::write(temp.path().join("game/assets/player.png"), [1, 2, 3, 4])
        .expect("failed to write asset");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let emitted =
        fs::read_to_string(temp.path().join("game/main.lua")).expect("failed to read output");
    let asset = fs::read(temp.path().join("game/assets/player.png")).expect("asset should remain");

    assert!(stdout.contains("warning TL2003"));
    assert!(stdout.contains("will be overwritten"));
    assert!(stdout.contains("built 1 file(s)"));
    assert!(emitted.contains("local hp = 2"));
    assert!(emitted.contains("hp = hp + 3"));
    assert_eq!(asset, [1, 2, 3, 4]);
}

#[test]
fn build_blocks_type_errors_by_default() {
    let temp = TempDir::new("build-blocks-type-errors");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local hp: number = "full"
return hp
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("main.tua:2:20 error TL1003"));
    assert!(stderr.contains("type mismatch: expected number, found string"));
    assert!(!temp.path().join("build/main.lua").exists());
}

#[test]
fn build_emit_on_error_writes_parseable_files_and_exits_nonzero() {
    let temp = TempDir::new("build-emit-on-error");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local hp: number = "full"
hp += 1
return hp
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .args(["build", "--emit-on-error"])
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build --emit-on-error");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("main.tua:2:20 error TL1003"));

    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");

    assert!(emitted.contains(r#"local hp = "full""#));
    assert!(emitted.contains("hp = hp + 1"));
    assert!(!emitted.contains(": number"));
}

#[test]
fn watch_rebuilds_after_debounced_source_change() {
    let temp = TempDir::new("watch-rebuild");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local hp: number = 1
return hp
"#,
    )
    .expect("failed to write initial source");

    let child = tua()
        .arg("watch")
        .current_dir(temp.path())
        .env("TUA_INTERNAL_WATCH_POLL_MS", "20")
        .env("TUA_INTERNAL_WATCH_DEBOUNCE_MS", "40")
        .env("TUA_INTERNAL_WATCH_EXIT_AFTER_REBUILDS", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run tua watch");

    wait_until(
        || temp.path().join("build/main.lua").is_file(),
        Duration::from_secs(3),
        "watch did not run the initial build",
    );

    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local hp: number = 2
hp += 3
return hp
"#,
    )
    .expect("failed to write changed source");

    let output = wait_for_child_output(child, Duration::from_secs(5));

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.is_empty(), "unexpected watch stderr:\n{stderr}");
    assert!(stdout.contains("watching src/**/*"));
    assert!(stdout.contains("initial build: built 1 file(s)"));
    assert_eq!(stdout.matches("changed 1 file(s)").count(), 1);
    assert_eq!(stdout.matches("rebuild: rebuilt 1 file(s)").count(), 1);

    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");

    assert!(emitted.contains("local hp = 2"));
    assert!(emitted.contains("hp = hp + 3"));
    assert!(!emitted.contains(": number"));
}

fn wait_until(mut predicate: impl FnMut() -> bool, timeout: Duration, failure_message: &str) {
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        if predicate() {
            return;
        }

        thread::sleep(Duration::from_millis(20));
    }

    panic!("{failure_message}");
}

fn wait_for_child_output(mut child: process::Child, timeout: Duration) -> Output {
    let deadline = Instant::now() + timeout;

    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child.wait_with_output().expect("failed to read output");
            }
            Ok(None) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(20));
            }
            Ok(None) => {
                let _ = child.kill();
                let output = child
                    .wait_with_output()
                    .expect("failed to read timed out output");
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                panic!("watch did not exit before timeout\nstdout:\n{stdout}\nstderr:\n{stderr}");
            }
            Err(error) => panic!("failed to wait for child process: {error}"),
        }
    }
}

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;
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

    for command in [
        "init",
        "check",
        "fmt",
        "lint",
        "build",
        "traceback",
        "watch",
        "lsp",
    ] {
        assert!(stdout.contains(command), "missing command `{command}`");
    }

    assert!(stdout.contains("--emit-on-error"));
    assert!(stdout.contains("--check"));
    assert!(stdout.contains("--strict"));
    assert!(stdout.contains("--maps"));
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

    assert!(stderr.contains(
        "tua.toml:1:13 error TL3001: string values must be quoted, expected literal string"
    ));
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

    let nested_map = fs::read_to_string(temp.path().join("build/systems/player/core.lua.map.json"))
        .expect("failed to read nested source map");
    let nested_map: Value = serde_json::from_str(&nested_map).expect("source map should be JSON");

    assert_eq!(nested_map["version"], 1);
    assert_eq!(nested_map["generated"], "build/systems/player/core.lua");
    assert_eq!(nested_map["source"], "src/systems/player/core.tua");
    assert!(nested_map["lineMappings"]
        .as_array()
        .is_some_and(|mappings| !mappings.is_empty()));
}

#[test]
fn build_respects_disabled_source_maps() {
    let temp = TempDir::new("build-no-source-maps");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"
source = "src"
out = "build"
target = "love11-luajit"
extension = "tua"
emit_maps = false
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(temp.path().join("src/main.tua"), "return 1\n").expect("failed to write source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());
    assert!(temp.path().join("build/main.lua").is_file());
    assert!(!temp.path().join("build/main.lua.map.json").exists());
}

#[test]
fn traceback_remaps_generated_lua_locations_from_source_maps() {
    let temp = TempDir::new("traceback-source-map");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"type Player = {
  hp: number,
}

local player: Player = { hp = 10 }
player.hp += 1
"#,
    )
    .expect("failed to write source");

    let build = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(
        build.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    fs::write(
        temp.path().join("sentry-traceback.txt"),
        "build/main.lua:2: attempt to index nil\nstack traceback:\n",
    )
    .expect("failed to write traceback");

    let output = tua()
        .args(["traceback", "--maps", "build", "sentry-traceback.txt"])
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua traceback");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");

    assert_eq!(
        stdout,
        "src/main.tua:6 (generated build/main.lua:2): attempt to index nil\nstack traceback:\n"
    );
}

#[test]
fn traceback_remaps_mixed_lowering_from_compiler_provenance() {
    let temp = TempDir::new("traceback-mixed-lowering");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"type Count = number

local function tick(value: Count = 1)
  local position = vec2(1, 2)
  local events = signal()
  value += 2
  if value is Count then
    events:emit("tick", position)
  end
  return value
end

tick()
"#,
    )
    .expect("failed to write source");

    let build = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");
    assert!(
        build.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let map = fs::read_to_string(temp.path().join("build/main.lua.map.json"))
        .expect("failed to read source map");
    let map: Value = serde_json::from_str(&map).expect("source map should be JSON");
    let mappings = map["lineMappings"]
        .as_array()
        .expect("source map should contain line mappings");
    let source_line_for = |generated_line: u64| {
        mappings
            .iter()
            .find(|mapping| mapping["generatedLine"] == generated_line)
            .and_then(|mapping| mapping["sourceLine"].as_u64())
    };

    assert_eq!(source_line_for(1), Some(5));
    assert_eq!(source_line_for(4), Some(3));
    assert_eq!(source_line_for(9), Some(6));
    assert_eq!(source_line_for(10), Some(7));

    fs::write(
        temp.path().join("mixed-traceback.txt"),
        "build/main.lua:1: runtime setup failed\nbuild/main.lua:9: arithmetic failed\n",
    )
    .expect("failed to write traceback");

    let output = tua()
        .args(["traceback", "--maps", "build", "mixed-traceback.txt"])
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua traceback");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout should be UTF-8"),
        "src/main.tua:5 (generated build/main.lua:1): runtime setup failed\nsrc/main.tua:6 (generated build/main.lua:9): arithmetic failed\n"
    );
}

#[test]
fn traceback_remaps_an_actual_emitted_lua_runtime_error() {
    let Some(lua) = working_lua_runtime() else {
        eprintln!("skipping runtime traceback test; no Lua executable was found");
        return;
    };
    let temp = TempDir::new("traceback-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"type Message = {
  text: string,
}

local greeting: string = "Olá"
local player: any = nil
print(player.name)
"#,
    )
    .expect("failed to write runtime traceback source");

    let build = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");
    assert!(
        build.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let runtime = Command::new(lua)
        .arg("build/main.lua")
        .current_dir(temp.path())
        .output()
        .expect("failed to execute emitted Lua");
    assert!(
        !runtime.status.success(),
        "emitted Lua should fail at the intentional nil field access"
    );
    let traceback = String::from_utf8(runtime.stderr).expect("Lua traceback should be UTF-8");
    assert!(
        traceback.contains("build/main.lua:3:"),
        "unexpected runtime traceback:\n{traceback}"
    );
    fs::write(temp.path().join("runtime-traceback.txt"), &traceback)
        .expect("failed to write captured runtime traceback");

    let remapped = tua()
        .args(["traceback", "--maps", "build", "runtime-traceback.txt"])
        .current_dir(temp.path())
        .output()
        .expect("failed to remap captured runtime traceback");
    assert!(
        remapped.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&remapped.stdout),
        String::from_utf8_lossy(&remapped.stderr)
    );
    let remapped = String::from_utf8(remapped.stdout).expect("remapped traceback should be UTF-8");

    assert!(
        remapped.contains("src/main.tua:7 (generated build/main.lua:3)"),
        "unexpected remapped traceback:\n{remapped}"
    );
}

#[test]
fn build_lowers_static_imports_to_require_calls() {
    let temp = TempDir::new("build-relative-import");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src/entities")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local Player: any = import("./entities/player")

return Player
"#,
    )
    .expect("failed to write main.tua");
    fs::write(
        temp.path().join("src/entities/player.tua"),
        r#"
return {
  name = "Ada",
}
"#,
    )
    .expect("failed to write player.tua");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let main_output =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read main output");

    assert!(main_output.contains(r#"local Player = require("entities.player")"#));
    assert!(!main_output.contains("import("));
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
fn build_writes_brinevector_runtime_when_vec2_is_used() {
    let temp = TempDir::new("build-vec2-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local position: Vec2 = vec2(100, 200)\nreturn { position = position }\n",
    )
    .expect("failed to write source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");
    let runtime = fs::read_to_string(temp.path().join("build/brinevector.lua"))
        .expect("failed to read BrineVector runtime");

    assert!(stdout.contains("built 2 file(s)"));
    assert!(emitted.contains(r#"local __tua_vec2 = require("brinevector")"#));
    assert!(emitted.contains("local position = __tua_vec2(100, 200)"));
    assert!(runtime.contains("BrineVector: a luajit ffi-accelerated vector library"));
}

#[test]
fn build_does_not_overwrite_project_brinevector_runtime() {
    let temp = TempDir::new("build-custom-vec2-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local position = vec2(100, 200)\nreturn { position = position }\n",
    )
    .expect("failed to write source");
    fs::write(
        temp.path().join("src/brinevector.lua"),
        "return function(x, y) return { x = x, y = y, custom = true } end\n",
    )
    .expect("failed to write custom runtime");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let runtime = fs::read_to_string(temp.path().join("build/brinevector.lua"))
        .expect("failed to read custom runtime");

    assert_eq!(
        runtime,
        "return function(x, y) return { x = x, y = y, custom = true } end\n"
    );
}

#[test]
fn build_writes_anim8_runtime_when_sprite_sheet_helper_is_used() {
    let temp = TempDir::new("build-sprite-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src/assets")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"local Player = spriteSheet({
  image = "assets/player.png",
  frame = { width = 16, height = 16 },
  animations = {
    idle = { from = 0, to = 3, fps = 8 },
  },
})

return Player
"#,
    )
    .expect("failed to write source");
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
    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");
    let runtime =
        fs::read_to_string(temp.path().join("build/anim8.lua")).expect("failed to read anim8");

    assert!(stdout.contains("built 3 file(s)"));
    assert!(emitted.contains(r#"local anim8 = require("anim8")"#));
    assert!(emitted.contains("idle = anim8.newAnimation(PlayerGrid(\"1-4\", 1), 1 / 8)"));
    assert!(runtime.contains("_VERSION     = 'anim8 v2.3.1'"));
    assert!(runtime.contains("_URL         = 'https://github.com/kikito/anim8'"));
}

#[test]
fn build_does_not_overwrite_project_anim8_runtime() {
    let temp = TempDir::new("build-custom-sprite-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src/assets")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"local Player = spriteSheet({
  image = "assets/player.png",
  frame = { width = 16, height = 16 },
  animations = {
    idle = { from = 0, to = 3, fps = 8 },
  },
})

return Player
"#,
    )
    .expect("failed to write source");
    fs::write(
        temp.path().join("src/assets/player.png"),
        [0x89, b'P', b'N', b'G'],
    )
    .expect("failed to write asset");
    fs::write(
        temp.path().join("src/anim8.lua"),
        "return { custom = true }\n",
    )
    .expect("failed to write custom runtime");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let runtime =
        fs::read_to_string(temp.path().join("build/anim8.lua")).expect("failed to read anim8");

    assert_eq!(runtime, "return { custom = true }\n");
}

#[test]
fn build_writes_tuasignal_runtime_when_signal_helper_is_used() {
    let temp = TempDir::new("build-signal-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"local events = signal()
events:on("player.hit", function(player, damage)
  print(player, damage)
end)
events:emit("player.hit", "ada", 4)
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");
    let runtime = fs::read_to_string(temp.path().join("build/tuasignal.lua"))
        .expect("failed to read tuasignal");

    assert!(stdout.contains("built 2 file(s)"));
    assert!(emitted.contains(r#"local __tua_signal = require("tuasignal")"#));
    assert!(emitted.contains("local events = __tua_signal()"));
    assert!(runtime.contains("function SignalBus:emit(name, ...)"));
}

#[test]
fn build_does_not_overwrite_project_tuasignal_runtime() {
    let temp = TempDir::new("build-custom-signal-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local events = signal()\nreturn events\n",
    )
    .expect("failed to write source");
    fs::write(
        temp.path().join("src/tuasignal.lua"),
        "return function() return { custom = true } end\n",
    )
    .expect("failed to write custom runtime");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let runtime = fs::read_to_string(temp.path().join("build/tuasignal.lua"))
        .expect("failed to read tuasignal");

    assert_eq!(runtime, "return function() return { custom = true } end\n");
}

#[test]
fn build_writes_tuaecs_runtime_when_builtin_ecs_is_used() {
    let temp = TempDir::new("build-ecs-runtime");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"local Position = ecs.component("Position", { x = "number" })
local world = ecs.world()
local entity = world:create()
world:add(entity, Position, { x = 10 })
"#,
    )
    .expect("failed to write source");

    let output = tua()
        .arg("build")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua build");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");
    let runtime =
        fs::read_to_string(temp.path().join("build/tuaecs.lua")).expect("failed to read tuaecs");

    assert!(stdout.contains("built 2 file(s)"));
    assert!(emitted.contains(r#"local ecs = require("tuaecs")"#));
    assert!(runtime.contains("function World:query(components)"));
    assert!(runtime.contains("function ecs.system(world, components, update)"));
}

#[test]
fn build_does_not_inject_tuaecs_when_ecs_support_is_disabled() {
    let temp = TempDir::new("build-disabled-ecs-runtime");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"source = "src"
out = "build"

[ecs]
enabled = false
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local world = ecs.world()\nreturn world\n",
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

    assert!(!emitted.contains("tuaecs"));
    assert!(!temp.path().join("build/tuaecs.lua").exists());
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
fn fmt_formats_project_files_in_place() {
    let temp = TempDir::new("fmt-in-place");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(temp.path().join("src/main.tua"), "local value=1\n").expect("failed to write source");

    let output = tua()
        .arg("fmt")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua fmt");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let formatted =
        fs::read_to_string(temp.path().join("src/main.tua")).expect("failed to read source");

    assert!(stdout.contains("formatted 1 file(s)"));
    assert_eq!(formatted, "local value = 1\n");
}

#[test]
fn fmt_check_reports_unformatted_files_without_writing() {
    let temp = TempDir::new("fmt-check");
    let original = "local value=1\n";

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(temp.path().join("src/main.tua"), original).expect("failed to write source");

    let output = tua()
        .args(["fmt", "--check"])
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua fmt --check");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");
    let current =
        fs::read_to_string(temp.path().join("src/main.tua")).expect("failed to read source");

    assert!(stderr.contains("format check failed"));
    assert!(stderr.contains("would reformat src/main.tua"));
    assert_eq!(current, original);
}

#[test]
fn lint_respects_project_config() {
    let temp = TempDir::new("lint-config");

    fs::write(
        temp.path().join(CONFIG_FILE),
        r#"source = "src"

[diagnostics]
duplicate_keys = true
"#,
    )
    .expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local config = { hp = 10, hp = 20 }\n",
    )
    .expect("failed to write source");

    let output = tua()
        .arg("lint")
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua lint");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("main.tua:1"));
    assert!(stderr.contains("warning TL4001"));
    assert!(stderr.contains("duplicate table key `hp`"));
}

#[test]
fn lint_strict_enables_all_lints() {
    let temp = TempDir::new("lint-strict");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local config = { hp = 10, hp = 20 }\n",
    )
    .expect("failed to write source");

    let output = tua()
        .args(["lint", "--strict"])
        .current_dir(temp.path())
        .output()
        .expect("failed to run tua lint --strict");

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.contains("warning TL4001"));
    assert!(stderr.contains("duplicate table key `hp`"));
}

#[test]
fn watch_rebuilds_after_debounced_source_change() {
    let _guard = watch_test_guard();
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
        .env("TUA_INTERNAL_WATCH_RECONCILE_MS", "100")
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

#[test]
fn watch_polling_fallback_reloads_configuration() {
    let _guard = watch_test_guard();
    let temp = TempDir::new("watch-config");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local hp: number = 1\nreturn hp\n",
    )
    .expect("failed to write source");

    let child = tua()
        .arg("watch")
        .current_dir(temp.path())
        .env("TUA_INTERNAL_WATCH_FORCE_POLL", "1")
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

    let reconfigured = DEFAULT_CONFIG_TEXT.replace("out = \"build\"", "out = \"dist\"");
    fs::write(temp.path().join(CONFIG_FILE), reconfigured).expect("failed to update config");

    let output = wait_for_child_output(child, Duration::from_secs(5));

    assert!(output.status.success());
    assert!(temp.path().join("dist/main.lua").is_file());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.is_empty(), "unexpected watch stderr:\n{stderr}");
    assert!(stdout.contains("using polling file watcher"));
    assert!(stdout.contains("changed 1 file(s)"));
    assert!(stdout.contains("configuration rebuild: rebuilt 1 file(s)"));
}

#[test]
fn watch_keeps_source_changes_when_configuration_reload_fails() {
    let _guard = watch_test_guard();
    let temp = TempDir::new("watch-invalid-config-batch");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/main.tua"),
        "local hp: number = 1\nreturn hp\n",
    )
    .expect("failed to write source");

    let child = tua()
        .arg("watch")
        .current_dir(temp.path())
        .env("TUA_INTERNAL_WATCH_FORCE_POLL", "1")
        .env("TUA_INTERNAL_WATCH_POLL_MS", "100")
        .env("TUA_INTERNAL_WATCH_DEBOUNCE_MS", "80")
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

    fs::write(temp.path().join(CONFIG_FILE), "[build\n").expect("failed to invalidate config");
    fs::write(
        temp.path().join("src/main.tua"),
        "local hp: number = 2\nreturn hp\n",
    )
    .expect("failed to update source");

    let output = wait_for_child_output(child, Duration::from_secs(5));

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");
    let emitted =
        fs::read_to_string(temp.path().join("build/main.lua")).expect("failed to read output");

    assert!(
        stdout.contains("configuration reload failed; keeping previous configuration"),
        "{stdout}"
    );
    assert!(stdout.contains("rebuild: rebuilt 1 file(s)"), "{stdout}");
    assert!(stderr.contains("tua.toml"), "{stderr}");
    assert!(emitted.contains("local hp = 2"), "{emitted}");
}

#[test]
fn watch_handles_asset_create_rename_and_remove_in_uniform_batches() {
    let _guard = watch_test_guard();
    let temp = TempDir::new("watch-assets");
    let source_assets = temp.path().join("src/assets");
    let output_assets = temp.path().join("build/assets");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(&source_assets).expect("failed to create assets");
    fs::write(
        temp.path().join("src/main.tua"),
        "local hp: number = 1\nreturn hp\n",
    )
    .expect("failed to write source");
    fs::write(source_assets.join("original.txt"), "original").expect("failed to write asset");

    let child = tua()
        .arg("watch")
        .current_dir(temp.path())
        .env("TUA_INTERNAL_WATCH_FORCE_POLL", "1")
        .env("TUA_INTERNAL_WATCH_POLL_MS", "20")
        .env("TUA_INTERNAL_WATCH_DEBOUNCE_MS", "40")
        .env("TUA_INTERNAL_WATCH_EXIT_AFTER_REBUILDS", "3")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run tua watch");

    wait_until(
        || output_assets.join("original.txt").is_file(),
        Duration::from_secs(3),
        "watch did not copy the initial asset",
    );

    fs::write(source_assets.join("created.txt"), "created").expect("failed to create asset");
    wait_until(
        || output_assets.join("created.txt").is_file(),
        Duration::from_secs(3),
        "watch did not copy the created asset",
    );

    fs::rename(
        source_assets.join("original.txt"),
        source_assets.join("renamed.txt"),
    )
    .expect("failed to rename asset");
    wait_until(
        || {
            output_assets.join("renamed.txt").is_file()
                && !output_assets.join("original.txt").exists()
        },
        Duration::from_secs(3),
        "watch did not apply the asset rename",
    );

    fs::remove_file(source_assets.join("created.txt")).expect("failed to remove asset");
    wait_until(
        || !output_assets.join("created.txt").exists(),
        Duration::from_secs(3),
        "watch did not remove the deleted asset output",
    );

    let output = wait_for_child_output(child, Duration::from_secs(5));

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(stderr.is_empty(), "unexpected watch stderr:\n{stderr}");
    assert_eq!(stdout.matches("changed ").count(), 3, "{stdout}");
    assert_eq!(stdout.matches("rebuild: rebuilt").count(), 3, "{stdout}");
    assert!(stdout.contains("changed 2 file(s)"), "{stdout}");
    assert!(stdout.contains("removed 1 output file(s)"), "{stdout}");
}

#[test]
fn watch_rechecks_unchanged_importers_after_exported_type_changes() {
    let _guard = watch_test_guard();
    let temp = TempDir::new("watch-dependent-types");

    fs::write(temp.path().join(CONFIG_FILE), DEFAULT_CONFIG_TEXT).expect("failed to write config");
    fs::create_dir_all(temp.path().join("src")).expect("failed to create source");
    fs::write(
        temp.path().join("src/types.tua"),
        r#"
local function accept(value: number): number
  return value
end

return { accept = accept }
"#,
    )
    .expect("failed to write initial type provider");
    fs::write(
        temp.path().join("src/main.tua"),
        r#"
local Types = require("types")
local value: number = Types.accept(10)

return value
"#,
    )
    .expect("failed to write unchanged importer");

    let child = tua()
        .arg("watch")
        .current_dir(temp.path())
        .env("TUA_INTERNAL_WATCH_POLL_MS", "20")
        .env("TUA_INTERNAL_WATCH_DEBOUNCE_MS", "40")
        .env("TUA_INTERNAL_WATCH_RECONCILE_MS", "100")
        .env("TUA_INTERNAL_WATCH_EXIT_AFTER_REBUILDS", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run tua watch");

    wait_until(
        || {
            temp.path().join("build/main.lua").is_file()
                && temp.path().join("build/types.lua").is_file()
        },
        Duration::from_secs(3),
        "watch did not run the initial dependency-aware build",
    );

    fs::write(
        temp.path().join("src/types.tua"),
        r#"
local function accept(value: string): string
  return value
end

return { accept = accept }
"#,
    )
    .expect("failed to change exported type");

    let output = wait_for_child_output(child, Duration::from_secs(5));

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");

    assert!(
        stdout.contains("initial build: built 2 file(s)"),
        "unexpected stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("changed 1 file(s)"),
        "unexpected stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("rebuild: 1 file(s) emitted with errors"),
        "unexpected stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("src/main.tua"),
        "unexpected stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("expected string, found number"),
        "unexpected stderr:\n{stderr}"
    );
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

fn watch_test_guard() -> MutexGuard<'static, ()> {
    static WATCH_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    WATCH_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
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

fn working_lua_runtime() -> Option<OsString> {
    [
        std::env::var_os("TUA_LUAJIT_BIN"),
        std::env::var_os("TUA_LUA_BIN"),
        Some(OsString::from("luajit")),
        Some(OsString::from("lua5.1")),
        Some(OsString::from("lua")),
        Some(OsString::from("lua5.4")),
        Some(OsString::from("lua5.3")),
        Some(OsString::from("lua5.2")),
    ]
    .into_iter()
    .flatten()
    .find(|candidate| {
        Command::new(candidate)
            .arg("-v")
            .output()
            .is_ok_and(|output| output.status.success())
    })
}

use super::*;
use crate::source_text::position_at;
use std::fs;
use std::path::PathBuf;

const SOURCE: &str = r#"type Vec2 = { x: number, y: number }
type Player = { pos: Vec2, hp: number, alive?: boolean }

local player: Player = {
  pos = { x = 0, y = 0 },
  hp = 10,
}

local payload: any = {}
local value = player.hp
local partial = player.alive
local nested = player.pos.x
local typed: Pl = nil
"#;

#[test]
fn completes_type_aliases_in_annotation_positions() {
    let completions = complete_at("typed: Pl").expect("alias completion should succeed");

    assert_eq!(labels(&completions), vec!["Player".to_owned()]);
    assert_eq!(completions[0].kind, CompletionKind::TypeAlias);
    assert_eq!(completions[0].detail.as_deref(), Some("type alias"));
}

#[test]
fn completes_lua_doc_annotation_tags_after_marker() {
    let source = "---@p\n";
    let completions = complete_tua_source("src/main.tua", source, position_after(source, "---@p"))
        .expect("annotation tag completion should succeed");

    assert_eq!(labels(&completions), vec!["param".to_owned()]);
    assert_eq!(completions[0].kind, CompletionKind::Keyword);
    assert_eq!(
        completions[0].detail.as_deref(),
        Some("function parameter annotation")
    );
}

#[test]
fn completes_lua_doc_type_names_in_tua_files() {
    let source = r#"type Player = { hp: number }
---@param player Pl
local function damage(player)
end
"#;
    let completions =
        complete_tua_source("src/main.tua", source, position_after(source, "player Pl"))
            .expect("annotation type completion should succeed");

    assert_eq!(labels(&completions), vec!["Player".to_owned()]);
    assert_eq!(completions[0].kind, CompletionKind::TypeAlias);
}

#[test]
fn completes_lua_doc_type_names_in_lua_files() {
    let source = r#"---@alias Player { hp: number }
---@return Pl
local function makePlayer()
end
"#;
    let completions = complete_tua_source(
        "src/player.lua",
        source,
        position_after(source, "return Pl"),
    )
    .expect("Lua annotation type completion should succeed");

    assert_eq!(labels(&completions), vec!["Player".to_owned()]);
    assert_eq!(completions[0].kind, CompletionKind::TypeAlias);
}

#[test]
fn completes_love_builtin_type_names() {
    let source = "local player: Im\n";
    let completions = complete_tua_source("src/main.tua", source, position_after(source, "Im"))
        .expect("LOVE type completion should succeed");

    assert_eq!(labels(&completions), vec!["Image".to_owned()]);
    assert_eq!(completions[0].kind, CompletionKind::TypeAlias);
    assert_eq!(completions[0].detail.as_deref(), Some("built-in type"));
}

#[test]
fn generates_function_documentation_snippet_from_tua_signature() {
    let source = r#"type Player = { hp: number }
---
local function damage(player: Player, amount: number): Player
end
"#;
    let completions = complete_tua_source("src/main.tua", source, position_after(source, "---"))
        .expect("documentation snippet completion should succeed");

    assert_eq!(labels(&completions), vec!["document function".to_owned()]);
    assert_eq!(completions[0].kind, CompletionKind::Snippet);
    assert!(completions[0].is_snippet);
    assert_eq!(
        completions[0].insert_text.as_deref(),
        Some(
            "@param player Player ${1:description}\n---@param amount number ${2:description}\n---@return Player ${3:description}"
        )
    );
}

#[test]
fn generates_function_documentation_snippet_from_lua_signature() {
    let source = r#"---
local function play(source, ...)
end
"#;
    let completions = complete_tua_source("src/audio.lua", source, position_after(source, "---"))
        .expect("Lua documentation snippet completion should succeed");

    assert_eq!(labels(&completions), vec!["document function".to_owned()]);
    assert_eq!(
        completions[0].insert_text.as_deref(),
        Some("@param source any ${1:description}\n---@param ... any ${2:description}")
    );
}

#[test]
fn annotation_completions_work_while_source_is_temporarily_invalid() {
    let source = "local broken =\n---@r\n";
    let completions = complete_tua_source("src/main.tua", source, position_after(source, "---@r"))
        .expect("lossy annotation completion should succeed");

    assert_eq!(labels(&completions), vec!["return".to_owned()]);
}

#[test]
fn completes_table_fields_after_dot_access() {
    let completions = complete_at("player.").expect("field completion should succeed");

    assert_eq!(
        labels(&completions),
        vec!["alive".to_owned(), "hp".to_owned(), "pos".to_owned()]
    );
    assert!(completions
        .iter()
        .all(|item| item.kind == CompletionKind::Field));
}

#[test]
fn filters_table_fields_by_partial_prefix() {
    let completions = complete_at("player.a").expect("field completion should succeed");

    assert_eq!(labels(&completions), vec!["alive".to_owned()]);
}

#[test]
fn completes_nested_table_fields_when_shape_is_known() {
    let completions = complete_at("player.pos.").expect("nested field completion should succeed");

    assert_eq!(labels(&completions), vec!["x".to_owned(), "y".to_owned()]);
}

#[test]
fn completes_lua_keywords_and_globals_in_expression_positions() {
    let source = "local value = ma\n";
    let position = position_after(source, "ma");
    let completions = complete_tua_source("src/main.tua", source, position)
        .expect("Lua baseline completion should succeed");

    assert!(labels(&completions).contains(&"math".to_owned()));
}

#[test]
fn completes_love_asset_loader_globals() {
    let source = "local player = im\n";
    let position = position_after(source, "im");
    let completions = complete_tua_source("src/main.tua", source, position)
        .expect("asset loader completion should succeed");

    let image = completions
        .iter()
        .find(|completion| completion.label == "image")
        .expect("image loader should complete");

    assert_eq!(image.kind, CompletionKind::Function);
    assert_eq!(
        image.detail.as_deref(),
        Some("function image(path: string): Image")
    );
}

#[test]
fn completes_extended_lua_global_items() {
    let underscore_source = "local value = _\n";
    let underscore = complete_tua_source(
        "src/main.tua",
        underscore_source,
        position_after(underscore_source, "_"),
    )
    .expect("underscore globals should complete");
    let raw_source = "local value = raw\n";
    let raw = complete_tua_source(
        "src/main.tua",
        raw_source,
        position_after(raw_source, "raw"),
    )
    .expect("raw globals should complete");
    let warn_source = "local value = wa\n";
    let warn = complete_tua_source(
        "src/main.tua",
        warn_source,
        position_after(warn_source, "wa"),
    )
    .expect("warn should complete");
    let arg_source = "local value = ar\n";
    let arg = complete_tua_source("src/main.tua", arg_source, position_after(arg_source, "ar"))
        .expect("arg should complete");

    assert!(labels(&underscore).contains(&"_ENV".to_owned()));
    assert!(labels(&underscore).contains(&"_G".to_owned()));
    assert!(labels(&raw).contains(&"rawlen".to_owned()));
    assert!(labels(&warn).contains(&"warn".to_owned()));
    assert!(labels(&arg).contains(&"arg".to_owned()));
}

#[test]
fn type_aliases_do_not_complete_as_runtime_values() {
    let source = "type Player = { hp: number }\nlocal value = Pl\n";
    let position = position_after(source, "Pl");
    let completions = complete_tua_source("src/main.tua", source, position)
        .expect("runtime completion should succeed");

    assert!(!labels(&completions).contains(&"Player".to_owned()));
}

#[test]
fn completes_lua_standard_library_fields() {
    let source = "local value = math.si\n";
    let position = position_after(source, "math.si");
    let completions = complete_tua_source("src/main.tua", source, position)
        .expect("Lua field completion should succeed");

    assert_eq!(labels(&completions), vec!["sin".to_owned()]);
    assert_eq!(completions[0].kind, CompletionKind::Function);
    assert_eq!(
        completions[0].detail.as_deref(),
        Some("function math.sin(x: number): number")
    );
    assert_eq!(
        completions[0].documentation.as_deref(),
        Some("Returns the sine.")
    );
}

#[test]
fn completes_lua_fields_when_source_is_temporarily_invalid() {
    let source = "local broken =\nlocal value = math.si\n";
    let position = position_after(source, "math.si");
    let completions = complete_tua_source("src/main.tua", source, position)
        .expect("lossy Lua completion should succeed");

    assert_eq!(labels(&completions), vec!["sin".to_owned()]);
}

#[test]
fn completes_love_fields() {
    let source = "love.graphics.pr\n";
    let position = position_after(source, "love.graphics.pr");
    let completions = complete_tua_source("src/main.tua", source, position)
        .expect("LÖVE field completion should succeed");

    assert_eq!(
        labels(&completions),
        vec![
            "present".to_owned(),
            "print".to_owned(),
            "printf".to_owned()
        ]
    );
}

#[test]
fn completes_love_api_generated_fields() {
    let graphics = complete_tua_source(
        "src/main.tua",
        "love.graphics.newCan\n",
        position_after("love.graphics.newCan\n", "love.graphics.newCan"),
    )
    .expect("LÖVE graphics completion should succeed");
    let physics = complete_tua_source(
        "src/main.tua",
        "love.physics.newWo\n",
        position_after("love.physics.newWo\n", "love.physics.newWo"),
    )
    .expect("LÖVE physics completion should succeed");

    assert_eq!(labels(&graphics), vec!["newCanvas".to_owned()]);
    assert_eq!(labels(&physics), vec!["newWorld".to_owned()]);
}

#[test]
fn completes_love_image_methods_for_inferred_asset_loader_locals() {
    let source = r#"
local player = image "assets/player.png"
player.getWi
"#;
    let completions = complete_tua_source("src/main.tua", source, position_after(source, "getWi"))
        .expect("inferred image method completion should succeed");

    assert_eq!(labels(&completions), vec!["getWidth".to_owned()]);
    assert_eq!(completions[0].kind, CompletionKind::Function);
    assert_eq!(
        completions[0].detail.as_deref(),
        Some("function Image:getWidth(): number")
    );
}

#[test]
fn completes_love_image_methods_after_colon_access() {
    let source = r#"
local player = image "assets/player.png"
player:getWi
"#;
    let completions = complete_tua_source("src/main.tua", source, position_after(source, "getWi"))
        .expect("colon image method completion should succeed");

    assert_eq!(labels(&completions), vec!["getWidth".to_owned()]);
}

#[test]
fn completes_love_image_methods_for_inferred_function_return_locals() {
    let source = r#"
local function loadPlayerImage()
  return image "assets/player.png"
end

local player = loadPlayerImage()

player.getH
"#;
    let completions = complete_tua_source("src/main.tua", source, position_after(source, "getH"))
        .expect("function-return image method completion should succeed");

    assert_eq!(labels(&completions), vec!["getHeight".to_owned()]);
    assert_eq!(
        completions[0].detail.as_deref(),
        Some("function Image:getHeight(): number")
    );
}

#[test]
fn completes_love_image_methods_after_empty_dot_for_inferred_function_return_locals() {
    let source = r#"
local function loadPlayerImage()
  return image "assets/player.png"
end

local function loadJumpSound()
  return sound "assets/jump.wav"
end

local player = loadPlayerImage()

player.

return {
  loadPlayerImage = loadPlayerImage,
  loadJumpSound = loadJumpSound,
}
"#;
    let completions = complete_tua_source(
        "src/main.tua",
        source,
        position_after_last(source, "player."),
    )
    .expect("empty-dot image method completion should succeed");
    let labels = labels(&completions);

    assert!(labels.contains(&"getWidth".to_owned()), "{labels:?}");
    assert!(labels.contains(&"getHeight".to_owned()), "{labels:?}");
    assert!(labels.contains(&"setFilter".to_owned()), "{labels:?}");
}

#[test]
fn completes_love_source_methods_for_inferred_asset_loader_locals() {
    let source = r#"
local jump = sound "assets/jump.wav"
jump.pl
"#;
    let completions = complete_tua_source("src/main.tua", source, position_after(source, "pl"))
        .expect("inferred source method completion should succeed");

    assert_eq!(labels(&completions), vec!["play".to_owned()]);
    assert_eq!(
        completions[0].detail.as_deref(),
        Some("function Source:play(): boolean")
    );
}

#[test]
fn completes_local_lua_symbols() {
    let source = r#"local score = 0
local function spawn_player(x, y)
  local player = {}
  return sco
end
"#;
    let position = position_after(source, "return sco");
    let completions = complete_tua_source("src/main.tua", source, position)
        .expect("local symbol completion should succeed");

    assert!(labels(&completions).contains(&"score".to_owned()));
}

#[test]
fn local_symbols_shadow_lua_globals_for_field_completion() {
    let source = "local math: any = {}\nlocal value = math.\n";
    let position = position_after(source, "math.");
    let completions = complete_tua_source("src/main.tua", source, position)
        .expect("shadowed Lua globals should stay dynamic");

    assert!(completions.is_empty());
}

#[test]
fn completes_require_paths_from_current_directory() {
    let root = temp_project_dir("require_completion");
    let src = root.join("src");
    let systems = src.join("systems");

    fs::create_dir_all(&systems).expect("test dirs should be created");
    fs::write(src.join("main.tua"), "local player = require(\"pla\")\n")
        .expect("main source should be written");
    fs::write(src.join("player.tua"), "return {}\n").expect("player source should be written");
    fs::write(systems.join("physics.lua"), "return {}\n").expect("Lua module should be written");

    let main_path = src.join("main.tua");
    let source = fs::read_to_string(&main_path).expect("main source should be readable");
    let completions = complete_tua_source(
        &main_path,
        &source,
        position_after(&source, "require(\"pla"),
    )
    .expect("require path completion should succeed");

    assert_eq!(labels(&completions), vec!["player".to_owned()]);

    let source = "local physics = require(\"systems.\")\n";
    let completions = complete_tua_source(&main_path, source, position_after(source, "systems."))
        .expect("nested require path completion should succeed");

    assert_eq!(labels(&completions), vec!["systems.physics".to_owned()]);

    fs::remove_dir_all(root).expect("test dir should be removed");
}

#[test]
fn completes_fields_from_required_tua_and_lua_modules() {
    let root = temp_project_dir("required_module_fields");
    let src = root.join("src");
    let game = root.join("game");

    fs::create_dir_all(&src).expect("src dir should be created");
    fs::create_dir_all(&game).expect("game dir should be created");
    fs::write(root.join("tua.toml"), "source = \"src\"\nout = \"game\"\n")
        .expect("config should be written");
    fs::write(
        src.join("player.tua"),
        r#"
type Player = {
  hp?: number,
  pos?: { x: number },
}

local player: Player = {}

function player.damage(amount: number)
end

return player
"#,
    )
    .expect("Tua module should be written");
    fs::write(
        game.join("utils.lua"),
        r#"
local utils = {}

function utils.clamp(value, min, max)
end

utils.version = "1"

return utils
"#,
    )
    .expect("Lua module should be written");

    let main_path = src.join("main.tua");
    let source = r#"
local player: any = require("player")
local hp = player.hp

local utils = require("utils")
local value = utils.clamp
"#;

    let player_completions =
        complete_tua_source(&main_path, source, position_after(source, "player."))
            .expect("required Tua module completion should succeed");
    let utils_completions =
        complete_tua_source(&main_path, source, position_after(source, "utils."))
            .expect("required Lua module completion should succeed");

    assert_eq!(
        labels(&player_completions),
        vec!["damage".to_owned(), "hp".to_owned(), "pos".to_owned()]
    );
    assert_eq!(
        labels(&utils_completions),
        vec!["clamp".to_owned(), "version".to_owned()]
    );

    fs::remove_dir_all(root).expect("test dir should be removed");
}

#[test]
fn completes_nested_fields_from_required_modules() {
    let root = temp_project_dir("nested_required_module_fields");
    let src = root.join("src");

    fs::create_dir_all(&src).expect("src dir should be created");
    fs::write(root.join("tua.toml"), "source = \"src\"\n").expect("config should be written");
    fs::write(
        src.join("settings.lua"),
        r#"
return {
  audio = {
    volume = 1,
    muted = false,
  },
}
"#,
    )
    .expect("Lua module should be written");

    let main_path = src.join("main.tua");
    let source = r#"
local settings = require("settings")
local volume = settings.audio.volume
"#;
    let completions = complete_tua_source(
        &main_path,
        source,
        position_after(source, "settings.audio."),
    )
    .expect("nested required module completion should succeed");

    assert_eq!(
        labels(&completions),
        vec!["muted".to_owned(), "volume".to_owned()]
    );

    fs::remove_dir_all(root).expect("test dir should be removed");
}

#[test]
fn completes_required_module_fields_with_lua_doc_documentation() {
    let root = temp_project_dir("annotated_required_module_completion");
    let src = root.join("src");

    fs::create_dir_all(&src).expect("src dir should be created");
    fs::write(root.join("tua.toml"), "source = \"src\"\n").expect("config should be written");
    fs::write(
        src.join("audio.lua"),
        r#"---@param source string audio source
---@return boolean success flag
local function play(source)
  return true
end

return {
  play = play,
}
"#,
    )
    .expect("Lua module should be written");

    let main_path = src.join("main.tua");
    let source = r#"
local Audio = require("audio")
local value = Audio.play
"#;
    let completions = complete_tua_source(&main_path, source, position_after(source, "Audio."))
        .expect("annotated module completion should succeed");

    assert_eq!(labels(&completions), vec!["play".to_owned()]);
    assert_eq!(completions[0].kind, CompletionKind::Function);
    assert_eq!(
        completions[0].detail.as_deref(),
        Some("field: function(string): boolean")
    );
    assert_eq!(
        completions[0].documentation.as_deref(),
        Some("audio source\nsuccess flag")
    );

    fs::remove_dir_all(root).expect("test dir should be removed");
}

#[test]
fn completion_remains_quiet_for_any_values() {
    let source = format!("{SOURCE}local dynamic = payload.\n");
    let position = position_after(&source, "payload.");
    let completions = complete_tua_source("src/main.tua", source, position)
        .expect("completion should not fail for any values");

    assert!(completions.is_empty());
}

#[test]
fn completion_remains_quiet_for_unknown_dynamic_values() {
    let source = "local value = missing.\n";
    let position = position_after(source, "missing.");
    let completions = complete_tua_source("src/main.tua", source, position)
        .expect("completion should not fail for unknown values");

    assert!(completions.is_empty());
}

fn complete_at(needle: &str) -> Result<Vec<Completion>, Vec<Diagnostic>> {
    complete_tua_source("src/main.tua", SOURCE, position_after(SOURCE, needle))
}

fn labels(completions: &[Completion]) -> Vec<String> {
    completions
        .iter()
        .map(|completion| completion.label.clone())
        .collect()
}

fn position_after(source: &str, needle: &str) -> SourcePosition {
    let start = position_at(source, needle);

    SourcePosition {
        line: start.line,
        column: start.column + needle.chars().count(),
    }
}

fn position_after_last(source: &str, needle: &str) -> SourcePosition {
    let end = source.rfind(needle).expect("needle should exist in source") + needle.len();
    let prefix = &source[..end];
    let line = prefix
        .chars()
        .filter(|character| *character == '\n')
        .count()
        + 1;
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix, |(_, line)| line)
        .chars()
        .count()
        + 1;

    SourcePosition { line, column }
}

fn temp_project_dir(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();

    path.push(format!("tua_{name}_{}", std::process::id()));

    if path.exists() {
        fs::remove_dir_all(&path).expect("stale test dir should be removed");
    }

    path
}

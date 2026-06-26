use super::*;
use crate::source_text::position_at;
use std::fs;
use std::path::PathBuf;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

const SOURCE: &str = r#"type Vec2 = { x: number, y: number }
type Player = { pos: Vec2, hp: number }

local player: Player = {
  pos = { x = 0, y = 0 },
  hp = 10,
}

local function damage(target: Player, amount: number): Player
  target.hp -= amount
  return target
end

damage(player, 2)
"#;

#[test]
fn returns_known_function_signature_and_active_parameter() {
    let first = signature_at_last(SOURCE, "damage(").expect("first parameter should have help");
    let second =
        signature_at_last(SOURCE, "damage(player, ").expect("second parameter should have help");

    assert_eq!(
        first.signatures[0].label,
        "function damage(target: Player, amount: number): Player"
    );
    assert_eq!(
        parameter_labels(&first),
        vec!["target: Player", "amount: number"]
    );
    assert_eq!(first.active_parameter, 0);
    assert_eq!(second.active_parameter, 1);
}

#[test]
fn returns_signature_for_incomplete_call() {
    let source = r#"type Player = { hp: number }

local function damage(target: Player, amount: number): Player
  return target
end

damage(
"#;
    let help = signature_at_last(source, "damage(").expect("incomplete call should have help");

    assert_eq!(
        help.signatures[0].label,
        "function damage(target: Player, amount: number): Player"
    );
}

#[test]
fn returns_parameter_documentation_for_lua_doc_function() {
    let source = r#"type Player = { hp: number }

---@param target Player player receiving damage
---@param amount number damage amount
---@return Player updated player
local function damage(target, amount)
  return target
end

damage(player,
"#;
    let help =
        signature_at(source, "damage(player,").expect("documented function should have help");

    assert_eq!(
        help.signatures[0].label,
        "function damage(target: Player, amount: number): Player"
    );
    assert_eq!(
        parameter_documentation(&help),
        vec![Some("player receiving damage"), Some("damage amount")]
    );
    assert_eq!(
        help.signatures[0].documentation.as_deref(),
        Some("player receiving damage\ndamage amount\nupdated player")
    );
}

#[test]
fn returns_lua_doc_overload_signatures_and_selects_active_overload() {
    let source = r#"---@overload fun(path: string): Image single image
---@overload fun(path: string, mode: "stream"): Source streaming source
---@param path string default path
---@return Image default image
local function load(path)
  return image(path)
end

load("assets/player.png",
"#;
    let help = signature_at(source, "load(\"assets/player.png\",")
        .expect("overloaded function should have signature help");

    assert_eq!(help.signatures.len(), 3);
    assert_eq!(
        help.signatures[0].label,
        "function load(path: string): Image"
    );
    assert_eq!(
        help.signatures[1].label,
        "function load(path: string): Image"
    );
    assert_eq!(
        help.signatures[2].label,
        "function load(path: string, mode: \"stream\"): Source"
    );
    assert_eq!(help.active_signature, 2);
    assert_eq!(help.active_parameter, 1);
    assert_eq!(
        help.signatures[2].documentation.as_deref(),
        Some("streaming source")
    );
}

#[test]
fn returns_typed_lua_doc_vararg_signature() {
    let source = r#"---@param message string message text
---@vararg identifier extra labels
local function log(message, ...)
end

log("hello",
"#;
    let help =
        signature_at(source, "log(\"hello\",").expect("vararg function should have signature help");

    assert_eq!(
        help.signatures[0].label,
        "function log(message: string, ...: string): any"
    );
    assert_eq!(
        parameter_labels(&help),
        vec!["message: string", "...: string"]
    );
    assert_eq!(help.active_parameter, 1);
}

#[test]
fn returns_lua_and_love_builtin_signatures() {
    let lua =
        signature_at("local value = math.sin(\n", "math.sin(").expect("math.sin should have help");
    let rawlen =
        signature_at("local value = rawlen(\n", "rawlen(").expect("rawlen should have help");
    let warn =
        signature_at("warn(\"message\", \n", "warn(\"message\", ").expect("warn should have help");
    let love = signature_at(
        "love.graphics.print(\"hi\", \n",
        "love.graphics.print(\"hi\", ",
    )
    .expect("love.graphics.print should have help");

    assert_eq!(
        lua.signatures[0].label,
        "function math.sin(x: number): number"
    );
    assert_eq!(parameter_labels(&lua), vec!["x: number"]);
    assert_eq!(
        rawlen.signatures[0].label,
        "function rawlen(v: string | table): number"
    );
    assert_eq!(parameter_labels(&rawlen), vec!["v: string | table"]);
    assert_eq!(
        warn.signatures[0].label,
        "function warn(message: string, ...)"
    );
    assert_eq!(warn.active_parameter, 1);
    assert!(warn.signatures[0]
        .documentation
        .as_deref()
        .expect("warn should have documentation")
        .contains("pdf-warn"));
    assert_eq!(
        love.signatures[0].label,
        "function love.graphics.print(text: string, x?: number, y?: number, r?: number, sx?: number, sy?: number, ox?: number, oy?: number, kx?: number, ky?: number)"
    );
    assert_eq!(
        parameter_labels(&love),
        vec![
            "text: string",
            "x?: number",
            "y?: number",
            "r?: number",
            "sx?: number",
            "sy?: number",
            "ox?: number",
            "oy?: number",
            "kx?: number",
            "ky?: number"
        ]
    );
    assert_eq!(love.active_parameter, 1);
}

#[test]
fn returns_love_asset_loader_signatures() {
    let image = signature_at("local player = image(\n", "image(")
        .expect("image loader should have signature help");
    let sound = signature_at("local jump = sound(\n", "sound(")
        .expect("sound loader should have signature help");

    assert_eq!(
        image.signatures[0].label,
        "function image(path: string): Image"
    );
    assert_eq!(
        sound.signatures[0].label,
        "function sound(path: string): Source"
    );
}

#[test]
fn returns_builtin_vec2_constructor_signature() {
    let help = signature_at("local position = vec2(100, \n", "vec2(100, ")
        .expect("vec2 constructor should have signature help");

    assert_eq!(
        help.signatures[0].label,
        "function vec2(x: number, y: number): Vec2"
    );
    assert_eq!(parameter_labels(&help), vec!["x: number", "y: number"]);
    assert_eq!(help.active_parameter, 1);
}

#[test]
fn returns_love_object_method_signature_from_inferred_asset_loader_return() {
    let source = r#"
local function loadPlayerImage()
  return image "assets/player.png"
end

local player = loadPlayerImage()

player:setFilter(
"#;
    let help = signature_at(source, "setFilter(").expect("Image method should have help");

    assert_eq!(
        help.signatures[0].label,
        "function Image:setFilter(min: FilterMode, mag?: FilterMode, anisotropy?: number)"
    );
    assert_eq!(
        parameter_labels(&help),
        vec!["min: FilterMode", "mag?: FilterMode", "anisotropy?: number"]
    );
    assert_eq!(help.active_parameter, 0);
}

#[test]
fn returns_love_object_method_signature_from_direct_constructor_local() {
    let source = r#"
local player = love.graphics.newImage("assets/player.png")

player:setFilter(
"#;
    let help = signature_at(source, "setFilter(").expect("Image method should have help");

    assert_eq!(
        help.signatures[0].label,
        "function Image:setFilter(min: FilterMode, mag?: FilterMode, anisotropy?: number)"
    );
    assert_eq!(
        parameter_labels(&help),
        vec!["min: FilterMode", "mag?: FilterMode", "anisotropy?: number"]
    );
}

#[test]
fn returns_typed_colon_method_signature_without_visible_self() {
    let source = r#"
type Account = {
  balance: number,
}

local Account = {}
Account.__index = Account

function Account:withdraw(amount: number): number
  self.balance -= amount
  return self.balance
end

local account = setmetatable({ balance = 100 }, Account)

account:withdraw(
"#;
    let help =
        signature_at(source, "account:withdraw(").expect("typed colon method should have help");

    assert_eq!(
        help.signatures[0].label,
        "function account:withdraw(number): number"
    );
    assert_eq!(parameter_labels(&help), vec!["number"]);
    assert_eq!(help.active_parameter, 0);
}

#[test]
fn selects_love_object_method_active_parameter() {
    let source = r#"
local player = image "assets/player.png"
player:setFilter("nearest",
"#;
    let help =
        signature_at(source, "nearest\",").expect("Image method should track active parameter");

    assert_eq!(help.active_parameter, 1);
}

#[test]
fn keeps_active_parameter_through_nested_calls_comments_and_strings() {
    let source = r#"local function damage(target: string, amount: number): string
  return target
end

damage(format("a,b"), -- comma, in comment
  0
)
"#;
    let help = signature_at(source, "comma, in comment")
        .expect("nested call with comment should still have help");

    assert_eq!(help.active_parameter, 1);
}

#[test]
fn returns_required_module_function_signature() {
    let temp = TempProject::new("required-module-signature");
    let source = r#"local Player = require("player")

local player = Player.makePlayer(
"#;
    let player = r#"type Player = { name: string, hp: number }

local function makePlayer(name: string): Player
  return { name = name, hp = 10 }
end

return {
  makePlayer = makePlayer,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/player.tua", player);

    let help = signature_help_tua_source(
        temp.path.join("src/main.tua"),
        source,
        position_after(source, "Player.makePlayer("),
    )
    .expect("signature help should succeed")
    .expect("module signature should resolve");

    assert_eq!(
        help.signatures[0].label,
        "function Player.makePlayer(string): Player"
    );
    assert_eq!(parameter_labels(&help), vec!["string"]);
}

#[test]
fn returns_annotated_lua_module_function_signature_and_docs() {
    let temp = TempProject::new("annotated-module-signature");
    let source = r#"local Audio = require("audio")

local ok = Audio.play(
"#;
    let audio = r#"---@param source string audio source
---@return boolean success flag
local function play(source)
  return true
end

return {
  play = play,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/audio.lua", audio);

    let help = signature_help_tua_source(
        temp.path.join("src/main.tua"),
        source,
        position_after(source, "Audio.play("),
    )
    .expect("signature help should succeed")
    .expect("annotated Lua module signature should resolve");

    assert_eq!(
        help.signatures[0].label,
        "function Audio.play(string): boolean"
    );
    assert_eq!(parameter_labels(&help), vec!["string"]);
    assert_eq!(parameter_documentation(&help), vec![Some("audio source")]);
    assert_eq!(
        help.signatures[0].documentation.as_deref(),
        Some("audio source\nsuccess flag")
    );
}

#[test]
fn returns_annotated_tua_module_function_parameter_docs() {
    let temp = TempProject::new("annotated-tua-module-signature");
    let source = r#"local Player = require("player")

local player = Player.makePlayer(
"#;
    let player = r#"type Player = { name: string, hp: number }

---@param name string player name
---@return Player created player
local function makePlayer(name)
  return { name = name, hp = 10 }
end

return {
  makePlayer = makePlayer,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/player.tua", player);

    let help = signature_help_tua_source(
        temp.path.join("src/main.tua"),
        source,
        position_after(source, "Player.makePlayer("),
    )
    .expect("signature help should succeed")
    .expect("annotated Tua module signature should resolve");

    assert_eq!(
        help.signatures[0].label,
        "function Player.makePlayer(string): Player"
    );
    assert_eq!(parameter_documentation(&help), vec![Some("player name")]);
}

#[test]
fn unknown_call_returns_none() {
    let source = "missing(\n";
    let help =
        signature_help_tua_source("src/main.tua", source, position_after(source, "missing("))
            .expect("unknown call should not fail");

    assert_eq!(help, None);
}

fn signature_at(source: &str, needle: &str) -> Option<SignatureHelp> {
    signature_help_tua_source("src/main.tua", source, position_after(source, needle))
        .expect("signature help should not fail")
}

fn signature_at_last(source: &str, needle: &str) -> Option<SignatureHelp> {
    signature_help_tua_source("src/main.tua", source, position_after_last(source, needle))
        .expect("signature help should not fail")
}

fn parameter_labels(help: &SignatureHelp) -> Vec<&str> {
    help.signatures[0]
        .parameters
        .iter()
        .map(|parameter| parameter.label.as_str())
        .collect()
}

fn parameter_documentation(help: &SignatureHelp) -> Vec<Option<&str>> {
    help.signatures[0]
        .parameters
        .iter()
        .map(|parameter| parameter.documentation.as_deref())
        .collect()
}

fn position_after(source: &str, needle: &str) -> SourcePosition {
    let mut position = position_at(source, needle);

    position.column += needle.chars().count();
    position
}

fn position_after_last(source: &str, needle: &str) -> SourcePosition {
    let index = source.rfind(needle).expect("needle should exist in source");
    let before = &source[..index];
    let mut position = SourcePosition {
        line: before.bytes().filter(|byte| *byte == b'\n').count() + 1,
        column: before
            .rsplit_once('\n')
            .map(|(_, line)| line.len())
            .unwrap_or(before.len())
            + 1,
    };

    position.column += needle.chars().count();
    position
}

struct TempProject {
    path: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "tua-core-signature-help-{name}-{}-{unique}",
            process::id()
        ));

        fs::create_dir_all(&path).expect("temp project should be created");

        Self { path }
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.path.join(relative);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent directory should be created");
        }

        fs::write(path, contents).expect("file should be written");
    }
}

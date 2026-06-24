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
player.hp = player.hp - 1
"#;

#[test]
fn hovers_known_type_alias() {
    let hover = hover_at("Vec2").expect("alias should hover");

    assert_eq!(hover.contents, "type Vec2 = { x: number, y: number }");
}

#[test]
fn hovers_known_local() {
    let hover = hover_at("player:").expect("local should hover");

    assert_eq!(hover.contents, "local player: Player");
}

#[test]
fn hovers_function_parameters() {
    let source = r#"local function recursive(a: string)
  if a == tostring(1) then
    return 1
  end

  if a == tostring(2) then
    return "value"
  end

  return recursive(a + 1)
end
"#;

    assert_hover_in_source(source, "a: string", 0, "parameter a: string");
    assert_hover_in_source(source, "a == tostring", 0, "parameter a: string");
    assert_hover_in_source(source, "a + 1", 0, "parameter a: string");
}

#[test]
fn hovers_innermost_matching_parameter() {
    let source = r#"local function outer(value: string)
  local function inner(value: number)
    return value
  end

  return value
end
"#;

    assert_hover_in_source(
        source,
        "return value",
        "return ".len(),
        "parameter value: number",
    );
    assert_hover_in_source(
        source,
        "return value\nend",
        "return ".len(),
        "parameter value: string",
    );
}

#[test]
fn hovers_flow_sensitive_narrowed_types_inside_guards() {
    let source = r#"local function read(value: number | nil, tagged: string | number)
  if value ~= nil then
    local copy = value
  end

  if type(tagged) == "string" then
    local label = tagged
  end
end
"#;

    assert_hover_in_source(
        source,
        "local copy = value",
        "local copy = ".len(),
        "parameter value: number",
    );
    assert_hover_in_source(
        source,
        "local label = tagged",
        "local label = ".len(),
        "parameter tagged: string",
    );
}

#[test]
fn hover_uses_assigned_type_after_mutation_in_narrowed_block() {
    let source = r#"local function read(value: number | nil)
  if value ~= nil then
    value = nil
    local copy = value
  end
end
"#;

    assert_hover_in_source(
        source,
        "local copy = value",
        "local copy = ".len(),
        "parameter value: number | nil",
    );
}

#[test]
fn parameter_hover_follows_definition_identity() {
    let source = r#"local value: number = 1

local function before_shadow(value: string)
  local result = value
  local value: boolean = true
  return value
end
"#;

    assert_hover_in_source(
        source,
        "local result = value",
        "local result = ".len(),
        "parameter value: string",
    );
    assert_hover_in_source(
        source,
        "return value",
        "return ".len(),
        "local value: boolean",
    );
}

#[test]
fn hovers_unannotated_local_values_returned_from_table() {
    let source = r#"
type Player = { hp: number }

local function makePlayer(): Player
  return { hp = 10 }
end

local player = makePlayer()
local clamped = 10
local message = tostring(clamped)
local built_in = tostring(1)

return {
  player = player,
  clamped = clamped,
  message = message,
  built_in = built_in,
}
"#;

    assert_hover_in_source(
        source,
        "player = player",
        "player = ".len(),
        "local player: Player",
    );
    assert_hover_in_source(
        source,
        "clamped = clamped",
        "clamped = ".len(),
        "local clamped: number",
    );
    assert_hover_in_source(
        source,
        "message = message",
        "message = ".len(),
        "local message: string",
    );
    assert_hover_in_source(
        source,
        "built_in = built_in",
        "built_in = ".len(),
        "local built_in: string",
    );
}

#[test]
fn hovers_known_function_signature() {
    let hover = hover_at("damage(player").expect("function should hover");

    assert_eq!(
        hover.contents,
        "function damage(target: Player, amount: number): Player"
    );
}

#[test]
fn hovers_annotated_function_docs_and_declaration_context() {
    let source = r#"type Player = { hp: number }

--- Applies damage.
---@param target Player affected player
---@return Player updated player
local function damage(target)
  return target
end

damage({ hp = 10 })
"#;
    let hover = hover_tua_source("src/main.tua", source, position_at(source, "damage({"))
        .expect("hover should succeed")
        .expect("annotated function should hover");

    assert_eq!(hover.contents, "function damage(target: Player): Player");
    assert_eq!(
        hover.documentation.as_deref(),
        Some("Applies damage.\naffected player\nupdated player")
    );
    assert!(hover.context.contains(&"kind: function".to_owned()));
    assert!(hover
        .context
        .iter()
        .any(|item| item.contains("declared: src/main.tua:")));
}

#[test]
fn hovers_inferred_function_return_type_on_definition_and_call() {
    let source = r#"function test(a: boolean)
  return ""
end

test(false)
"#;
    let definition = hover_tua_source("src/main.tua", source, position_at(source, "test(a"))
        .expect("hover should succeed")
        .expect("function definition should hover");
    let call = hover_tua_source("src/main.tua", source, position_at(source, "test(false"))
        .expect("hover should succeed")
        .expect("function call should hover");

    assert_eq!(definition.contents, "function test(a: boolean): string");
    assert_eq!(call.contents, "function test(a: boolean): string");
}

#[test]
fn hovers_local_function_return_type_inferred_from_parameter() {
    let source = r#"local function incomplete(value: string)
  return value
end

incomplete("ok")
"#;
    let definition = hover_tua_source(
        "src/main.tua",
        source,
        position_at(source, "incomplete(value"),
    )
    .expect("hover should succeed")
    .expect("function definition should hover");
    let call = hover_tua_source(
        "src/main.tua",
        source,
        position_at(source, "incomplete(\"ok\")"),
    )
    .expect("hover should succeed")
    .expect("function call should hover");

    assert_eq!(
        definition.contents,
        "function incomplete(value: string): string"
    );
    assert_eq!(call.contents, "function incomplete(value: string): string");
}

#[test]
fn hovers_local_function_return_types_inferred_from_required_module_exports() {
    let temp = TempProject::new("required-module-return-hover");
    let source = r#"local Functions = require("functions")

local function macarrons()
  return Functions.makePlayer("lua interop")
end

local function make_function()
  return Functions.makePlayer
end

macarrons()
make_function()
"#;
    let functions = r#"type Player = {
  name: string,
  hp: number,
}

local function makePlayer(name: string): Player
  return {
    name = name,
    hp = 100,
  }
end

return {
  makePlayer = makePlayer,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/functions.tua", functions);

    let main_path = temp.path.join("src/main.tua");
    let macarrons = hover_tua_source(&main_path, source, position_at(source, "macarrons()"))
        .expect("hover should succeed")
        .expect("module call return should hover");
    let make_function =
        hover_tua_source(&main_path, source, position_at(source, "make_function()"))
            .expect("hover should succeed")
            .expect("module function value return should hover");

    assert_eq!(macarrons.contents, "function macarrons(): Player");
    assert_eq!(
        make_function.contents,
        "function make_function(): function(string): Player"
    );
}

#[test]
fn hovers_union_return_type_from_conditional_returns() {
    let source = r#"local function recursive(a: string)
  if a == 1 then
    return 1
  end

  if a == 2 then
    return "value"
  end
end

recursive("1")
"#;
    let hover = hover_tua_source(
        "src/main.tua",
        source,
        position_at(source, "recursive(\"1\")"),
    )
    .expect("hover should succeed")
    .expect("recursive function should hover");

    assert_eq!(
        hover.contents,
        "function recursive(a: string): number | string | nil"
    );
}

#[test]
fn hovers_union_return_type_from_recursive_conditional_returns() {
    let source = r#"local function recursive(a: string)
  if a == 1 then
    return 1
  end

  if a == 2 then
    return "value"
  end

  return recursive(a + 1)
end

recursive("1")
"#;
    let hover = hover_tua_source(
        "src/main.tua",
        source,
        position_at(source, "recursive(\"1\")"),
    )
    .expect("hover should succeed")
    .expect("recursive function should hover");

    assert_eq!(
        hover.contents,
        "function recursive(a: string): number | string"
    );
}

#[test]
fn hovers_known_table_field() {
    let hover = hover_at("hp = player").expect("table field should hover");

    assert_eq!(hover.contents, "(field) hp: number");
}

#[test]
fn unknown_symbols_do_not_hover() {
    let source = "local result = missing\n";
    let position = position_at(source, "missing");
    let hover = hover_tua_source("src/main.tua", source, position)
        .expect("hover should not fail for unknown values");

    assert_eq!(hover, None);
}

#[test]
fn hovers_lua_standard_library_items() {
    let source = "local value = math.sin(1)\n";
    let position = position_at(source, "sin");
    let hover = hover_tua_source("src/main.tua", source, position)
        .expect("hover should not fail")
        .expect("math.sin should hover");

    assert_eq!(hover.contents, "function math.sin(x: number): number");
}

#[test]
fn hovers_extended_lua_globals() {
    let source = "_ENV.raw = rawlen({})\nwarn(\"careful\")\n";
    let environment = hover_tua_source("src/main.tua", source, position_at(source, "_ENV"))
        .expect("hover should not fail")
        .expect("_ENV should hover");
    let rawlen = hover_tua_source("src/main.tua", source, position_at(source, "rawlen"))
        .expect("hover should not fail")
        .expect("rawlen should hover");
    let warn = hover_tua_source("src/main.tua", source, position_at(source, "warn"))
        .expect("hover should not fail")
        .expect("warn should hover");

    assert_eq!(environment.contents, "global environment table");
    assert_eq!(
        rawlen.contents,
        "function rawlen(v: string | table): number"
    );
    assert_eq!(warn.contents, "function warn(message: string, ...)");
}

#[test]
fn hovers_love_items() {
    let source = "love.graphics.print(\"hi\", 0, 0)\n";
    let position = position_at(source, "print");
    let hover = hover_tua_source("src/main.tua", source, position)
        .expect("hover should not fail")
        .expect("love.graphics.print should hover");

    assert_eq!(
        hover.contents,
        "function love.graphics.print(text: string, x?: number, y?: number, r?: number, sx?: number, sy?: number, ox?: number, oy?: number, kx?: number, ky?: number)"
    );
    assert!(hover
        .documentation
        .as_deref()
        .unwrap_or_default()
        .contains("Draws text on screen"));
}

#[test]
fn hovers_love_asset_loader_items_and_inferred_locals() {
    let source = r#"
local player = image "assets/player.png"
local jump = sound "assets/jump.wav"
"#;
    let image = hover_tua_source("src/main.tua", source, position_at(source, "image"))
        .expect("hover should not fail")
        .expect("image loader should hover");
    let player = hover_tua_source("src/main.tua", source, position_at(source, "player"))
        .expect("hover should not fail")
        .expect("inferred image local should hover");
    let jump = hover_tua_source("src/main.tua", source, position_at(source, "jump"))
        .expect("hover should not fail")
        .expect("inferred sound local should hover");

    assert_eq!(image.contents, "function image(path: string): Image");
    assert_eq!(player.contents, "local player: Image");
    assert_eq!(jump.contents, "local jump: Source");
}

#[test]
fn hovers_love_object_methods_from_inferred_asset_loader_returns() {
    let source = r#"
local function loadPlayerImage()
  return image "assets/player.png"
end

local player = loadPlayerImage()

player:setFilter()
"#;
    let hover = hover_tua_source("src/main.tua", source, position_at(source, "setFilter"))
        .expect("hover should not fail")
        .expect("Image method should hover");

    assert_eq!(
        hover.contents,
        "function Image:setFilter(min: FilterMode, mag?: FilterMode, anisotropy?: number)"
    );
    assert!(hover
        .documentation
        .as_deref()
        .unwrap_or_default()
        .contains("filter mode"));
}

#[test]
fn hovers_love_object_methods_from_direct_constructor_locals() {
    let source = r#"
local player = love.graphics.newImage("assets/player.png")

player:setFilter()
"#;
    let hover = hover_tua_source("src/main.tua", source, position_at(source, "setFilter"))
        .expect("hover should not fail")
        .expect("Image method should hover");

    assert_eq!(
        hover.contents,
        "function Image:setFilter(min: FilterMode, mag?: FilterMode, anisotropy?: number)"
    );
}

#[test]
fn hovers_love_object_method_return_types_from_inferred_asset_loader_returns() {
    let source = r#"
local player = image "assets/hero.png"
local w = player:getWidth()
local a, b = player.getDimensions()
local minFilter, magFilter, anisotropy = player:getFilter()
local copied = anisotropy
"#;
    let width = hover_tua_source("src/main.tua", source, position_at(source, "w ="))
        .expect("hover should not fail")
        .expect("width local should hover");
    let first_dimension = hover_tua_source("src/main.tua", source, position_at(source, "a, b"))
        .expect("hover should not fail")
        .expect("first dimension should hover");
    let second_dimension = hover_tua_source("src/main.tua", source, position_at(source, "b ="))
        .expect("hover should not fail")
        .expect("second dimension should hover");
    let copied = hover_tua_source("src/main.tua", source, position_at(source, "copied ="))
        .expect("hover should not fail")
        .expect("copied return should hover");

    assert_eq!(width.contents, "local w: number");
    assert_eq!(first_dimension.contents, "local a: number");
    assert_eq!(second_dimension.contents, "local b: number");
    assert_eq!(copied.contents, "local copied: number");
}

#[test]
fn hovers_love_method_return_type_through_local_function_body() {
    let source = r#"
local function loadPlayerImage()
  return image "assets/player.png"
end

local function loadJumpSound()
  return sound "assets/jump.wav"
end

local player = loadPlayerImage()

local a, b = player.getDimensions()

local function azul()
  return player.getDimensions()
end

local c = azul()
local z = c + b
local literal = 1 + 3
local label = "width: " .. z

return {
  loadPlayerImage = loadPlayerImage,
  loadJumpSound = loadJumpSound,
}
"#;
    let function = hover_tua_source("src/main.tua", source, position_at(source, "azul()"))
        .expect("hover should not fail")
        .expect("azul should hover");
    let local = hover_tua_source("src/main.tua", source, position_at(source, "c ="))
        .expect("hover should not fail")
        .expect("c should hover");
    let sum = hover_tua_source("src/main.tua", source, position_at(source, "z ="))
        .expect("hover should not fail")
        .expect("z should hover");
    let literal = hover_tua_source("src/main.tua", source, position_at(source, "literal ="))
        .expect("hover should not fail")
        .expect("literal should hover");
    let label = hover_tua_source("src/main.tua", source, position_at(source, "label ="))
        .expect("hover should not fail")
        .expect("label should hover");

    assert_eq!(function.contents, "function azul(): number, number");
    assert_eq!(local.contents, "local c: number");
    assert_eq!(sum.contents, "local z: number");
    assert_eq!(literal.contents, "local literal: number");
    assert_eq!(label.contents, "local label: string");
}

#[test]
fn local_symbols_shadow_lua_global_hover() {
    let source = "local math: any = {}\nlocal value = math.sin\n";
    let position = position_at(source, "sin");
    let hover = hover_tua_source("src/main.tua", source, position)
        .expect("hover should not fail for shadowed values");

    assert_eq!(hover, None);
}

#[test]
fn hovers_required_module_literal_binding_and_tua_exports() {
    let temp = TempProject::new("required-module-hover");
    let source = r#"local Player = require("player")

local player = Player.makePlayer("Ada")
local hp = Player.defaultHp
"#;
    let player = r#"type Player = { name: string, hp: number }

local function makePlayer(name: string): Player
  return { name = name, hp = 10 }
end

return {
  makePlayer = makePlayer,
  defaultHp = 10,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/player.tua", player);

    let main_path = temp.path.join("src/main.tua");
    let literal = hover_tua_source(&main_path, source, position_at(source, "player\")"))
        .expect("hover should not fail")
        .expect("require literal should hover");
    let binding = hover_tua_source(&main_path, source, position_at(source, "Player.makePlayer"))
        .expect("hover should not fail")
        .expect("module binding should hover");
    let function = hover_tua_source(&main_path, source, position_at(source, "makePlayer("))
        .expect("hover should not fail")
        .expect("module function should hover");
    let field = hover_tua_source(&main_path, source, position_at(source, "defaultHp"))
        .expect("hover should not fail")
        .expect("module field should hover");

    assert!(literal.contents.contains("(module) player ->"));
    assert!(literal.contents.contains("player.tua"));
    assert!(binding
        .contents
        .contains("(module) Player = require(\"player\") ->"));
    assert_eq!(
        function.contents,
        "function Player.makePlayer(string): Player"
    );
    assert_eq!(field.contents, "(field) Player.defaultHp: number");
}

#[test]
fn hovers_required_lua_module_exports() {
    let temp = TempProject::new("required-lua-module-hover");
    let source = r#"local Legacy = require("legacy")

local label = Legacy.describe("hp", 10)
"#;
    let legacy = r#"return {
  describe = function(label, value)
    return label
  end,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/legacy.lua", legacy);

    let main_path = temp.path.join("src/main.tua");
    let hover = hover_tua_source(&main_path, source, position_at(source, "describe("))
        .expect("hover should not fail")
        .expect("Lua module function should hover");

    assert_eq!(hover.contents, "function Legacy.describe(any, any): any");
}

#[test]
fn hovers_annotated_lua_module_export_docs() {
    let temp = TempProject::new("annotated-required-lua-module-hover");
    let source = r#"local Audio = require("audio")
local ok = Audio.play("theme")
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
    let main_path = temp.path.join("src/main.tua");

    let hover = hover_tua_source(&main_path, source, position_at(source, "play("))
        .expect("hover should not fail")
        .expect("annotated Lua module function should hover");

    assert_eq!(hover.contents, "function Audio.play(string): boolean");
    assert_eq!(
        hover.documentation.as_deref(),
        Some("audio source\nsuccess flag")
    );
    assert!(hover.context.iter().any(|item| item.contains("audio.lua")));
    assert!(hover.context.contains(&"kind: function".to_owned()));
}

#[test]
fn hovers_module_export_plain_doc_comment_and_origin_context() {
    let temp = TempProject::new("module-export-doc-hover");
    let source = r#"local Audio = require("audio")
local ok = Audio.play("theme")
"#;
    let audio = r#"--- Plays a source.
---@param source string audio source
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

    let hover = hover_tua_source(
        temp.path.join("src/main.tua"),
        source,
        position_at(source, "play("),
    )
    .expect("hover should not fail")
    .expect("module export should hover");

    assert_eq!(hover.contents, "function Audio.play(string): boolean");
    assert_eq!(
        hover.documentation.as_deref(),
        Some("Plays a source.\naudio source\nsuccess flag")
    );
    assert!(hover.context.contains(&"kind: function".to_owned()));
    assert!(hover.context.iter().any(|item| item.contains("module:")));
    assert!(hover.context.iter().any(|item| item.contains("declared:")));
}

#[test]
fn unresolved_required_module_fields_do_not_hover() {
    let temp = TempProject::new("missing-required-module-hover");
    let source = r#"local Missing = require("missing")

local value = Missing.nope
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);

    let hover = hover_tua_source(
        temp.path.join("src/main.tua"),
        source,
        position_at(source, "nope"),
    )
    .expect("hover should not fail");

    assert_eq!(hover, None);
}

fn hover_at(needle: &str) -> Option<Hover> {
    let position = position_at(SOURCE, needle);

    hover_tua_source("src/main.tua", SOURCE, position).expect("source should parse and type")
}

fn assert_hover_in_source(source: &str, needle: &str, offset: usize, expected: &str) {
    let mut position = position_at(source, needle);

    position.column += offset;

    let hover = hover_tua_source("src/main.tua", source, position)
        .expect("source should parse")
        .expect("hover should resolve");

    assert_eq!(hover.contents, expected);
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
        let path =
            std::env::temp_dir().join(format!("tua-core-hover-{name}-{}-{unique}", process::id()));

        fs::create_dir_all(&path).expect("temp project should be created");

        Self { path }
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.path.join(relative);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent directory should be created");
        }

        fs::write(path, contents).expect("fixture should be written");
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

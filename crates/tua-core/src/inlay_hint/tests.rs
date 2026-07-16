use super::*;
use std::fs;
use std::path::PathBuf;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

const SOURCE: &str = r#"
type Player = { hp: number }

local function makePlayer(): Player
  return { hp = 10 }
end

local function damage(target: Player, amount: number): Player
  target.hp -= amount
  return target
end

local player = makePlayer()
local hp = player.hp
local total = 10
local named: Player = makePlayer()
local unknown = missing()

damage(player, 10)
"#;

#[test]
fn hints_unannotated_local_types_from_known_expressions() {
    let hints = inlay_hints();
    let type_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Type)
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(type_hints, vec![": Player", ": number", ": number"]);
    assert_hint_position(&hints, ": Player", 13, "local player".len() + 1);
    assert_hint_position(&hints, ": number", 14, "local hp".len() + 1);
    assert_hint_position(&hints, ": number", 15, "local total".len() + 1);
}

#[test]
fn hints_anonymous_function_valued_table_fields() {
    let source = r#"
local lookupObject = {
  count = 0,
  y = function(self:any, amount:number)
    self.count = self.count + amount

    return self.count
  end,
}
"#;
    let hints = inlay_hints_tua_source("src/methods.tua", source).expect("source should parse");
    let type_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Type)
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        type_hints,
        vec![": { count: number, y: (self: any, amount: number) -> any }"]
    );
}

#[test]
fn hints_loop_variable_types_from_numeric_ipairs_and_pairs_loops() {
    let source = r#"
local numbers = { 1, 2, 3 }
local player = { name = "Ada", score = 10 }

for i = 1, #numbers do
  local indexed = numbers[i]
end

for _, value in ipairs(numbers) do
  local doubled = value + 1
end

for key, value in pairs(player) do
  local seen = value
end
"#;
    let hints = inlay_hints_tua_source("src/loops.tua", source).expect("source should parse");
    let type_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Type)
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();

    assert!(type_hints.contains(&": number[]"));
    assert!(type_hints.contains(&": { name: string, score: number }"));
    assert!(type_hints.contains(&": number"));
    assert!(type_hints.contains(&": \"name\" | \"score\""));
    assert!(type_hints.contains(&": string | number"));
}

#[test]
fn hints_loop_variable_types_from_iterator_metamethods() {
    let source = r#"
---@param value any
---@return ["name" | "score", number]
local function iterPairs(value)
end

---@param value any
---@return [number, string]
local function iterIpairs(value)
end

local typedMap = {}
setmetatable(typedMap, { __pairs = iterPairs })

local typedArray = {}
setmetatable(typedArray, { __ipairs = iterIpairs })

for key, value in pairs(typedMap) do
  local seenKey = key
  local seenValue = value
end

for index, value in ipairs(typedArray) do
  local seenIndex = index
  local seenItem = value
end
"#;
    let hints = inlay_hints_tua_source("src/iterators.tua", source).expect("source should parse");
    let type_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Type)
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();
    assert!(type_hints.contains(&": \"name\" | \"score\""));
    assert!(type_hints.contains(&": number"));
    assert!(type_hints.contains(&": string"));
}

#[test]
fn skips_annotated_unknown_and_any_local_type_hints() {
    let hints = inlay_hints();

    assert!(!hints_at_line(&hints, 16).any(|hint| hint.kind == InlayHintKind::Type));
    assert!(!hints_at_line(&hints, 17).any(|hint| hint.kind == InlayHintKind::Type));
}

#[test]
fn hints_parameter_names_for_known_calls() {
    let hints = inlay_hints();
    let parameter_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Parameter)
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(parameter_hints, vec!["target:", "amount:"]);
    assert_hint_position(&hints, "target:", 19, "damage(".len() + 1);
    assert_hint_position(&hints, "amount:", 19, "damage(player, ".len() + 1);
}

#[test]
fn hints_colon_method_parameters_from_self_index_prototype() {
    let source = r#"
type Account = {
  balance: number,
}

local Account = {}
Account.__index = Account

function Account:deposit(amount: number): number
  self.balance = self.balance + amount

  return self.balance
end

local account = setmetatable({ balance = 50 }, Account)
account:deposit(25)
"#;
    let hints = inlay_hints_tua_source("src/account.tua", source).expect("source should parse");
    let parameter_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Parameter)
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();

    assert!(parameter_hints.contains(&"amount:"));
    assert!(!parameter_hints.contains(&"self:"));
    assert_hint_position(&hints, "amount:", 16, "account:deposit(".len() + 1);
}

#[test]
fn hints_love_asset_loader_local_types() {
    let source = r#"
local player = image "assets/player.png"
local jump = sound "assets/jump.wav"
local theme = music "assets/theme.ogg"
local mainFont = font("assets/ui.ttf", 18)
local wave = shader "assets/wave.glsl"
local data = imageData "assets/player.png"
local target = canvas(320, 180)
local frame = quad(0, 0, 16, 16, 64, 64)
local batch = spriteBatch(player, 128)
local sparks = particles(player, 64)
"#;
    let hints = inlay_hints_tua_source("src/assets.tua", source).expect("source should parse");
    let type_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Type)
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        type_hints,
        vec![
            ": Image",
            ": Source",
            ": Source",
            ": Font",
            ": Shader",
            ": ImageData",
            ": Canvas",
            ": Quad",
            ": SpriteBatch",
            ": ParticleSystem"
        ]
    );
}

#[test]
fn hints_direct_love_constructor_local_types() {
    let source = r#"
local player = love.graphics.newImage("assets/player.png")
local canvas = love.graphics.newCanvas()
local font = love.graphics.newFont("assets/sniglet.otf", 16)
local data = love.image.newImageData(16, 16)
local world = love.physics.newWorld()
local body = love.physics.newBody(world, 0, 0)
"#;
    let hints = inlay_hints_tua_source("src/assets.tua", source).expect("source should parse");
    let type_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Type)
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        type_hints,
        vec![
            ": Image",
            ": Canvas",
            ": Font",
            ": ImageData",
            ": World",
            ": Body"
        ]
    );
}

#[test]
fn hints_love_object_method_return_types_from_inferred_asset_loader_returns() {
    let source = r#"
local player = image "assets/hero.png"
local w = player:getWidth()
local a, b = player.getDimensions()
local minFilter, magFilter, anisotropy = player:getFilter()
"#;
    let hints = inlay_hints_tua_source("src/assets.tua", source).expect("source should parse");
    let type_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Type)
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        type_hints,
        vec![
            ": Image",
            ": number",
            ": number",
            ": number",
            ": FilterMode",
            ": FilterMode",
            ": number"
        ]
    );
}

#[test]
fn hints_love_method_return_type_through_local_function_body() {
    let source = r#"
local function loadPlayerImage()
  return image "assets/player.png"
end

local player = loadPlayerImage()

local function azul()
  return player.getDimensions()
end

local c = azul()
local z = c + 1
local literal = 1 + 3
local label = "width: " .. z
"#;
    let hints = inlay_hints_tua_source("src/assets.tua", source).expect("source should parse");
    let type_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Type)
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        type_hints,
        vec![": Image", ": number", ": number", ": number", ": string"]
    );
}

#[test]
fn hints_required_tua_and_lua_module_call_parameters() {
    let temp = TempProject::new("required-module-inlay-hints");
    let source = r#"local Player = require("entities.player")
local Legacy = require("legacy")

local player = Player.new("Ada")
local label = Legacy.label(player.name)
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write(
        "src/entities/player.tua",
        r#"type Player = {
  name: string,
  hp: number,
}

local M = {}

function M.new(name: string): Player
  return {
    name = name,
    hp = 100,
  }
end

return M
"#,
    );
    temp.write(
        "src/legacy.lua",
        r#"local M = {}

---@param name number
---@return string
function M.label(name)
  return "player:" .. name
end

return M
"#,
    );

    let hints = inlay_hints_tua_source(temp.path.join("src/main.tua"), source)
        .expect("source should parse");
    let parameter_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Parameter)
        .filter(|hint| hint.label == "name:")
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(parameter_hints, vec!["name:", "name:"]);
    assert_hint_position(&hints, "name:", 4, "local player = Player.new(".len() + 1);
    assert_hint_position(&hints, "name:", 5, "local label = Legacy.label(".len() + 1);
}

#[test]
fn hints_required_module_alias_and_inline_lua_export_call_parameters() {
    let temp = TempProject::new("required-module-alias-inlay-hints");
    let source = r#"local Functions = require("functions")
local Legacy = require("legacy")

local makePlayer = Functions.makePlayer
local player = makePlayer("require graph")
local description = Legacy.describe("hp", player.hp)
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write(
        "src/functions.tua",
        r#"type Player = {
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
"#,
    );
    temp.write(
        "src/legacy.lua",
        r#"return {
  describe = function(label, value)
    return label
  end,
}
"#,
    );

    let hints = inlay_hints_tua_source(temp.path.join("src/main.tua"), source)
        .expect("source should parse");
    let parameter_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Parameter)
        .filter(|hint| hint.label != "modname:")
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(parameter_hints, vec!["name:", "label:", "value:"]);
    assert_hint_position(&hints, "name:", 5, "local player = makePlayer(".len() + 1);
    assert_hint_position(
        &hints,
        "label:",
        6,
        "local description = Legacy.describe(".len() + 1,
    );
    assert_hint_position(
        &hints,
        "value:",
        6,
        "local description = Legacy.describe(\"hp\", ".len() + 1,
    );
}

#[test]
fn hints_imported_module_alias_call_parameters() {
    let temp = TempProject::new("imported-module-alias-inlay-hints");
    let source = r#"local Functions = import("./functions")

local makePlayer = Functions.makePlayer
local player = makePlayer("import graph")
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write(
        "src/functions.tua",
        r#"type Player = {
  name: string,
}

local function makePlayer(name: string): Player
  return {
    name = name,
  }
end

return {
  makePlayer = makePlayer,
}
"#,
    );

    let hints = inlay_hints_tua_source(temp.path.join("src/main.tua"), source)
        .expect("source should parse");

    assert!(hints
        .iter()
        .any(|hint| hint.kind == InlayHintKind::Parameter && hint.label == "name:"));
    assert_hint_position(&hints, "name:", 4, "local player = makePlayer(".len() + 1);
}

fn inlay_hints() -> Vec<InlayHint> {
    inlay_hints_tua_source("src/player.tua", SOURCE).expect("source should parse")
}

fn hints_at_line(hints: &[InlayHint], line: usize) -> impl Iterator<Item = &InlayHint> {
    hints.iter().filter(move |hint| hint.position.line == line)
}

fn assert_hint_position(hints: &[InlayHint], label: &str, line: usize, column: usize) {
    let hint = hints
        .iter()
        .find(|hint| hint.label == label && hint.position.line == line)
        .unwrap_or_else(|| panic!("expected hint `{label}` on line {line}"));

    assert_eq!(hint.position.column, column);
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
        let path = std::env::temp_dir().join(format!("tua-core-{name}-{}-{unique}", process::id()));

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

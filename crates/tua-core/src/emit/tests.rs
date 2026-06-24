use super::*;
use crate::{DiagnosticCategory, DiagnosticCode};

#[test]
fn snapshot_type_aliases_emit_no_runtime_code() {
    let emitted = emit_tua_source(
        "src/types.tua",
        r#"
type Vec2 = {
  x: number,
  y: number,
}

type MaybeNumber = number | nil
"#,
    )
    .expect("type-only source should emit");

    assert_eq!(emitted, "");
}

#[test]
fn snapshot_erases_local_annotations() {
    let emitted = emit_tua_source(
        "src/locals.tua",
        r#"
type Player = {
  hp: number,
}

local score: number = 0
local title: string = "Arena"
local current: Player | nil = nil

return score
"#,
    )
    .expect("annotated locals should emit");

    assert_eq!(
        emitted,
        r#"local score = 0
local title = "Arena"
local current = nil

return score
"#
    );
}

#[test]
fn snapshot_erases_function_annotations() {
    let emitted = emit_tua_source(
        "src/functions.tua",
        r#"
type Player = {
  hp: number,
}

local function damage(player: Player, amount: number): Player
  player.hp = player.hp - amount
  return player
end

function heal(player: Player, amount: number): Player
  player.hp = player.hp + amount
  return player
end
"#,
    )
    .expect("annotated functions should emit");

    assert_eq!(
        emitted,
        r#"local function damage(player, amount)
  player.hp = player.hp - amount
  return player
end

function heal(player, amount)
  player.hp = player.hp + amount
  return player
end
"#
    );
}

#[test]
fn snapshot_emits_luacats_annotations() {
    let emitted = emit_tua_source_with_mode(
        "src/functions.tua",
        r#"
type Player = {
  hp: number,
  alive: boolean,
}

local score: number = 0

function heal(player: Player, amount: number): Player
  player.hp = player.hp + amount
  return player
end
"#,
        AnnotationMode::Luacats,
    )
    .expect("luacats mode should emit type annotations");

    assert_eq!(
        emitted,
        r#"---@alias Player { hp: number, alive: boolean }

---@type number
local score = 0

---@param player Player
---@param amount number
---@return Player
function heal(player, amount)
  player.hp = player.hp + amount
  return player
end
"#
    );
}

#[test]
fn snapshot_emits_luacats_annotations_for_local_functions_and_exports() {
    let emitted = emit_tua_source_with_mode(
        "src/player_factory.tua",
        r#"
type Vec2 = {
  x: number,
  y: number,
}

type Player = {
  name: string,
  pos: Vec2,
}

local default_player: Player = {
  name = "hero",
  pos = { x = 0, y = 0 },
}

local function make_player(name: string): Player
  return {
    name = name,
    pos = { x = 0, y = 0 },
  }
end

return {
  default_player = default_player,
  make_player = make_player,
}
"#,
        AnnotationMode::Luacats,
    )
    .expect("luacats mode should annotate stable module exports");

    assert_eq!(
        emitted,
        r#"---@alias Vec2 { x: number, y: number }
---@alias Player { name: string, pos: Vec2 }

---@type Player
local default_player = {
  name = "hero",
  pos = { x = 0, y = 0 },
}

---@param name string
---@return Player
local function make_player(name)
  return {
    name = name,
    pos = { x = 0, y = 0 },
  }
end

---@class PlayerFactoryExports
---@field default_player Player
---@field make_player fun(name: string): Player
return {
  default_player = default_player,
  make_player = make_player,
}
"#
    );
}

#[test]
fn snapshot_emits_luacats_type_only_alias_without_runtime_code() {
    let emitted = emit_tua_source_with_mode(
        "src/types.tua",
        r#"
type Player = {
  hp: number,
}
"#,
        AnnotationMode::Luacats,
    )
    .expect("luacats mode should keep alias information");

    assert_eq!(
        emitted,
        r#"---@alias Player { hp: number }
"#
    );
}

#[test]
fn snapshot_luacats_annotations_follow_nested_indentation() {
    let emitted = emit_tua_source_with_mode(
        "src/nested.tua",
        r#"
if ready then
  local score: number = 1
end
"#,
        AnnotationMode::Luacats,
    )
    .expect("luacats comments should match declaration indentation");

    assert_eq!(
        emitted,
        r#"if ready then
  ---@type number
  local score = 1
end
"#
    );
}

#[test]
fn snapshot_luacats_mode_does_not_emit_annotations_by_default() {
    let emitted = emit_tua_source_with_mode(
        "src/functions.tua",
        r#"
type Player = {
  hp: number,
  alive: boolean,
}

local score: number = 0
"#,
        AnnotationMode::Off,
    )
    .expect("off mode should preserve erased output");

    assert_eq!(
        emitted,
        r#"local score = 0
"#
    );
}

#[test]
fn snapshot_preserves_ordinary_lua_statements_and_expressions() {
    let input = r#"
local Player = require("player")
local player = {
  hp = 10,
  alive = true,
}

if player.hp > 0 then
  print(Player, player.hp)
end
"#;
    let emitted = emit_tua_source("src/plain.tua", input).expect("plain Lua should emit");

    assert_eq!(
        emitted,
        r#"local Player = require("player")
local player = {
  hp = 10,
  alive = true,
}

if player.hp > 0 then
  print(Player, player.hp)
end
"#
    );
}

#[test]
fn snapshot_erases_annotations_in_nested_blocks() {
    let emitted = emit_tua_source(
        "src/nested.tua",
        r#"
if ready then
  type LocalState = {
    ok: boolean,
  }
  local state: LocalState = { ok = true }
  for index: number = 1, 3 do
    local value: number = index
    print(value)
  end
end
"#,
    )
    .expect("nested annotations should emit");

    assert_eq!(
        emitted,
        r#"if ready then
  local state = { ok = true }
  for index = 1, 3 do
    local value = index
    print(value)
  end
end
"#
    );
}

#[test]
fn snapshot_lowers_all_supported_compound_assignments() {
    let emitted = emit_tua_source(
        "src/compound.tua",
        r#"
local x = 10
x += 5
x -= 2
x *= scale
x /= 3
x %= 4
player.hp -= amount
"#,
    )
    .expect("supported compound assignments should emit");

    assert_eq!(
        emitted,
        r#"local x = 10
x = x + 5
x = x - 2
x = x * scale
x = x / 3
x = x % 4
player.hp = player.hp - amount
"#
    );
}

#[test]
fn snapshot_lowers_love_asset_loader_calls() {
    let emitted = emit_tua_source(
        "src/assets.tua",
        r#"
local player = image "assets/player.png"
local jump = sound "assets/jump.wav"
local ui = image("assets/ui.png")
"#,
    )
    .expect("asset loader calls should emit");

    assert_eq!(
        emitted,
        r#"local player = love.graphics.newImage("assets/player.png")
local jump = love.audio.newSource("assets/jump.wav", "static")
local ui = love.graphics.newImage("assets/ui.png")
"#
    );
}

#[test]
fn snapshot_does_not_lower_love_asset_loader_names_in_strings_or_comments() {
    let emitted = emit_tua_source(
        "src/assets.tua",
        r#"
-- image "assets/player.png"
local text = "sound \"assets/jump.wav\""
"#,
    )
    .expect("comments and strings should emit");

    assert_eq!(
        emitted,
        r#"-- image "assets/player.png"
local text = "sound \"assets/jump.wav\""
"#
    );
}

#[test]
fn unsupported_compound_assignment_lhs_emits_lowering_diagnostic() {
    let diagnostics = emit_tua_source("src/compound.tua", "player[\"hp\"] += 1\n")
        .expect_err("computed-index compound assignment should not lower");

    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::LOWERING_COMPOUND_ASSIGNMENT)
    );
    assert_eq!(
        diagnostics[0].code.unwrap().category(),
        DiagnosticCategory::LoweringEmission
    );
    assert!(diagnostics[0].format_cli().contains("error TL2001"));
    assert!(diagnostics[0].message.contains("left-hand side"));
}

#[test]
fn call_compound_assignment_lhs_emits_lowering_diagnostic() {
    let diagnostics = emit_tua_source("src/compound.tua", "get_player().hp += 1\n")
        .expect_err("call-derived compound assignment should not lower");

    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::LOWERING_COMPOUND_ASSIGNMENT)
    );
    assert!(diagnostics[0].message.contains("left-hand side"));
}

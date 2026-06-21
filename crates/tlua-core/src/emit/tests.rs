use super::*;
use crate::{DiagnosticCategory, DiagnosticCode};

#[test]
fn snapshot_type_aliases_emit_no_runtime_code() {
    let emitted = emit_tlua_source(
        "src/types.tlua",
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
    let emitted = emit_tlua_source(
        "src/locals.tlua",
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
    let emitted = emit_tlua_source(
        "src/functions.tlua",
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
    let emitted = emit_tlua_source("src/plain.tlua", input).expect("plain Lua should emit");

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
    let emitted = emit_tlua_source(
        "src/nested.tlua",
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
    let emitted = emit_tlua_source(
        "src/compound.tlua",
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
fn unsupported_compound_assignment_lhs_emits_lowering_diagnostic() {
    let diagnostics = emit_tlua_source("src/compound.tlua", "player[\"hp\"] += 1\n")
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
    let diagnostics = emit_tlua_source("src/compound.tlua", "get_player().hp += 1\n")
        .expect_err("call-derived compound assignment should not lower");

    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::LOWERING_COMPOUND_ASSIGNMENT)
    );
    assert!(diagnostics[0].message.contains("left-hand side"));
}

use super::*;
use crate::DiagnosticCode;

#[test]
fn accepts_matching_annotated_local_initializers() {
    check_tua_source(
        "src/main.tua",
        r#"
local hp: number = 100
local name: string = "kyon"
local alive: boolean = true
local maybe_hp: number | nil = nil
"#,
    )
    .expect("matching primitive literals should type-check");
}

#[test]
fn accepts_typed_love_asset_loader_initializers() {
    check_tua_source(
        "src/assets.tua",
        r#"
local player: Image = image "assets/player.png"
local jump: Source = sound "assets/jump.wav"
"#,
    )
    .expect("LOVE asset loaders should produce their nominal asset types");
}

#[test]
fn rejects_love_asset_loader_initializer_mismatch() {
    let diagnostics = check_tua_source(
        "src/assets.tua",
        r#"
local player: Source = image "assets/player.png"
"#,
    )
    .expect_err("asset loader type mismatch should be reported");

    assert_single_mismatch(&diagnostics, "expected Source, found Image");
}

#[test]
fn rejects_primitive_annotated_local_initializer_mismatch() {
    let diagnostics = check_tua_source(
        "src/main.tua",
        r#"
local hp: number = "full"
"#,
    )
    .expect_err("primitive mismatch should be reported");

    assert_single_mismatch(&diagnostics, "expected number, found string");
}

#[test]
fn accepts_assignments_to_known_typed_locals_and_fields() {
    check_tua_source(
        "src/player.tua",
        r#"
type Player = {
  hp: number,
  alive: boolean,
}

local hp: number = 10
hp = 11

local player: Player = { hp = 10, alive = true }
player.hp = 9
player.alive = false
"#,
    )
    .expect("known typed locals and table fields should accept matching assignments");
}

#[test]
fn rejects_assignment_to_known_typed_local_mismatch() {
    let diagnostics = check_tua_source(
        "src/main.tua",
        r#"
local hp: number = 10
hp = "low"
"#,
    )
    .expect_err("typed local assignment mismatch should be reported");

    assert_single_mismatch(&diagnostics, "expected number, found string");
}

#[test]
fn rejects_assignment_to_known_table_field_mismatch() {
    let diagnostics = check_tua_source(
        "src/player.tua",
        r#"
type Player = {
  hp: number,
  alive: boolean,
}

local player: Player = { hp = 10, alive = true }
player.hp = "low"
"#,
    )
    .expect_err("typed table field assignment mismatch should be reported");

    assert_single_mismatch(&diagnostics, "expected number, found string");
}

#[test]
fn rejects_table_field_mismatch_in_annotated_local_initializer() {
    let diagnostics = check_tua_source(
        "src/player.tua",
        r#"
type Player = {
  hp: number,
}

local player: Player = { hp = "low" }
"#,
    )
    .expect_err("typed table initializer mismatch should be reported");

    assert_single_mismatch(&diagnostics, "expected Player, found { hp: string }");
}

#[test]
fn unannotated_dynamic_code_remains_permissive() {
    check_tua_source(
        "src/dynamic.tua",
        r#"
local hp = 10
hp = "low"

local player = make_player()
player.hp = "low"

type Player = {
  hp: number,
}

local typed_player: Player = make_player()
typed_player.hp = unknown_value()
"#,
    )
    .expect("unannotated and unknown-expression code should stay permissive");
}

#[test]
fn accepts_matching_function_call_arguments_and_returns() {
    check_tua_source(
        "src/combat.tua",
        r#"
local function add(left: number, right: number): number
  return 1
end

local total: number = add(1, 2)
"#,
    )
    .expect("matching function boundaries should type-check");
}

#[test]
fn accepts_love_method_multi_return_from_inferred_function_body() {
    check_tua_source(
        "src/assets.tua",
        r#"
local function loadPlayerImage()
  return image "assets/player.png"
end

local player = loadPlayerImage()

local function azul()
  return player.getDimensions()
end

local c = azul()
local z: number = c + 1
local literal: number = 1 + 3
local label: string = "width: " .. z
"#,
    )
    .expect("Love method multi-return should satisfy inferred function return checks");
}

#[test]
fn accepts_love_method_enum_string_arguments() {
    check_tua_source(
        "src/assets.tua",
        r#"
local player = image "assets/player.png"

player:setFilter("nearest")
player:setFilter("nearest", "linear", 4)
"#,
    )
    .expect("Love enum-like parameters should accept string values");
}

#[test]
fn rejects_love_method_argument_count_mismatch() {
    let diagnostics = check_tua_source(
        "src/assets.tua",
        r#"
local player = image "assets/player.png"

player:setFilter()
"#,
    )
    .expect_err("Love method argument count mismatch should be reported");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::TYPE_ARGUMENT_COUNT)
    );
    assert!(diagnostics[0]
        .message
        .contains("argument count mismatch: expected 1, found 0"));
}

#[test]
fn rejects_love_method_argument_type_mismatch() {
    let diagnostics = check_tua_source(
        "src/assets.tua",
        r#"
local player = image "assets/player.png"

player:setFilter(123)
"#,
    )
    .expect_err("Love method argument type mismatch should be reported");

    assert_single_mismatch(&diagnostics, "expected FilterMode, found number");
}

#[test]
fn rejects_function_call_argument_count_mismatch() {
    let diagnostics = check_tua_source(
        "src/combat.tua",
        r#"
local function add(left: number, right: number): number
  return 1
end

add(1)
"#,
    )
    .expect_err("argument count mismatch should be reported");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::TYPE_ARGUMENT_COUNT)
    );
    assert!(diagnostics[0]
        .message
        .contains("argument count mismatch: expected 2, found 1"));
    assert!(diagnostics[0].format_cli().contains("error TL1004"));
}

#[test]
fn rejects_function_call_argument_type_mismatch() {
    let diagnostics = check_tua_source(
        "src/combat.tua",
        r#"
local function add(left: number, right: number): number
  return 1
end

local total: number = add("one", 2)
"#,
    )
    .expect_err("argument type mismatch should be reported");

    assert_single_mismatch(&diagnostics, "expected number, found string");
}

#[test]
fn rejects_return_type_mismatch() {
    let diagnostics = check_tua_source(
        "src/combat.tua",
        r#"
local function hp(): number
  return "full"
end
"#,
    )
    .expect_err("return type mismatch should be reported");

    assert_single_mismatch(&diagnostics, "expected number, found string");
}

#[test]
fn calls_through_any_remain_permissive() {
    check_tua_source(
        "src/dynamic.tua",
        r#"
local dynamic_call: any = get_dynamic_call()
local hp: number = dynamic_call("one", false, {})
"#,
    )
    .expect("calls through any should remain permissive");
}

#[test]
fn warns_for_impossible_equality_comparisons() {
    let diagnostics = check_tua_source(
        "src/combat.tua",
        r#"
local function recursive(a: string)
  if a == 1 then
    return 1
  end

  if a ~= 2 then
    return "value"
  end
end
"#,
    )
    .expect_err("impossible comparisons should warn");

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::TYPE_IMPOSSIBLE_COMPARISON)
            && diagnostic.severity == crate::DiagnosticSeverity::Warning
    }));
    assert!(diagnostics[0]
        .message
        .contains("comparison is always false: string and number do not overlap"));
    assert!(diagnostics[1]
        .message
        .contains("comparison is always true: string and number do not overlap"));
    assert!(diagnostics[0].format_cli().contains("warning TL1005"));
}

#[test]
fn dynamic_equality_comparisons_remain_permissive() {
    check_tua_source(
        "src/dynamic.tua",
        r#"
local value: any = get_value()

if value == 1 then
  print(value)
end
"#,
    )
    .expect("comparisons through any should stay permissive");
}

fn assert_single_mismatch(diagnostics: &[Diagnostic], message: &str) {
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::TYPE_MISMATCH));
    assert_eq!(
        diagnostics[0].code.unwrap().category(),
        crate::DiagnosticCategory::Type
    );
    assert!(diagnostics[0].message.contains(message));
    assert!(diagnostics[0].format_cli().contains("error TL1003"));
}

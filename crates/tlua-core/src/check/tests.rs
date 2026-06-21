use super::*;
use crate::DiagnosticCode;

#[test]
fn accepts_matching_annotated_local_initializers() {
    check_tlua_source(
        "src/main.tlua",
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
fn rejects_primitive_annotated_local_initializer_mismatch() {
    let diagnostics = check_tlua_source(
        "src/main.tlua",
        r#"
local hp: number = "full"
"#,
    )
    .expect_err("primitive mismatch should be reported");

    assert_single_mismatch(&diagnostics, "expected number, found string");
}

#[test]
fn accepts_assignments_to_known_typed_locals_and_fields() {
    check_tlua_source(
        "src/player.tlua",
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
    let diagnostics = check_tlua_source(
        "src/main.tlua",
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
    let diagnostics = check_tlua_source(
        "src/player.tlua",
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
    let diagnostics = check_tlua_source(
        "src/player.tlua",
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
    check_tlua_source(
        "src/dynamic.tlua",
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

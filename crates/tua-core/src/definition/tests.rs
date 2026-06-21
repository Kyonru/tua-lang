use super::*;
use crate::source_text::{byte_for_column, line_at, position_at};

const SOURCE: &str = r#"type Vec2 = { x: number, y: number }
type Player = { pos: Vec2, hp: number, alive?: boolean }

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
local payload: any = {}
payload.value = 1
missing.value = 2
"#;

#[test]
fn resolves_type_alias_reference() {
    assert_definition(
        "player: Player",
        "player: ".len(),
        DefinitionKind::TypeAlias,
        "Player",
    );
}

#[test]
fn type_annotations_prefer_aliases_over_runtime_values() {
    let source = r#"type Player = { hp: number }
local Player: any = nil
local player: Player = nil
"#;
    let mut position = position_at(source, "player: Player");

    position.column += "player: ".len();

    let definition = definition_tua_source("src/conflict.tua", source, position)
        .expect("source should parse")
        .expect("alias definition should resolve");

    assert_eq!(definition.kind, DefinitionKind::TypeAlias);
    assert_eq!(source_text_from(source, definition.span), "Player");
}

#[test]
fn resolves_function_call() {
    assert_definition("damage(player", 0, DefinitionKind::Function, "damage");
}

#[test]
fn resolves_global_function_call() {
    let source = r#"function test(a: boolean)
  return ""
end

test(false)
"#;
    let position = position_at(source, "test(false");
    let definition = definition_tua_source("src/main.tua", source, position)
        .expect("source should parse")
        .expect("global function call should resolve");

    assert_eq!(definition.kind, DefinitionKind::Function);
    assert_eq!(source_text_from(source, definition.span), "test");
}

#[test]
fn resolves_definition_when_an_unrelated_line_is_incomplete() {
    let source = r#"type Player = { hp: number }
local player: Player = { hp = 10 }
player.

function test(a: boolean)
  return ""
end

test(false)
"#;
    let position = position_at(source, "test(false");
    let definition = definition_tua_source("src/main.tua", source, position)
        .expect("definition should recover from one incomplete line")
        .expect("global function call should resolve");

    assert_eq!(definition.kind, DefinitionKind::Function);
    assert_eq!(source_text_from(source, definition.span), "test");
}

#[test]
fn resolves_local_usage() {
    assert_definition(
        "damage(player",
        "damage(".len(),
        DefinitionKind::Local,
        "player",
    );
}

#[test]
fn resolves_parameter_usage() {
    assert_definition(
        "return target",
        "return ".len(),
        DefinitionKind::Parameter,
        "target",
    );
}

#[test]
fn resolves_table_field_to_type_shape_field() {
    assert_definition(
        "player.hp =",
        "player.".len(),
        DefinitionKind::TableField,
        "hp",
    );
}

#[test]
fn resolves_parameter_table_field_to_type_shape_field() {
    assert_definition(
        "target.hp -=",
        "target.".len(),
        DefinitionKind::TableField,
        "hp",
    );
}

#[test]
fn unknown_symbols_and_any_fields_have_no_definition() {
    assert_no_definition("missing.value", 0);
    assert_no_definition("payload.value", "payload.".len());
}

fn assert_definition(needle: &str, offset: usize, kind: DefinitionKind, expected_text: &str) {
    let definition = definition_at(needle, offset).expect("definition should resolve");

    assert_eq!(definition.kind, kind);
    assert_eq!(source_text(definition.span), expected_text);
}

fn assert_no_definition(needle: &str, offset: usize) {
    assert_eq!(definition_at(needle, offset), None);
}

fn definition_at(needle: &str, offset: usize) -> Option<Definition> {
    let mut position = position_at(SOURCE, needle);

    position.column += offset;

    definition_tua_source("src/main.tua", SOURCE, position).expect("source should parse")
}

fn source_text(span: SourceSpan) -> &'static str {
    source_text_from(SOURCE, span)
}

fn source_text_from(source: &'static str, span: SourceSpan) -> &'static str {
    let line = line_at(source, span.start.line).expect("definition line should exist");
    let start = byte_for_column(line, span.start.column);
    let end = byte_for_column(line, span.end.column);

    &line[start..end]
}

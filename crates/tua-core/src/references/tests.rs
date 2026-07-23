use super::*;
use crate::source_text::{byte_for_column, line_at, position_at};

const SOURCE: &str = r#"
type Vec2 = { x: number, y: number }
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
local dynamic = payload.value
local unresolved = missing
"#;

#[test]
fn alias_references_include_declaration_and_type_positions() {
    let references = references_at("player: Player", "player: ".len(), true);

    assert_eq!(
        reference_texts(&references),
        vec!["Player", "Player", "Player", "Player"]
    );
    assert!(references
        .iter()
        .all(|reference| reference.kind == ReferenceKind::TypeAlias));
}

#[test]
fn alias_references_can_exclude_declaration() {
    let references = references_at("player: Player", "player: ".len(), false);

    assert_eq!(
        reference_texts(&references),
        vec!["Player", "Player", "Player"]
    );
}

#[test]
fn function_references_include_declaration_and_calls() {
    let references = references_at("damage(player", 0, true);

    assert_eq!(reference_texts(&references), vec!["damage", "damage"]);
    assert!(references
        .iter()
        .all(|reference| reference.kind == ReferenceKind::Function));
}

#[test]
fn local_references_include_declaration_and_usages() {
    let references = references_at("damage(player", "damage(".len(), true);

    assert_eq!(
        reference_texts(&references),
        vec!["player", "player", "player", "player"]
    );
    assert!(references
        .iter()
        .all(|reference| reference.kind == ReferenceKind::Local));
}

#[test]
fn parameter_references_include_declaration_and_usages() {
    let references = references_at("return target", "return ".len(), true);

    assert_eq!(
        reference_texts(&references),
        vec!["target", "target", "target"]
    );
    assert!(references
        .iter()
        .all(|reference| reference.kind == ReferenceKind::Parameter));
}

#[test]
fn table_field_references_include_shape_field_and_typed_accesses() {
    let references = references_at("target.hp", "target.".len(), true);

    assert_eq!(reference_texts(&references), vec!["hp", "hp", "hp", "hp"]);
    assert!(references
        .iter()
        .all(|reference| reference.kind == ReferenceKind::TableField));
}

#[test]
fn table_field_references_can_exclude_shape_field() {
    let references = references_at("target.hp", "target.".len(), false);

    assert_eq!(reference_texts(&references), vec!["hp", "hp", "hp"]);
}

#[test]
fn unknown_symbols_and_any_fields_return_no_references() {
    assert!(references_at("missing", 0, true).is_empty());
    assert!(references_at("payload.value", "payload.".len(), true).is_empty());
}

#[test]
fn same_name_locals_in_nested_scopes_reference_independently() {
    let source = r#"
local value = 1
local before = value

do
  local value = 2
  local inside = value
end

local after = value
"#;
    let outer_position = position_at(source, "before = value");
    let inner_position = position_at(source, "inside = value");
    let mut outer_use = outer_position;
    let mut inner_use = inner_position;

    outer_use.column += "before = ".len();
    inner_use.column += "inside = ".len();

    let outer = references_tua_source("src/shadow.tua", source, outer_use, true)
        .expect("source should parse");
    let inner = references_tua_source("src/shadow.tua", source, inner_use, true)
        .expect("source should parse");

    assert_eq!(texts(source, &outer), vec!["value", "value", "value"]);
    assert_eq!(texts(source, &inner), vec!["value", "value"]);
}

#[test]
fn class_names_fields_and_methods_reference_original_source_symbols() {
    let source = r#"
open class Entity {
  health: number = 10

  damage(amount: number) {
    self.health -= amount
  }
}

local entity = Entity.new()
local typed: Entity = entity
print(entity.health)
entity:damage(1)
"#;
    let class_references = references_tua_source(
        "src/entity.tua",
        source,
        position_at(source, "Entity.new"),
        true,
    )
    .expect("source should parse");
    let mut field_position = position_at(source, "entity.health");
    field_position.column += "entity.".len();
    let field_references = references_tua_source("src/entity.tua", source, field_position, true)
        .expect("source should parse");
    let mut method_position = position_at(source, "entity:damage");
    method_position.column += "entity:".len();
    let method_references = references_tua_source("src/entity.tua", source, method_position, true)
        .expect("source should parse");

    assert_eq!(
        texts(source, &class_references),
        vec!["Entity", "Entity", "Entity"]
    );
    assert_eq!(
        texts(source, &field_references),
        vec!["health", "health", "health"]
    );
    assert_eq!(texts(source, &method_references), vec!["damage", "damage"]);
}

fn references_at(needle: &str, offset: usize, include_declaration: bool) -> Vec<Reference> {
    let mut position = position_at(SOURCE, needle);

    position.column += offset;

    references_tua_source("src/player.tua", SOURCE, position, include_declaration)
        .expect("source should parse")
}

fn reference_texts(references: &[Reference]) -> Vec<&str> {
    texts(SOURCE, references)
}

fn texts<'a>(source: &'a str, references: &[Reference]) -> Vec<&'a str> {
    references
        .iter()
        .map(|reference| reference_text(source, reference))
        .collect()
}

fn reference_text<'a>(source: &'a str, reference: &Reference) -> &'a str {
    let line = line_at(source, reference.span.start.line).expect("reference line should exist");
    let start = byte_for_column(line, reference.span.start.column);
    let end = byte_for_column(line, reference.span.end.column);

    &line[start..end]
}

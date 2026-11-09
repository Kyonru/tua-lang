use super::*;
use crate::source_text::position_at;

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
fn prepares_rename_for_supported_symbols() {
    let target = prepare_at("player: Player", "player: ".len()).expect("target should exist");

    assert_eq!(target.placeholder, "Player");
    assert_eq!(target.kind, RenameKind::TypeAlias);
}

#[test]
fn renames_type_aliases_within_the_file() {
    let edit = rename_at("player: Player", "player: ".len(), "Actor").expect("edit should exist");

    assert_eq!(edit_texts(&edit), vec!["Actor", "Actor", "Actor", "Actor"]);
    assert!(apply_edit(SOURCE, &edit).contains("type Actor"));
}

#[test]
fn renames_generic_alias_parameters_and_only_their_bound_uses() {
    let source = "type Box<T> = { value: T, next: T }\ntype Other<T> = { value: T }\n";
    let mut position = position_at(source, "Box<T>");
    position.column += "Box<".len();
    let edit = rename_tua_source("src/generic_alias.tua", source, position, "Item")
        .expect("generic alias should parse")
        .expect("generic alias binder should support rename");

    assert_eq!(edit_texts(&edit), ["Item", "Item", "Item"]);
    let renamed = apply_edit(source, &edit);
    assert!(renamed.contains("type Box<Item> = { value: Item, next: Item }"));
    assert!(renamed.contains("type Other<T> = { value: T }"));
}

#[test]
fn renames_functions_within_the_file() {
    let edit = rename_at("damage(player", 0, "hurt").expect("edit should exist");

    assert_eq!(edit_texts(&edit), vec!["hurt", "hurt"]);
    let renamed = apply_edit(SOURCE, &edit);

    assert!(renamed.contains("local function hurt"));
    assert!(renamed.contains("hurt(player, 2)"));
}

#[test]
fn renames_locals_within_the_file() {
    let edit = rename_at("damage(player", "damage(".len(), "hero").expect("edit should exist");

    assert_eq!(edit_texts(&edit), vec!["hero", "hero", "hero", "hero"]);
    let renamed = apply_edit(SOURCE, &edit);

    assert!(renamed.contains("local hero: Player"));
    assert!(renamed.contains("damage(hero, 2)"));
    assert!(renamed.contains("hero.hp = hero.hp - 1"));
}

#[test]
fn renames_parameters_within_the_file() {
    let edit = rename_at("return target", "return ".len(), "victim").expect("edit should exist");

    assert_eq!(edit_texts(&edit), vec!["victim", "victim", "victim"]);
    let renamed = apply_edit(SOURCE, &edit);

    assert!(renamed.contains("damage(victim: Player"));
    assert!(renamed.contains("victim.hp -= amount"));
    assert!(renamed.contains("return victim"));
}

#[test]
fn renaming_parameter_updates_only_its_attached_param_tags_and_keeps_prose() {
    let source = "---Damage target safely.\n---@param target Player target role\n---@param amount number damage amount\nlocal function damage(target: Player, amount: number)\n  return target\nend\n";
    let edit = rename_tua_source(
        "src/player.tua",
        source,
        position_at(source, "target: Player"),
        "victim",
    )
    .expect("source should parse")
    .expect("parameter rename should include its declaration and references");
    let renamed = apply_edit(source, &edit);
    assert!(renamed.contains("---Damage target safely."));
    assert!(renamed.contains("---@param victim Player target role"));
    assert!(renamed.contains("---@param amount number damage amount"));
    assert!(renamed.contains("local function damage(victim: Player"));
}

#[test]
fn renames_class_generic_parameters_without_touching_value_identifiers() {
    let source = r#"final class Box<T> {
  value: T

  get(): T {
    local T = self.value
    return T
  }
}
"#;
    let mut binder_position = position_at(source, "class Box<T>");
    binder_position.column += "class Box<".len();
    let target = prepare_rename_tua_source("src/generic_class.tua", source, binder_position)
        .expect("source should parse")
        .expect("class generic binder should support prepare rename");
    assert_eq!(target.kind, RenameKind::TypeParameter);
    assert_eq!(target.placeholder, "T");

    let edit = rename_tua_source("src/generic_class.tua", source, binder_position, "Item")
        .expect("source should parse")
        .expect("class generic binder should rename");
    assert_eq!(edit_texts(&edit), ["Item", "Item", "Item"]);
    let renamed = apply_edit(source, &edit);
    assert!(renamed.contains("class Box<Item>"));
    assert!(renamed.contains("value: Item"));
    assert!(renamed.contains("get(): Item"));
    assert!(renamed.contains("local T = self.value"));
    assert!(renamed.contains("return T"));
}

#[test]
fn renames_resolved_table_fields_and_rejects_unknowns_and_invalid_names() {
    let target = prepare_at("target.hp", "target.".len()).expect("field should be renameable");
    let edit = rename_at("target.hp", "target.".len(), "health").expect("field should rename");

    assert_eq!(target.kind, RenameKind::TableField);
    assert_eq!(
        edit_texts(&edit),
        vec!["health", "health", "health", "health"]
    );
    assert!(rename_at("payload.value", "payload.".len(), "value2").is_none());
    assert!(rename_at("missing", 0, "known").is_none());
    assert!(rename_at("player: Player", "player: ".len(), "not-valid").is_none());
    assert!(rename_at("player: Player", "player: ".len(), "while").is_none());
}

#[test]
fn rename_respects_nested_local_shadowing() {
    let source = r#"
local value = 1
local before = value

do
  local value = 2
  local inside = value
end

local after = value
"#;
    let mut position = position_at(source, "inside = value");

    position.column += "inside = ".len();

    let edit = rename_tua_source("src/shadow.tua", source, position, "inner")
        .expect("source should parse")
        .expect("inner local should rename");
    let renamed = apply_edit(source, &edit);

    assert!(renamed.contains("local value = 1"));
    assert!(renamed.contains("local before = value"));
    assert!(renamed.contains("local inner = 2"));
    assert!(renamed.contains("local inside = inner"));
    assert!(renamed.contains("local after = value"));
}

#[test]
fn rename_rejects_capture_of_locals_and_globals() {
    for source in [
        "local x = 1\nlocal y = 2\nprint(x, y)\n",
        "local x = 1\nprint(x, y)\n",
        "local x = 1\ndo local y = 2; print(x, y) end\n",
    ] {
        let position = SourcePosition { line: 1, column: 7 };
        assert!(rename_tua_source("capture.tua", source, position, "y")
            .unwrap()
            .is_none());
        let mut host = crate::AnalysisHost::new();
        host.set_file("capture.tua", source);
        assert!(host
            .snapshot()
            .rename("capture.tua", position, "y")
            .is_none());
        assert!(host
            .snapshot()
            .rename("capture.tua", position, "fresh_name")
            .is_some());
    }
}

#[test]
fn renames_class_names_fields_and_methods_in_original_source() {
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
    let class_edit = rename_tua_source(
        "src/entity.tua",
        source,
        position_at(source, "Entity.new"),
        "Actor",
    )
    .expect("source should parse")
    .expect("class should rename");
    let mut field_position = position_at(source, "entity.health");
    field_position.column += "entity.".len();
    let field_edit = rename_tua_source("src/entity.tua", source, field_position, "hitPoints")
        .expect("source should parse")
        .expect("field should rename");
    let mut method_position = position_at(source, "entity:damage");
    method_position.column += "entity:".len();
    let method_edit = rename_tua_source("src/entity.tua", source, method_position, "hurt")
        .expect("source should parse")
        .expect("method should rename");

    assert_eq!(class_edit.edits.len(), 3);
    assert_eq!(field_edit.edits.len(), 3);
    assert_eq!(method_edit.edits.len(), 2);
    assert!(apply_edit(source, &class_edit).contains("open class Actor"));
    assert!(apply_edit(source, &field_edit).contains("self.hitPoints"));
    assert!(apply_edit(source, &method_edit).contains("entity:hurt(1)"));
}

fn prepare_at(needle: &str, offset: usize) -> Option<RenameTarget> {
    let mut position = position_at(SOURCE, needle);

    position.column += offset;

    prepare_rename_tua_source("src/player.tua", SOURCE, position).expect("source should parse")
}

fn rename_at(needle: &str, offset: usize, new_name: &str) -> Option<RenameEdit> {
    let mut position = position_at(SOURCE, needle);

    position.column += offset;

    rename_tua_source("src/player.tua", SOURCE, position, new_name).expect("source should parse")
}

fn edit_texts(edit: &RenameEdit) -> Vec<&str> {
    edit.edits
        .iter()
        .map(|edit| edit.new_text.as_str())
        .collect()
}

fn apply_edit(source: &str, edit: &RenameEdit) -> String {
    let mut output = source.to_owned();
    let mut replacements = edit
        .edits
        .iter()
        .map(|edit| {
            (
                byte_index(source, edit.span.start),
                byte_index(source, edit.span.end),
                edit.new_text.as_str(),
            )
        })
        .collect::<Vec<_>>();

    replacements.sort_by_key(|(start, _, _)| *start);

    for (start, end, new_text) in replacements.into_iter().rev() {
        output.replace_range(start..end, new_text);
    }

    output
}

fn byte_index(source: &str, position: SourcePosition) -> usize {
    let mut line = 1usize;
    let mut column = 1usize;

    for (index, character) in source.char_indices() {
        if line == position.line && column == position.column {
            return index;
        }

        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    source.len()
}

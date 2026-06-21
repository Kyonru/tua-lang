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
fn rejects_table_fields_unknowns_any_fields_and_invalid_names() {
    assert!(prepare_at("target.hp", "target.".len()).is_none());
    assert!(rename_at("target.hp", "target.".len(), "health").is_none());
    assert!(rename_at("payload.value", "payload.".len(), "value2").is_none());
    assert!(rename_at("missing", 0, "known").is_none());
    assert!(rename_at("player: Player", "player: ".len(), "not-valid").is_none());
    assert!(rename_at("player: Player", "player: ".len(), "while").is_none());
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

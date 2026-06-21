use super::*;
use crate::source_text::position_at;

const SOURCE: &str = r#"type Vec2 = { x: number, y: number }
type Player = { pos: Vec2, hp: number, alive?: boolean }

local player: Player = {
  pos = { x = 0, y = 0 },
  hp = 10,
}

local payload: any = {}
local value = player.hp
local partial = player.alive
local nested = player.pos.x
local typed: Pl = nil
"#;

#[test]
fn completes_type_aliases_in_annotation_positions() {
    let completions = complete_at("typed: Pl").expect("alias completion should succeed");

    assert_eq!(labels(&completions), vec!["Player".to_owned()]);
    assert_eq!(completions[0].kind, CompletionKind::TypeAlias);
    assert_eq!(completions[0].detail.as_deref(), Some("type alias"));
}

#[test]
fn completes_table_fields_after_dot_access() {
    let completions = complete_at("player.").expect("field completion should succeed");

    assert_eq!(
        labels(&completions),
        vec!["alive".to_owned(), "hp".to_owned(), "pos".to_owned()]
    );
    assert!(completions
        .iter()
        .all(|item| item.kind == CompletionKind::Field));
}

#[test]
fn filters_table_fields_by_partial_prefix() {
    let completions = complete_at("player.a").expect("field completion should succeed");

    assert_eq!(labels(&completions), vec!["alive".to_owned()]);
}

#[test]
fn completes_nested_table_fields_when_shape_is_known() {
    let completions = complete_at("player.pos.").expect("nested field completion should succeed");

    assert_eq!(labels(&completions), vec!["x".to_owned(), "y".to_owned()]);
}

#[test]
fn completion_remains_quiet_for_any_values() {
    let source = format!("{SOURCE}local dynamic = payload.\n");
    let position = position_after(&source, "payload.");
    let completions = complete_tlua_source("src/main.tlua", source, position)
        .expect("completion should not fail for any values");

    assert!(completions.is_empty());
}

#[test]
fn completion_remains_quiet_for_unknown_dynamic_values() {
    let source = "local value = missing.\n";
    let position = position_after(source, "missing.");
    let completions = complete_tlua_source("src/main.tlua", source, position)
        .expect("completion should not fail for unknown values");

    assert!(completions.is_empty());
}

fn complete_at(needle: &str) -> Result<Vec<Completion>, Vec<Diagnostic>> {
    complete_tlua_source("src/main.tlua", SOURCE, position_after(SOURCE, needle))
}

fn labels(completions: &[Completion]) -> Vec<String> {
    completions
        .iter()
        .map(|completion| completion.label.clone())
        .collect()
}

fn position_after(source: &str, needle: &str) -> SourcePosition {
    let start = position_at(source, needle);

    SourcePosition {
        line: start.line,
        column: start.column + needle.chars().count(),
    }
}

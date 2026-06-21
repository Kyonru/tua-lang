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
fn highlights_local_declaration_and_usages() {
    let highlights = highlights_at("damage(player", "damage(".len());

    assert_eq!(
        highlight_texts(&highlights),
        vec!["player", "player", "player", "player"]
    );
    assert!(highlights
        .iter()
        .all(|highlight| highlight.kind == DocumentHighlightKind::Text));
}

#[test]
fn highlights_function_declaration_and_calls() {
    let highlights = highlights_at("damage(player", 0);

    assert_eq!(highlight_texts(&highlights), vec!["damage", "damage"]);
}

#[test]
fn highlights_table_shape_field_and_typed_accesses() {
    let highlights = highlights_at("target.hp", "target.".len());

    assert_eq!(highlight_texts(&highlights), vec!["hp", "hp", "hp", "hp"]);
}

#[test]
fn unknown_symbols_have_no_document_highlights() {
    assert!(highlights_at("missing", 0).is_empty());
    assert!(highlights_at("payload.value", "payload.".len()).is_empty());
}

fn highlights_at(needle: &str, offset: usize) -> Vec<DocumentHighlight> {
    let mut position = position_at(SOURCE, needle);

    position.column += offset;

    document_highlights_tua_source("src/player.tua", SOURCE, position).expect("source should parse")
}

fn highlight_texts(highlights: &[DocumentHighlight]) -> Vec<&str> {
    highlights.iter().map(highlight_text).collect()
}

fn highlight_text(highlight: &DocumentHighlight) -> &str {
    let line = line_at(SOURCE, highlight.span.start.line).expect("highlight line should exist");
    let start = byte_for_column(line, highlight.span.start.column);
    let end = byte_for_column(line, highlight.span.end.column);

    &line[start..end]
}

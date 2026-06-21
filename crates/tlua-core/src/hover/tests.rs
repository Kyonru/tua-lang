use super::*;

const SOURCE: &str = r#"type Vec2 = { x: number, y: number }
type Player = { pos: Vec2, hp: number }

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
"#;

#[test]
fn hovers_known_type_alias() {
    let hover = hover_at("Vec2").expect("alias should hover");

    assert_eq!(hover.contents, "type Vec2 = { x: number, y: number }");
}

#[test]
fn hovers_known_local() {
    let hover = hover_at("player:").expect("local should hover");

    assert_eq!(hover.contents, "local player: Player");
}

#[test]
fn hovers_known_function_signature() {
    let hover = hover_at("damage(player").expect("function should hover");

    assert_eq!(
        hover.contents,
        "function damage(target: Player, amount: number): Player"
    );
}

#[test]
fn hovers_known_table_field() {
    let hover = hover_at("hp = player").expect("table field should hover");

    assert_eq!(hover.contents, "(field) hp: number");
}

#[test]
fn unknown_symbols_do_not_hover() {
    let source = "local result = missing\n";
    let position = position_of(source, "missing");
    let hover = hover_tlua_source("src/main.tlua", source, position)
        .expect("hover should not fail for unknown values");

    assert_eq!(hover, None);
}

fn hover_at(needle: &str) -> Option<Hover> {
    let position = position_of(SOURCE, needle);

    hover_tlua_source("src/main.tlua", SOURCE, position).expect("source should parse and type")
}

fn position_of(source: &str, needle: &str) -> SourcePosition {
    let index = source.find(needle).expect("needle should exist in source");
    let before = &source[..index];
    let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = before
        .rsplit_once('\n')
        .map(|(_, line)| line.len())
        .unwrap_or(before.len())
        + 1;

    SourcePosition { line, column }
}

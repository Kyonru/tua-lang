use super::*;
use crate::source_text::position_at;

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
fn hovers_inferred_function_return_type_on_definition_and_call() {
    let source = r#"function test(a: boolean)
  return ""
end

test(false)
"#;
    let definition = hover_tua_source("src/main.tua", source, position_at(source, "test(a"))
        .expect("hover should succeed")
        .expect("function definition should hover");
    let call = hover_tua_source("src/main.tua", source, position_at(source, "test(false"))
        .expect("hover should succeed")
        .expect("function call should hover");

    assert_eq!(definition.contents, "function test(a: boolean): string");
    assert_eq!(call.contents, "function test(a: boolean): string");
}

#[test]
fn hovers_known_table_field() {
    let hover = hover_at("hp = player").expect("table field should hover");

    assert_eq!(hover.contents, "(field) hp: number");
}

#[test]
fn unknown_symbols_do_not_hover() {
    let source = "local result = missing\n";
    let position = position_at(source, "missing");
    let hover = hover_tua_source("src/main.tua", source, position)
        .expect("hover should not fail for unknown values");

    assert_eq!(hover, None);
}

#[test]
fn hovers_lua_standard_library_items() {
    let source = "local value = math.sin(1)\n";
    let position = position_at(source, "sin");
    let hover = hover_tua_source("src/main.tua", source, position)
        .expect("hover should not fail")
        .expect("math.sin should hover");

    assert_eq!(hover.contents, "function math.sin(x: number): number");
}

#[test]
fn hovers_love_items() {
    let source = "love.graphics.print(\"hi\", 0, 0)\n";
    let position = position_at(source, "print");
    let hover = hover_tua_source("src/main.tua", source, position)
        .expect("hover should not fail")
        .expect("love.graphics.print should hover");

    assert_eq!(hover.contents, "function love.graphics.print(text, x?, y?)");
}

#[test]
fn local_symbols_shadow_lua_global_hover() {
    let source = "local math: any = {}\nlocal value = math.sin\n";
    let position = position_at(source, "sin");
    let hover = hover_tua_source("src/main.tua", source, position)
        .expect("hover should not fail for shadowed values");

    assert_eq!(hover, None);
}

fn hover_at(needle: &str) -> Option<Hover> {
    let position = position_at(SOURCE, needle);

    hover_tua_source("src/main.tua", SOURCE, position).expect("source should parse and type")
}

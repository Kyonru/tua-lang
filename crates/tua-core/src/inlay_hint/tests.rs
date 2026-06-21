use super::*;

const SOURCE: &str = r#"
type Player = { hp: number }

local function makePlayer(): Player
  return { hp = 10 }
end

local function damage(target: Player, amount: number): Player
  target.hp -= amount
  return target
end

local player = makePlayer()
local hp = player.hp
local total = 10
local named: Player = makePlayer()
local unknown = missing()

damage(player, 10)
"#;

#[test]
fn hints_unannotated_local_types_from_known_expressions() {
    let hints = inlay_hints();
    let type_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Type)
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(type_hints, vec![": Player", ": number"]);
    assert_hint_position(&hints, ": Player", 13, "local player".len() + 1);
    assert_hint_position(&hints, ": number", 15, "local total".len() + 1);
}

#[test]
fn skips_annotated_unknown_and_any_local_type_hints() {
    let hints = inlay_hints();

    assert!(!hints_at_line(&hints, 16).any(|hint| hint.kind == InlayHintKind::Type));
    assert!(!hints_at_line(&hints, 17).any(|hint| hint.kind == InlayHintKind::Type));
}

#[test]
fn hints_parameter_names_for_known_calls() {
    let hints = inlay_hints();
    let parameter_hints = hints
        .iter()
        .filter(|hint| hint.kind == InlayHintKind::Parameter)
        .map(|hint| hint.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(parameter_hints, vec!["target:", "amount:"]);
    assert_hint_position(&hints, "target:", 19, "damage(".len() + 1);
    assert_hint_position(&hints, "amount:", 19, "damage(player, ".len() + 1);
}

fn inlay_hints() -> Vec<InlayHint> {
    inlay_hints_tua_source("src/player.tua", SOURCE).expect("source should parse")
}

fn hints_at_line(hints: &[InlayHint], line: usize) -> impl Iterator<Item = &InlayHint> {
    hints.iter().filter(move |hint| hint.position.line == line)
}

fn assert_hint_position(hints: &[InlayHint], label: &str, line: usize, column: usize) {
    let hint = hints
        .iter()
        .find(|hint| hint.label == label && hint.position.line == line)
        .unwrap_or_else(|| panic!("expected hint `{label}` on line {line}"));

    assert_eq!(hint.position.column, column);
}

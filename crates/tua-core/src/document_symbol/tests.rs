use super::*;
use crate::source_text::{byte_for_column, line_at};

const SOURCE: &str = r#"
type Vec2 = {
  x: number,
  y: number,
}

type Player = {
  pos: Vec2,
  hp: number,
}

local player: Player = nil

local function damage(target: Player, amount: number): Player
  return target
end

local runtime = {}

return {
  damage = damage,
  player = player,
  ["ignored"] = runtime,
  [runtime] = runtime,
  runtime,
}
"#;

#[test]
fn type_aliases_include_table_field_children() {
    let symbols = document_symbols();
    let player = find_symbol(&symbols, "Player");

    assert_eq!(player.kind, DocumentSymbolKind::TypeAlias);
    assert_eq!(
        player.detail.as_deref(),
        Some("type { pos: Vec2, hp: number }")
    );
    assert_eq!(names(&player.children), vec!["pos", "hp"]);

    let hp = find_symbol(&player.children, "hp");

    assert_eq!(hp.kind, DocumentSymbolKind::Field);
    assert_eq!(hp.detail.as_deref(), Some("number"));
    assert_symbol_text(hp, "hp");
}

#[test]
fn functions_include_typed_parameter_children() {
    let symbols = document_symbols();
    let damage = find_symbol(&symbols, "damage");

    assert_eq!(damage.kind, DocumentSymbolKind::Function);
    assert_eq!(damage.detail.as_deref(), Some("returns Player"));
    assert_eq!(names(&damage.children), vec!["target", "amount"]);
    assert_eq!(damage.children[0].kind, DocumentSymbolKind::Parameter);
    assert_eq!(damage.children[0].detail.as_deref(), Some("Player"));
}

#[test]
fn annotated_locals_appear_as_top_level_symbols() {
    let symbols = document_symbols();
    let player = find_symbol(&symbols, "player");

    assert_eq!(player.kind, DocumentSymbolKind::Local);
    assert_eq!(player.detail.as_deref(), Some("Player"));
    assert_symbol_text(player, "player");
}

#[test]
fn simple_final_returned_table_creates_exports_symbol() {
    let symbols = document_symbols();
    let exports = find_symbol(&symbols, "exports");

    assert_eq!(exports.kind, DocumentSymbolKind::ModuleExports);
    assert_eq!(exports.detail.as_deref(), Some("module exports"));
    assert_eq!(names(&exports.children), vec!["damage", "player"]);
    assert!(exports
        .children
        .iter()
        .all(|symbol| symbol.kind == DocumentSymbolKind::Field));
}

#[test]
fn selection_ranges_are_contained_in_full_ranges() {
    for symbol in document_symbols() {
        assert_ranges_contain_selection(&symbol);
    }
}

#[test]
fn symbols_are_returned_in_source_order() {
    let symbols = document_symbols();

    assert_eq!(
        names(&symbols),
        vec!["Vec2", "Player", "player", "damage", "exports"]
    );
}

#[test]
fn classes_include_typed_field_constructor_and_method_children() {
    let source = r#"
final class BeatManager {
  steps: number[]

  init() {
    self.steps = {}
  }

  add(step: number): number {
    return step
  }
}
"#;
    let symbols =
        document_symbols_tua_source("src/classes.tua", source).expect("class source should parse");
    let class = find_symbol(&symbols, "BeatManager");

    assert_eq!(class.kind, DocumentSymbolKind::Class);
    assert_eq!(names(&class.children), vec!["steps", "init", "add"]);
    assert_eq!(class.children[0].detail.as_deref(), Some("number[]"));
    assert_eq!(class.children[1].kind, DocumentSymbolKind::Constructor);
    assert_eq!(class.children[2].kind, DocumentSymbolKind::Method);
    assert_eq!(
        class.children[2].detail.as_deref(),
        Some("(step: number) -> number")
    );
}

fn document_symbols() -> Vec<DocumentSymbol> {
    document_symbols_tua_source("src/main.tua", SOURCE).expect("source should parse")
}

fn find_symbol<'a>(symbols: &'a [DocumentSymbol], name: &str) -> &'a DocumentSymbol {
    symbols
        .iter()
        .find(|symbol| symbol.name == name)
        .unwrap_or_else(|| panic!("expected symbol `{name}`"))
}

fn names(symbols: &[DocumentSymbol]) -> Vec<&str> {
    symbols.iter().map(|symbol| symbol.name.as_str()).collect()
}

fn assert_symbol_text(symbol: &DocumentSymbol, expected: &str) {
    let line =
        line_at(SOURCE, symbol.selection_range.start.line).expect("symbol line should exist");
    let start = byte_for_column(line, symbol.selection_range.start.column);
    let end = byte_for_column(line, symbol.selection_range.end.column);

    assert_eq!(&line[start..end], expected);
}

fn assert_ranges_contain_selection(symbol: &DocumentSymbol) {
    assert!(
        contains(symbol.range, symbol.selection_range),
        "`{}` selection range {:?} should be contained in {:?}",
        symbol.name,
        symbol.selection_range,
        symbol.range
    );

    for child in &symbol.children {
        assert_ranges_contain_selection(child);
    }
}

fn contains(outer: SourceSpan, inner: SourceSpan) -> bool {
    position_key(outer.start) <= position_key(inner.start)
        && position_key(inner.end) <= position_key(outer.end)
}

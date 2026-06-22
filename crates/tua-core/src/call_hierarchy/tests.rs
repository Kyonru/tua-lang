use super::*;
use crate::parse_tua_source;
use crate::source_text::{line_at, position_at};
use crate::SourcePosition;

const SOURCE: &str = r#"
local function makePlayer()
  return {}
end

local function caller()
  makePlayer()
end

return {
  makePlayer = makePlayer,
  inline = function()
    makePlayer()
  end,
}
"#;

#[test]
fn prepares_known_function_symbols() {
    let parsed = parsed_source(SOURCE);
    let position = position_at(SOURCE, "makePlayer()");
    let symbol = call_hierarchy_prepare_parsed_source(&parsed, position)
        .expect("function declaration should prepare call hierarchy");

    assert_eq!(symbol.name, "makePlayer");
    assert_eq!(symbol.kind, CallHierarchySymbolKind::Function);
    assert_eq!(
        symbol.target,
        CallHierarchySymbolTarget::Local("makePlayer".to_owned())
    );
    assert_span_text(symbol.selection_range, "makePlayer");
}

#[test]
fn collects_calls_with_containing_callers() {
    let parsed = parsed_source(SOURCE);
    let calls = call_hierarchy_calls_parsed_source(&parsed);
    let make_player_calls = calls
        .iter()
        .filter(|call| call.target == CallHierarchyCallTarget::Name("makePlayer".to_owned()))
        .collect::<Vec<_>>();

    assert_eq!(make_player_calls.len(), 2);

    let caller_names = make_player_calls
        .iter()
        .filter_map(|call| call.caller.as_ref())
        .map(|definition| source_text_from(definition.span))
        .collect::<Vec<_>>();

    assert_eq!(caller_names, vec!["caller", "inline"]);
}

#[test]
fn collects_returned_module_exports() {
    let parsed = parsed_source(SOURCE);
    let exports = call_hierarchy_exports_parsed_source(&parsed);

    assert_eq!(exports.len(), 2);
    assert_eq!(exports[0].field, "makePlayer");
    assert_eq!(exports[0].target_name.as_deref(), Some("makePlayer"));
    assert_eq!(exports[1].field, "inline");
    assert_eq!(exports[1].target_name, None);
}

#[test]
fn collects_method_symbols_and_calls_without_guessing_receiver_types() {
    let source = r#"
function Player:heal()
end

local function caller(player)
  player:heal()
end
"#;
    let parsed = parsed_source(source);
    let symbols = call_hierarchy_symbols_parsed_source(&parsed);
    let method = symbols
        .iter()
        .find(|symbol| symbol.name == "Player:heal")
        .expect("method symbol should be collected");

    assert_eq!(
        method.target,
        CallHierarchySymbolTarget::Method {
            receiver: "Player".to_owned(),
            method: "heal".to_owned()
        }
    );

    let calls = call_hierarchy_calls_parsed_source(&parsed);

    assert!(calls.iter().any(|call| {
        call.target
            == CallHierarchyCallTarget::Method {
                receiver: "player".to_owned(),
                method: "heal".to_owned(),
            }
    }));
}

#[test]
fn symbol_ranges_contain_selection_ranges() {
    let source = r#"
local assigned = function()
end

return {
  inline = function()
  end,
}
"#;
    let parsed = parsed_source(source);

    for symbol in call_hierarchy_symbols_parsed_source(&parsed) {
        assert!(
            span_contains_span(symbol.range, symbol.selection_range),
            "{} range {:?} should contain selection {:?}",
            symbol.name,
            symbol.range,
            symbol.selection_range
        );
    }
}

fn parsed_source(source: &str) -> ParsedSource {
    parse_tua_source("src/main.tua", source).expect("source should parse")
}

fn assert_span_text(span: SourceSpan, expected: &str) {
    assert_eq!(source_text_from(span), expected);
}

fn source_text_from(span: SourceSpan) -> String {
    let line = line_at(SOURCE, span.start.line).expect("span line should exist");
    let start = span.start.column.saturating_sub(1);
    let end = span.end.column.saturating_sub(1);

    line[start..end].to_owned()
}

fn span_contains_span(outer: SourceSpan, inner: SourceSpan) -> bool {
    position_key(outer.start) <= position_key(inner.start)
        && position_key(inner.end) <= position_key(outer.end)
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

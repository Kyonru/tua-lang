use super::*;
use crate::parse_tua_source;
use crate::source_text::position_at;

const SOURCE: &str = r#"
local Player = require("player")
local value = 1

local function outer(value: number)
  local result = value

  do
    local value = result + 1
    result = value
  end

  return value + result
end

print(Player.make(), love.graphics, configured, missing)
"#;

fn scope_map() -> ScopeMap {
    let parsed = parse_tua_source("src/main.tua", SOURCE).expect("source should parse");

    scope_map_parsed_source_with_globals(&parsed, ["configured"])
}

#[test]
fn same_name_locals_in_nested_scopes_resolve_independently() {
    let map = scope_map();
    let outer_value = map
        .symbol_at_position(position_at(SOURCE, "value: number"))
        .expect("outer parameter should resolve");
    let inner_value = map
        .symbol_at_position(position_at(SOURCE, "value = result + 1"))
        .expect("inner local should resolve");

    assert_ne!(outer_value, inner_value);

    let outer_references = map.references_to_symbol(outer_value, true);
    let inner_references = map.references_to_symbol(inner_value, true);
    let outer_texts = reference_names(&outer_references);
    let inner_texts = reference_names(&inner_references);

    assert_eq!(outer_texts, vec!["value", "value", "value"]);
    assert_eq!(inner_texts, vec!["value", "value"]);
    assert_eq!(
        map.declaration(inner_value)
            .and_then(|declaration| declaration.shadows),
        Some(outer_value)
    );
}

#[test]
fn module_aliases_are_lexical_symbols_with_module_metadata() {
    let map = scope_map();
    let player = map
        .symbol_at_position(position_at(SOURCE, "Player.make"))
        .expect("module alias usage should resolve");
    let declaration = map
        .declaration(player)
        .expect("module alias declaration should exist");

    assert_eq!(declaration.name, "Player");
    assert_eq!(
        declaration.kind,
        ScopeDeclarationKind::ModuleAlias {
            module: "player".to_owned()
        }
    );
    assert_eq!(
        reference_names(&map.references_to_symbol(player, true)),
        vec!["Player", "Player"]
    );
}

#[test]
fn globals_and_unresolved_names_use_the_same_lookup_path() {
    let map = scope_map();
    let globals = map.globals();

    assert_eq!(
        globals.get("print").map(|global| global.kind),
        Some(ScopeGlobalKind::LuaBuiltin)
    );
    assert_eq!(
        globals.get("love").map(|global| global.kind),
        Some(ScopeGlobalKind::LoveBuiltin)
    );
    assert_eq!(
        globals.get("configured").map(|global| global.kind),
        Some(ScopeGlobalKind::Configured)
    );
    assert!(map
        .unresolved_references()
        .any(|reference| reference.name == "missing"));
}

#[test]
fn colon_method_self_resolves_to_implicit_parameter() {
    let source = r#"
local Account = {}

function Account:deposit(amount)
  self.balance = self.balance + amount
end
"#;
    let parsed = parse_tua_source("src/account.tua", source).expect("source should parse");
    let map = scope_map_parsed_source(&parsed);
    let self_symbol = map
        .symbol_at_position(position_at(source, "self.balance"))
        .expect("self reference should resolve");
    let declaration = map
        .declaration(self_symbol)
        .expect("self declaration should exist");

    assert_eq!(declaration.name, "self");
    assert_eq!(declaration.kind, ScopeDeclarationKind::Parameter);
}

#[test]
fn table_shape_fields_are_available_as_scope_declarations() {
    let parsed = parse_tua_source(
        "src/player.tua",
        r#"
type Player = {
  hp: number,
}
"#,
    )
    .expect("source should parse");
    let map = scope_map_parsed_source(&parsed);
    let field = map
        .declarations()
        .iter()
        .find(|declaration| declaration.name == "hp")
        .expect("table field should be present");

    assert_eq!(field.kind, ScopeDeclarationKind::TableField);
}

fn reference_names(references: &[ScopeReference]) -> Vec<&str> {
    references
        .iter()
        .map(|reference| reference.name.as_str())
        .collect()
}

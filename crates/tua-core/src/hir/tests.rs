use super::*;
use crate::parse_tua_source;

#[test]
fn lowers_declarations_with_stable_symbol_ids_and_spans() {
    let parsed = parse_tua_source(
        "src/player.tua",
        r#"
type Player = {
  hp: number,
}

local hp: number = 10

local function damage(player: Player, amount: number): Player
  local next_hp = hp - amount
  return player
end
"#,
    )
    .expect("source should parse");

    let module = lower_parsed_source(&parsed);
    let names = module
        .symbols()
        .iter()
        .map(|symbol| (symbol.id, symbol.kind, symbol.name.as_str()))
        .collect::<Vec<_>>();

    assert!(names.contains(&(HirSymbolId(0), HirSymbolKind::TypeAlias, "Player")));
    assert!(names.contains(&(HirSymbolId(1), HirSymbolKind::TableField, "hp")));
    assert!(names.contains(&(HirSymbolId(2), HirSymbolKind::Local, "hp")));
    assert!(names.contains(&(HirSymbolId(3), HirSymbolKind::Function, "damage")));
    assert!(names.contains(&(HirSymbolId(4), HirSymbolKind::Parameter, "player")));
    assert!(names.contains(&(HirSymbolId(5), HirSymbolKind::Parameter, "amount")));
    assert!(names.contains(&(HirSymbolId(6), HirSymbolKind::Local, "next_hp")));

    assert_eq!(
        module.symbol(HirSymbolId(4)).unwrap().parent,
        Some(HirSymbolId(3))
    );
    assert_eq!(
        module.symbol(HirSymbolId(6)).unwrap().parent,
        Some(HirSymbolId(3))
    );

    let field = module.symbol(HirSymbolId(1)).expect("field should exist");
    assert_eq!(field.parent, Some(HirSymbolId(0)));
    assert_eq!(field.name_span.start.line, 3);
    assert_eq!(field.origin_span.start.line, 3);

    assert_eq!(module.declarations.len(), module.symbols.len());
}

#[test]
fn lowers_require_bindings_and_simple_returned_exports() {
    let parsed = parse_tua_source(
        "src/main.tua",
        r#"
local Player = require("player")

local M = {}

function M.new()
  return Player.new()
end

return {
  new = M,
  dynamic = Player.new(),
}
"#,
    )
    .expect("source should parse");

    let module = lower_parsed_source(&parsed);

    assert_eq!(module.require_bindings.len(), 1);
    assert_eq!(module.require_bindings[0].alias, "Player");
    assert_eq!(module.require_bindings[0].module, "player");

    let simple = module
        .exports
        .iter()
        .find(|export| export.name == "new")
        .expect("simple export should exist");
    assert!(matches!(simple.value, HirExportValue::Symbol(_)));

    let dynamic = module
        .exports
        .iter()
        .find(|export| export.name == "dynamic")
        .expect("dynamic export should exist");
    assert_eq!(dynamic.value, HirExportValue::Unknown);
}

#[test]
fn resolves_nearest_same_file_value_definition() {
    let parsed = parse_tua_source(
        "src/main.tua",
        r#"
local value = 1

local function run(value: number)
  return value
end
"#,
    )
    .expect("source should parse");

    let module = lower_parsed_source(&parsed);
    let symbol = module
        .definition_for_value(
            "value",
            SourcePosition {
                line: 5,
                column: 10,
            },
        )
        .expect("parameter should be nearest value");

    assert_eq!(symbol.kind, HirSymbolKind::Parameter);
    assert_eq!(symbol.name_span.start.line, 4);
}

#[test]
fn dynamic_return_expression_remains_unknown() {
    let parsed = parse_tua_source(
        "src/main.tua",
        r#"
local M = {}
return {
  value = get_value().field,
}
"#,
    )
    .expect("source should parse");

    let module = lower_parsed_source(&parsed);

    assert_eq!(module.exports[0].value, HirExportValue::Unknown);
}

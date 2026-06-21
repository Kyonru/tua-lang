use super::*;
use std::path::PathBuf;

#[test]
fn parses_local_assignments() {
    let parsed = parse_lua_source("src/local.tlua", "local x = 1\nlocal y = x + 2\n")
        .expect("locals should parse");

    assert_eq!(parsed.path, PathBuf::from("src/local.tlua"));
    assert_eq!(parsed.statement_count(), 2);
    assert!(parsed.span.is_some());
}

#[test]
fn parses_functions_and_returns() {
    let parsed = parse_lua_source(
        "src/function.tlua",
        r#"
local function add(a, b)
  return a + b
end
"#,
    )
    .expect("function should parse");

    assert_eq!(parsed.statement_count(), 1);
    assert!(parsed.source.contains("return a + b"));
}

#[test]
fn parses_table_constructors() {
    let parsed = parse_lua_source(
        "src/table.tlua",
        r#"
local player = {
  hp = 100,
  alive = true,
}
"#,
    )
    .expect("table should parse");

    assert_eq!(parsed.statement_count(), 1);
    assert!(parsed.source.contains("alive"));
}

#[test]
fn parses_return_statements() {
    let parsed =
        parse_lua_source("src/return.tlua", "return { ok = true }\n").expect("return should parse");

    assert_eq!(parsed.statement_count(), 0);
    assert!(parsed.has_return());
}

#[test]
fn parses_require_calls() {
    let parsed = parse_lua_source(
        "src/require.tlua",
        r#"
local Player = require("player")
return Player
"#,
    )
    .expect("require should parse");

    assert_eq!(parsed.statement_count(), 1);
    assert!(parsed.has_return());
    assert!(parsed.source.contains("require"));
}

#[test]
fn parses_type_aliases_and_table_shapes() {
    let parsed = parse_tlua_source(
        "src/types.tlua",
        r#"
type Vec2 = {
  x: number,
  y: number,
}

type Player = {
  pos: Vec2,
  hp: number,
  alive: boolean,
}
"#,
    )
    .expect("type aliases and table shapes should parse");

    assert_eq!(parsed.type_aliases.len(), 2);

    let vec2 = &parsed.type_aliases[0];
    assert_eq!(vec2.name, "Vec2");

    let TluaType::Table(fields) = &vec2.ty else {
        panic!("Vec2 should be represented as a table shape");
    };

    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name, "x");
    assert_eq!(fields[0].ty, TluaType::Named("number".to_owned()));
    assert_eq!(fields[1].name, "y");
    assert_eq!(fields[1].ty, TluaType::Named("number".to_owned()));

    let player = &parsed.type_aliases[1];
    assert_eq!(player.name, "Player");

    let TluaType::Table(fields) = &player.ty else {
        panic!("Player should be represented as a table shape");
    };

    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0].name, "pos");
    assert_eq!(fields[0].ty, TluaType::Named("Vec2".to_owned()));
    assert_eq!(fields[1].name, "hp");
    assert_eq!(fields[1].ty, TluaType::Named("number".to_owned()));
    assert_eq!(fields[2].name, "alive");
    assert_eq!(fields[2].ty, TluaType::Named("boolean".to_owned()));
}

#[test]
fn parses_optional_field_syntax_from_spec() {
    let parsed = parse_tlua_source(
        "src/save_slot.tlua",
        r#"
type SaveSlot = {
  name?: string,
  level: number,
  checkpoint?: string
}
"#,
    )
    .expect("optional table fields should parse");

    let alias = &parsed.type_aliases[0];
    assert_eq!(alias.name, "SaveSlot");

    let TluaType::Table(fields) = &alias.ty else {
        panic!("SaveSlot should be represented as a table shape");
    };

    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0].name, "name");
    assert!(fields[0].optional);
    assert_eq!(fields[0].ty, TluaType::Named("string".to_owned()));
    assert_eq!(fields[1].name, "level");
    assert!(!fields[1].optional);
    assert_eq!(fields[1].ty, TluaType::Named("number".to_owned()));
    assert_eq!(fields[2].name, "checkpoint");
    assert!(fields[2].optional);
    assert_eq!(fields[2].ty, TluaType::Named("string".to_owned()));
}

#[test]
fn parses_union_type_aliases() {
    let parsed = parse_tlua_source("src/maybe.tlua", "type MaybeNumber = number | nil\n")
        .expect("union type aliases should parse");

    assert_eq!(parsed.type_aliases.len(), 1);
    assert_eq!(parsed.type_aliases[0].name, "MaybeNumber");
    assert_eq!(
        parsed.type_aliases[0].ty,
        TluaType::Union(vec![
            TluaType::Named("number".to_owned()),
            TluaType::Named("nil".to_owned())
        ])
    );
}

#[test]
fn parses_local_annotations() {
    let parsed = parse_tlua_source(
        "src/locals.tlua",
        r#"
local score: number = 0
local title: string = "Arena"
local current: Player | nil = nil
local payload: any = nil
"#,
    )
    .expect("local annotations should parse");

    assert_eq!(parsed.local_annotations.len(), 4);
    assert_eq!(parsed.local_annotations[0].name, "score");
    assert_eq!(
        parsed.local_annotations[0].ty,
        TluaType::Named("number".to_owned())
    );
    assert_eq!(parsed.local_annotations[1].name, "title");
    assert_eq!(
        parsed.local_annotations[1].ty,
        TluaType::Named("string".to_owned())
    );
    assert_eq!(parsed.local_annotations[2].name, "current");
    assert_eq!(
        parsed.local_annotations[2].ty,
        TluaType::Union(vec![
            TluaType::Named("Player".to_owned()),
            TluaType::Named("nil".to_owned())
        ])
    );
    assert_eq!(parsed.local_annotations[3].name, "payload");
    assert_eq!(
        parsed.local_annotations[3].ty,
        TluaType::Named("any".to_owned())
    );
}

#[test]
fn parses_function_parameter_and_return_annotations() {
    let parsed = parse_tlua_source(
        "src/damage.tlua",
        r#"
local function damage(player: Player, amount: number): Player
  return player
end
"#,
    )
    .expect("function annotations should parse");

    assert_eq!(parsed.function_annotations.len(), 1);

    let function = &parsed.function_annotations[0];
    assert_eq!(function.name.as_deref(), Some("damage"));
    assert_eq!(function.parameters.len(), 2);
    assert_eq!(function.parameters[0].name, "player");
    assert_eq!(
        function.parameters[0].ty,
        TluaType::Named("Player".to_owned())
    );
    assert_eq!(function.parameters[1].name, "amount");
    assert_eq!(
        function.parameters[1].ty,
        TluaType::Named("number".to_owned())
    );
    assert_eq!(
        function.return_type,
        Some(TluaType::Named("Player".to_owned()))
    );
}

#[test]
fn parses_valid_compound_assignments() {
    let parsed = parse_tlua_source(
        "src/compound.tlua",
        r#"
local x = 1
x += 2
player.hp -= amount
world.player.hp *= scale
x /= 2
player.hp %= 4

function tick()
  score += 1
end
"#,
    )
    .expect("compound assignments should parse");

    let assignments = &parsed.compound_assignments;

    assert_eq!(assignments.len(), 6);
    assert_eq!(assignments[0].lhs, parts(&["x"]));
    assert_eq!(assignments[0].operator, CompoundOperator::Add);
    assert_eq!(assignments[0].operator.as_sugar(), "+=");
    assert_eq!(assignments[0].operator.as_binary(), "+");
    assert_eq!(assignments[0].rhs, "2");

    assert_eq!(assignments[1].lhs, parts(&["player", "hp"]));
    assert_eq!(assignments[1].operator, CompoundOperator::Subtract);
    assert_eq!(assignments[1].rhs, "amount");

    assert_eq!(assignments[2].lhs, parts(&["world", "player", "hp"]));
    assert_eq!(assignments[2].operator, CompoundOperator::Multiply);
    assert_eq!(assignments[2].rhs, "scale");

    assert_eq!(assignments[3].lhs, parts(&["x"]));
    assert_eq!(assignments[3].operator, CompoundOperator::Divide);
    assert_eq!(assignments[3].rhs, "2");

    assert_eq!(assignments[4].lhs, parts(&["player", "hp"]));
    assert_eq!(assignments[4].operator, CompoundOperator::Modulo);
    assert_eq!(assignments[4].rhs, "4");

    assert_eq!(assignments[5].lhs, parts(&["score"]));
    assert_eq!(assignments[5].operator, CompoundOperator::Add);
    assert_eq!(assignments[5].rhs, "1");
}

#[test]
fn rejects_computed_index_compound_assignment_lhs() {
    assert_compound_lhs_error("player[\"hp\"] += 1\n");
}

#[test]
fn rejects_call_compound_assignment_lhs() {
    assert_compound_lhs_error("get_player().hp += 1\n");
}

#[test]
fn rejects_expression_compound_assignment_lhs() {
    assert_compound_lhs_error("(player).hp += 1\n");
}

#[test]
fn rejects_unsupported_luau_compound_assignment_operators() {
    let diagnostics = parse_tlua_source("src/compound.tlua", "x ^= 1\n")
        .expect_err("operators outside tlua v1 should produce diagnostics");

    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::SYNTAX_COMPOUND_ASSIGNMENT)
    );
    assert!(diagnostics[0].message.contains("unsupported"));
}

#[test]
fn keeps_type_syntax_out_of_lua_compatible_parser() {
    let diagnostics = parse_lua_source("src/typed.tlua", "local score: number = 0\n")
        .expect_err("plain Lua parser should reject erased type syntax");

    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::SYNTAX_PARSE));
}

#[test]
fn syntax_errors_produce_tl000x_diagnostics_with_locations() {
    let diagnostics =
        parse_lua_source("src/broken.tlua", "local x = \n").expect_err("invalid Lua should fail");

    assert!(!diagnostics.is_empty());

    let first = &diagnostics[0];

    assert_eq!(first.file, PathBuf::from("src/broken.tlua"));
    assert_eq!(first.severity, crate::DiagnosticSeverity::Error);
    assert_eq!(first.code, Some(DiagnosticCode::SYNTAX_PARSE));
    assert!(first.line >= 1);
    assert!(first.column >= 1);
    assert!(first.format_cli().contains("error TL0001"));
}

fn parts(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}

fn assert_compound_lhs_error(source: &str) {
    let diagnostics = parse_tlua_source("src/compound.tlua", source)
        .expect_err("complex compound assignment targets should produce diagnostics");

    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::SYNTAX_COMPOUND_ASSIGNMENT)
    );
    assert!(diagnostics[0].message.contains("left-hand side"));
    assert!(diagnostics[0].line >= 1);
    assert!(diagnostics[0].column >= 1);
}

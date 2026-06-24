use super::*;
use crate::{check_tua_source, check_tua_source_with_diagnostics_config};
use std::path::PathBuf;

#[test]
fn raw_lua_parse_contains_lua_compatible_syntax_only() {
    let raw = parse_raw_lua_source("src/plain.lua", "local value = 1\n")
        .expect("plain Lua should raw parse");

    assert_eq!(raw.path, PathBuf::from("src/plain.lua"));
    assert_eq!(raw.language, ParsedLanguage::Lua);
    assert_eq!(raw.source, raw.parser_source);
    assert!(!raw.uses_normalized_parser_source());
    assert_eq!(raw.statement_count(), 1);
    assert!(raw.span.is_some());
}

#[test]
fn raw_tua_parse_uses_normalized_parser_source() {
    let raw = parse_raw_tua_source(
        "src/types.tua",
        r#"
type Player = {
  name?: string,
}
"#,
    )
    .expect("Tua source should raw parse after normalization");

    assert_eq!(raw.language, ParsedLanguage::Tua);
    assert_ne!(raw.source, raw.parser_source);
    assert!(raw.source.contains("name?: string"));
    assert!(raw.parser_source.contains("name: string?"));
    assert!(raw.uses_normalized_parser_source());
}

#[test]
fn normalized_source_preserves_original_and_exposes_parser_text() {
    let normalized = normalize_tua_source("type Slot = { name?: string }\n");

    assert_eq!(normalized.original, "type Slot = { name?: string }\n");
    assert_eq!(normalized.parser_source, "type Slot = { name: string? }\n");
}

#[test]
fn typed_source_extraction_is_separate_from_raw_parse() {
    let raw = parse_raw_tua_source(
        "src/player.tua",
        r#"
---@class Player documented from LuaDoc
type Player = {
  hp: number,
}

local player: Player = { hp = 10 }
"#,
    )
    .expect("raw source should parse");

    assert_eq!(raw.statement_count(), 2);

    let typed = extract_tua_source_model(&raw).expect("typed model should extract");

    assert_eq!(typed.lua_doc_annotations.len(), 1);
    assert_eq!(typed.type_aliases.len(), 1);
    assert_eq!(typed.local_annotations.len(), 1);
    assert!(typed.function_annotations.is_empty());
    assert!(typed.compound_assignments.is_empty());
}

#[test]
fn parsed_source_keeps_compatibility_views_over_raw_and_typed_models() {
    let parsed = parse_tua_source(
        "src/compat.tua",
        r#"
type Player = { hp: number }
local player: Player = { hp = 10 }
"#,
    )
    .expect("source should parse");

    assert_eq!(parsed.raw.language, ParsedLanguage::Tua);
    assert_eq!(parsed.typed.type_aliases.len(), 1);
    assert_eq!(parsed.path, parsed.raw.path);
    assert_eq!(parsed.source, parsed.raw.source);
    assert_eq!(parsed.type_aliases, parsed.typed.type_aliases);
    assert_eq!(parsed.local_annotations, parsed.typed.local_annotations);
}

#[test]
fn parses_local_assignments() {
    let parsed = parse_lua_source("src/local.tua", "local x = 1\nlocal y = x + 2\n")
        .expect("locals should parse");

    assert_eq!(parsed.path, PathBuf::from("src/local.tua"));
    assert_eq!(parsed.statement_count(), 2);
    assert!(parsed.span.is_some());
}

#[test]
fn parses_functions_and_returns() {
    let parsed = parse_lua_source(
        "src/function.tua",
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
        "src/table.tua",
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
        parse_lua_source("src/return.tua", "return { ok = true }\n").expect("return should parse");

    assert_eq!(parsed.statement_count(), 0);
    assert!(parsed.has_return());
}

#[test]
fn parses_require_calls() {
    let parsed = parse_lua_source(
        "src/require.tua",
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
    let parsed = parse_tua_source(
        "src/types.tua",
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

    let TuaType::Table(fields) = &vec2.ty else {
        panic!("Vec2 should be represented as a table shape");
    };

    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name, "x");
    assert_eq!(fields[0].ty, TuaType::Named("number".to_owned()));
    assert_eq!(fields[1].name, "y");
    assert_eq!(fields[1].ty, TuaType::Named("number".to_owned()));

    let player = &parsed.type_aliases[1];
    assert_eq!(player.name, "Player");

    let TuaType::Table(fields) = &player.ty else {
        panic!("Player should be represented as a table shape");
    };

    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0].name, "pos");
    assert_eq!(fields[0].ty, TuaType::Named("Vec2".to_owned()));
    assert_eq!(fields[1].name, "hp");
    assert_eq!(fields[1].ty, TuaType::Named("number".to_owned()));
    assert_eq!(fields[2].name, "alive");
    assert_eq!(fields[2].ty, TuaType::Named("boolean".to_owned()));
}

#[test]
fn parses_optional_field_syntax_from_spec() {
    let parsed = parse_tua_source(
        "src/save_slot.tua",
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

    let TuaType::Table(fields) = &alias.ty else {
        panic!("SaveSlot should be represented as a table shape");
    };

    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0].name, "name");
    assert!(fields[0].optional);
    assert_eq!(fields[0].ty, TuaType::Named("string".to_owned()));
    assert_eq!(fields[1].name, "level");
    assert!(!fields[1].optional);
    assert_eq!(fields[1].ty, TuaType::Named("number".to_owned()));
    assert_eq!(fields[2].name, "checkpoint");
    assert!(fields[2].optional);
    assert_eq!(fields[2].ty, TuaType::Named("string".to_owned()));
}

#[test]
fn parses_union_type_aliases() {
    let parsed = parse_tua_source("src/maybe.tua", "type MaybeNumber = number | nil\n")
        .expect("union type aliases should parse");

    assert_eq!(parsed.type_aliases.len(), 1);
    assert_eq!(parsed.type_aliases[0].name, "MaybeNumber");
    assert_eq!(
        parsed.type_aliases[0].ty,
        TuaType::Union(vec![
            TuaType::Named("number".to_owned()),
            TuaType::Named("nil".to_owned())
        ])
    );
}

#[test]
fn parses_local_annotations() {
    let parsed = parse_tua_source(
        "src/locals.tua",
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
        TuaType::Named("number".to_owned())
    );
    assert_eq!(parsed.local_annotations[1].name, "title");
    assert_eq!(
        parsed.local_annotations[1].ty,
        TuaType::Named("string".to_owned())
    );
    assert_eq!(parsed.local_annotations[2].name, "current");
    assert_eq!(
        parsed.local_annotations[2].ty,
        TuaType::Union(vec![
            TuaType::Named("Player".to_owned()),
            TuaType::Named("nil".to_owned())
        ])
    );
    assert_eq!(parsed.local_annotations[3].name, "payload");
    assert_eq!(
        parsed.local_annotations[3].ty,
        TuaType::Named("any".to_owned())
    );
}

#[test]
fn parses_function_parameter_and_return_annotations() {
    let parsed = parse_tua_source(
        "src/damage.tua",
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
        TuaType::Named("Player".to_owned())
    );
    assert_eq!(function.parameters[1].name, "amount");
    assert_eq!(
        function.parameters[1].ty,
        TuaType::Named("number".to_owned())
    );
    assert_eq!(
        function.return_type,
        Some(TuaType::Named("Player".to_owned()))
    );
}

#[test]
fn parses_luahelper_doc_annotations_into_source_model() {
    let parsed = parse_tua_source(
        "src/lua_doc.tua",
        r#"---@class Player : Entity playable character
---@field public hp number current health
---@field name? string optional display name
---@alias PlayerId string|number identifier
---@type { hp: number, alive?: boolean }
---@generic T:Player, U
---@version JIT
---@param player Player target player
---@param amount? number optional amount
---@return Player updated player
local function damage(player, amount)
  return player
end
"#,
    )
    .expect("Lua doc annotations should parse with Tua source");

    assert_eq!(parsed.lua_doc_annotations.len(), 10);

    let LuaDocAnnotationKind::Class(class) = &parsed.lua_doc_annotations[0].kind else {
        panic!("first annotation should be a class");
    };
    assert_eq!(class.name, "Player");
    assert_eq!(class.extends.as_deref(), Some("Entity"));
    assert_eq!(
        parsed.lua_doc_annotations[0].documentation.as_deref(),
        Some("playable character")
    );
    assert_eq!(class.name_span.start.line, 1);

    let LuaDocAnnotationKind::Field(field) = &parsed.lua_doc_annotations[1].kind else {
        panic!("second annotation should be a field");
    };
    assert_eq!(field.name, "hp");
    assert_eq!(field.visibility.as_deref(), Some("public"));
    assert!(!field.optional);
    assert_eq!(field.ty, TuaType::Named("number".to_owned()));
    assert_eq!(
        parsed.lua_doc_annotations[1].documentation.as_deref(),
        Some("current health")
    );

    let LuaDocAnnotationKind::Field(field) = &parsed.lua_doc_annotations[2].kind else {
        panic!("third annotation should be a field");
    };
    assert_eq!(field.name, "name");
    assert!(field.optional);

    let LuaDocAnnotationKind::Alias(alias) = &parsed.lua_doc_annotations[3].kind else {
        panic!("fourth annotation should be an alias");
    };
    assert_eq!(alias.name, "PlayerId");
    assert_eq!(
        alias.ty,
        TuaType::Union(vec![
            TuaType::Named("string".to_owned()),
            TuaType::Named("number".to_owned())
        ])
    );

    let LuaDocAnnotationKind::Type(type_annotation) = &parsed.lua_doc_annotations[4].kind else {
        panic!("fifth annotation should be a type annotation");
    };
    let TuaType::Table(fields) = &type_annotation.ty else {
        panic!("type annotation should contain a table shape");
    };
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name, "hp");
    assert!(!fields[0].optional);
    assert_eq!(fields[1].name, "alive");
    assert!(fields[1].optional);

    let LuaDocAnnotationKind::Generic(generic) = &parsed.lua_doc_annotations[5].kind else {
        panic!("sixth annotation should be generic");
    };
    assert_eq!(generic.parameters.len(), 2);
    assert_eq!(generic.parameters[0].name, "T");
    assert_eq!(
        generic.parameters[0].bound,
        Some(TuaType::Named("Player".to_owned()))
    );
    assert_eq!(generic.parameters[1].name, "U");

    let LuaDocAnnotationKind::Version(version) = &parsed.lua_doc_annotations[6].kind else {
        panic!("seventh annotation should be version");
    };
    assert_eq!(version.expression, "JIT");

    let LuaDocAnnotationKind::Param(param) = &parsed.lua_doc_annotations[7].kind else {
        panic!("eighth annotation should be a param");
    };
    assert_eq!(param.name, "player");
    assert_eq!(param.ty, TuaType::Named("Player".to_owned()));
    assert!(!param.optional);

    let LuaDocAnnotationKind::Param(param) = &parsed.lua_doc_annotations[8].kind else {
        panic!("ninth annotation should be a param");
    };
    assert_eq!(param.name, "amount");
    assert!(param.optional);

    let LuaDocAnnotationKind::Return(return_annotation) = &parsed.lua_doc_annotations[9].kind
    else {
        panic!("tenth annotation should be return");
    };
    assert_eq!(return_annotation.returns.len(), 1);
    assert_eq!(
        return_annotation.returns[0].ty,
        TuaType::Named("Player".to_owned())
    );
    assert_eq!(
        parsed.lua_doc_annotations[9].documentation.as_deref(),
        Some("updated player")
    );
    assert_eq!(parsed.lua_doc_annotations[9].span.start.line, 10);
    assert_eq!(parsed.lua_doc_annotations[9].tag_span.start.column, 4);
}

#[test]
fn parses_lua_doc_string_literal_union_fields_as_string() {
    let parsed = parse_lua_source(
        "src/lua_doc.lua",
        r#"---@class Enemy
---@field type 'friendly' | 'mad'
local Enemy = {}
"#,
    )
    .expect("Lua doc annotations should parse with Lua source");

    assert_eq!(parsed.lua_doc_annotations.len(), 2);

    let LuaDocAnnotationKind::Field(field) = &parsed.lua_doc_annotations[1].kind else {
        panic!("second annotation should be a field");
    };

    assert_eq!(field.name, "type");
    assert_eq!(field.ty, TuaType::Named("string".to_owned()));
}

#[test]
fn parses_lua_doc_annotations_without_requiring_valid_lua() {
    let result = parse_lua_doc_annotations(
        "src/broken.lua",
        r#"---@class Broken
local value =
"#,
    );

    assert_eq!(result.annotations.len(), 1);
    assert!(result.diagnostics.is_empty());
}

#[test]
fn malformed_lua_doc_annotations_are_diagnostic_only_when_enabled() {
    let source = r#"---@class
---@field hp
---@param value
---@return
"#;
    let quiet = parse_lua_doc_annotations("src/bad_doc.lua", source);
    let noisy = parse_lua_doc_annotations_with_options(
        "src/bad_doc.lua",
        source,
        LuaDocParseOptions {
            diagnostics_enabled: true,
        },
    );

    assert_eq!(quiet.annotations.len(), 0);
    assert!(quiet.diagnostics.is_empty());
    assert_eq!(noisy.diagnostics.len(), 4);
    assert!(noisy.diagnostics.iter().all(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::SYNTAX_DOC_ANNOTATION)
            && diagnostic.severity == crate::DiagnosticSeverity::Warning
    }));
}

#[test]
fn lua_doc_annotation_diagnostics_are_reported_only_when_configured() {
    let source = "---@param value\nlocal value = 1\n";

    check_tua_source("src/doc_quiet.tua", source)
        .expect("malformed Lua doc annotations should be quiet by default");

    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/doc_noisy.tua",
        source,
        &crate::DiagnosticsConfig {
            doc_annotations: true,
            ..crate::DiagnosticsConfig::default()
        },
    )
    .expect_err("enabled doc annotation diagnostics should report malformed comments");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::SYNTAX_DOC_ANNOTATION)
    );
}

#[test]
fn parses_valid_compound_assignments() {
    let parsed = parse_tua_source(
        "src/compound.tua",
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
    let diagnostics = parse_tua_source("src/compound.tua", "x ^= 1\n")
        .expect_err("operators outside tua v1 should produce diagnostics");

    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::SYNTAX_COMPOUND_ASSIGNMENT)
    );
    assert!(diagnostics[0].message.contains("unsupported"));
}

#[test]
fn keeps_type_syntax_out_of_lua_compatible_parser() {
    let diagnostics = parse_lua_source("src/typed.tua", "local score: number = 0\n")
        .expect_err("plain Lua parser should reject erased type syntax");

    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::SYNTAX_PARSE));
}

#[test]
fn syntax_errors_produce_tl000x_diagnostics_with_locations() {
    let diagnostics =
        parse_lua_source("src/broken.tua", "local x = \n").expect_err("invalid Lua should fail");

    assert!(!diagnostics.is_empty());

    let first = &diagnostics[0];

    assert_eq!(first.file, PathBuf::from("src/broken.tua"));
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
    let diagnostics = parse_tua_source("src/compound.tua", source)
        .expect_err("complex compound assignment targets should produce diagnostics");

    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::SYNTAX_COMPOUND_ASSIGNMENT)
    );
    assert!(diagnostics[0].message.contains("left-hand side"));
    assert!(diagnostics[0].line >= 1);
    assert!(diagnostics[0].column >= 1);
}

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
fn normalized_source_erases_enum_blocks_for_luau_parser() {
    let normalized = normalize_tua_source(
        r#"enum FilterMode
  "linear"
  "nearest"
end

local mode: FilterMode = "linear"
"#,
    );

    assert!(normalized.original.contains("enum FilterMode"));
    assert!(!normalized.parser_source.contains("enum FilterMode"));
    assert!(normalized.parser_source.contains("local mode: FilterMode"));
    assert_eq!(
        normalized.original.lines().count(),
        normalized.parser_source.lines().count()
    );
}

#[test]
fn normalized_source_rewrites_condition_is_checks_for_luau_parser() {
    let normalized = normalize_tua_source(
        r#"if value is Player then
  print(value)
elseif other is string then
  print(other)
while current is table do
  current = nil
end
end
"#,
    );

    assert!(normalized
        .parser_source
        .contains(r#"if __tua_is(value, "Player") then"#));
    assert!(normalized
        .parser_source
        .contains(r#"elseif __tua_is(other, "string") then"#));
    assert!(normalized
        .parser_source
        .contains(r#"while __tua_is(current, "table") do"#));
    parse_tua_source("src/runtime_is.tua", normalized.original)
        .expect("normalized is checks should parse");
}

#[test]
fn normalized_source_does_not_treat_keyword_suffixes_as_condition_keywords() {
    let source = r#"local stiff = 1
local whiled = 2
local ended = 3
if value is string then
  print(stiff, whiled, ended)
end
"#;
    let normalized = normalize_tua_source(source);

    assert!(normalized.parser_source.contains("local stiff = 1"));
    assert!(normalized.parser_source.contains("local whiled = 2"));
    assert!(normalized.parser_source.contains("local ended = 3"));
    assert!(normalized
        .parser_source
        .contains(r#"if __tua_is(value, "string") then"#));
    parse_tua_source("src/runtime_is_keywords.tua", source)
        .expect("identifiers ending in keyword text should parse");
}

#[test]
fn normalized_source_keeps_unsupported_is_target_text_for_diagnostics() {
    let normalized = normalize_tua_source(
        r#"if value is Array<string> then
  print(value)
end
"#,
    );

    assert!(normalized
        .parser_source
        .contains(r#"if __tua_is(value, "Array<string>") then"#));
    parse_tua_source("src/runtime_is.tua", normalized.original)
        .expect("unsupported is target should still parse for checker diagnostics");
}

#[test]
fn normalized_source_lowers_class_syntax_to_metatable_lua() {
    let source = r#"open class Entity {
  x: number
  y: number

  init(x: number, y: number) {
    self.x = x
    self.y = y
  }

  update(dt: number) {
  }
}

final class Player extends Entity {
  speed: number = 120

  init(x: number, y: number) {
    super(x, y)
  }

  override final update(dt: number) {
    self.x += self.speed * dt
  }
}
"#;
    let normalized = normalize_tua_source(source);

    assert!(normalized.parser_source.contains("local Entity = {}"));
    assert!(normalized.parser_source.contains("Entity.__type_id = 1"));
    assert!(normalized
        .parser_source
        .contains("setmetatable(Player, { __index = Entity })"));
    assert!(normalized
        .parser_source
        .contains("local function Player_update(self: Player, dt: number)"));
    assert!(normalized
        .parser_source
        .contains("Player.update = Player_update"));
    parse_tua_source("src/classes.tua", source).expect("class syntax should parse after lowering");
}

#[test]
fn class_array_fields_parse_after_lowering() {
    let source = r#"final class BeatManager {
  steps: number[]
  y: number

  init(y: number) {
    self.steps = { 1, 2, 3 }
    self.y = y
  }
}
"#;

    parse_tua_source("src/classes.tua", source)
        .expect("class array field syntax should parse after lowering");
}

#[test]
fn class_fields_parse_grouped_and_function_array_types() {
    let source = r#"final class Registry {
  values: (number | string)[]
  handlers: ((value: number) -> string)[]
}
"#;
    let parsed = parse_tua_source("src/classes.tua", source)
        .expect("grouped and function class array fields should parse");
    let class = &parsed.class_declarations[0];

    assert!(matches!(
        &class.fields[0].parsed_ty,
        TuaType::Array(item) if matches!(item.as_ref(), TuaType::Union(types) if types.len() == 2)
    ));
    assert!(matches!(
        &class.fields[1].parsed_ty,
        TuaType::Array(item) if matches!(item.as_ref(), TuaType::Function { .. })
    ));
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
    let vec2_span = vec2
        .span
        .expect("table alias should retain its declaration span");
    assert_eq!(vec2_span.start.line, 2);
    assert_eq!(vec2_span.end.line, 5);

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
    let player_span = player
        .span
        .expect("second table alias should retain its declaration span");
    assert_eq!(player_span.start.line, 7);
    assert_eq!(player_span.end.line, 11);

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
fn parses_literal_string_type_aliases() {
    let parsed = parse_tua_source(
        "src/load_mode.tua",
        r#"type LoadMode = "static" | "stream"
"#,
    )
    .expect("literal string aliases should parse");

    assert_eq!(
        parsed.type_aliases[0].ty,
        TuaType::Union(vec![
            TuaType::StringLiteral("static".to_owned()),
            TuaType::StringLiteral("stream".to_owned())
        ])
    );
}

#[test]
fn parses_enum_blocks_as_literal_union_aliases() {
    let parsed = parse_tua_source(
        "src/filter.tua",
        r#"enum FilterMode
  "linear"
  "nearest"
end

local mode: FilterMode = "linear"
"#,
    )
    .expect("enum blocks should parse");

    assert_eq!(parsed.statement_count(), 1);
    assert_eq!(parsed.type_aliases.len(), 1);
    assert_eq!(parsed.type_aliases[0].name, "FilterMode");
    assert_eq!(parsed.type_aliases[0].kind, TypeAliasKind::Enum);
    assert_eq!(
        parsed.type_aliases[0].ty,
        TuaType::Union(vec![
            TuaType::StringLiteral("linear".to_owned()),
            TuaType::StringLiteral("nearest".to_owned())
        ])
    );
}

#[test]
fn rejects_invalid_enum_members() {
    let diagnostics = parse_tua_source(
        "src/filter.tua",
        r#"enum FilterMode
  "linear"
  nearest
end
"#,
    )
    .expect_err("invalid enum members should be reported");

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("invalid enum member: expected a string literal"));
    assert!(diagnostics[0].format_cli().contains("error TL0001"));
}

#[test]
fn rejects_duplicate_enum_members() {
    let diagnostics = parse_tua_source(
        "src/filter.tua",
        r#"enum FilterMode
  "linear"
  "linear"
end
"#,
    )
    .expect_err("duplicate enum members should be reported");

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("duplicate enum member `linear`"));
    assert!(diagnostics[0].format_cli().contains("error TL0001"));
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
fn parses_default_parameters_on_named_functions() {
    let parsed = parse_tua_source(
        "src/shake.tua",
        r#"
function shake(duration: number = 0.3, strength = 1)
  return duration
end
"#,
    )
    .expect("default parameters should parse");

    let function = &parsed.function_annotations[0];

    assert_eq!(function.parameter_count, 2);
    assert_eq!(function.required_parameter_count, 0);
    assert_eq!(function.default_parameters.len(), 2);
    assert_eq!(function.default_parameters[0].name, "duration");
    assert_eq!(function.default_parameters[0].expression, "0.3");
    assert_eq!(function.default_parameters[1].name, "strength");
    assert_eq!(function.default_parameters[1].expression, "1");
}

#[test]
fn parses_optional_parameters_on_named_functions() {
    let parsed = parse_tua_source(
        "src/shake.tua",
        r#"
function shake(duration: number, easing?: string)
  return duration
end
"#,
    )
    .expect("optional parameters should parse");

    let function = &parsed.function_annotations[0];

    assert_eq!(function.parameter_count, 2);
    assert_eq!(function.required_parameter_count, 1);
    assert_eq!(function.optional_parameters.len(), 1);
    assert_eq!(function.optional_parameters[0].name, "easing");
    assert_eq!(
        function.parameters[1].ty,
        TuaType::Union(vec![
            TuaType::Named("string".to_owned()),
            TuaType::Named("nil".to_owned()),
        ])
    );
}

#[test]
fn rejects_non_trailing_optional_parameters() {
    let diagnostics = parse_tua_source(
        "src/shake.tua",
        r#"
function shake(easing?: string, duration: number)
  return duration
end
"#,
    )
    .expect_err("non-trailing optional parameters should be rejected");

    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::SYNTAX_PARSE));
    assert!(diagnostics[0]
        .message
        .contains("optional parameters must be trailing"));
}

#[test]
fn rejects_non_trailing_default_parameters() {
    let diagnostics = parse_tua_source(
        "src/shake.tua",
        r#"
function shake(duration = 0.3, strength)
  return duration + strength
end
"#,
    )
    .expect_err("non-trailing defaults should be rejected");

    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::SYNTAX_PARSE));
    assert!(diagnostics[0]
        .message
        .contains("default parameters must be trailing"));
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
fn parses_lua_doc_string_literal_union_fields_as_literals() {
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
    assert_eq!(
        field.ty,
        TuaType::Union(vec![
            TuaType::StringLiteral("friendly".to_owned()),
            TuaType::StringLiteral("mad".to_owned()),
        ])
    );
}

#[test]
fn parses_lua_doc_luacats_type_forms() {
    let parsed = parse_lua_source(
        "src/lua_doc.lua",
        r#"---@alias MaybeName string?
---@alias Images Image[]
---@alias Point [number, number]
---@alias Sources { [string]: Source }
---@alias ImageMap table<string, Image>
---@alias Loader fun(path: string, cache?: boolean): Image
"#,
    )
    .expect("Lua doc annotations should parse with Lua source");

    assert_eq!(parsed.lua_doc_annotations.len(), 6);

    let LuaDocAnnotationKind::Alias(alias) = &parsed.lua_doc_annotations[0].kind else {
        panic!("first annotation should be alias");
    };
    assert_eq!(
        alias.ty,
        TuaType::Union(vec![
            TuaType::Named("string".to_owned()),
            TuaType::Named("nil".to_owned()),
        ])
    );

    let LuaDocAnnotationKind::Alias(alias) = &parsed.lua_doc_annotations[1].kind else {
        panic!("second annotation should be alias");
    };
    assert_eq!(
        alias.ty,
        TuaType::Array(Box::new(TuaType::Named("Image".to_owned())))
    );

    let LuaDocAnnotationKind::Alias(alias) = &parsed.lua_doc_annotations[2].kind else {
        panic!("third annotation should be alias");
    };
    assert_eq!(
        alias.ty,
        TuaType::Tuple(vec![
            TuaType::Named("number".to_owned()),
            TuaType::Named("number".to_owned()),
        ])
    );

    let LuaDocAnnotationKind::Alias(alias) = &parsed.lua_doc_annotations[3].kind else {
        panic!("fourth annotation should be alias");
    };
    assert_eq!(
        alias.ty,
        TuaType::Dictionary {
            key: Box::new(TuaType::Named("string".to_owned())),
            value: Box::new(TuaType::Named("Source".to_owned())),
        }
    );

    let LuaDocAnnotationKind::Alias(alias) = &parsed.lua_doc_annotations[4].kind else {
        panic!("fifth annotation should be alias");
    };
    assert_eq!(
        alias.ty,
        TuaType::Dictionary {
            key: Box::new(TuaType::Named("string".to_owned())),
            value: Box::new(TuaType::Named("Image".to_owned())),
        }
    );

    let LuaDocAnnotationKind::Alias(alias) = &parsed.lua_doc_annotations[5].kind else {
        panic!("sixth annotation should be alias");
    };
    let TuaType::Function {
        parameters,
        returns,
    } = &alias.ty
    else {
        panic!("function alias should parse as function type");
    };
    assert_eq!(parameters.len(), 2);
    assert_eq!(parameters[0].name.as_deref(), Some("path"));
    assert_eq!(parameters[0].ty, TuaType::Named("string".to_owned()));
    assert_eq!(parameters[1].name.as_deref(), Some("cache"));
    assert!(parameters[1].optional);
    assert_eq!(returns, &vec![TuaType::Named("Image".to_owned())]);
}

#[test]
fn parses_lua_doc_overload_vararg_and_identifier_type() {
    let parsed = parse_lua_source(
        "src/lua_doc.lua",
        r#"---@overload fun(path: identifier): Image path overload
---@vararg identifier extra names
local function load(path, ...)
end
"#,
    )
    .expect("LuaDoc overload and vararg annotations should parse");

    assert_eq!(parsed.lua_doc_annotations.len(), 2);

    let LuaDocAnnotationKind::Overload(overload) = &parsed.lua_doc_annotations[0].kind else {
        panic!("first annotation should be overload");
    };
    let TuaType::Function {
        parameters,
        returns,
    } = &overload.ty
    else {
        panic!("overload should parse as a function type");
    };
    assert_eq!(parameters.len(), 1);
    assert_eq!(parameters[0].name.as_deref(), Some("path"));
    assert_eq!(parameters[0].ty, TuaType::Named("string".to_owned()));
    assert_eq!(returns, &vec![TuaType::Named("Image".to_owned())]);
    assert_eq!(
        parsed.lua_doc_annotations[0].documentation.as_deref(),
        Some("path overload")
    );
    assert_eq!(overload.type_span.start.line, 1);

    let LuaDocAnnotationKind::Vararg(vararg) = &parsed.lua_doc_annotations[1].kind else {
        panic!("second annotation should be vararg");
    };
    assert_eq!(vararg.ty, TuaType::Named("string".to_owned()));
    assert_eq!(
        parsed.lua_doc_annotations[1].documentation.as_deref(),
        Some("extra names")
    );
    assert_eq!(vararg.type_span.start.line, 2);
}

#[test]
fn parses_native_function_types_for_aliases_and_fields() {
    let parsed = parse_tua_source(
        "src/callbacks.tua",
        r#"type Player = { name: string }
type EventHandler = (player: Player, damage: number, ...string) -> (boolean, string)
type Hooks = {
  onHit: EventHandler,
  format: (value: number?) -> string,
}
"#,
    )
    .expect("native function types should parse");

    let handler = parsed
        .type_aliases
        .iter()
        .find(|alias| alias.name == "EventHandler")
        .expect("handler alias should exist");
    let TuaType::Function {
        parameters,
        returns,
    } = &handler.ty
    else {
        panic!("handler alias should be a function type");
    };

    assert_eq!(parameters.len(), 3);
    assert_eq!(parameters[0].name.as_deref(), Some("player"));
    assert_eq!(parameters[0].ty, TuaType::Named("Player".to_owned()));
    assert_eq!(parameters[1].name.as_deref(), Some("damage"));
    assert_eq!(parameters[1].ty, TuaType::Named("number".to_owned()));
    assert!(parameters[2].variadic);
    assert_eq!(parameters[2].ty, TuaType::Named("string".to_owned()));
    assert_eq!(
        returns,
        &vec![
            TuaType::Named("boolean".to_owned()),
            TuaType::Named("string".to_owned()),
        ]
    );

    let hooks = parsed
        .type_aliases
        .iter()
        .find(|alias| alias.name == "Hooks")
        .expect("hooks alias should exist");
    let TuaType::Table(fields) = &hooks.ty else {
        panic!("hooks should be a table type");
    };
    let TuaType::Function { parameters, .. } = &fields[1].ty else {
        panic!("format field should be a function type");
    };
    assert!(parameters[0].optional);
}

#[test]
fn parses_additional_luals_annotation_tags() {
    let parsed = parse_lua_source(
        "src/lua_doc.lua",
        r#"---@meta love.graphics
---@module 'graphics'
---@enum BlendMode
---@diagnostic disable-next-line: undefined-global
---@cast value +Player
---@as Player
---@deprecated use newPlayer
---@nodiscard
"#,
    )
    .expect("Lua doc annotations should parse with Lua source");

    assert_eq!(parsed.lua_doc_annotations.len(), 8);
    assert!(matches!(
        parsed.lua_doc_annotations[0].kind,
        LuaDocAnnotationKind::Meta(_)
    ));
    assert!(matches!(
        parsed.lua_doc_annotations[1].kind,
        LuaDocAnnotationKind::Module(_)
    ));
    assert!(matches!(
        parsed.lua_doc_annotations[2].kind,
        LuaDocAnnotationKind::Enum(_)
    ));
    assert!(matches!(
        parsed.lua_doc_annotations[3].kind,
        LuaDocAnnotationKind::Diagnostic(_)
    ));
    assert!(matches!(
        parsed.lua_doc_annotations[4].kind,
        LuaDocAnnotationKind::Cast(_)
    ));
    assert!(matches!(
        parsed.lua_doc_annotations[5].kind,
        LuaDocAnnotationKind::As(_)
    ));
    assert!(matches!(
        parsed.lua_doc_annotations[6].kind,
        LuaDocAnnotationKind::Deprecated(_)
    ));
    assert!(matches!(
        parsed.lua_doc_annotations[7].kind,
        LuaDocAnnotationKind::NoDiscard(_)
    ));
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

use super::*;
use crate::check_tua_source_with_diagnostics_config;
use proptest::prelude::*;
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
fn parses_exported_aliases_and_erases_type_only_imports_for_the_runtime_ast() {
    let source = r#"import   type Shared   from   "./shared_types" -- compile-time only
import type {
  Player,
  Room as ImportedRoom,
} from "./models"
export type { Player as PublicPlayer } from "./models"
export type Room = { name: string, owner: Shared.Player }
local room: Room
"#;
    let parsed =
        parse_tua_source("src/rooms.tua", source).expect("type-only module syntax should parse");

    assert_eq!(parsed.type_imports.len(), 3);
    assert_eq!(parsed.type_imports[0].namespace, "Shared");
    assert_eq!(parsed.type_imports[0].module, "./shared_types");
    assert_eq!(parsed.type_imports[1].bindings.len(), 2);
    assert_eq!(parsed.type_imports[1].bindings[1].imported, "Room");
    assert_eq!(parsed.type_imports[1].bindings[1].local, "ImportedRoom");
    assert!(parsed.type_imports[2].reexport);
    assert_eq!(parsed.type_imports[2].bindings[0].local, "PublicPlayer");
    assert_eq!(parsed.type_aliases.len(), 1);
    assert!(parsed.type_aliases[0].exported);
    assert!(!parsed.raw.parser_source.contains("import"));
    assert!(!parsed.raw.parser_source.contains("PublicPlayer"));
    assert_eq!(parsed.statement_count(), 2);
}

#[test]
fn raw_tua_parse_keeps_large_plain_lua_literals_on_the_lua51_fast_path() {
    let records = (0..4_000)
        .map(|index| {
            format!(
                "  {{ id = {index}, name = \"room-{index}\", bounds = {{ min = 0, max = 100 }} }},\n"
            )
        })
        .collect::<String>();
    let source = format!("local rooms = {{\n{records}}}\nreturn rooms\n");
    let raw = parse_raw_tua_source("src/generated_rooms.tua", source.clone())
        .expect("plain generated Lua should parse as Tua");

    assert_eq!(raw.language, ParsedLanguage::Tua);
    assert_eq!(raw.parser_source, source);
    assert!(!raw.uses_normalized_parser_source());
    assert_eq!(raw.statement_count(), 1);

    extract_tua_source_model(&raw).expect("large plain generated data should build a Tua model");
}

#[test]
fn class_scanning_remains_bounded_for_keyword_heavy_generated_data() {
    let fields = (0..4_000)
        .map(|index| format!("class = {index}, open = {index}, final = {index}, "))
        .collect::<String>();
    let source = format!("--!tua generated\nreturn {{ {fields} }}\n");
    let raw = parse_raw_tua_source("src/keyword_data.tua", source)
        .expect("keyword-shaped generated fields should remain plain data");

    extract_tua_source_model(&raw)
        .expect("class discovery should reject candidate headers without rescanning the source");
}

#[test]
fn generated_directive_keeps_plain_lua_functions_on_the_fast_path() {
    let source = r#"--!tua generated
local values = { 1, 2, 3 }
local Module = {}
function Module.all()
  return values
end
return Module
"#;
    let raw = parse_raw_tua_source("src/generated_values.tua", source)
        .expect("explicit generated Lua modules should parse directly");

    assert_eq!(raw.source, raw.parser_source);
    assert!(!raw.uses_normalized_parser_source());
}

#[test]
fn generated_directive_does_not_admit_tua_syntax_to_the_lua_fast_path() {
    let diagnostics = parse_raw_tua_source(
        "src/invalid_generated.tua",
        "--!tua generated\nfunction make(value = 1) return value end\n",
    )
    .expect_err("generated modules are an explicitly Lua-compatible boundary");

    assert!(!diagnostics.is_empty());
}

#[test]
fn grammar_classifier_ignores_tua_tokens_inside_generated_data_strings_and_comments() {
    let source = r#"-- a generator may mention `type`, class, or obj:method here
return {
  marker = "type Player = { name?: string }",
  url = "asset://rooms/final",
}
"#;

    assert!(!normalize::requires_tua_grammar(source));
    let raw = parse_raw_tua_source("src/generated_strings.tua", source)
        .expect("protected text must remain on the plain Lua fast path");
    assert!(!raw.uses_normalized_parser_source());
}

#[test]
fn grammar_classifier_conservatively_routes_executable_and_typed_syntax() {
    for source in [
        "local value: number = 1\n",
        "local value = if ready then 1 else 0\n",
        "function make(value = 1) return value end\n",
        "value += 1\n",
        "return `value {value}`\n",
    ] {
        assert!(
            normalize::requires_tua_grammar(source),
            "expected Tua grammar for {source:?}"
        );
    }
}

#[test]
fn normalized_source_preserves_original_and_exposes_parser_text() {
    let normalized = normalize_tua_source("type Slot = { name?: string }\n");

    assert_eq!(normalized.original, "type Slot = { name?: string }\n");
    assert_eq!(normalized.parser_source, "type Slot = { name: string? }\n");
}

#[test]
fn normalized_source_rewrites_native_array_types_without_touching_data_or_comments() {
    let source = r#"local short = "Actor[]"
local long = [=[Actor[]]=]
-- Actor[]
type Actors = Actor[]
local actors: Actor[][] = {}
"#;
    let normalized = normalize_tua_source(source);

    assert!(normalized.parser_source.contains(r#""Actor[]""#));
    assert!(normalized.parser_source.contains("[=[Actor[]]=]"));
    assert!(normalized.parser_source.contains("-- Actor[]"));
    assert!(normalized.parser_source.contains("type Actors = Actor  "));
    assert!(normalized
        .parser_source
        .contains("local actors: Actor     = {}"));
    assert_eq!(normalized.native_array_types.len(), 2);
}

#[test]
fn an_unclosed_parameter_list_keeps_every_line_while_typing() {
    // Typing `local function f(` above existing code leaves the parameter list
    // open to the end of the file, so the default-value eraser reaches every
    // following line. It must keep their newlines.
    let source = "local function f(\n  local t = { x = 1 }\nend\nreturn f\n";
    let normalized = normalize_tua_source(source);

    assert_eq!(
        normalized.parser_source.lines().count(),
        source.lines().count()
    );
    assert!(parse_tua_source("src/main.tua", source).is_err());
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
fn class_scanning_ignores_lua_long_strings_and_comments() {
    let source = r#"local text = [=[
class FakeString {
  value: number
}
]=]

--[==[
class FakeComment {
}
]==]

final class Real {
  text(): string {
    return [===[} class FakeBody {]===]
  }
}
"#;
    let normalized = normalize_tua_source(source);

    assert!(normalized.parser_source.contains("class FakeString"));
    assert!(normalized.parser_source.contains("class FakeComment"));
    assert!(!normalized.parser_source.contains("local FakeString = {}"));
    assert!(!normalized.parser_source.contains("local FakeComment = {}"));
    assert!(!normalized.parser_source.contains("local FakeBody = {}"));
    assert!(normalized.parser_source.contains("local Real = {}"));

    let parsed = parse_tua_source("src/classes.tua", source)
        .expect("class-like text in long strings and comments should remain data");
    assert_eq!(
        parsed
            .class_declarations
            .iter()
            .map(|class| class.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Real"]
    );
    assert_eq!(parsed.class_declarations[0].methods.len(), 1);
}

#[test]
fn malformed_class_members_report_a_class_diagnostic() {
    let diagnostics = parse_tua_source(
        "src/classes.tua",
        r#"final class Broken {
  value number
  accidental = 42
}
"#,
    )
    .expect_err("unrecognized class members should not be silently erased");
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_CLASS))
        .expect("malformed class member should report TL1008");

    assert_eq!((diagnostic.line, diagnostic.column), (2, 3));
    assert_eq!(
        diagnostic.message,
        "unrecognized class member; expected a typed field or method declaration"
    );
}

#[test]
fn class_methods_accept_table_shaped_return_types() {
    let source = r#"final class Factory {
  make(): { value: number } {
    return { value = 1 }
  }
}
"#;
    let parsed = parse_tua_source("src/classes.tua", source)
        .expect("table-shaped class method returns should parse");
    let method = &parsed.class_declarations[0].methods[0];

    assert_eq!(method.return_type.as_deref(), Some("{ value: number }"));
    assert!(matches!(
        parse_tua_type_text(method.return_type.as_deref().expect("return type")),
        TuaType::Table(fields) if fields.len() == 1 && fields[0].name == "value"
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
local names: { [string]: Player } = {}
"#,
    )
    .expect("local annotations should parse");

    assert_eq!(parsed.local_annotations.len(), 5);
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
    assert_eq!(
        parsed.local_annotations[4].ty,
        TuaType::Dictionary {
            key: Box::new(TuaType::Named("string".to_owned())),
            value: Box::new(TuaType::Named("Player".to_owned())),
        }
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
fn parses_native_array_types_in_all_tua_annotation_positions() {
    let parsed = parse_tua_source(
        "src/actors.tua",
        r#"
type Actor = {
  attack: (self: Actor) -> nil,
}
type Actors = Actor[]
type Group = {
  members: Actor[],
}

local actors: Actor[] = {}
local nested: Actor[][] = {}
local optional_items: Actor?[] = {}
local optional_array: Actor[]? = nil
local mixed: (Actor | string)[] = {}
local callbacks: ((actor: Actor) -> nil)[] = {}
local records: { name: string }[] = {}
local multiline: (
  Actor
  | string
)[] = {}

local function keep(values: Actor[]): Actor[]
  return values
end

final class Squad {
  members: Actor[]

  keep(values: Actor[]): Actor[] {
    return values
  }
}
"#,
    )
    .expect("native array syntax should parse in every Tua annotation position");

    assert!(matches!(
        &parsed.type_aliases[1].ty,
        TuaType::Array(item) if item.as_ref() == &TuaType::Named("Actor".to_owned())
    ));
    assert!(matches!(
        &parsed.type_aliases[2].ty,
        TuaType::Table(fields)
            if matches!(&fields[0].ty, TuaType::Array(item) if item.as_ref() == &TuaType::Named("Actor".to_owned()))
    ));
    assert!(matches!(
        &parsed.local_annotations[0].ty,
        TuaType::Array(item) if item.as_ref() == &TuaType::Named("Actor".to_owned())
    ));
    assert!(matches!(
        &parsed.local_annotations[1].ty,
        TuaType::Array(item)
            if matches!(item.as_ref(), TuaType::Array(nested) if nested.as_ref() == &TuaType::Named("Actor".to_owned()))
    ));
    assert!(matches!(
        &parsed.local_annotations[2].ty,
        TuaType::Array(item)
            if matches!(item.as_ref(), TuaType::Union(types) if types.contains(&TuaType::Named("nil".to_owned())))
    ));
    assert!(matches!(
        &parsed.local_annotations[3].ty,
        TuaType::Union(types)
            if types.iter().any(|ty| matches!(ty, TuaType::Array(item) if item.as_ref() == &TuaType::Named("Actor".to_owned())))
                && types.contains(&TuaType::Named("nil".to_owned()))
    ));
    assert!(matches!(
        &parsed.local_annotations[4].ty,
        TuaType::Array(item) if matches!(item.as_ref(), TuaType::Union(types) if types.len() == 2)
    ));
    assert!(matches!(
        &parsed.local_annotations[5].ty,
        TuaType::Array(item) if matches!(item.as_ref(), TuaType::Function { .. })
    ));
    assert!(matches!(
        &parsed.local_annotations[6].ty,
        TuaType::Array(item) if matches!(item.as_ref(), TuaType::Table(_))
    ));
    assert!(matches!(
        &parsed.local_annotations[7].ty,
        TuaType::Array(item) if matches!(item.as_ref(), TuaType::Union(types) if types.len() == 2)
    ));

    let keep = parsed
        .function_annotations
        .iter()
        .find(|function| function.name.as_deref() == Some("keep"))
        .expect("local function signature should be retained");
    assert!(matches!(keep.parameters[0].ty, TuaType::Array(_)));
    assert!(matches!(keep.return_type, Some(TuaType::Array(_))));

    let squad = parsed
        .class_declarations
        .iter()
        .find(|class| class.name == "Squad")
        .expect("class declaration should be retained");
    assert!(matches!(squad.fields[0].parsed_ty, TuaType::Array(_)));
    let class_keep = parsed
        .function_annotations
        .iter()
        .find(|function| function.name.as_deref() == Some("Squad_keep"))
        .expect("lowered class method signature should parse");
    assert!(matches!(class_keep.parameters[1].ty, TuaType::Array(_)));
    assert!(matches!(class_keep.return_type, Some(TuaType::Array(_))));
}

#[test]
fn native_array_lowering_preserves_the_item_type_token_position() {
    use full_moon::ast::luau::TypeInfo;
    use full_moon::visitors::Visitor;

    #[derive(Default)]
    struct ActorTypePosition {
        spans: Vec<SourceSpan>,
    }

    impl Visitor for ActorTypePosition {
        fn visit_type_info(&mut self, type_info: &TypeInfo) {
            if matches!(type_info, TypeInfo::Basic(token) if token.to_string().trim() == "Actor") {
                if let Some((start, end)) = type_info.range() {
                    self.spans.push(SourceSpan::from_range(start, end));
                }
            }
        }
    }

    let raw = parse_raw_tua_source("src/actors.tua", "local actors: Actor[] = {}\n")
        .expect("native array syntax should raw parse");
    let mut positions = ActorTypePosition::default();
    positions.visit_ast(&raw.ast);

    assert_eq!(
        positions.spans,
        vec![SourceSpan {
            start: SourcePosition {
                line: 1,
                column: 15,
            },
            end: SourcePosition {
                line: 1,
                column: 20,
            },
        }]
    );
}

#[test]
fn rejects_luau_array_type_syntax_and_array_suffixes_outside_types() {
    let luau_diagnostics = parse_tua_source("src/actors.tua", "local actors: { Actor } = {}\n")
        .expect_err("Luau array syntax should not become Tua syntax");

    assert_eq!(luau_diagnostics.len(), 1);
    assert_eq!(
        luau_diagnostics[0].code,
        Some(DiagnosticCode::SYNTAX_UNSUPPORTED_DIALECT)
    );
    assert!(luau_diagnostics[0].message.contains("use native `T[]`"));

    let expression_diagnostics = parse_tua_source("src/actors.tua", "local actors = Actor[]\n")
        .expect_err("array suffixes should not rewrite runtime expressions");

    assert_eq!(expression_diagnostics.len(), 1);
    assert_eq!(
        expression_diagnostics[0].code,
        Some(DiagnosticCode::SYNTAX_PARSE)
    );
    assert!(expression_diagnostics[0]
        .message
        .contains("only valid after a type"));
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
fn records_free_references_in_default_parameter_expressions() {
    let parsed = parse_tua_source(
        "src/pick.tua",
        r#"
local fallback = 10
local function pick(value = transform(fallback))
  return value
end
"#,
    )
    .expect("default parameter references should parse");

    let references = parsed
        .parameter_default_references
        .iter()
        .map(|reference| reference.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(references, vec!["transform", "fallback"]);
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
    assert_eq!(
        parsed.lua_doc_annotations[6].documentation.as_deref(),
        Some("use newPlayer")
    );
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

    check_tua_source_with_diagnostics_config(
        "src/doc_quiet.tua",
        source,
        &crate::DiagnosticsConfig {
            rules: false,
            ..crate::DiagnosticsConfig::default()
        },
    )
    .expect("malformed Lua doc annotations should be quiet by default");

    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/doc_noisy.tua",
        source,
        &crate::DiagnosticsConfig {
            rules: false,
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
fn rejects_luau_only_forms_at_the_tua_parse_boundary() {
    let cases = [
        (
            "continue",
            "while running do\n  continue\nend\n",
            "`continue`",
        ),
        (
            "type assertion",
            "local value = 1 :: number\n",
            "assertions",
        ),
        (
            "conditional expression",
            "local value = if ready then 1 else 2\n",
            "conditional expressions",
        ),
        (
            "interpolated string",
            "local value = `hello {name}`\n",
            "interpolated strings",
        ),
        ("floor division", "local value = 5 // 2\n", "floor division"),
        ("const declaration", "const value = 1\n", "`const`"),
        (
            "const function",
            "const function run() end\n",
            "`const function`",
        ),
        (
            "function attribute",
            "@native local function run() end\n",
            "attributes",
        ),
        (
            "generic declaration",
            "type Box<T> = { value: T }\n",
            "generic declarations",
        ),
        (
            "generic type application",
            "local value: Box<number>\n",
            "generic type applications",
        ),
        (
            "intersection type",
            "type Value = number & string\n",
            "intersection types",
        ),
        (
            "typeof type",
            "type Value = typeof(value)\n",
            "`typeof(...)`",
        ),
        (
            "type function",
            "type function Value() return types.number end\n",
            "type functions",
        ),
        (
            "exported type function",
            "export type function Value() return types.number end\n",
            "exported type functions",
        ),
        (
            "field access modifier",
            "type Value = { read name: string }\n",
            "access modifiers",
        ),
        (
            "array access modifier",
            "type Value = { read number }\n",
            "access modifiers",
        ),
        (
            "module generic type application",
            "local value: Module.Box<number>\n",
            "generic type applications",
        ),
        (
            "generic return pack",
            "type Callback = () -> T...\n",
            "generic type packs",
        ),
        (
            "runtime type instantiation",
            "local value = identity<<number>>(1)\n",
            "runtime type instantiation",
        ),
    ];

    for (name, source, expected_message) in cases {
        let diagnostics = parse_tua_source(format!("src/{name}.tua"), source)
            .expect_err("Luau-only syntax should not be accepted as Tua");
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == Some(DiagnosticCode::SYNTAX_UNSUPPORTED_DIALECT))
            .unwrap_or_else(|| panic!("{name} did not produce TL0004: {diagnostics:#?}"));

        assert!(
            diagnostic.message.contains(expected_message),
            "{name} produced an unexpected message: {diagnostic:#?}"
        );
    }
}

#[test]
fn tua_dialect_validation_preserves_documented_type_forms() {
    parse_tua_source(
        "src/types.tua",
        r#"
type Name = string
type OptionalName = Name?
type Choice = "one" | "two"
type Lookup = { [string]: number }
type Callback = (value: number, ...string) -> (number, string)
type Qualified = Module.Value

local callback: Callback
"#,
    )
    .expect("documented Tua type forms should remain valid");
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

#[test]
fn malformed_backtick_after_string_is_a_recoverable_syntax_error() {
    let source = "\"\"`";

    assert!(parse_raw_tua_source("src/untrusted.tua", source).is_err());
    assert!(parse_raw_lua_source("src/untrusted.lua", source).is_err());
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

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 128,
        max_shrink_iters: 1_024,
        ..ProptestConfig::default()
    })]

    #[test]
    fn bounded_arbitrary_unicode_never_panics_parser_pipeline(
        characters in proptest::collection::vec(any::<char>(), 0..=512),
    ) {
        let source: String = characters.into_iter().collect();
        let normalized = normalize_tua_source(source.clone());

        prop_assert_eq!(&normalized.original, &source);

        // Syntax errors are expected for arbitrary input. The property under test is that
        // untrusted editor and CLI input remains a recoverable parser result.
        let parsed = parse_raw_tua_source("fuzz/input.tua", source.clone());
        let _ = parse_raw_lua_source("fuzz/input.lua", source);
        if parsed.is_ok() {
            let _ = crate::check_tua_source("fuzz/input.tua", normalized.original.clone());
            let _ = crate::format_tua_source(
                "fuzz/input.tua",
                normalized.original.clone(),
                &crate::FormatOptions::default(),
            );
            let _ = crate::emit_tua_source("fuzz/input.tua", &normalized.original);
        }
    }
}

#[test]
fn parses_multiline_lua_doc_alias_members() {
    let parsed = parse_lua_source(
        "src/lua_doc.lua",
        r#"---@alias DrawMode
---| "fill" # solid shape
---|> "line"
---| number

---@alias Single
---| "only"
local value = 1
"#,
    )
    .expect("multi-line aliases should parse with Lua source");

    assert_eq!(parsed.lua_doc_annotations.len(), 2);

    let LuaDocAnnotationKind::Alias(alias) = &parsed.lua_doc_annotations[0].kind else {
        panic!("first annotation should be an alias");
    };
    assert_eq!(alias.name, "DrawMode");
    assert_eq!(
        alias.ty,
        TuaType::Union(vec![
            TuaType::StringLiteral("fill".to_owned()),
            TuaType::StringLiteral("line".to_owned()),
            TuaType::Named("number".to_owned()),
        ])
    );
    assert_eq!(alias.type_span.start.line, 2);
    assert_eq!(alias.type_span.end.line, 4);

    let LuaDocAnnotationKind::Alias(alias) = &parsed.lua_doc_annotations[1].kind else {
        panic!("second annotation should be an alias");
    };
    assert_eq!(alias.name, "Single");
    assert_eq!(alias.ty, TuaType::StringLiteral("only".to_owned()));
}

#[test]
fn bare_lua_doc_alias_without_members_still_reports_its_missing_type() {
    let parsed = parse_lua_doc_annotations_with_options(
        "src/lua_doc.lua",
        "---@alias Missing\nlocal value = 1\n",
        LuaDocParseOptions {
            diagnostics_enabled: true,
        },
    );

    assert!(parsed.annotations.is_empty());
    assert_eq!(parsed.diagnostics.len(), 1, "{:?}", parsed.diagnostics);
}

use super::*;
use crate::{parse_lua_source, parse_tua_source, DiagnosticCode};

#[test]
fn primitive_assignability_requires_matching_primitives() {
    assert!(is_assignable(&Type::number(), &Type::number()));
    assert!(is_assignable(&Type::string(), &Type::string()));
    assert!(is_assignable(&Type::boolean(), &Type::boolean()));
    assert!(is_assignable(&Type::nil(), &Type::nil()));

    assert!(!is_assignable(&Type::number(), &Type::string()));
    assert!(!is_assignable(&Type::boolean(), &Type::number()));
    assert!(!is_assignable(&Type::nil(), &Type::number()));
}

#[test]
fn any_is_assignable_in_both_directions() {
    assert!(is_assignable(&Type::Any, &Type::number()));
    assert!(is_assignable(&Type::number(), &Type::Any));
    assert!(is_assignable(&Type::Any, &Type::empty_table()));
}

#[test]
fn unions_accept_values_assignable_to_any_member() {
    let number_or_nil = Type::union([Type::number(), Type::nil()]);

    assert!(is_assignable(&Type::number(), &number_or_nil));
    assert!(is_assignable(&Type::nil(), &number_or_nil));
    assert!(!is_assignable(&Type::string(), &number_or_nil));
}

#[test]
fn source_unions_must_have_every_member_assignable_to_target() {
    let numeric = Type::union([Type::number(), Type::nil()]);
    let number_or_string_or_nil = Type::union([Type::number(), Type::string(), Type::nil()]);
    let number_or_nil = Type::union([Type::number(), Type::nil()]);

    assert!(is_assignable(&numeric, &number_or_nil));
    assert!(!is_assignable(&number_or_string_or_nil, &number_or_nil));
}

#[test]
fn aliases_resolve_through_type_context() {
    let context = TypeContext::new().with_alias(TypeAlias::new("HitPoints", Type::number()));

    assert!(context.is_assignable(&Type::alias("HitPoints"), &Type::number()));
    assert!(context.is_assignable(&Type::number(), &Type::alias("HitPoints")));
    assert!(!context.is_assignable(&Type::string(), &Type::alias("HitPoints")));
}

#[test]
fn unknown_aliases_are_only_assignable_to_themselves() {
    assert!(is_assignable(&Type::alias("Vec2"), &Type::alias("Vec2")));
    assert!(!is_assignable(&Type::alias("Vec2"), &Type::alias("Player")));
    assert!(!is_assignable(&Type::alias("Vec2"), &Type::empty_table()));
}

#[test]
fn table_shape_compatibility_is_structural() {
    let source = Type::table([
        ("x", TableField::required(Type::number())),
        ("y", TableField::required(Type::number())),
        ("debug_name", TableField::required(Type::string())),
    ]);
    let target = Type::table([
        ("x", TableField::required(Type::number())),
        ("y", TableField::required(Type::number())),
    ]);

    assert!(is_assignable(&source, &target));
    assert!(!is_assignable(&target, &source));
}

#[test]
fn table_fields_must_be_assignable() {
    let source = Type::table([("hp", TableField::required(Type::string()))]);
    let target = Type::table([("hp", TableField::required(Type::number()))]);

    assert!(!is_assignable(&source, &target));
}

#[test]
fn optional_target_fields_may_be_missing() {
    let source = Type::table([("level", TableField::required(Type::number()))]);
    let target = Type::table([
        ("level", TableField::required(Type::number())),
        ("name", TableField::optional(Type::string())),
    ]);

    assert!(is_assignable(&source, &target));
}

#[test]
fn optional_source_fields_do_not_satisfy_required_target_fields() {
    let source = Type::table([("name", TableField::optional(Type::string()))]);
    let target = Type::table([("name", TableField::required(Type::string()))]);

    assert!(!is_assignable(&source, &target));
}

#[test]
fn optional_fields_still_check_present_value_types() {
    let source = Type::table([("name", TableField::optional(Type::number()))]);
    let target = Type::table([("name", TableField::optional(Type::string()))]);

    assert!(!is_assignable(&source, &target));
}

#[test]
fn function_types_are_contravariant_in_parameters_and_covariant_in_return() {
    let any_to_number = Type::function([Type::Any], Type::number());
    let number_to_number = Type::function([Type::number()], Type::number());
    let string_to_number = Type::function([Type::string()], Type::number());
    let any_to_string = Type::function([Type::Any], Type::string());

    assert!(is_assignable(&any_to_number, &number_to_number));
    assert!(!is_assignable(&string_to_number, &number_to_number));
    assert!(!is_assignable(&any_to_string, &number_to_number));
}

#[test]
fn string_literals_are_assignable_to_string_and_exact_literal_targets() {
    let mode = Type::union([
        Type::string_literal("static"),
        Type::string_literal("stream"),
    ]);

    assert!(is_assignable(
        &Type::string_literal("static"),
        &Type::string()
    ));
    assert!(is_assignable(&Type::string_literal("static"), &mode));
    assert!(!is_assignable(&Type::string_literal("dynamic"), &mode));
    assert!(!is_assignable(&Type::string(), &mode));
}

#[test]
fn container_types_are_assignable_by_shape() {
    assert!(is_assignable(
        &Type::array(Type::alias("Image")),
        &Type::array(Type::alias("Image")),
    ));
    assert!(is_assignable(
        &Type::tuple([Type::number(), Type::number()]),
        &Type::tuple([Type::number(), Type::number()]),
    ));
    assert!(is_assignable(
        &Type::dictionary(Type::string(), Type::alias("Source")),
        &Type::dictionary(Type::string(), Type::alias("Source")),
    ));
}

#[test]
fn builds_per_file_type_environment_from_aliases_and_annotations() {
    let parsed = parse_tua_source(
        "src/player.tua",
        r#"
type Vec2 = {
  x: number,
  y: number,
}

type Player = {
  pos: Vec2,
  name?: string,
}

local player: Player = nil

local function move(player: Player, delta: Vec2): Player
  return player
end
"#,
    )
    .expect("typed source should parse");
    let environment =
        build_type_environment(&parsed).expect("type environment should collect annotations");

    assert_eq!(environment.path(), parsed.path.as_path());
    assert_eq!(environment.alias("Vec2"), Some(&vec2_type()));
    assert_eq!(environment.local("player"), Some(&Type::alias("Player")));

    let functions = environment.functions();
    assert_eq!(functions.len(), 1);
    assert_eq!(functions[0].name.as_deref(), Some("move"));
    assert_eq!(functions[0].parameter_count, 2);
    assert!(!functions[0].has_varargs);
    assert!(functions[0].has_fully_annotated_parameters());
    assert_eq!(functions[0].parameters[0].name, "player");
    assert_eq!(functions[0].parameters[0].ty, Type::alias("Player"));
    assert_eq!(functions[0].parameters[1].name, "delta");
    assert_eq!(functions[0].parameters[1].ty, Type::alias("Vec2"));
    assert_eq!(functions[0].return_type, Some(Type::alias("Player")));
}

#[test]
fn infers_function_return_types_from_annotated_parameters() {
    let parsed = parse_tua_source(
        "src/functions.tua",
        r#"
type Player = { hp: number }

local function identity(value: string)
  return value
end

function getHp(player: Player)
  return player.hp
end
"#,
    )
    .expect("typed source should parse");
    let environment =
        build_type_environment(&parsed).expect("type environment should infer returns");

    assert_eq!(
        environment
            .function("identity")
            .and_then(|function| function.return_type.clone()),
        Some(Type::string())
    );
    assert_eq!(
        environment
            .function("getHp")
            .and_then(|function| function.return_type.clone()),
        Some(Type::number())
    );
}

#[test]
fn infers_function_return_types_with_flow_sensitive_narrowing() {
    let parsed = parse_tua_source(
        "src/narrowing.tua",
        r#"
local function fromOptional(value: number | nil)
  if value ~= nil then
    return value
  end

  return 0
end

local function fromTypeGuard(value: string | number)
  if type(value) == "string" then
    return value
  end

  return ""
end
"#,
    )
    .expect("typed source should parse");
    let environment =
        build_type_environment(&parsed).expect("type environment should infer narrowed returns");

    assert_eq!(
        environment
            .function("fromOptional")
            .and_then(|function| function.return_type.clone()),
        Some(Type::number())
    );
    assert_eq!(
        environment
            .function("fromTypeGuard")
            .and_then(|function| function.return_type.clone()),
        Some(Type::string())
    );
}

#[test]
fn infers_love_method_function_returns_from_inferred_locals() {
    let parsed = parse_tua_source(
        "src/love_returns.tua",
        r#"
local function loadPlayerImage()
  return image "assets/player.png"
end

local player = loadPlayerImage()

local function azul()
  return player.getDimensions()
end

local c = azul()

local function sumWidth()
  return c + 1
end
"#,
    )
    .expect("typed source should parse");
    let environment =
        build_type_environment(&parsed).expect("type environment should infer Love returns");

    let azul = environment
        .function("azul")
        .expect("azul should have an inferred function type");

    assert_eq!(azul.return_type, Some(Type::number()));
    assert_eq!(azul.return_types, vec![Type::number(), Type::number()]);
    assert_eq!(
        environment
            .function("sumWidth")
            .and_then(|function| function.return_type.clone()),
        Some(Type::number())
    );
}

#[test]
fn lowers_lua_doc_annotations_into_type_environment() {
    let parsed = parse_lua_source(
        "src/player.lua",
        r#"---@class Player player object
---@field hp number current health
---@field name? string display name
---@alias PlayerId string|number stable id
---@type Player
Player = {}

---@type number global score
Score = 0

---@param name string player name
---@return Player created player
function makePlayer(name)
  return Player
end
"#,
    )
    .expect("Lua source with doc annotations should parse");
    let environment = build_type_environment(&parsed).expect("Lua doc annotations should lower");

    assert_eq!(
        environment.alias("PlayerId"),
        Some(&Type::union([Type::string(), Type::number()]))
    );
    assert_eq!(
        environment.alias_documentation("Player"),
        Some("player object")
    );
    assert_eq!(environment.local("Player"), Some(&Type::alias("Player")));
    assert_eq!(
        environment.local_documentation("Score"),
        Some("global score")
    );

    let Type::Table(player) = environment.alias("Player").expect("Player should exist") else {
        panic!("Player should lower to a table shape");
    };

    assert_eq!(
        player.field("hp").map(|field| &field.ty),
        Some(&Type::number())
    );
    assert_eq!(player.field("name").map(|field| field.optional), Some(true));

    let make_player = environment
        .function("makePlayer")
        .expect("doc function should lower");

    assert_eq!(make_player.parameters[0].name, "name");
    assert_eq!(make_player.parameters[0].ty, Type::string());
    assert_eq!(
        make_player.parameters[0].documentation.as_deref(),
        Some("player name")
    );
    assert_eq!(make_player.return_type, Some(Type::alias("Player")));
    assert_eq!(
        make_player.documentation.as_deref(),
        Some("player name\ncreated player")
    );
}

#[test]
fn lowers_lua_doc_string_literal_union_fields_as_literals() {
    let parsed = parse_lua_source(
        "src/enemy.lua",
        r#"---@class Enemy
---@field hp number
---@field type 'friendly' | 'mad'
local Enemy = {}
"#,
    )
    .expect("Lua source with doc annotations should parse");
    let environment = build_type_environment(&parsed).expect("Lua doc annotations should lower");

    let Type::Table(enemy) = environment.alias("Enemy").expect("Enemy should exist") else {
        panic!("Enemy should lower to a table shape");
    };

    assert_eq!(
        enemy.field("type").map(|field| &field.ty),
        Some(&Type::union([
            Type::string_literal("friendly"),
            Type::string_literal("mad"),
        ]))
    );
}

#[test]
fn reports_conflicts_between_tua_and_lua_doc_annotations() {
    let parsed = parse_tua_source(
        "src/conflict.tua",
        r#"---@param value string documented type
local function test(value: number): number
  return value
end
"#,
    )
    .expect("source should parse");
    let diagnostics = build_type_environment(&parsed)
        .expect_err("conflicting doc and Tua annotations should be rejected");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::TYPE_DOC_CONFLICT));
    assert!(diagnostics[0]
        .message
        .contains("conflicting annotations for `test` parameter `value`"));
}

#[test]
fn aliases_resolve_within_the_file_type_context() {
    let parsed = parse_tua_source(
        "src/aliases.tua",
        r#"
type HitPoints = number
type Health = HitPoints

local hp: Health = 10
"#,
    )
    .expect("typed source should parse");
    let environment =
        build_type_environment(&parsed).expect("aliases should resolve through context");
    let context = environment.to_type_context();

    assert!(context.is_assignable(&Type::number(), environment.local("hp").unwrap()));
    assert!(context.is_assignable(&Type::alias("Health"), &Type::alias("HitPoints")));
}

#[test]
fn aliases_can_refer_to_later_aliases_in_the_same_file() {
    let parsed = parse_tua_source(
        "src/forward.tua",
        r#"
type Player = {
  pos: Vec2,
}

type Vec2 = {
  x: number,
  y: number,
}
"#,
    )
    .expect("typed source should parse");
    let environment =
        build_type_environment(&parsed).expect("forward aliases should be known per file");

    let Type::Table(player) = environment
        .alias("Player")
        .expect("Player alias should exist")
    else {
        panic!("Player should be a table shape");
    };

    assert_eq!(player.field("pos").unwrap().ty, Type::alias("Vec2"));
}

#[test]
fn unknown_type_names_produce_type_diagnostics() {
    let parsed = parse_tua_source(
        "src/player.tua",
        r#"
local player: Player = nil
"#,
    )
    .expect("typed source should parse");
    let diagnostics =
        build_type_environment(&parsed).expect_err("unknown types should produce diagnostics");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::TYPE_UNKNOWN_NAME));
    assert_eq!(
        diagnostics[0].code.unwrap().category(),
        crate::DiagnosticCategory::Type
    );
    assert!(diagnostics[0].message.contains("unknown type `Player`"));
    assert!(diagnostics[0].format_cli().contains("error TL1001"));
}

#[test]
fn unknown_type_names_inside_table_shapes_produce_type_diagnostics() {
    let parsed = parse_tua_source(
        "src/player.tua",
        r#"
type Player = {
  pos: Vec2,
}
"#,
    )
    .expect("typed source should parse");
    let diagnostics =
        build_type_environment(&parsed).expect_err("unknown field types should be reported");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::TYPE_UNKNOWN_NAME));
    assert!(diagnostics[0].message.contains("unknown type `Vec2`"));
}

#[test]
fn duplicate_type_aliases_produce_deterministic_diagnostics() {
    let parsed = parse_tua_source(
        "src/types.tua",
        r#"
type Vec2 = {
  x: number,
}

type Player = {
  name: string,
}

type Vec2 = {
  y: number,
}

type Player = {
  hp: number,
}
"#,
    )
    .expect("typed source should parse");
    let diagnostics =
        build_type_environment(&parsed).expect_err("duplicate aliases should be rejected");

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>(),
        vec![
            "duplicate type alias `Vec2`",
            "duplicate type alias `Player`"
        ]
    );
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_DUPLICATE_ALIAS)));
    assert!(diagnostics[0].format_cli().contains("error TL1002"));
}

fn vec2_type() -> Type {
    Type::table([
        ("x", TableField::required(Type::number())),
        ("y", TableField::required(Type::number())),
    ])
}

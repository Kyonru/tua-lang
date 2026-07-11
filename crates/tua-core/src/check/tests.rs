use super::*;
use crate::DiagnosticCode;
use std::fs;
use std::path::PathBuf;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn accepts_matching_annotated_local_initializers() {
    check_tua_source(
        "src/main.tua",
        r#"
local hp: number = 100
local name: string = "kyon"
local alive: boolean = true
local maybe_hp: number | nil = nil
"#,
    )
    .expect("matching primitive literals should type-check");
}

#[test]
fn accepts_string_concatenation_with_literal_operands() {
    check_tua_source(
        "src/concat.tua",
        r#"
type TextOrNumber = string | number

local function label(value: string): string
  return "value:" .. value
end

local function stringify(value: TextOrNumber): string
  return "" .. value
end
"#,
    )
    .expect("Lua string concatenation should infer string for literal operands");
}

#[test]
fn accepts_number_and_string_ordering_expressions() {
    check_tua_source(
        "src/comparison.tua",
        r#"
local hp: number = 10
local alive: boolean = hp > 0
local passing: boolean = hp >= 1
local sorted: boolean = "Ada" <= "Mina"
"#,
    )
    .expect("Lua ordering operators should accept number-number and string-string operands");
}

#[test]
fn accepts_typed_love_asset_loader_initializers() {
    check_tua_source_with_asset_roots_and_diagnostics_config(
        "src/assets.tua",
        r#"
local player: Image = image "assets/player.png"
local jump: Source = sound "assets/jump.wav"
local theme: Source = music "assets/theme.ogg"
local mainFont: Font = font("assets/ui.ttf", 18)
local wave: Shader = shader "assets/wave.glsl"
local data: ImageData = imageData "assets/player.png"
local target: Canvas = canvas(320, 180)
local frame: Quad = quad(0, 0, 16, 16, 64, 64)
local batch: SpriteBatch = spriteBatch(player, 128)
local sparks: ParticleSystem = particles(player, 64)
"#,
        &[],
        &crate::DiagnosticsConfig::default(),
    )
    .expect("LOVE asset loaders should produce their nominal asset types");
}

#[test]
fn accepts_builtin_vec2_constructor_initializers() {
    check_tua_source(
        "src/vector.tua",
        r#"
local position: Vec2 = vec2(100, 200)
local x: number = position.x
local y: number = position.y
"#,
    )
    .expect("built-in Vec2 constructor should produce Vec2");
}

#[test]
fn accepts_builtin_vec2_brinevector_operations() {
    check_tua_source(
        "src/vector.tua",
        r#"
local position: Vec2 = vec2(100, 200)
local velocity = vec2(12, -4)
local moved: Vec2 = position + velocity
local scaled: Vec2 = velocity * 2
local speed: number = velocity.length
local direction: Vec2 = velocity.normalized
local distanceSquared: number = position:dot(velocity)
"#,
    )
    .expect("built-in Vec2 should model common BrineVector operations");
}

#[test]
fn rejects_builtin_vec2_constructor_argument_mismatch() {
    let diagnostics = check_tua_source(
        "src/vector.tua",
        r#"
local position = vec2("100", 200)
"#,
    )
    .expect_err("invalid Vec2 constructor argument should be reported");

    assert_single_mismatch(&diagnostics, "expected number, found string");
}

#[test]
fn rejects_love_asset_loader_initializer_mismatch() {
    let diagnostics = check_tua_source_with_asset_roots_and_diagnostics_config(
        "src/assets.tua",
        r#"
local player: Source = image "assets/player.png"
"#,
        &[],
        &crate::DiagnosticsConfig::default(),
    )
    .expect_err("asset loader type mismatch should be reported");

    assert_single_mismatch(&diagnostics, "expected Source, found Image");
}

#[test]
fn accepts_existing_love_asset_loader_paths() {
    let temp = TempProject::new("existing-assets");
    let source = r#"
local player = image "assets/player.png"
local jump = sound "assets/jump.wav"
local theme = music "assets/theme.ogg"
local mainFont = font("assets/ui.ttf", 18)
local wave = shader "assets/wave.glsl"
local data = imageData "assets/player.png"
"#;

    temp.write("src/assets.tua", source);
    temp.write("assets/player.png", "");
    temp.write("assets/jump.wav", "");
    temp.write("assets/theme.ogg", "");
    temp.write("assets/ui.ttf", "");
    temp.write("assets/wave.glsl", "");

    check_tua_source(temp.path.join("src/assets.tua"), source)
        .expect("existing supported assets should check");
}

#[test]
fn rejects_missing_love_asset_loader_paths() {
    let temp = TempProject::new("missing-assets");
    let source = r#"
local player = image "assets/missing.png"
"#;

    temp.write("src/assets.tua", source);

    let diagnostics = check_tua_source(temp.path.join("src/assets.tua"), source)
        .expect_err("missing asset should be reported");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::ASSET_MISSING));
    assert!(diagnostics[0]
        .message
        .contains("image asset not found: assets/missing.png"));
    assert!(diagnostics[0].format_cli().contains("error TL2004"));
}

#[test]
fn rejects_unsupported_love_asset_loader_extensions() {
    let temp = TempProject::new("unsupported-assets");
    let source = r#"
local player = image "assets/player.txt"
"#;

    temp.write("src/assets.tua", source);
    temp.write("assets/player.txt", "");

    let diagnostics = check_tua_source(temp.path.join("src/assets.tua"), source)
        .expect_err("unsupported extension should be reported");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::ASSET_UNSUPPORTED_EXTENSION)
    );
    assert!(diagnostics[0]
        .message
        .contains("unsupported image asset extension"));
    assert!(diagnostics[0].format_cli().contains("error TL2005"));
}

#[test]
fn accepts_love_asset_loader_paths_from_configured_asset_roots() {
    let temp = TempProject::new("configured-asset-roots");
    let source = r#"
local player = image "assets/player.png"
"#;

    temp.write("src/assets.tua", source);
    temp.write("game/assets/player.png", "");

    check_tua_source_with_asset_roots_and_diagnostics_config(
        temp.path.join("src/assets.tua"),
        source,
        &[temp.path.join("game")],
        &crate::DiagnosticsConfig::default(),
    )
    .expect("configured asset roots should be searched");
}

#[test]
fn rejects_primitive_annotated_local_initializer_mismatch() {
    let diagnostics = check_tua_source(
        "src/main.tua",
        r#"
local hp: number = "full"
"#,
    )
    .expect_err("primitive mismatch should be reported");

    assert_single_mismatch(&diagnostics, "expected number, found string");
}

#[test]
fn accepts_assignments_to_known_typed_locals_and_fields() {
    check_tua_source(
        "src/player.tua",
        r#"
type Player = {
  hp: number,
  alive: boolean,
}

local hp: number = 10
hp = 11

local player: Player = { hp = 10, alive = true }
player.hp = 9
player.alive = false
"#,
    )
    .expect("known typed locals and table fields should accept matching assignments");
}

#[test]
fn rejects_assignment_to_known_typed_local_mismatch() {
    let diagnostics = check_tua_source(
        "src/main.tua",
        r#"
local hp: number = 10
hp = "low"
"#,
    )
    .expect_err("typed local assignment mismatch should be reported");

    assert_single_mismatch(&diagnostics, "expected number, found string");
}

#[test]
fn rejects_assignment_to_known_table_field_mismatch() {
    let diagnostics = check_tua_source(
        "src/player.tua",
        r#"
type Player = {
  hp: number,
  alive: boolean,
}

local player: Player = { hp = 10, alive = true }
player.hp = "low"
"#,
    )
    .expect_err("typed table field assignment mismatch should be reported");

    assert_single_mismatch(&diagnostics, "expected number, found string");
}

#[test]
fn rejects_love_conf_config_field_assignment_mismatch() {
    let diagnostics = check_tua_source(
        "src/conf.tua",
        r#"
function love.conf(t)
  t.window.title = "Tua LOVE minimal"
  t.window.width = 320
  t.window.msaa = "12"
end
"#,
    )
    .expect_err("love.conf config field assignment mismatch should be reported");

    assert_single_mismatch(&diagnostics, "expected number, found string");
}

#[test]
fn rejects_table_field_mismatch_in_annotated_local_initializer() {
    let diagnostics = check_tua_source(
        "src/player.tua",
        r#"
type Player = {
  hp: number,
}

local player: Player = { hp = "low" }
"#,
    )
    .expect_err("typed table initializer mismatch should be reported");

    assert_single_mismatch(&diagnostics, "expected Player, found { hp: string }");
}

#[test]
fn unannotated_dynamic_code_remains_permissive() {
    check_tua_source(
        "src/dynamic.tua",
        r#"
local hp = 10
hp = "low"

local player = make_player()
player.hp = "low"

type Player = {
  hp: number,
}

local typed_player: Player = make_player()
typed_player.hp = unknown_value()
"#,
    )
    .expect("unannotated and unknown-expression code should stay permissive");
}

#[test]
fn accepts_matching_function_call_arguments_and_returns() {
    check_tua_source(
        "src/combat.tua",
        r#"
local function add(left: number, right: number): number
  return 1
end

local total: number = add(1, 2)
"#,
    )
    .expect("matching function boundaries should type-check");
}

#[test]
fn accepts_omitted_trailing_default_parameters() {
    check_tua_source(
        "src/shake.tua",
        r#"
local function shake(duration: number = 0.3): number
  return duration
end

local defaulted: number = shake()
local explicit: number = shake(1)
"#,
    )
    .expect("omitted default parameters should type-check");
}

#[test]
fn accepts_love_method_multi_return_from_inferred_function_body() {
    check_tua_source_with_asset_roots_and_diagnostics_config(
        "src/assets.tua",
        r#"
local function loadPlayerImage()
  return image "assets/player.png"
end

local player = loadPlayerImage()

local function azul()
  return player:getDimensions()
end

local c = azul()
local z: number = c + 1
local literal: number = 1 + 3
local label: string = "width: " .. z
"#,
        &[],
        &crate::DiagnosticsConfig::default(),
    )
    .expect("Love method multi-return should satisfy inferred function return checks");
}

#[test]
fn accepts_required_module_function_returns_in_checked_function_bodies() {
    let temp = TempProject::new("required-module-check-return");
    let source = r#"local Functions = require("functions")

local function macarrons()
  return Functions.makePlayer("lua interop")
end

local function make_function()
  return Functions.makePlayer
end

macarrons()
make_function()
"#;
    let functions = r#"type Player = {
  name: string,
  hp: number,
}

local function makePlayer(name: string): Player
  return {
    name = name,
    hp = 100,
  }
end

return {
  makePlayer = makePlayer,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/functions.tua", functions);

    check_tua_source(temp.path.join("src/main.tua"), source)
        .expect("required module function returns should satisfy checker return validation");
}

#[test]
fn accepts_love_method_enum_string_arguments() {
    check_tua_source_with_asset_roots_and_diagnostics_config(
        "src/assets.tua",
        r#"
local player = image "assets/player.png"

player:setFilter("nearest")
player:setFilter("nearest", "linear", 4)
"#,
        &[],
        &crate::DiagnosticsConfig::default(),
    )
    .expect("Love enum-like parameters should accept string values");
}

#[test]
fn rejects_invalid_love_method_enum_string_arguments() {
    let diagnostics = check_tua_source_with_asset_roots_and_diagnostics_config(
        "src/assets.tua",
        r#"
local player = image "assets/player.png"

player:setFilter("blurry")
"#,
        &[],
        &crate::DiagnosticsConfig::default(),
    )
    .expect_err("invalid Love enum-like method parameter should be reported");

    assert_single_mismatch(&diagnostics, r#"expected FilterMode, found "blurry""#);
}

#[test]
fn rejects_invalid_generated_love_api_enum_string_arguments() {
    let diagnostics = check_tua_source(
        "src/draw.tua",
        r#"
function love.update(dt)
  love.graphics.circle("fillter", 100, 100, 2)
end
"#,
    )
    .expect_err("invalid generated Love enum-like parameter should be reported");

    assert_single_mismatch(&diagnostics, r#"expected DrawMode, found "fillter""#);
}

#[test]
fn accepts_generated_love_api_type_annotations() {
    check_tua_source(
        "src/love_types.tua",
        r#"
local file: File = love.filesystem.newFile("save.txt")
local channel: Channel = love.thread.newChannel()

file:open("r")
channel:push("ready")
"#,
    )
    .expect("generated LOVE API type names and methods should be available");
}

#[test]
fn checks_lua_doc_literal_union_arguments() {
    check_tua_source(
        "src/mode.tua",
        r#"
---@param mode "static" | "stream"
local function load(mode)
end

load("static")
"#,
    )
    .expect("matching literal union argument should be accepted");

    let diagnostics = check_tua_source(
        "src/mode.tua",
        r#"
---@param mode "static" | "stream"
local function load(mode)
end

load("dynamic")
"#,
    )
    .expect_err("non-member literal union argument should be rejected");

    assert_single_mismatch(
        &diagnostics,
        r#"expected "static" | "stream", found "dynamic""#,
    );
}

#[test]
fn checks_lua_doc_overload_arguments() {
    check_tua_source(
        "src/load.tua",
        r#"
---@overload fun(path: string): Image
---@overload fun(path: string, mode: "stream"): Source
local function load(path, mode)
  return nil
end

load("assets/player.png")
load("assets/jump.wav", "stream")
"#,
    )
    .expect("matching overload calls should be accepted");

    let diagnostics = check_tua_source(
        "src/load.tua",
        r#"
---@overload fun(path: string): Image
---@overload fun(path: string, mode: "stream"): Source
local function load(path, mode)
  return nil
end

load("assets/jump.wav", "static")
"#,
    )
    .expect_err("non-matching overload call should be rejected");

    assert_single_mismatch(&diagnostics, r#"expected "stream", found "static""#);
}

#[test]
fn checks_lua_doc_typed_varargs() {
    check_tua_source(
        "src/log.tua",
        r#"
---@param message string
---@vararg identifier
local function log(message, ...)
end

log("hello", "one", "two")
"#,
    )
    .expect("string varargs should be accepted");

    let diagnostics = check_tua_source(
        "src/log.tua",
        r#"
---@param message string
---@vararg identifier
local function log(message, ...)
end

log("hello", 10)
"#,
    )
    .expect_err("non-string vararg should be rejected");

    assert_single_mismatch(&diagnostics, "expected string, found number");
}

#[test]
fn checks_tua_literal_union_and_enum_arguments() {
    let diagnostics = check_tua_source(
        "src/mode.tua",
        r#"
type LoadMode = "static" | "stream"

enum FilterMode
  "linear"
  "nearest"
end

local function load(mode: LoadMode)
end

local function setFilter(mode: FilterMode)
end

load("static")
setFilter("nearest")
load("dynamic")
"#,
    )
    .expect_err("non-member literal union value should be rejected");

    assert_single_mismatch(
        &diagnostics,
        r#"expected "static" | "stream", found "dynamic""#,
    );
}

#[test]
fn warns_for_love_method_dot_calls() {
    let diagnostics = check_tua_source_with_asset_roots_and_diagnostics_config(
        "src/assets.tua",
        r#"
local player = image "assets/player.png"

player.setFilter("nearest")
"#,
        &[],
        &crate::DiagnosticsConfig::default(),
    )
    .expect_err("dot method call should be reported");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].severity, crate::DiagnosticSeverity::Warning);
    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::LINT_DOT_METHOD_CALL)
    );
    assert!(diagnostics[0]
        .message
        .contains("use `player:setFilter(...)`"));
    assert!(diagnostics[0].format_cli().contains("warning TL4018"));
}

#[test]
fn warns_for_dot_calls_to_every_generated_love_object_method() {
    for (type_name, methods) in love_object_methods_by_type() {
        let source = love_dot_method_source(type_name, &methods);
        let diagnostics = check_tua_source_with_asset_roots_and_diagnostics_config(
            "src/assets.tua",
            source,
            &[],
            &crate::DiagnosticsConfig::default(),
        )
        .unwrap_err();

        for method in methods {
            let expected = format!("use `receiver:{method}(...)`");

            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic.code
                    == Some(DiagnosticCode::LINT_DOT_METHOD_CALL)
                    && diagnostic.message.contains(&expected)),
                "{type_name}.{method} should warn with colon guidance; got {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn inline_ignores_are_disabled_by_default() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
-- tua-ignore-next-line TL1003
local hp: number = "full"
"#,
        &crate::DiagnosticsConfig::default(),
    )
    .expect_err("disabled inline ignore should not suppress diagnostics");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::TYPE_MISMATCH));
}

#[test]
fn inline_ignores_suppress_matching_type_diagnostics() {
    let mut config = crate::DiagnosticsConfig {
        inline_ignores: true,
        ..crate::DiagnosticsConfig::default()
    };

    check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
-- tua-ignore-next-line TL1003
local hp: number = "full"
"#,
        &config,
    )
    .expect("enabled inline ignore should suppress matching diagnostic");

    config
        .inline_ignore_scopes
        .push("tua-ignore-file".to_owned());

    check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
-- tua-ignore-file TL1003
local hp: number = "full"
"#,
        &config,
    )
    .expect("configured file-scope inline ignore should suppress matching diagnostic");
}

#[test]
fn inline_ignores_report_unused_and_invalid_comments() {
    let config = crate::DiagnosticsConfig {
        inline_ignores: true,
        ..crate::DiagnosticsConfig::default()
    };
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
-- tua-ignore-next-line TL1003
local ok: number = 1

-- tua-ignore-next-line TL9999
local hp: number = "full"
"#,
        &config,
    )
    .expect_err("unused and invalid inline ignores should be reported");

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_INLINE_IGNORE_UNUSED)));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_INLINE_IGNORE_INVALID)));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)));
}

#[test]
fn accepts_table_field_dot_and_colon_method_call_equivalence() {
    check_tua_source(
        "src/methods.tua",
        r#"
local lookupObject = {
  count = 0,
  y = function(self, amount)
    self.count = self.count + amount
    return self.count
  end,
}

lookupObject.y(lookupObject, 1)
lookupObject:y(2)
"#,
    )
    .expect("colon calls should provide self for known function-valued fields");
}

#[test]
fn accepts_array_index_and_loop_variable_inference() {
    check_tua_source(
        "src/loops.tua",
        r#"
local numbers = { 1, 2, 3 }
local first: number = numbers[1]

for i = 1, #numbers do
  local indexed: number = numbers[i]
end

for _, value in ipairs(numbers) do
  local doubled: number = value + 1
end
"#,
    )
    .expect("array indexes and numeric/ipairs loop variables should infer number");
}

#[test]
fn accepts_pairs_loop_variable_inference_for_record_tables() {
    check_tua_source(
        "src/pairs.tua",
        r#"
local player = { name = "Ada", score = 10 }
local score: number = player["score"]

for key, value in pairs(player) do
  local known_key: "name" | "score" = key
  local known_value: string | number = value
end
"#,
    )
    .expect("pairs over record-like tables should infer key and value unions");
}

#[test]
fn rejects_mismatched_ipairs_loop_value_type() {
    let diagnostics = check_tua_source(
        "src/loops.tua",
        r#"
local words = { "a", "b" }

for _, value in ipairs(words) do
  local bad: number = value
end
"#,
    )
    .expect_err("ipairs value should keep the array element type");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::TYPE_MISMATCH));
}

#[test]
fn accepts_static_metatable_operator_and_index_patterns() {
    check_tua_source(
        "src/vector.tua",
        r#"
type Vector = {
  x: number,
  y: number,
  length: number,
}

local vectorA: Vector = { x = 10, y = 20, length = 0 }
local vectorB: Vector = { x = 5, y = 15, length = 0 }
local vectorMetatable = {
  __add = function(left, right)
    return {
      x = left.x + right.x,
      y = left.y + right.y,
      length = 0,
    }
  end,
  __tostring = function(value)
    return "(" .. value.x .. ", " .. value.y .. ")"
  end,
  __index = function(_, key)
    if key == "length" then
      return 0
    end

    return nil
  end,
  __newindex = function(tableValue, key, value)
    rawset(tableValue, key, value)
  end,
}

setmetatable(vectorA, vectorMetatable)
setmetatable(vectorB, vectorMetatable)

local vectorSum: Vector = vectorA + vectorB
local vectorText: string = tostring(vectorSum)
local vectorMissingLength: number = vectorA.length
vectorA.label = "uses __newindex"
"#,
    )
    .expect("static metatable patterns should feed conservative type inference");
}

#[test]
fn accepts_static_metamethod_operator_and_call_patterns() {
    check_tua_source(
        "src/vector.tua",
        r#"
type Vector = {
  x: number,
  y: number,
  length: number,
}

local function vectorBinary(left: Vector, right: Vector): Vector
  return {
    x = left.x + right.x,
    y = left.y + right.y,
    length = 0,
  }
end

local function vectorCompare(left: Vector, right: Vector): boolean
  return true
end

local function vectorLen(value: Vector): number
  return value.length
end

local function vectorCall(value: Vector, scale: number): Vector
  return {
    x = value.x * scale,
    y = value.y * scale,
    length = value.length,
  }
end

local vectorMetatable = {
  __add = vectorBinary,
  __sub = vectorBinary,
  __mul = vectorBinary,
  __div = vectorBinary,
  __eq = vectorCompare,
  __lt = vectorCompare,
  __le = vectorCompare,
  __len = vectorLen,
  __call = vectorCall,
}

local vectorA = { x = 10, y = 20, length = 30 }
local vectorB = { x = 5, y = 15, length = 20 }
setmetatable(vectorA, vectorMetatable)
setmetatable(vectorB, vectorMetatable)

local added: Vector = vectorA + vectorB
local subtracted: Vector = vectorA - vectorB
local multiplied: Vector = vectorA * vectorB
local divided: Vector = vectorA / vectorB
local equal: boolean = vectorA == vectorB
local different: boolean = vectorA ~= vectorB
local less: boolean = vectorA < vectorB
local lessEqual: boolean = vectorA <= vectorB
local greater: boolean = vectorA > vectorB
local greaterEqual: boolean = vectorA >= vectorB
local length: number = #vectorA
local scaled: Vector = vectorA(2)
"#,
    )
    .expect("static metamethods should infer operator and call results");
}

#[test]
fn rejects_static_call_metamethod_argument_mismatch() {
    let diagnostics = check_tua_source(
        "src/vector.tua",
        r#"
type Vector = {
  x: number,
  y: number,
}

local function vectorCall(value: Vector, scale: number): Vector
  return {
    x = value.x * scale,
    y = value.y * scale,
  }
end

local vectorMetatable = {
  __call = vectorCall,
}

local vector = { x = 10, y = 20 }
setmetatable(vector, vectorMetatable)

local scaled = vector("large")
"#,
    )
    .expect_err("call metamethod should check visible function arguments");

    assert_single_mismatch(&diagnostics, "expected number, found string");
}

#[test]
fn rejects_static_newindex_function_value_mismatch_for_literal_key() {
    let diagnostics = check_tua_source(
        "src/newindex.tua",
        r#"
local meta = {
  __newindex = function(t, key: "label", value: string)
  end,
}

local obj = {}
setmetatable(obj, meta)

obj.label = 123
"#,
    )
    .expect_err("visible __newindex function should check matching literal key assignments");

    assert_single_mismatch(&diagnostics, "expected string, found number");
}

#[test]
fn accepts_static_newindex_function_matching_value_type() {
    check_tua_source(
        "src/newindex.tua",
        r#"
local meta = {
  __newindex = function(t, key: "label", value: string)
  end,
}

local obj = {}
setmetatable(obj, meta)

obj.label = "ready"
"#,
    )
    .expect("visible __newindex function should accept matching literal key assignments");
}

#[test]
fn rejects_static_newindex_table_value_mismatch() {
    let diagnostics = check_tua_source(
        "src/newindex.tua",
        r#"
local meta = {
  __newindex = {
    label = string,
  },
}

local obj = {}
setmetatable(obj, meta)

obj.label = 123
"#,
    )
    .expect_err("visible __newindex table should check matching field assignments");

    assert_single_mismatch(&diagnostics, "expected string, found number");
}

#[test]
fn accepts_static_newindex_table_matching_value_type() {
    check_tua_source(
        "src/newindex.tua",
        r#"
local meta = {
  __newindex = {
    label = string,
  },
}

local obj = {}
setmetatable(obj, meta)

obj.label = "ready"
"#,
    )
    .expect("visible __newindex table should accept matching field assignments");
}

#[test]
fn unknown_newindex_keys_remain_permissive() {
    check_tua_source(
        "src/newindex.tua",
        r#"
local meta = {
  __newindex = function(t, key: "label", value: string)
  end,
}

local obj = {}
setmetatable(obj, meta)

obj.count = 123
"#,
    )
    .expect("unknown __newindex keys should remain gradual");
}

#[test]
fn accepts_self_index_class_style_metatable_methods() {
    check_tua_source(
        "src/account.tua",
        r#"
local Account = {}
Account.__index = Account

function Account:withdraw(amount)
end

local account = setmetatable({}, Account)
account:withdraw(10)
"#,
    )
    .expect("self-index class-style metatables should expose methods on instances");
}

#[test]
fn accepts_self_index_constructor_returning_setmetatable_self() {
    check_tua_source(
        "src/account.tua",
        r#"
local Account = {}
Account.__index = Account

function Account:new(balance)
  local instance = { balance = balance }
  return setmetatable(instance, self)
end

function Account:withdraw(amount)
  self.balance = self.balance - amount
end

local account = Account:new(100)
account:withdraw(10)
"#,
    )
    .expect("constructor methods returning setmetatable(instance, self) should infer instances");
}

#[test]
fn accepts_metatable_index_chain_inherited_methods() {
    check_tua_source(
        "src/inheritance.tua",
        r#"
local Base = {}
Base.__index = Base

function Base:describe(): string
  return "base"
end

local Mid = {}
Mid.__index = Mid
setmetatable(Mid, { __index = Base })

function Mid:level(): number
  return 2
end

local Leaf = {}
Leaf.__index = Leaf
setmetatable(Leaf, { __index = Mid })

local leaf = setmetatable({ name = "sprout" }, Leaf)
local description: string = leaf:describe()
local descriptionTwo: string = leaf.describe(leaf)
local level: number = leaf:level()
"#,
    )
    .expect("prototype-style metatable chains should expose inherited methods");
}

#[test]
fn accepts_callable_tables_with_static_call_metamethods() {
    check_tua_source(
        "src/callable.tua",
        r#"
type Product = {
  name: string,
}

local function makeProduct(factory, name: string): Product
  return { name = name }
end

local ConstructorLike = {}
setmetatable(ConstructorLike, {
  __call = makeProduct,
})

local product: Product = ConstructorLike("widget")
"#,
    )
    .expect("tables with visible __call metamethods should be callable");
}

#[test]
fn accepts_typed_pairs_and_ipairs_metamethod_loop_variables() {
    check_tua_source(
        "src/iterators.tua",
        r#"
---@param value any
---@return ["name" | "score", number]
local function iterPairs(value)
end

---@param value any
---@return [number, string]
local function iterIpairs(value)
end

local typedMap = {}
setmetatable(typedMap, {
  __pairs = iterPairs,
})

local typedArray = {}
setmetatable(typedArray, {
  __ipairs = iterIpairs,
})

for key, value in pairs(typedMap) do
  local checkedKey: "name" | "score" = key
  local checkedPairValue: number = value
end

for index, value in ipairs(typedArray) do
  local checkedIndex: number = index
  local checkedArrayValue: string = value
end
"#,
    )
    .expect("typed iterator metamethods should feed generic for loop variables");
}

#[test]
fn checks_typed_colon_method_arguments_and_self_fields() {
    check_tua_source(
        "src/account.tua",
        r#"
type Account = {
  balance: number,
}

local Account = {}
Account.__index = Account

function Account:withdraw(amount: number): number
  self.balance -= amount
  return self.balance
end

local account = setmetatable({ balance = 100 }, Account)
account:withdraw(10)
"#,
    )
    .expect("typed colon methods should check self fields and valid arguments");
}

#[test]
fn rejects_typed_colon_method_argument_mismatch() {
    let diagnostics = check_tua_source(
        "src/account.tua",
        r#"
type Account = {
  balance: number,
}

local Account = {}
Account.__index = Account

function Account:withdraw(amount: number): number
  self.balance -= amount
  return self.balance
end

local account = setmetatable({ balance = 100 }, Account)
account:withdraw("ten")
"#,
    )
    .expect_err("typed colon methods should check explicit arguments after self");

    assert_single_mismatch(&diagnostics, "expected number, found string");
}

#[test]
fn rejects_love_method_argument_count_mismatch() {
    let diagnostics = check_tua_source_with_asset_roots_and_diagnostics_config(
        "src/assets.tua",
        r#"
local player = image "assets/player.png"

player:setFilter()
"#,
        &[],
        &crate::DiagnosticsConfig::default(),
    )
    .expect_err("Love method argument count mismatch should be reported");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::TYPE_ARGUMENT_COUNT)
    );
    assert!(diagnostics[0]
        .message
        .contains("argument count mismatch: expected 1, found 0"));
}

#[test]
fn rejects_love_method_argument_type_mismatch() {
    let diagnostics = check_tua_source_with_asset_roots_and_diagnostics_config(
        "src/assets.tua",
        r#"
local player = image "assets/player.png"

player:setFilter(123)
"#,
        &[],
        &crate::DiagnosticsConfig::default(),
    )
    .expect_err("Love method argument type mismatch should be reported");

    assert_single_mismatch(&diagnostics, "expected FilterMode, found number");
}

#[test]
fn rejects_function_call_argument_count_mismatch() {
    let diagnostics = check_tua_source(
        "src/combat.tua",
        r#"
local function add(left: number, right: number): number
  return 1
end

add(1)
"#,
    )
    .expect_err("argument count mismatch should be reported");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::TYPE_ARGUMENT_COUNT)
    );
    assert!(diagnostics[0]
        .message
        .contains("argument count mismatch: expected 2, found 1"));
    assert!(diagnostics[0].format_cli().contains("error TL1004"));
}

#[test]
fn rejects_function_call_argument_type_mismatch() {
    let diagnostics = check_tua_source(
        "src/combat.tua",
        r#"
local function add(left: number, right: number): number
  return 1
end

local total: number = add("one", 2)
"#,
    )
    .expect_err("argument type mismatch should be reported");

    assert_single_mismatch(&diagnostics, "expected number, found string");
}

#[test]
fn rejects_return_type_mismatch() {
    let diagnostics = check_tua_source(
        "src/combat.tua",
        r#"
local function hp(): number
  return "full"
end
"#,
    )
    .expect_err("return type mismatch should be reported");

    assert_single_mismatch(&diagnostics, "expected number, found string");
}

#[test]
fn calls_through_any_remain_permissive() {
    check_tua_source(
        "src/dynamic.tua",
        r#"
local dynamic_call: any = get_dynamic_call()
local hp: number = dynamic_call("one", false, {})
"#,
    )
    .expect("calls through any should remain permissive");
}

#[test]
fn warns_for_impossible_equality_comparisons() {
    let diagnostics = check_tua_source(
        "src/combat.tua",
        r#"
local function recursive(a: string)
  if a == 1 then
    return 1
  end

  if a ~= 2 then
    return "value"
  end
end
"#,
    )
    .expect_err("impossible comparisons should warn");

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::TYPE_IMPOSSIBLE_COMPARISON)
            && diagnostic.severity == crate::DiagnosticSeverity::Warning
    }));
    assert!(diagnostics[0]
        .message
        .contains("comparison is always false: string and number do not overlap"));
    assert!(diagnostics[1]
        .message
        .contains("comparison is always true: string and number do not overlap"));
    assert!(diagnostics[0].format_cli().contains("warning TL1005"));
}

#[test]
fn dynamic_equality_comparisons_remain_permissive() {
    check_tua_source(
        "src/dynamic.tua",
        r#"
local value: any = get_value()

if value == 1 then
  print(value)
end
"#,
    )
    .expect("comparisons through any should stay permissive");
}

#[test]
fn accepts_flow_sensitive_narrowing_inside_simple_guards() {
    check_tua_source(
        "src/narrowing.tua",
        r#"
local function from_optional(value: number | nil): number
  if value ~= nil then
    return value
  end

  return 0
end

local function nil_branch(value: number | nil): nil
  if value == nil then
    return value
  end

  return nil
end

local function from_type_guard(value: string | number): string
  if type(value) == "string" then
    return value
  end

  return ""
end
"#,
    )
    .expect("simple guard narrowing should type-check");
}

#[test]
fn mutation_inside_guard_clears_narrowed_assignment_target() {
    let diagnostics = check_tua_source(
        "src/narrowing_mutation.tua",
        r#"
local function mutated(value: number | nil): number
  if value ~= nil then
    value = nil
    return value
  end

  return 0
end
"#,
    )
    .expect_err("mutation should clear the non-nil narrowing");

    assert_single_mismatch(&diagnostics, "expected number, found nil");
}

fn assert_single_mismatch(diagnostics: &[Diagnostic], message: &str) {
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::TYPE_MISMATCH));
    assert_eq!(
        diagnostics[0].code.unwrap().category(),
        crate::DiagnosticCategory::Type
    );
    assert!(diagnostics[0].message.contains(message));
    assert!(diagnostics[0].format_cli().contains("error TL1003"));
}

fn love_object_methods_by_type() -> std::collections::BTreeMap<&'static str, Vec<&'static str>> {
    let mut cases: std::collections::BTreeMap<&'static str, Vec<&'static str>> =
        std::collections::BTreeMap::new();

    for case in crate::love_api_signatures::object_method_cases() {
        let methods = cases.entry(case.type_name).or_default();

        if !methods.contains(&case.method_name) {
            methods.push(case.method_name);
        }
    }

    cases
}

fn love_dot_method_source(type_name: &str, methods: &[&str]) -> String {
    let mut source = format!("local receiver: {type_name} | nil = nil\n");

    for method in methods {
        source.push_str(&format!("receiver.{method}()\n"));
    }

    source
}

struct TempProject {
    path: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("tua-core-check-{name}-{}-{unique}", process::id()));

        fs::create_dir_all(&path).expect("temp project should be created");

        Self { path }
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.path.join(relative);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent directory should be created");
        }

        fs::write(path, contents).expect("fixture should be written");
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

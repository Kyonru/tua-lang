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
fn accepts_builtin_signal_bus_methods_and_payloads() {
    check_tua_source(
        "src/signals.tua",
        r#"
local events: SignalBus = signal()
local handler = events:on("player.hit", function(player, damage)
  print(player, damage)
end)

events:once("ready", function()
  print("ready")
end)

events:declare("game.ready")
events:emit("player.hit", { hp = 10 }, 4, true)
events:off("player.hit", handler)
events:off("player.hit")
events:clear("player.hit")
events:clear()

local hasHit: boolean = events:has("player.hit")
local currentListeners: any = events:listeners("player.hit")
"#,
    )
    .expect("built-in signal bus should accept common event operations");
}

#[test]
fn accepts_typed_builtin_object_pool_lifecycle() {
    check_tua_source(
        "src/pool.tua",
        r#"
type Bullet = { x: number, y: number, active: boolean }

local bullets = objectPool({
  create = function(): Bullet
    return { x = 0, y = 0, active = false }
  end,
  reset = function(bullet: Bullet)
    bullet.active = false
  end,
  initial = 32,
})

local bullet: Bullet = bullets:acquire()
bullet.active = true
local released: boolean = bullets:release(bullet)
local warmed: number = bullets:prewarm(64)
local active: number = bullets:activeCount()
local available: number = bullets:availableCount()
local capacity: number = bullets:capacity()
local owned: boolean = bullets:owns(bullet)
bullets:clear()
"#,
    )
    .expect("typed object pools should preserve their factory item type");
}

#[test]
fn rejects_releasing_the_wrong_type_to_a_typed_object_pool() {
    let diagnostics = check_tua_source(
        "src/pool.tua",
        r#"
type Bullet = { active: boolean }
local bullets = objectPool({
  create = function(): Bullet
    return { active = false }
  end,
})

bullets:release("not a bullet")
"#,
    )
    .expect_err("typed object pools should reject an incompatible release");

    assert_single_mismatch(&diagnostics, "expected Bullet, found string");
}

#[test]
fn rejects_object_pool_options_without_a_factory() {
    let diagnostics = check_tua_source(
        "src/pool.tua",
        r#"local bullets = objectPool({ initial = 16 })
"#,
    )
    .expect_err("objectPool should require a create callback");

    assert!(diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.message.contains("expected ObjectPoolOptions") }));
}

#[test]
fn preserves_an_explicit_object_pool_function_binding() {
    check_tua_source(
        "src/custom_pool.tua",
        r#"local function objectPool(options: any): string
  return "custom"
end

local result: string = objectPool({})
"#,
    )
    .expect("an explicit objectPool function should override the built-in helper");
}

#[test]
fn checks_signal_emit_payloads_from_typed_listener_callbacks() {
    check_tua_source(
        "src/signals.tua",
        r#"
type Player = {
  name: string,
  hp: number,
}

local events = signal()
local player: Player = { name = "Ada", hp = 10 }

events:on("player.hit", function(player: Player, damage: number)
  player.hp -= damage
end)

events:emit("player.hit", player, 12)
"#,
    )
    .expect("matching SignalBus payload should type-check");

    let diagnostics = check_tua_source(
        "src/signals.tua",
        r#"
type Player = {
  name: string,
  hp: number,
}

local events = signal()

events:on("player.hit", function(player: Player, damage: number)
  player.hp -= damage
end)
events:on("player.hit", function(target: Player, amount: number)
  print(target.name, amount)
end)

events:emit("player.hit", "Ada", 12)
"#,
    )
    .expect_err("mismatched SignalBus payload should be reported");

    assert_single_mismatch(&diagnostics, "expected Player, found string");
}

#[test]
fn checks_signal_emit_payloads_from_declare_schema() {
    let diagnostics = check_tua_source(
        "src/signals.tua",
        r#"
type Player = {
  name: string,
  hp: number,
}

local events = signal()
local player: Player = { name = "Ada", hp = 10 }

events:declare("player.hit", function(player: Player, damage: number) end)
events:emit("player.hit", player, "hard")
"#,
    )
    .expect_err("declare schema should validate emit payloads");

    assert_single_mismatch(&diagnostics, "expected number, found string");
}

#[test]
fn leaves_untyped_signal_listener_payloads_permissive() {
    check_tua_source(
        "src/signals.tua",
        r#"
local events = signal()

events:on("player.hit", function(player, damage)
  print(player, damage)
end)

events:emit("player.hit", "Ada", "hard", true)
"#,
    )
    .expect("untyped SignalBus listeners should not create a payload schema");
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
fn rejects_wrong_love_asset_loader_source_types() {
    let temp = TempProject::new("wrong-asset-kind");
    let source = r#"
local player = image "assets/jump.wav"
"#;

    temp.write("src/assets.tua", source);
    temp.write("assets/jump.wav", "");

    let diagnostics = check_tua_source(temp.path.join("src/assets.tua"), source)
        .expect_err("wrong asset source type should be reported");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::ASSET_WRONG_KIND));
    assert!(diagnostics[0]
        .message
        .contains("wrong image asset type for `assets/jump.wav`; found audio asset"));
    assert!(diagnostics[0].format_cli().contains("error TL2006"));
}

#[test]
fn validates_native_love_asset_constructor_paths() {
    let temp = TempProject::new("native-asset-constructors");
    let source = r#"
local missing = love.graphics.newImage("assets/missing.png")
local unsupported = love.sound.newSoundData("assets/readme.txt")
local wrong = love.graphics.newImage("assets/jump.wav")
local stream = love.audio.newSource("assets/theme.ogg", "stream")
"#;

    temp.write("src/assets.tua", source);
    temp.write("assets/readme.txt", "");
    temp.write("assets/jump.wav", "");
    temp.write("assets/theme.ogg", "");

    let diagnostics = check_tua_source(temp.path.join("src/assets.tua"), source)
        .expect_err("native LOVE asset constructor diagnostics should be reported");
    let codes = diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();

    assert_eq!(
        codes,
        vec![
            DiagnosticCode::ASSET_MISSING,
            DiagnosticCode::ASSET_UNSUPPORTED_EXTENSION,
            DiagnosticCode::ASSET_WRONG_KIND,
        ]
    );
    assert!(diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("image asset not found: assets/missing.png")));
    assert!(diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("unsupported sound data asset extension for `assets/readme.txt`")));
    assert!(diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("wrong image asset type for `assets/jump.wav`; found audio asset")));
}

#[test]
fn accepts_inline_native_love_shader_source_strings() {
    check_tua_source_with_diagnostics_config(
        "src/shader.tua",
        r#"
local shader = love.graphics.newShader("vec4 effect(vec4 color, Image tex, vec2 tc, vec2 sc) { return color; }")
"#,
        &DiagnosticsConfig {
            formatting_lints: false,
            ..DiagnosticsConfig::default()
        },
    )
    .expect("inline shader source strings should not be treated as asset paths");
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
fn inferred_local_reassignments_use_a_stable_widened_contract() {
    check_tua_source(
        "src/reassignment.tua",
        r#"
local score = 10
score = 11

local state = "idle"
state = "running"

local dynamic: any = 1
dynamic = "allowed"
"#,
    )
    .expect("matching inferred reassignments and explicit any should remain valid");

    let diagnostics = check_tua_source(
        "src/reassignment.tua",
        r#"
local score = 10
score = "high"
"#,
    )
    .expect_err("an inferred number local should reject a string reassignment");

    assert_single_mismatch(&diagnostics, "expected number, found string");
}

#[test]
fn inferred_local_contracts_survive_control_flow_and_shadowing() {
    check_tua_source(
        "src/reassignment.tua",
        r#"
local function update(active: boolean)
  local score = 10

  if active then
    score = 11
  else
    score = 12
  end

  do
    local score = "shadow"
    score = "still shadowed"
  end

  local checked: number = score
end
"#,
    )
    .expect("branch assignments and shadowed locals should keep their own contracts");

    let diagnostics = check_tua_source(
        "src/reassignment.tua",
        r#"
local function update(active: boolean)
  local score = 10

  if active then
    score = 11
  else
    score = "high"
  end
end
"#,
    )
    .expect_err("a branch should not widen an inferred local at the join");

    assert_single_mismatch(&diagnostics, "expected number, found string");
}

#[test]
fn nil_initialization_requires_an_optional_or_dynamic_annotation() {
    check_tua_source(
        "src/reassignment.tua",
        r#"
local optional: number | nil = nil
optional = 10

local dynamic: any = nil
dynamic = "loaded"
"#,
    )
    .expect("optional and any annotations should support delayed initialization");

    let diagnostics = check_tua_source(
        "src/reassignment.tua",
        r#"
local inferred = nil
inferred = 10
"#,
    )
    .expect_err("an unannotated nil local should keep the inferred nil contract");

    assert_single_mismatch(&diagnostics, "expected nil, found number");

    let diagnostics = check_tua_source(
        "src/reassignment.tua",
        r#"
local inferred
inferred = 10
"#,
    )
    .expect_err("a local without an initializer should infer its runtime nil value");

    assert_single_mismatch(&diagnostics, "expected nil, found number");

    let diagnostics = check_tua_source(
        "src/reassignment.tua",
        r#"
local required: number
"#,
    )
    .expect_err("a non-optional local without an initializer is nil at runtime");

    assert_single_mismatch(&diagnostics, "expected number, found nil");
}

#[test]
fn inferred_local_compound_assignments_check_the_stable_contract() {
    check_tua_source(
        "src/reassignment.tua",
        r#"
local score = 10
score += 2
"#,
    )
    .expect("matching compound assignment should remain valid");

    let diagnostics = check_tua_source(
        "src/reassignment.tua",
        r#"
local score = 10
score += "two"
"#,
    )
    .expect_err("compound assignment should validate its value against the local contract");

    assert_single_mismatch(&diagnostics, "expected number, found string");
}

#[test]
fn ordinary_lua_inferred_locals_remain_permissive_without_annotations() {
    let parsed = crate::parse_lua_source(
        "src/legacy.lua",
        r#"
local value = 1
value = "dynamic Lua"
"#,
    )
    .expect("Lua source should parse");

    check_parsed_source(&parsed)
        .expect("ordinary Lua should not adopt Tua inferred reassignment contracts");
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
fn validates_love_conf_fullscreen_type_values() {
    check_tua_source(
        "src/conf.tua",
        r#"
function love.conf(t)
  t.window.fullscreentype = "desktop"
end
"#,
    )
    .expect("documented LOVE fullscreen types should be accepted");

    let diagnostics = check_tua_source(
        "src/conf.tua",
        r#"
function love.conf(t)
  t.window.fullscreentype = "borderless"
end
"#,
    )
    .expect_err("unknown LOVE fullscreen types should be rejected");

    assert_single_mismatch(
        &diagnostics,
        r#"expected "desktop" | "exclusive", found "borderless""#,
    );
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
local hp = unknown_value()
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
fn accepts_omitted_trailing_optional_parameters() {
    check_tua_source(
        "src/shake.tua",
        r#"
local function shake(duration: number, easing?: string): number
  return duration
end

local basic: number = shake(1)
local explicit: number = shake(1, "out")
"#,
    )
    .expect("omitted optional parameters should type-check");
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
fn accepts_omitted_optional_arguments_for_required_module_exports() {
    let temp = TempProject::new("required-module-optional-argument");
    let source = r#"local Functions = require("functions")

local state: string = Functions.describe()
"#;
    let functions = r#"function describe(mode?: string): string
  if mode == "short" then
    return "up"
  end

  return "alive"
end

return {
  describe = describe,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/functions.tua", functions);

    check_tua_source(temp.path.join("src/main.tua"), source)
        .expect("required module exports should preserve optional parameter arity");
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
fn accepts_love_object_subtypes_for_supertype_parameters() {
    check_tua_source(
        "src/draw.tua",
        r#"
local function drawImage(image: Image)
  love.graphics.draw(image, 0, 0)
end
"#,
    )
    .expect("Image should be assignable to the generated Drawable supertype");
}

#[test]
fn accepts_generated_love_api_overload_arguments() {
    let temp = TempProject::new("love-api-overload");
    let source = r#"
local font: Font = love.graphics.newFont("assets/ui.ttf", 32)
"#;

    temp.write("src/font.tua", source);
    temp.write("assets/ui.ttf", "");

    check_tua_source(temp.path.join("src/font.tua"), source)
        .expect("generated LOVE function overloads should participate in argument checking");
}

#[test]
fn keeps_dynamic_love_variant_arguments_permissive() {
    check_tua_source(
        "src/events.tua",
        r#"
love.event.push("keypressed", 42, true, { ready = true })
"#,
    )
    .expect("LOVE Variant parameters should accept ordinary Lua values");
}

#[test]
fn rejects_arguments_that_match_no_generated_love_api_overload() {
    let diagnostics = check_tua_source(
        "src/font.tua",
        r#"
local font = love.graphics.newFont(true, false)
"#,
    )
    .expect_err("arguments outside every generated LOVE overload should be rejected");

    assert_single_mismatch(&diagnostics, "expected string, found boolean");
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
fn checks_native_function_alias_callbacks_and_typed_varargs() {
    check_tua_source(
        "src/callbacks.tua",
        r#"type Handler = (value: number, ...string) -> string
type Decoder = (text: string) -> (boolean, string)

local handler: Handler = function(value: number, ...: string): string
  return tostring(value)
end

local decoder: Decoder = function(text: string): (boolean, string)
  return true, text
end

handler(1, "one", "two")
local ok, decoded = decoder("value")
"#,
    )
    .expect("matching native callback assignments and calls should pass");

    let diagnostics = check_tua_source(
        "src/callbacks.tua",
        r#"type Handler = (value: number, ...string) -> string

local handler: Handler = function(value: number, ...: string): string
  return tostring(value)
end

handler("wrong")
handler(1, 2)
"#,
    )
    .expect_err("native callback argument mismatches should be diagnosed");

    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH))
            .count(),
        2
    );
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
fn accepts_self_type_in_colon_method_annotations() {
    check_tua_source(
        "src/account.tua",
        r#"
type Account = {
  balance: number,
}

local Account = {}
Account.__index = Account

function Account:deposit(amount: number): Self
  self.balance += amount
  return self
end

local account = setmetatable({ balance = 100 }, Account)
local updated: Account = account:deposit(25)
updated.balance = updated.balance + 1
"#,
    )
    .expect("Self should resolve to the receiver type in typed colon methods");
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
fn accepts_is_primitive_and_alias_narrowing_inside_conditions() {
    check_tua_source(
        "src/runtime_is.tua",
        r#"
type UserId = number

local function from_string(value: string | number): string
  if value is string then
    return value
  end

  return ""
end

local function from_alias(value: any): number
  if value is UserId then
    return value
  end

  return 0
end
"#,
    )
    .expect("primitive and primitive-alias is checks should narrow true branches");
}

#[test]
fn accepts_is_metatable_narrowing_inside_conditions() {
    check_tua_source(
        "src/runtime_is.tua",
        r#"
local Player = {
  health = 0,
}
Player.__index = Player

local function damage(value: any): number
  if value is Player then
    value.health = value.health - 1
    return value.health
  end

  return 0
end
"#,
    )
    .expect("visible self-index metatable targets should narrow to instance fields");
}

#[test]
fn accepts_is_metatable_narrowing_with_same_name_shape_alias() {
    let source = r#"
type Account = {
  name: string,
  balance: number,
}

local Account = {}
Account.__index = Account

function Account:deposit(amount: number): Self
  self.balance = self.balance + amount

  return self
end

function Account:withdraw(amount: number): number
  self.balance = self.balance - amount

  return self.balance
end

local account = setmetatable({ name = "checking", balance = 50 }, Account)
local accountAfterDeposit: Account = account:deposit(25)
account:withdraw(10)
local value: any = account

if value is Account then
  value:deposit(5)
  value.balance = value.balance - 1
end
"#;

    check_tua_source("src/runtime_is.tua", source)
        .expect("runtime metatable checks should merge same-name table aliases");

    let ecs_config = EcsConfig {
        enabled: true,
        ..EcsConfig::default()
    };
    let parsed = parse_tua_source("src/runtime_is.tua", source).expect("source should parse");
    let diagnostics_config = DiagnosticsConfig {
        inline_ignores: true,
        undefined_variables: true,
        ..DiagnosticsConfig::default()
    };

    check_parsed_source_with_asset_roots_diagnostics_jit_and_ecs_config(
        &parsed,
        &[],
        &diagnostics_config,
        &JitConfig::default(),
        &ecs_config,
        &FormatOptions::default(),
    )
    .expect("ECS-enabled diagnostics should preserve runtime metatable narrowing");
}

#[test]
fn rejects_unsupported_is_targets() {
    let diagnostics = check_tua_source(
        "src/runtime_is.tua",
        r#"
type Player = {
  health: number,
}

local value: any = {}

if value is Player then
  value = value
end

if value is Array<string> then
  value = value
end
"#,
    )
    .expect_err("erased runtime is targets should be rejected");

    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.code == Some(DiagnosticCode::TYPE_UNSUPPORTED_RUNTIME_CHECK)
            })
            .count(),
        1
    );
    assert!(diagnostics[0].format_cli().contains("error TL1007"));
}

#[test]
fn accepts_optimized_class_constructor_fields_and_final_dot_calls() {
    check_tua_source(
        "src/classes.tua",
        r#"
open class Entity {
  x: number
  y: number

  init(x: number, y: number) {
    self.x = x
    self.y = y
  }

  update(dt: number) {
  }
}

final class Particle extends Entity {
  speed: number = 120

  init(x: number, y: number) {
    super(x, y)
  }

  override final update(dt: number) {
    self.x += self.speed * dt
  }
}

local particle = Particle.new(1, 2)
particle.update(0.1)
particle:update(0.2)
particle.x = particle.speed
"#,
    )
    .expect("optimized class calls and fields should type-check");
}

#[test]
fn rejects_extending_final_classes_and_overriding_final_methods() {
    let diagnostics = check_tua_source(
        "src/classes.tua",
        r#"
class Base {
  final update(dt: number) {
  }
}

class Child extends Base {
  override update(dt: number) {
  }
}
"#,
    )
    .expect_err("class inheritance violations should be reported");

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_CLASS)));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("extends final class")));
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

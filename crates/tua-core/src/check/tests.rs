use super::*;
use crate::DiagnosticCode;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

// Checker fixtures isolate type and domain behavior; linter tests cover the
// default standard-rule profile separately.
fn check_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<(), Vec<Diagnostic>> {
    super::check_tua_source_with_diagnostics_config(
        path,
        source,
        &diagnostics_without_standard_rules(),
    )
}

fn check_parsed_source(parsed: &ParsedSource) -> Result<(), Vec<Diagnostic>> {
    super::check_parsed_source_with_diagnostics_config(
        parsed,
        &diagnostics_without_standard_rules(),
    )
}

fn diagnostics_without_standard_rules() -> DiagnosticsConfig {
    DiagnosticsConfig {
        rules: false,
        ..DiagnosticsConfig::default()
    }
}

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
        &diagnostics_without_standard_rules(),
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

    assert!(diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("type mismatch at options.create: required field is missing")));
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
fn accepts_typed_store_state_updates_and_subscriptions() {
    check_tua_source(
        "src/store.tua",
        r#"
type GameState = { score: number, paused: boolean }

local game = store({ score = 0, paused = false })
local state: GameState = game:get()

game:update(function(state: GameState)
  state.score += 1
end)
game:set({ score = 10, paused = true })

local unsubscribe = game:subscribe(function(state: GameState, previous: GameState)
  print(state.score, previous.score)
end)
local stopScore = game:watch("score", function(value: number, previous: number, state: GameState)
  print(value, previous, state.paused)
end)

unsubscribe()
stopScore()
"#,
    )
    .expect("stores should preserve their state type across methods and listeners");
}

#[test]
fn accepts_optional_store_watch_values_as_nilable() {
    check_tua_source(
        "src/store.tua",
        r#"
type UiState = { selected?: number }
local initial: UiState = {}
local ui = store(initial)

ui:watch("selected", function(value: number | nil, previous: number | nil, state: UiState)
  print(value, previous, state.selected)
end)
"#,
    )
    .expect("optional state fields should produce nilable watcher values");
}

#[test]
fn rejects_invalid_typed_store_state_and_watch_keys() {
    let diagnostics = check_tua_source(
        "src/store.tua",
        r#"
local game = store({ score = 0, paused = false })
game:set({ score = "high", paused = false })
game:watch("missing", function(value) print(value) end)
"#,
    )
    .expect_err("stores should reject incompatible replacement state and unknown watch keys");

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("type mismatch at next.score: expected number, found string")),
        "unexpected diagnostics: {diagnostics:?}"
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("missing")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn preserves_an_explicit_store_function_binding() {
    check_tua_source(
        "src/custom_store.tua",
        r#"local function store(initial: any): string
  return "custom"
end

local result: string = store({})
"#,
    )
    .expect("an explicit store function should override the built-in helper");
}

#[test]
fn accepts_typed_builtin_state_machine_events_and_states() {
    check_tua_source(
        "src/state_machine.tua",
        r#"
local light = stateMachine({
  initial = "green",
  events = {
    { name = "warn", from = "green", to = "yellow" },
    { name = "panic", from = "yellow", to = "red" },
    { name = "reset", from = { "red", "yellow" }, to = "green" },
  },
  callbacks = {
    onenterred = function(
      self: StateMachine,
      event: "panic" | "reset" | "warn",
      from: "green" | "red" | "yellow",
      to: "green" | "red" | "yellow"
    )
      print(self.current, event, from, to)
    end,
  },
})

local current: "green" | "red" | "yellow" = light.current
local moving: "panic" | "reset" | "warn" | nil = light.currentTransitioningEvent
local warned: boolean = light:warn("payload")
local canPanic: boolean, destination: "green" | "red" | "yellow" | nil = light:can("panic")
local isGreen: boolean = light:is("green")
light:reset()
"#,
    )
    .expect("static state machines should derive event methods and state unions");
}

#[test]
fn accepts_unannotated_sprite_on_loop_callbacks() {
    check_tua_source(
        "src/sprites.tua",
        r#"local Player = spriteSheet({
  image = "assets/player.png",
  frame = { width = 128, height = 128 },
  animations = {
    green = {
      from = 0,
      to = 3,
      row = 2,
      durations = { 0.1, 0.5, 0.1, 0.1 },
      onLoop = function(animation, loops)
        if loops > 1 then
          animation:pause()
        end
      end,
    },
  },
})

return Player
"#,
    )
    .expect("sprite onLoop callbacks may omit annotations and return values");
}

#[test]
fn rejects_unknown_typed_state_machine_states_and_events() {
    let state_diagnostics = check_tua_source(
        "src/state_machine.tua",
        r#"local light = stateMachine({
  initial = "green",
  events = {
    { name = "warn", from = "green", to = "yellow" },
    { name = "panic", from = "yellow", to = "red" },
  },
})
light:is("blue")
"#,
    )
    .expect_err("unknown states should be rejected");
    assert_single_mismatch(
        &state_diagnostics,
        r#"expected "green" | "red" | "yellow", found "blue""#,
    );

    let event_diagnostics = check_tua_source(
        "src/state_machine.tua",
        r#"local light = stateMachine({
  initial = "green",
  events = {
    { name = "warn", from = "green", to = "yellow" },
  },
})
light:can("panic")
"#,
    )
    .expect_err("unknown events should be rejected");
    assert_single_mismatch(&event_diagnostics, r#"expected "warn", found "panic""#);
}

#[test]
fn preserves_an_explicit_state_machine_function_binding() {
    check_tua_source(
        "src/custom_state_machine.tua",
        r#"local function stateMachine(options: any): string
  return "custom"
end

local result: string = stateMachine({})
"#,
    )
    .expect("an explicit stateMachine function should override the built-in helper");
}

#[test]
fn keeps_dynamic_state_machine_configuration_gradual() {
    check_tua_source(
        "src/dynamic_state_machine.tua",
        r#"local options: any = loadStateMachineOptions()
local machine = stateMachine(options)
local current: string = machine.current
local canFire: boolean = machine:can("runtime.event")
machine:transition("runtime.event")
"#,
    )
    .expect("computed state machine configuration should use the generic surface");
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
        &diagnostics_without_standard_rules(),
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
fn rejects_empty_love_asset_loader_paths() {
    let temp = TempProject::new("empty-assets");
    let source = r#"
local helper = image ""
local native = love.graphics.newImage("")
"#;

    temp.write("src/assets.tua", source);

    let diagnostics = check_tua_source(temp.path.join("src/assets.tua"), source)
        .expect_err("empty asset paths should be reported");
    let codes = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();

    assert_eq!(
        codes,
        vec![
            Some(DiagnosticCode::ASSET_MISSING),
            Some(DiagnosticCode::ASSET_MISSING),
        ]
    );
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.message.contains("image asset path is empty")));
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
        &diagnostics_without_standard_rules(),
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
        &diagnostics_without_standard_rules(),
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

    assert_single_mismatch(
        &diagnostics,
        "type mismatch at hp: expected number, found string",
    );
}

#[test]
fn reports_nested_call_argument_mismatch_paths_and_leaf_ranges() {
    let diagnostics = check_tua_source(
        "src/animations.tua",
        r#"type Animation = { row: number }
type Config = { animations: { [string]: Animation } }
local function configure(config: Config)
end

configure({
  animations = {
    green = { row = "two" },
  },
})
"#,
    )
    .expect_err("the nested animation row should be rejected");

    assert_single_mismatch(
        &diagnostics,
        "type mismatch at config.animations.green.row: expected number, found string",
    );
    assert_eq!(diagnostics[0].line, 8);
    assert_eq!(diagnostics[0].column, 21);
    assert_eq!(diagnostics[0].end_line, 8);
    assert_eq!(diagnostics[0].end_column, 26);
}

#[test]
fn reports_nested_callback_return_mismatch_paths() {
    let diagnostics = check_tua_source(
        "src/callbacks.tua",
        r#"type Config = {
  callbacks: {
    onChange: (value: number) -> number,
  },
}
local function configure(config: Config)
end

configure({
  callbacks = {
    onChange = function(value: number): string
      return "bad"
    end,
  },
})
"#,
    )
    .expect_err("the nested callback return should be rejected");

    assert_single_mismatch(
        &diagnostics,
        "type mismatch at config.callbacks.onChange return value: expected number, found string",
    );
}

#[test]
fn reports_nested_callback_parameter_mismatch_paths() {
    let diagnostics = check_tua_source(
        "src/callback-parameters.tua",
        r#"type Config = {
  callbacks: {
    onChange: (value: number) -> number,
  },
}
local function configure(config: Config)
end

configure({
  callbacks = {
    onChange = function(value: string): number
      return 1
    end,
  },
})
"#,
    )
    .expect_err("the nested callback parameter should be rejected");

    assert_single_mismatch(
        &diagnostics,
        "type mismatch at config.callbacks.onChange parameter `value`: expected number, found string",
    );
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
        &diagnostics_without_standard_rules(),
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
        &diagnostics_without_standard_rules(),
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
        &diagnostics_without_standard_rules(),
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
fn diagnoses_misspelled_love_callback_declarations() {
    let diagnostics = check_tua_source("src/main.tua", "function love.udpate(dt: number)\nend\n")
        .expect_err("misspelled LOVE callbacks should be reported");
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == Some(DiagnosticCode::DOMAIN_LOVE_CALLBACK))
        .expect("LOVE callback diagnostic should be present");

    assert_eq!(diagnostic.severity, crate::DiagnosticSeverity::Warning);
    assert!(diagnostic.message.contains("did you mean `love.update`"));
    assert_eq!((diagnostic.line, diagnostic.column), (1, 15));
}

#[test]
fn accepts_love_callbacks_that_omit_unused_parameters() {
    check_tua_source(
        "src/main.tua",
        r#"function love.keypressed(key: string)
end

function love.update()
end
"#,
    )
    .expect("Lua callbacks may omit parameters they do not use");
}

#[test]
fn accepts_generated_love_callback_overload_signatures() {
    check_tua_source(
        "src/main.tua",
        r#"function love.keypressed(key: string, isrepeat: boolean)
end
"#,
    )
    .expect("generated callback overloads should be considered during validation");
}

#[test]
fn rejects_incompatible_love_callback_parameter_types() {
    let diagnostics = check_tua_source("src/main.tua", "function love.update(dt: string)\nend\n")
        .expect_err("incompatible LOVE callback parameter types should be reported");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::DOMAIN_LOVE_CALLBACK)
            && diagnostic
                .message
                .contains("expected `function love.update(dt: number)`")
    }));
}

#[test]
fn rejects_love_callbacks_that_require_extra_parameters() {
    let diagnostics = check_tua_source("src/main.tua", "function love.update(dt, extra)\nend\n")
        .expect_err("LOVE callbacks should not require parameters the runtime never supplies");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::DOMAIN_LOVE_CALLBACK)
            && diagnostic.message.contains("receives at most 1 parameter")
    }));
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
        &diagnostics_without_standard_rules(),
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
            &diagnostics_without_standard_rules(),
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
fn inline_ignores_are_enabled_by_default() {
    check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
-- tua-ignore-next-line TL1003
local hp: number = "full"
"#,
        &diagnostics_without_standard_rules(),
    )
    .expect("default inline ignore should suppress the exact diagnostic code");
}

#[test]
fn inline_ignores_suppress_matching_type_diagnostics() {
    let config = crate::DiagnosticsConfig {
        inline_ignores: true,
        ..diagnostics_without_standard_rules()
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

    check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
-- tua-ignore-file TL1003
local hp: number = "full"
"#,
        &config,
    )
    .expect("default file-scope inline ignore should suppress matching diagnostic");
}

#[test]
fn default_inline_ignores_support_exact_file_and_line_lint_codes() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"-- tua-ignore-file TL4048
local condition = true
local fallback = 42
local result = condition and false or fallback
local unused = 1
print(result)
"#,
        &DiagnosticsConfig::default(),
    )
    .expect_err("an exact file ignore should preserve unrelated lint diagnostics");

    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_MISLEADING_AND_OR))
            .count(),
        0,
        "{diagnostics:#?}"
    );
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_UNUSED_LOCAL)));

    check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"local condition = true
local fallback = 42
local result = condition and false or fallback -- tua-ignore-line TL4048
print(result)
"#,
        &DiagnosticsConfig::default(),
    )
    .expect("an exact same-line ignore should suppress its matching lint");
}

#[test]
fn inline_ignores_report_unused_and_invalid_comments() {
    let config = crate::DiagnosticsConfig {
        inline_ignores: true,
        ..diagnostics_without_standard_rules()
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
fn accepts_native_class_arrays_across_locals_functions_and_polymorphic_loops() {
    check_tua_source(
        "src/actors.tua",
        r#"
open class Actor {
  attack() {
  }
}

final class Fighter extends Actor {
  override attack() {
  }
}

final class Mage extends Actor {
  override attack() {
  }
}

local function keep(values: Actor[]): Actor[]
  return values
end

local actors: Actor[] = keep({ Fighter.new(), Mage.new() })
for _, actor in ipairs(actors) do
  actor:attack()
end
"#,
    )
    .expect("native arrays should preserve their class item type through functions and loops");
}

#[test]
fn rejects_values_that_do_not_match_a_native_array_item_type() {
    let diagnostics = check_tua_source(
        "src/actors.tua",
        r#"
local actors: number[] = { 1, "not a number" }
"#,
    )
    .expect_err("native array annotations should constrain initializer items");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)
            && diagnostic.message.contains("expected number, found string")
    }));
}

#[test]
fn rejects_indexed_assignments_that_do_not_match_a_native_array_item_type() {
    let diagnostics = check_tua_source(
        "src/actors.tua",
        r#"
local actors: number[] = {}

actors[1] = "not a number"
"#,
    )
    .expect_err("native array annotations should constrain indexed assignments");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)
            && diagnostic.message.contains("expected number, found string")
            && diagnostic.line == 4
    }));
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
fn assignments_shadow_metatable_index_fields_instead_of_overriding_them() {
    check_tua_source(
        "src/inheritance.tua",
        r#"
local Base = { value = 1 }
local Child = {}
setmetatable(Child, { __index = Base })

Child.value = "child"
local value: string = Child.value
"#,
    )
    .expect("a direct table write may shadow a differently typed __index field");
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
        &diagnostics_without_standard_rules(),
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
        &diagnostics_without_standard_rules(),
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
        ..diagnostics_without_standard_rules()
    };

    check_parsed_source_with_asset_roots_diagnostics_jit_and_ecs_config(
        &parsed,
        &[],
        &diagnostics_config,
        &JitConfig::default(),
        &ecs_config,
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
fn accepts_independent_subclass_constructor_signatures_and_checks_super_arguments() {
    check_tua_source(
        "src/classes.tua",
        r#"
open class Base {
  name: string

  init(name: string) {
    self.name = name
  }
}

final class Child extends Base {
  id: number

  init(id: number, name: string) {
    super(name)
    self.id = id
  }
}

local base = Base.new("base")
local child = Child.new(1, "child")
"#,
    )
    .expect("subclass constructors may intentionally differ from their parent");

    let diagnostics = check_tua_source(
        "src/classes.tua",
        r#"
open class Base {
  name: string

  init(name: string) {
    self.name = name
  }
}

final class Child extends Base {
  id: number

  init(id: number) {
    super(id)
    self.id = id
  }
}
"#,
    )
    .expect_err("super arguments should still match the parent initializer");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)
            && diagnostic.message.contains("expected string, found number")
    }));
}

#[test]
fn accepts_class_any_fields_initialized_with_table_literals() {
    check_tua_source(
        "src/classes.tua",
        r#"
final class BeatManager {
  steps: any
  y: number

  init(y: number) {
    self.steps = {}
    self.y = y
  }
}

local manager: BeatManager = BeatManager.new(1)
manager.steps = {}
"#,
    )
    .expect("class any fields should accept table literal assignments");
}

#[test]
fn accepts_class_array_fields_and_indexing() {
    check_tua_source(
        "src/classes.tua",
        r#"
final class BeatManager {
  steps: number[]
  y: number

  init(y: number) {
    self.steps = { 1, 2, 3 }
    self.y = y
  }
}

local manager: BeatManager = BeatManager.new(1)
local first: number = manager.steps[1]
"#,
    )
    .expect("class array fields should type-check and support indexing");
}

#[test]
fn accepts_empty_and_class_typed_arrays_in_class_fields() {
    check_tua_source(
        "src/classes.tua",
        r#"
final class Step {
  value: number

  init(value: number) {
    self.value = value
  }
}

final class BeatManager {
  steps: Step[]

  init() {
    self.steps = {}
  }

  add(step: Step): number {
    table.insert(self.steps, step)
    return step.value
  }
}

local manager = BeatManager.new()
local value: number = manager.add(Step.new(1))
local first: Step = manager.steps[1]
"#,
    )
    .expect("class-typed arrays should accept empty initialization and preserve item types");
}

#[test]
fn checks_class_method_parameter_and_return_types() {
    let diagnostics = check_tua_source(
        "src/classes.tua",
        r#"
final class Counter {
  value: number

  init() {
    self.value = 0
  }

  add(amount: number): number {
    return self.value + amount
  }
}

local counter = Counter.new()
local value: string = counter.add(counter, "wrong")
"#,
    )
    .expect_err("class method calls should use their declared signature");

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)));
}

#[test]
fn class_body_type_diagnostics_use_source_provenance() {
    let source = r#"final class Counter {
  value: number

  init() {
    self.value = "wrong"
  }
}"#;
    let diagnostics = check_tua_source("src/classes.tua", source)
        .expect_err("class body type mismatches should be reported");
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH))
        .expect("class body mismatch should be present");

    assert_eq!((diagnostic.line, diagnostic.column), (1, 1));
    assert!(diagnostic.end_line <= source.lines().count());
}

#[test]
fn rejects_unknown_class_members_but_keeps_any_dynamic() {
    let diagnostics = check_tua_source(
        "src/classes.tua",
        r#"
final class Counter {
  value: number
  dynamic: any

  init() {
    self.value = 0
    self.missing = 1
    self.dynamic.missing = 1
  }

  read(): number {
    return self.other
  }
}
"#,
    )
    .expect_err("unknown closed class members should be rejected");

    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_CLASS))
            .count(),
        2
    );
    assert!(diagnostics
        .iter()
        .all(|diagnostic| !diagnostic.message.contains("dynamic")));
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
fn rejects_duplicate_initializers_inheritance_cycles_and_override_type_drift() {
    let diagnostics = check_tua_source(
        "src/classes.tua",
        r#"
open class First extends Second {
  init() {
  }

  init(value: number) {
  }

}

open class Second extends First {
}
"#,
    )
    .expect_err("invalid class structure should be diagnosed before type checking");

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("duplicate class member `init`")));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("inheritance cycle")));

    let diagnostics = check_tua_source(
        "src/overrides.tua",
        r#"
open class Base {
  update(value: number): number {
    return value
  }
}

final class Child extends Base {
  override update(value: string): number {
    return 1
  }
}
"#,
    )
    .expect_err("override signatures should remain compatible");

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("signature incompatible")));
}

#[test]
fn rejects_incompatible_inherited_class_field_redeclarations() {
    let diagnostics = check_tua_source(
        "src/classes.tua",
        r#"
open class Base {
  value: number = 1
}

final class Child extends Base {
  value: string = "wrong"
}
"#,
    )
    .expect_err("inherited fields should keep their parent contract");

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.code == Some(DiagnosticCode::TYPE_CLASS)
                && diagnostic.message.contains("inherited field `value`")
        })
        .expect("incompatible inherited field should report TL1008");

    assert_eq!(
        diagnostic.message,
        "inherited field `value` in class `Child` has type `string`, but class `Base` declares `number`"
    );

    check_tua_source(
        "src/classes.tua",
        r#"
type Count = number

open class Base {
  value: Count = 1
}

final class Child extends Base {
  value: number = 2
}
"#,
    )
    .expect("an inherited field may keep the resolved parent contract");
}

#[test]
fn requires_class_fields_to_be_initialized_on_every_constructor_path() {
    let diagnostics = check_tua_source(
        "src/classes.tua",
        r#"
final class Missing {
  value: number

  init(flag: boolean) {
    if flag then
      self.value = 1
    end
  }
}

final class EarlyReturn {
  value: number = 1

  init(skip: boolean) {
    if skip then
      return
    end
  }
}

open class Base {
  inherited: number

  init() {
    self.inherited = 1
  }
}

final class Child extends Base {
  init() {
  }
}
"#,
    )
    .expect_err("required fields should be initialized on every constructor path");

    let messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_CLASS))
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert!(messages.contains(&"class `Missing` does not initialize required field `value`"));
    assert!(messages.contains(&"class `EarlyReturn` does not initialize required field `value`"));
    assert!(messages.contains(&"class `Child` does not initialize inherited field `inherited`"));
    assert_eq!(messages.len(), 3);

    check_tua_source(
        "src/classes.tua",
        r#"
final class Complete extends Base {
  value: number
  maybe: number | nil

  init() {
    super()
    if self.inherited > 0 then
      self.value = 1
    else
      self.value = 2
    end
  }
}

open class Base {
  inherited: number

  init() {
    self.inherited = 1
  }
}
"#,
    )
    .expect(
        "all branches, forward super initialization, and nil-capable fields should be accepted",
    );
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

#[test]
fn checks_typed_baton_input_controls_pairs_and_devices() {
    check_tua_source(
        "main.tua",
        r#"local controls = input({
  controls = {
    left = { "key:left", "axis:leftx-" },
    right = { "key:right", "axis:leftx+" },
    up = { "key:up" },
    down = { "key:down" },
    confirm = { "key:return", "button:a" },
  },
  pairs = {
    move = { "left", "right", "up", "down" },
  },
})

controls:update()
local confirm: number = controls:get("confirm")
local x, y = controls:get("move")
local typed_x: number = x
local typed_y: number = y
local pressed: boolean = controls:pressed("confirm")
local device: "kbm" | "joy" | "none" = controls:getActiveDevice()
"#,
    )
    .expect("static input config should derive valid method types");
}

#[test]
fn rejects_unknown_static_input_names_and_invalid_config() {
    let unknown = check_tua_source(
        "main.tua",
        r#"local controls = input({ controls = { confirm = { "key:return" } } })
controls:pressed("cancel")
"#,
    )
    .expect_err("unknown input name should fail");
    assert!(unknown.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)
            && diagnostic.message.contains("\"cancel\"")
    }));

    let invalid = check_tua_source(
        "main.tua",
        r#"local controls = input({
  controls = { left = { "keyboard:left" } },
  pairs = { move = { "left", "right", "up" } },
})
"#,
    )
    .expect_err("invalid input config should fail");
    assert!(invalid
        .iter()
        .all(|diagnostic| { diagnostic.code == Some(DiagnosticCode::DOMAIN_INPUT_CONFIG) }));
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

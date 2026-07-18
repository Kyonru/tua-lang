use super::*;
use crate::{DiagnosticCategory, DiagnosticCode, SourceSpan};

#[test]
fn snapshot_type_aliases_emit_no_runtime_code() {
    let emitted = emit_tua_source(
        "src/types.tua",
        r#"
type Vec2 = {
  x: number,
  y: number,
}

type MaybeNumber = number | nil
"#,
    )
    .expect("type-only source should emit");

    assert_eq!(emitted, "");
}

#[test]
fn snapshot_enums_emit_no_runtime_code() {
    let emitted = emit_tua_source(
        "src/enums.tua",
        r#"
enum FilterMode
  "linear"
  "nearest"
end
"#,
    )
    .expect("enum-only source should emit");

    assert_eq!(emitted, "");
}

#[test]
fn snapshot_erases_local_annotations() {
    let emitted = emit_tua_source(
        "src/locals.tua",
        r#"
type Player = {
  hp: number,
}

local score: number = 0
local title: string = "Arena"
local current: Player | nil = nil

return score
"#,
    )
    .expect("annotated locals should emit");

    assert_eq!(
        emitted,
        r#"local score = 0
local title = "Arena"
local current = nil

return score
"#
    );
}

#[test]
fn snapshot_erases_function_annotations() {
    let emitted = emit_tua_source(
        "src/functions.tua",
        r#"
type Player = {
  hp: number,
}

local function damage(player: Player, amount: number): Player
  player.hp = player.hp - amount
  return player
end

function heal(player: Player, amount: number): Player
  player.hp = player.hp + amount
  return player
end
"#,
    )
    .expect("annotated functions should emit");

    assert_eq!(
        emitted,
        r#"local function damage(player, amount)
  player.hp = player.hp - amount
  return player
end

function heal(player, amount)
  player.hp = player.hp + amount
  return player
end
"#
    );
}

#[test]
fn snapshot_erases_anonymous_function_annotations() {
    let emitted = emit_tua_source(
        "src/functions.tua",
        r#"
local callbacks = {
  add = function(self: any, amount: number): number
    self.count = self.count + amount
    return self.count
  end,
}
"#,
    )
    .expect("annotated anonymous functions should emit");

    assert_eq!(
        emitted,
        r#"local callbacks = {
  add = function(self, amount)
    self.count = self.count + amount
    return self.count
  end,
}
"#
    );
}

#[test]
fn snapshot_lowers_runtime_is_checks_to_lua_type_and_metatable_checks() {
    let emitted = emit_tua_source(
        "src/runtime_is.tua",
        r#"
type UserId = number

local Player = {
  health = 0,
}
Player.__index = Player

if value is string then
  print(value)
end

if value is UserId then
  print(value)
end

if value is Player then
  value.health = value.health - 1
end
"#,
    )
    .expect("runtime is checks should emit");

    assert_eq!(
        emitted,
        r#"local Player = {
  health = 0,
}
Player.__index = Player

if type(value) == "string" then
  print(value)
end

if type(value) == "number" then
  print(value)
end

if getmetatable(value) == Player then
  value.health = value.health - 1
end
"#
    );
}

#[test]
fn unsupported_runtime_is_target_emits_diagnostic() {
    let diagnostics = emit_tua_source(
        "src/runtime_is.tua",
        r#"
if value is Array<string> then
  print(value)
end
"#,
    )
    .expect_err("unsupported runtime is target should not lower");

    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::TYPE_UNSUPPORTED_RUNTIME_CHECK)
    );
    assert!(diagnostics[0].format_cli().contains("error TL1007"));
}

#[test]
fn runtime_is_target_must_be_visible_at_check_site() {
    let diagnostics = emit_tua_source(
        "src/runtime_is_scope.tua",
        r#"
if value is Player then
  print(value)
end

local Player = {}
"#,
    )
    .expect_err("runtime is target declared after the check should not lower");

    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::TYPE_UNSUPPORTED_RUNTIME_CHECK)
    );
}

#[test]
fn snapshot_lowers_classes_to_optimized_metatable_lua() {
    let emitted = emit_tua_source(
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

  move(dx: number) {
    self.x += dx
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
particle.move(1)
particle:move(2)
local entity = Entity.new(0, 0)
entity.update(0.25)
entity:update(0.3)

if particle is Entity then
  particle.x = 3
end
"#,
    )
    .expect("class syntax should emit optimized Lua");

    assert!(emitted.contains("Entity.__type_id = 1"));
    assert!(emitted.contains("Particle.__is = {\n  [1] = true,\n  [2] = true,\n}"));
    assert!(emitted.contains("local function Particle_update(self, dt)"));
    assert!(emitted.contains(
        "return setmetatable({\n    x = x,\n    y = y,\n    speed = 120,\n  }, Particle)"
    ));
    assert!(emitted.contains("Particle_update(particle, 0.1)"));
    assert!(emitted.contains("Particle_update(particle, 0.2)"));
    assert!(emitted.contains("Entity_move(particle, 1)"));
    assert!(emitted.contains("Entity_move(particle, 2)"));
    assert!(emitted.contains("entity.update(entity, 0.25)"));
    assert!(emitted.contains("entity:update(0.3)"));
    assert!(emitted.contains(".__is[Entity.__type_id] == true"));
}

#[test]
fn snapshot_emits_luacats_annotations() {
    let emitted = emit_tua_source_with_mode(
        "src/functions.tua",
        r#"
type Player = {
  hp: number,
  alive: boolean,
}

local score: number = 0

function heal(player: Player, amount: number): Player
  player.hp = player.hp + amount
  return player
end
"#,
        AnnotationMode::Luacats,
    )
    .expect("luacats mode should emit type annotations");

    assert_eq!(
        emitted,
        r#"---@alias Player { hp: number, alive: boolean }

---@type number
local score = 0

---@param player Player
---@param amount number
---@return Player
function heal(player, amount)
  player.hp = player.hp + amount
  return player
end
"#
    );
}

#[test]
fn snapshot_emits_luacats_annotations_for_local_functions_and_exports() {
    let emitted = emit_tua_source_with_mode(
        "src/player_factory.tua",
        r#"
type Vec2 = {
  x: number,
  y: number,
}

type Player = {
  name: string,
  pos: Vec2,
}

local default_player: Player = {
  name = "hero",
  pos = { x = 0, y = 0 },
}

local function make_player(name: string): Player
  return {
    name = name,
    pos = { x = 0, y = 0 },
  }
end

return {
  default_player = default_player,
  make_player = make_player,
}
"#,
        AnnotationMode::Luacats,
    )
    .expect("luacats mode should annotate stable module exports");

    assert_eq!(
        emitted,
        r#"---@alias Vec2 { x: number, y: number }
---@alias Player { name: string, pos: Vec2 }

---@type Player
local default_player = {
  name = "hero",
  pos = { x = 0, y = 0 },
}

---@param name string
---@return Player
local function make_player(name)
  return {
    name = name,
    pos = { x = 0, y = 0 },
  }
end

---@class PlayerFactoryExports
---@field default_player Player
---@field make_player fun(name: string): Player
return {
  default_player = default_player,
  make_player = make_player,
}
"#
    );
}

#[test]
fn snapshot_emits_luacats_type_only_alias_without_runtime_code() {
    let emitted = emit_tua_source_with_mode(
        "src/types.tua",
        r#"
type Player = {
  hp: number,
}
"#,
        AnnotationMode::Luacats,
    )
    .expect("luacats mode should keep alias information");

    assert_eq!(
        emitted,
        r#"---@alias Player { hp: number }
"#
    );
}

#[test]
fn snapshot_emits_native_function_alias_as_luacats_function_type() {
    let emitted = emit_tua_source_with_mode(
        "src/types.tua",
        r#"
type Handler = (value: number, ...string) -> string
"#,
        AnnotationMode::Luacats,
    )
    .expect("native function aliases should emit LuaCATS function types");

    assert_eq!(
        emitted,
        r#"---@alias Handler fun(value: number, ...: string): string
"#
    );
}

#[test]
fn snapshot_emits_luacats_alias_for_enums() {
    let emitted = emit_tua_source_with_mode(
        "src/enums.tua",
        r#"
enum FilterMode
  "linear"
  "nearest"
end
"#,
        AnnotationMode::Luacats,
    )
    .expect("luacats mode should emit enum aliases");

    assert_eq!(
        emitted,
        r#"---@alias FilterMode "linear" | "nearest"
"#
    );
}

#[test]
fn snapshot_luacats_annotations_follow_nested_indentation() {
    let emitted = emit_tua_source_with_mode(
        "src/nested.tua",
        r#"
if ready then
  local score: number = 1
end
"#,
        AnnotationMode::Luacats,
    )
    .expect("luacats comments should match declaration indentation");

    assert_eq!(
        emitted,
        r#"if ready then
  ---@type number
  local score = 1
end
"#
    );
}

#[test]
fn snapshot_luacats_mode_does_not_emit_annotations_by_default() {
    let emitted = emit_tua_source_with_mode(
        "src/functions.tua",
        r#"
type Player = {
  hp: number,
  alive: boolean,
}

local score: number = 0
"#,
        AnnotationMode::Off,
    )
    .expect("off mode should preserve erased output");

    assert_eq!(
        emitted,
        r#"local score = 0
"#
    );
}

#[test]
fn snapshot_preserves_ordinary_lua_statements_and_expressions() {
    let input = r#"
local Player = require("player")
local player = {
  hp = 10,
  alive = true,
}

if player.hp > 0 then
  print(Player, player.hp)
end
"#;
    let emitted = emit_tua_source("src/plain.tua", input).expect("plain Lua should emit");

    assert_eq!(
        emitted,
        r#"local Player = require("player")
local player = {
  hp = 10,
  alive = true,
}

if player.hp > 0 then
  print(Player, player.hp)
end
"#
    );
}

#[test]
fn snapshot_lowers_static_imports_with_project_resolution() {
    let parsed = parse_tua_source(
        "src/main.tua",
        r#"
local Player: any = import("./entities/player")
local Shared = import("shared")
"#,
    )
    .expect("source should parse");
    let resolver = |specifier: &str| match specifier {
        "./entities/player" => Some("entities.player".to_owned()),
        "shared" => Some("shared".to_owned()),
        _ => None,
    };
    let emitted =
        emit_lua_with_mode_and_import_resolver(&parsed, AnnotationMode::Off, Some(&resolver))
            .expect("static imports should emit");

    assert_eq!(
        emitted,
        r#"local Player = require("entities.player")
local Shared = require("shared")
"#
    );
}

#[test]
fn snapshot_lowers_project_imports_without_project_resolution() {
    let emitted = emit_tua_source("src/main.tua", "local Shared = import(\"shared\")\n")
        .expect("project module imports should emit like require");

    assert_eq!(emitted, "local Shared = require(\"shared\")\n");
}

#[test]
fn snapshot_leaves_runtime_import_calls_alone() {
    let emitted = emit_tua_source("src/runtime_import.tua", "print(import(\"./runtime\"))\n")
        .expect("ordinary runtime import calls should remain ordinary Lua calls");

    assert_eq!(emitted, "print(import(\"./runtime\"))\n");
}

#[test]
fn unresolved_child_relative_imports_do_not_emit_runtime_import_calls() {
    let diagnostics = emit_tua_source("src/main.tua", "local Player = import(\"./player\")\n")
        .expect_err("project-less child imports should not emit");

    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::LINT_BAD_REQUIRE));
    assert!(diagnostics[0]
        .message
        .contains("without project module resolution"));
}

#[test]
fn snapshot_erases_annotations_in_nested_blocks() {
    let emitted = emit_tua_source(
        "src/nested.tua",
        r#"
if ready then
  type LocalState = {
    ok: boolean,
  }
  local state: LocalState = { ok = true }
  for index: number = 1, 3 do
    local value: number = index
    print(value)
  end
end
"#,
    )
    .expect("nested annotations should emit");

    assert_eq!(
        emitted,
        r#"if ready then
  local state = { ok = true }
  for index = 1, 3 do
    local value = index
    print(value)
  end
end
"#
    );
}

#[test]
fn snapshot_lowers_default_parameters() {
    let emitted = emit_tua_source(
        "src/shake.tua",
        r#"
function shake(duration: number = 0.3, strength = 1)
  return duration * strength
end
"#,
    )
    .expect("default parameters should emit");

    assert_eq!(
        emitted,
        r#"function shake(duration, strength)
  if duration == nil then
    duration = 0.3
  end
  if strength == nil then
    strength = 1
  end
  return duration * strength
end
"#
    );
}

#[test]
fn snapshot_erases_optional_parameters() {
    let emitted = emit_tua_source(
        "src/shake.tua",
        r#"
function shake(duration: number, easing?: string): number
  return duration
end
"#,
    )
    .expect("optional parameters should emit");

    assert_eq!(
        emitted,
        r#"function shake(duration, easing)
  return duration
end
"#
    );
}

#[test]
fn snapshot_lowers_all_supported_compound_assignments() {
    let emitted = emit_tua_source(
        "src/compound.tua",
        r#"
local x = 10
x += 5
x -= 2
x *= scale
x /= 3
x %= 4
player.hp -= amount
"#,
    )
    .expect("supported compound assignments should emit");

    assert_eq!(
        emitted,
        r#"local x = 10
x = x + 5
x = x - 2
x = x * scale
x = x / 3
x = x % 4
player.hp = player.hp - amount
"#
    );
}

#[test]
fn snapshot_lowers_love_asset_loader_calls() {
    let emitted = emit_tua_source(
        "src/assets.tua",
        r#"
local player = image "assets/player.png"
local jump = sound "assets/jump.wav"
local theme = music "assets/theme.ogg"
local mainFont = font("assets/ui.ttf", 18)
local wave = shader "assets/wave.glsl"
local data = imageData "assets/player.png"
local target = canvas(320, 180)
local frame = quad(0, 0, 16, 16, 64, 64)
local batch = spriteBatch(player, 128)
local sparks = particles(player, 64)
local ui = image("assets/ui.png")
"#,
    )
    .expect("asset loader calls should emit");

    assert_eq!(
        emitted,
        r#"local player = love.graphics.newImage("assets/player.png")
local jump = love.audio.newSource("assets/jump.wav", "static")
local theme = love.audio.newSource("assets/theme.ogg", "stream")
local mainFont = love.graphics.newFont("assets/ui.ttf", 18)
local wave = love.graphics.newShader("assets/wave.glsl")
local data = love.image.newImageData("assets/player.png")
local target = love.graphics.newCanvas(320, 180)
local frame = love.graphics.newQuad(0, 0, 16, 16, 64, 64)
local batch = love.graphics.newSpriteBatch(player, 128)
local sparks = love.graphics.newParticleSystem(player, 64)
local ui = love.graphics.newImage("assets/ui.png")
"#
    );
}

#[test]
fn snapshot_lowers_sprite_declarations_to_anim8_structure() {
    let emitted = emit_tua_source(
        "src/sprites.tua",
        r#"local Player: SpriteSheet = spriteSheet({
  image = "assets/player.png",
  frame = { width = 16, height = 16 },
  animations = {
    idle = { from = 0, to = 3, fps = 8 },
    run = { from = 4, to = 9, fps = 12, loop = false },
  },
})
"#,
    )
    .expect("sprite helper should emit");

    assert_eq!(
        emitted,
        r#"-- Sprite helper: Player lowers to anim8-backed Lua.
local anim8 = require("anim8")
local PlayerImage = love.graphics.newImage("assets/player.png")
local PlayerGrid = anim8.newGrid(16, 16, PlayerImage:getWidth(), PlayerImage:getHeight())
local Player = {
  image = PlayerImage,
  grid = PlayerGrid,
  frame = { width = 16, height = 16, left = 0, top = 0, border = 0 },
  animations = {
    idle = anim8.newAnimation(PlayerGrid("1-4", 1), 1 / 8),
    run = anim8.newAnimation(PlayerGrid("5-10", 1), 1 / 12, "pauseAtEnd"),
  },
}
"#
    );
}

#[test]
fn snapshot_lowers_builtin_vec2_constructor_calls_to_brinevector() {
    let emitted = emit_tua_source(
        "src/vector.tua",
        r#"
local position: Vec2 = vec2(100, 200)
local offset = vec2(player.x + 4, math.max(0, player.y))
"#,
    )
    .expect("Vec2 constructor calls should emit");

    assert_eq!(
        emitted,
        r#"local __tua_vec2 = require("brinevector")
local position = __tua_vec2(100, 200)
local offset = __tua_vec2(player.x + 4, math.max(0, player.y))
"#
    );
}

#[test]
fn snapshot_lowers_builtin_signal_constructor_calls_to_tuasignal() {
    let emitted = emit_tua_source(
        "src/signals.tua",
        r#"
local events: SignalBus = signal()
events:on("player.hit", function(player, damage)
  print(player, damage)
end)
events:emit("player.hit", "ada", 4)
"#,
    )
    .expect("SignalBus constructor calls should emit");

    assert_eq!(
        emitted,
        r#"local __tua_signal = require("tuasignal")
local events = __tua_signal()
events:on("player.hit", function(player, damage)
  print(player, damage)
end)
events:emit("player.hit", "ada", 4)
"#
    );
}

#[test]
fn snapshot_lowers_builtin_object_pool_constructor_to_bundled_runtime() {
    let emitted = emit_tua_source(
        "src/pool.tua",
        r#"
type Bullet = { active: boolean }
local bullets = objectPool({
  create = function(): Bullet
    return { active = false }
  end,
  initial = 16,
})
local bullet = bullets:acquire()
bullets:release(bullet)
"#,
    )
    .expect("objectPool constructor calls should emit");

    assert_eq!(
        emitted,
        r#"local __tua_object_pool = require("tuaobjectpool")
local bullets = __tua_object_pool({
  create = function()
    return { active = false }
  end,
  initial = 16,
})
local bullet = bullets:acquire()
bullets:release(bullet)
"#
    );
}

#[test]
fn snapshot_preserves_an_explicit_object_pool_binding() {
    let emitted = emit_tua_source(
        "src/pool.tua",
        r#"
local objectPool = require("project_pool")
local bullets = objectPool({ create = function() return {} end })
"#,
    )
    .expect("explicit objectPool binding should emit unchanged");

    assert_eq!(
        emitted,
        r#"local objectPool = require("project_pool")
local bullets = objectPool({ create = function() return {} end })
"#
    );
}

#[test]
fn snapshot_injects_builtin_ecs_runtime_when_ecs_is_used() {
    let emitted = emit_tua_source(
        "src/ecs.tua",
        r#"
local Position = ecs.component("Position", { x = "number" })
local world = ecs.world()
local entity = world:create()
world:add(entity, Position, { x = 10 })
"#,
    )
    .expect("built-in ECS calls should emit");

    assert_eq!(
        emitted,
        r#"local ecs = require("tuaecs")
local Position = ecs.component("Position", { x = "number" })
local world = ecs.world()
local entity = world:create()
world:add(entity, Position, { x = 10 })
"#
    );
}

#[test]
fn snapshot_preserves_an_explicit_ecs_runtime_binding() {
    let emitted = emit_tua_source(
        "src/ecs.tua",
        r#"
local ecs = require("project_ecs")
local world = ecs.world()
"#,
    )
    .expect("explicit ECS runtime should emit unchanged");

    assert_eq!(
        emitted,
        r#"local ecs = require("project_ecs")
local world = ecs.world()
"#
    );
}

#[test]
fn snapshot_does_not_treat_ecs_table_fields_as_runtime_bindings() {
    let emitted = emit_tua_source(
        "src/ecs.tua",
        r#"
local config = { ecs = true }
config.ecs = false
local world = ecs.world()
"#,
    )
    .expect("ECS field assignments should emit");

    assert!(emitted.starts_with("local ecs = require(\"tuaecs\")\n"));
}

#[test]
fn snapshot_injects_ecs_when_an_explicit_binding_appears_after_builtin_usage() {
    let emitted = emit_tua_source(
        "src/ecs.tua",
        r#"
local builtInWorld = ecs.world()
local ecs = require("project_ecs")
local projectWorld = ecs.world()
"#,
    )
    .expect("later ECS binding should not affect earlier built-in usage");

    assert!(emitted.starts_with("local ecs = require(\"tuaecs\")\n"));
}

#[test]
fn snapshot_does_not_lower_love_asset_loader_names_in_strings_or_comments() {
    let emitted = emit_tua_source(
        "src/assets.tua",
        r#"
-- image "assets/player.png"
local text = "sound \"assets/jump.wav\""
"#,
    )
    .expect("comments and strings should emit");

    assert_eq!(
        emitted,
        r#"-- image "assets/player.png"
local text = "sound \"assets/jump.wav\""
"#
    );
}

#[test]
fn unsupported_compound_assignment_lhs_emits_lowering_diagnostic() {
    let diagnostics = emit_tua_source("src/compound.tua", "player[\"hp\"] += 1\n")
        .expect_err("computed-index compound assignment should not lower");

    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::LOWERING_COMPOUND_ASSIGNMENT)
    );
    assert_eq!(
        diagnostics[0].code.unwrap().category(),
        DiagnosticCategory::LoweringEmission
    );
    assert!(diagnostics[0].format_cli().contains("error TL2001"));
    assert!(diagnostics[0].message.contains("left-hand side"));
}

#[test]
fn call_compound_assignment_lhs_emits_lowering_diagnostic() {
    let diagnostics = emit_tua_source("src/compound.tua", "get_player().hp += 1\n")
        .expect_err("call-derived compound assignment should not lower");

    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::LOWERING_COMPOUND_ASSIGNMENT)
    );
    assert!(diagnostics[0].message.contains("left-hand side"));
}

#[test]
fn mixed_lowering_pipeline_emits_stable_lua_with_source_provenance() {
    let emission = emit_tua_source_artifact(
        "src/mixed.tua",
        r#"type Count = number

local function tick(value: Count = 1)
  local shared = import("shared")
  local position = vec2(1, 2)
  local events = signal()
  local icon = image "assets/icon.png"
  value += 2
  if value is Count then
    events:emit("tick", position, icon)
  end
  return shared, value
end
"#,
    )
    .expect("mixed lowering should emit");

    assert_eq!(
        emission.code,
        r#"local __tua_signal = require("tuasignal")
local __tua_vec2 = require("brinevector")
local function tick(value)
  if value == nil then
    value = 1
  end
  local shared = require("shared")
  local position = __tua_vec2(1, 2)
  local events = __tua_signal()
  local icon = love.graphics.newImage("assets/icon.png")
  value = value + 2
  if type(value) == "number" then
    events:emit("tick", position, icon)
  end
  return shared, value
end
"#
    );

    let provenance_for = |needle: &str| {
        let line = emission
            .code
            .lines()
            .position(|line| line.trim() == needle)
            .expect("generated line should exist");
        emission.provenance[line]
    };

    assert_eq!(
        provenance_for("local __tua_signal = require(\"tuasignal\")"),
        LuaLineProvenance {
            generated_line: 1,
            source_span: SourceSpan {
                start: SourcePosition { line: 6, column: 1 },
                end: SourcePosition {
                    line: 6,
                    column: 26,
                },
            },
            lowering: LoweringKind::BuiltinCall,
        }
    );
    assert_eq!(
        provenance_for("if value == nil then").lowering,
        LoweringKind::DefaultParameter
    );
    assert_eq!(
        provenance_for("if value == nil then")
            .source_span
            .start
            .line,
        3
    );
    assert_eq!(
        provenance_for("local shared = require(\"shared\")").lowering,
        LoweringKind::Import
    );
    assert_eq!(
        provenance_for("value = value + 2").lowering,
        LoweringKind::CompoundAssignment
    );
    assert_eq!(
        provenance_for("if type(value) == \"number\" then").lowering,
        LoweringKind::RuntimeTypeCheck
    );
}

#[test]
fn multiline_types_and_non_ascii_defaults_keep_operation_provenance() {
    let emission = emit_tua_source_artifact(
        "src/non_ascii.tua",
        r#"type Message = {
  text: string,
}

local function greet(name: string = "Olá")
  local count = 1
  count += 1
  return name, count
end
"#,
    )
    .expect("multiline non-ASCII fixture should emit");

    assert_eq!(
        emission.code,
        r#"local function greet(name)
  if name == nil then
    name = "Olá"
  end
  local count = 1
  count = count + 1
  return name, count
end
"#
    );
    assert_eq!(emission.provenance[0].source_span.start.line, 5);
    assert!(emission.provenance[1..4]
        .iter()
        .all(|line| line.lowering == LoweringKind::DefaultParameter));
    assert!(emission.provenance[1..4]
        .iter()
        .all(|line| line.source_span.start.line == 5));
    assert_eq!(
        emission.provenance[5],
        LuaLineProvenance {
            generated_line: 6,
            source_span: SourceSpan {
                start: SourcePosition { line: 7, column: 1 },
                end: SourcePosition {
                    line: 7,
                    column: 13,
                },
            },
            lowering: LoweringKind::CompoundAssignment,
        }
    );
}

#[test]
fn class_and_sprite_expansions_keep_declaration_provenance() {
    let emission = emit_tua_source_artifact(
        "src/domain.tua",
        r#"final class Player {
  hp: number = 10
}

local afterClass = 1

local Sheet = spriteSheet({
  image = "assets/player.png",
  frame = { width = 16, height = 16 },
  animations = {
    idle = { from = 0, to = 3, fps = 8 },
  },
})
"#,
    )
    .expect("domain lowering should emit");

    let provenance_for = |needle: &str| {
        let line = emission
            .code
            .lines()
            .position(|line| line.trim() == needle)
            .expect("generated line should exist");
        emission.provenance[line]
    };

    let class_line = provenance_for("local Player = {}");
    assert_eq!(class_line.lowering, LoweringKind::Class);
    assert_eq!(class_line.source_span.start.line, 1);

    let after_class = provenance_for("local afterClass = 1");
    assert_eq!(after_class.lowering, LoweringKind::Source);
    assert_eq!(after_class.source_span.start.line, 5);

    let sprite_comment = provenance_for("-- Sprite helper: Sheet lowers to anim8-backed Lua.");
    let sprite_runtime = provenance_for("local anim8 = require(\"anim8\")");
    assert_eq!(sprite_comment.lowering, LoweringKind::Sprite);
    assert_eq!(sprite_runtime.lowering, LoweringKind::Sprite);
    assert_eq!(sprite_comment.source_span.start.line, 7);
    assert_eq!(sprite_runtime.source_span, sprite_comment.source_span);
}

#[test]
fn class_normalization_preserves_following_multiline_and_non_ascii_provenance() {
    let emission = emit_tua_source_artifact(
        "src/class_following.tua",
        r#"final class Player {
  hp: number = 10
}

type Message = {
  text: string,
}

local greeting: string = "Olá"
local count = 1
count += 1
"#,
    )
    .expect("class and following typed source should emit");

    let provenance_for = |needle: &str| {
        let line = emission
            .code
            .lines()
            .position(|line| line.trim() == needle)
            .expect("generated line should exist");
        emission.provenance[line]
    };

    let class_line = provenance_for("local Player = {}");
    assert_eq!(class_line.lowering, LoweringKind::Class);
    assert_eq!(class_line.source_span.start.line, 1);
    assert_eq!(class_line.source_span.end.line, 3);

    let greeting = provenance_for("local greeting = \"Olá\"");
    assert_eq!(greeting.lowering, LoweringKind::Source);
    assert_eq!(greeting.source_span.start.line, 9);

    let compound = provenance_for("count = count + 1");
    assert_eq!(compound.lowering, LoweringKind::CompoundAssignment);
    assert_eq!(compound.source_span.start.line, 11);
}

#[test]
fn luacats_operations_keep_declaration_provenance() {
    let emission = emit_tua_source_artifact_with_mode(
        "src/annotated_module.tua",
        r#"type Player = {
  name: string,
}

local score: number = 0

local function heal(player: Player, amount: number): Player
  return player
end

return { score = score, heal = heal }
"#,
        AnnotationMode::Luacats,
    )
    .expect("LuaCats annotations should emit");

    let provenance_for = |needle: &str| {
        let line = emission
            .code
            .lines()
            .position(|line| line.trim() == needle)
            .expect("annotation line should exist");
        emission.provenance[line]
    };

    let alias = provenance_for("---@alias Player { name: string }");
    assert_eq!(alias.lowering, LoweringKind::Annotation);
    assert_eq!(alias.source_span.start.line, 1);
    assert_eq!(alias.source_span.end.line, 3);

    let local = provenance_for("---@type number");
    assert_eq!(local.lowering, LoweringKind::Annotation);
    assert_eq!(local.source_span.start.line, 5);

    let parameter = provenance_for("---@param player Player");
    assert_eq!(parameter.lowering, LoweringKind::Annotation);
    assert_eq!(parameter.source_span.start.line, 7);

    let exports = provenance_for("---@class AnnotatedModuleExports");
    assert_eq!(exports.lowering, LoweringKind::Annotation);
    assert_eq!(exports.source_span.start.line, 11);
}

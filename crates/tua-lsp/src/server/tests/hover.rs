use super::*;

#[test]
fn hover_returns_known_alias_local_function_and_table_field_types() {
    let uri = "file:///tmp/hover.tua";
    let source = r#"type Vec2 = { x: number, y: number }
type Player = { pos: Vec2, hp: number }

local player: Player = {
  pos = { x = 0, y = 0 },
  hp = 10,
}

local function damage(target: Player, amount: number): Player
  target.hp -= amount
  return target
end

damage(player, 2)
player.hp = player.hp - 1
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        hover_request(2, uri, source, "Vec2"),
        hover_request(3, uri, source, "player:"),
        hover_request(4, uri, source, "damage(player"),
        hover_request(5, uri, source, "hp = player"),
        hover_request(6, uri, source, "target.hp"),
    ]);

    assert_hover_contains(&output, 2, "type Vec2 = { x: number, y: number }");
    assert_hover_contains(&output, 3, "local player: Player");
    assert_hover_contains(
        &output,
        4,
        "function damage(target: Player, amount: number): Player",
    );
    assert_hover_contains(&output, 5, "(field) hp: number");
    assert_hover_contains(&output, 6, "parameter target: Player");
}

#[test]
fn hover_displays_generic_function_binders() {
    let uri = "file:///tmp/hover-generic-function.tua";
    let source = r#"local function identity<T>(value: T): T return value end
identity("text")
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        hover_request(2, uri, source, "identity<T>"),
    ]);

    assert_hover_contains(&output, 2, "function identity<T>(value: T): T");
}

#[test]
fn hover_preserves_pretty_printed_multiline_types_in_markdown() {
    let uri = "file:///tmp/pretty-hover.tua";
    let source = r#"type Feedback = {
  animation: string,
  callback: (event: string, previous: number, state: boolean) -> boolean,
  duration: number,
  event: string,
  payload: { channel: string, cue: string, volume: number },
  restart: boolean,
  target: string,
}
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        hover_request(2, uri, source, "Feedback"),
    ]);
    let contents = hover_contents(&output, 2);

    assert!(contents.starts_with("```tua\ntype Feedback = {\n"));
    assert!(contents.contains("\n  callback: "));
    assert!(contents.contains("\n  payload: "));
    assert!(contents.contains("\n}\n```"));
}

#[test]
fn hover_returns_inferred_function_return_type() {
    let uri = "file:///tmp/inferred-return-hover.tua";
    let source = r#"function test(a: boolean)
  return ""
end

test(false)
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        hover_request(2, uri, source, "test(a"),
        hover_request(3, uri, source, "test(false"),
    ]);

    assert_hover_contains(&output, 2, "function test(a: boolean): string");
    assert_hover_contains(&output, 3, "function test(a: boolean): string");
}

#[test]
fn hover_resolves_indexed_assignment_parts_without_comparison_fallbacks() {
    let uri = "file:///tmp/indexed-assignment-hover.tua";
    let source = r#"local actors: number[] = {}

actors[1] = 'a'
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        hover_request(2, uri, source, "actors[1]"),
        hover_request(3, uri, source, "1] ="),
        hover_request(4, uri, source, "'a'"),
        hover_request(5, uri, source, "= 'a'"),
    ]);

    assert_hover_contains(&output, 2, "local actors: number[]");
    assert_hover_contains(&output, 3, "expression: number");
    assert_hover_contains(&output, 4, "expression: string");
    assert!(response_with_id(&output, 5)["result"].is_null());
}

#[test]
fn hover_renders_docs_and_declaration_context_as_markdown() {
    let uri = "file:///tmp/documented-hover.tua";
    let source = r#"type Player = { hp: number }

--- Applies damage.
---@param target Player affected player
---@return Player updated player
local function damage(target)
  return target
end

damage({ hp = 10 })
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        hover_request(2, uri, source, "damage({"),
    ]);
    let contents = hover_contents(&output, 2);

    assert!(contents.starts_with("```tua\nfunction damage(target: Player): Player\n```"));
    assert!(contents.contains("Applies damage.\naffected player\nupdated player"));
    assert!(contents.contains("- kind: function"));
    assert!(contents.contains(&format!(
        "- declared: {}:",
        tua_core::path_from_file_uri(uri).display()
    )));
}
#[test]
fn hover_unknown_symbol_returns_null_result() {
    let uri = "file:///tmp/unknown-hover.tua";
    let source = "local result = missing\n";
    let output = run_messages(vec![
        did_open(uri, source),
        hover_request(2, uri, source, "missing"),
    ]);

    let response = response_with_id(&output, 2);

    assert!(response["result"].is_null());
    assert!(response.get("error").is_none());
}
#[test]
fn hover_uses_current_file_fast_path_before_project_snapshot() {
    let temp = TempProject::new("hover-current-file-fast-path");
    let uri = temp.uri_for("src/main.tua");
    let source = "local outside_function = 1\nprint(outside_function)\n";
    let mut server = TestServer::new();

    temp.write("tua.toml", "source = \"src\"\n");
    for index in 0..12 {
        let path = format!("src/other_{index}.tua");
        let contents = format!("type Other{index} = {{ value: number }}\n");

        temp.write(&path, &contents);
    }
    server.send(initialize_with_pull_diagnostics(1, temp.uri(), false));
    server.send(did_open(&uri, source));
    server.send(cache_stats_request(2));
    server.send(hover_request(3, &uri, source, "outside_function"));
    server.send(cache_stats_request(4));

    assert_hover_contains(server.output(), 3, "local outside_function: number");

    let before = &response_with_id(server.output(), 2)["result"];
    let after = &response_with_id(server.output(), 4)["result"];

    assert_eq!(
        after["parse"]["misses"].as_u64(),
        Some(before["parse"]["misses"].as_u64().unwrap_or_default() + 1),
        "current-file hover should parse only the active document, not every project file"
    );
}
#[test]
fn hover_returns_lua_and_love_items() {
    let uri = "file:///tmp/lua-hover.tua";
    let source = r#"local value = math.sin(1)
love.graphics.print("hi", 0, 0)
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        hover_request(2, uri, source, "sin"),
        hover_request(3, uri, source, "print"),
    ]);

    assert_hover_contains(&output, 2, "function math.sin(x: number): number");
    assert_hover_contains(
        &output,
        3,
        "function love.graphics.print(text: string, x?: number, y?: number",
    );
    assert_hover_contains(&output, 3, "Draws text on screen");
    assert_hover_contains(
        &output,
        3,
        "[Open LOVE wiki](https://love2d.org/wiki/love.graphics.print)",
    );
}

#[test]
fn hover_returns_typed_signal_event_schema() {
    let uri = "file:///tmp/signal-hover.tua";
    let source = r#"local events = signal()
events:declare("player.hit", function(player: string, damage: number) end)
events:emit("player.hit", "Ada", 12)
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        hover_request(2, uri, source, "\"player.hit\", \"Ada\""),
    ]);
    let contents = hover_contents(&output, 2);
    let response = response_with_id(&output, 2);

    assert!(contents.contains("SignalBus event"));
    assert!(contents.contains("player: string, damage: number"));
    assert_eq!(
        range_text(source, &response["result"]["range"]),
        "player.hit"
    );
}
#[test]
fn hover_returns_required_module_bindings_and_exports() {
    let temp = TempProject::new("required-module-hover");
    let source = r#"local Player = require("player")
local Legacy = require("legacy")

local player = Player.makePlayer("Ada")
local hp = Player.defaultHp
local label = Legacy.describe("hp", hp)
"#;
    let player = r#"type Player = { name: string, hp: number }

local function makePlayer(name: string): Player
  return { name = name, hp = 10 }
end

return {
  makePlayer = makePlayer,
  defaultHp = 10,
}
"#;
    let legacy = r#"return {
  describe = function(label, value)
    return label
  end,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/player.tua", player);
    temp.write("src/legacy.lua", legacy);

    let uri = temp.uri_for("src/main.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
        hover_request(2, &uri, source, "player\")"),
        hover_request(3, &uri, source, "Player.makePlayer"),
        hover_request(4, &uri, source, "makePlayer("),
        hover_request(5, &uri, source, "defaultHp"),
        hover_request(6, &uri, source, "describe("),
    ]);

    assert_hover_contains(&output, 2, "(module) player ->");
    assert_hover_contains(&output, 3, "(module) Player = require(\"player\") ->");
    assert_hover_contains(
        &output,
        4,
        "function Player.makePlayer(name: string): Player",
    );
    assert_hover_contains(&output, 5, "(field) Player.defaultHp: number");
    assert_hover_contains(
        &output,
        6,
        "function Legacy.describe(label: any, value: any): any",
    );
}

#[test]
fn imported_generic_function_hover_and_signature_keep_the_scheme() {
    let temp = TempProject::new("imported-generic-function-editor");
    let source = r#"local Generic = require("generic")
local generic_text = Generic.identity("ready")
"#;
    let generic = r#"local function identity<T>(value: T): T
  return value
end
return { identity = identity }
"#;
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/generic.tua", generic);

    let parsed_generic = tua_core::parse_tua_source("src/generic.tua", generic).unwrap();
    let exports = tua_core::module_index::infer_module_exports(&parsed_generic)
        .expect("generic module should export its returned table");
    let Some(tua_core::Type::Function(exported_identity)) =
        exports.fields.get("identity").map(|field| &field.ty)
    else {
        panic!("identity export should retain its function type: {exports:#?}");
    };
    assert_eq!(
        exported_identity.generic_parameters.len(),
        1,
        "{exports:#?}"
    );

    let uri = temp.uri_for("src/main.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
        hover_request(2, &uri, source, "identity(\"ready\""),
        signature_help_request(3, &uri, source, "identity(\"ready\""),
    ]);

    assert_hover_contains(&output, 2, "function Generic.identity<T>(value: T): T");
    assert_signature_label(
        &output,
        3,
        "function Generic.identity(value: string): string",
    );
}

#[test]
fn imported_luadoc_generic_function_editor_surfaces_preserve_instantiation() {
    let temp = TempProject::new("imported-luadoc-generic-function-editor");
    let source = r#"local Generic = require("generic")
local generic_text = Generic.identity("ready")
local generic_record = Generic.identity({ label = "ready" })
generic_record.
"#;
    let generic = r#"---@generic T
---@param value T
---@return T
local function identity(value)
  return value
end
return { identity = identity }
"#;
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/generic.lua", generic);

    let uri = temp.uri_for("src/main.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
        hover_request(2, &uri, source, "identity(\"ready\""),
        signature_help_request(3, &uri, source, "identity(\"ready\""),
        hover_request(4, &uri, source, "generic_text = Generic.identity"),
        definition_request_at(
            5,
            &uri,
            tua_core::SourcePosition {
                line: 2,
                column: 31,
            },
        ),
        completion_request(6, &uri, source, "generic_record."),
    ]);

    assert_hover_contains(&output, 2, "function Generic.identity<T>(value: T): T");
    assert_signature_label(
        &output,
        3,
        "function Generic.identity(value: string): string",
    );
    assert_hover_contains(&output, 4, "local generic_text: string");
    assert_eq!(
        response_with_id(&output, 5)["result"]["uri"],
        temp.uri_for("src/generic.lua")
    );
    assert!(
        completion_labels(&output, 6).contains(&"label"),
        "the imported LuaDoc generic result should preserve its record members"
    );
}

#[test]
fn hover_preserves_inferred_ecs_types_from_required_module_exports() {
    let temp = TempProject::new("required-ecs-module-hover");
    let source = r#"local ECS = require("ecs_state")
local world = ECS.world
local velocity = world:get(ECS.player, ECS.VelocityComponent)
"#;
    let ecs_state = r#"local VelocityComponent = ecs.component("Velocity", { x = "number", y = "number" })
local world = ecs.world()
local player = world:create()

return {
  player = player,
  VelocityComponent = VelocityComponent,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n[ecs]\nenabled = true\n");
    temp.write("src/main.tua", source);
    temp.write("src/ecs_state.tua", ecs_state);

    let uri = temp.uri_for("src/main.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
        hover_request(2, &uri, source, "player,"),
        hover_request(3, &uri, source, "VelocityComponent)"),
    ]);

    assert_hover_contains(&output, 2, "(field) ECS.player: EcsEntityId");
    assert_hover_contains(&output, 3, "(field) ECS.VelocityComponent: EcsComponent");
}

#[test]
fn hover_preserves_project_inference_for_locals_copied_from_module_exports() {
    let temp = TempProject::new("required-state-machine-local-hover");
    let source = r#"local Globals = require("global")
local flow = Globals.flow
flow:pause()
"#;
    let global = r#"import type Shared from "./types"

local bullets = objectPool({
  create = function(): Shared.Bullet
    return { x = 0, y = 0, dx = 0, dy = 0, active = false }
  end,
  reset = function(bullet: Shared.Bullet)
    bullet.x = 0
    bullet.y = 0
    bullet.dx = 0
    bullet.dy = 0
    bullet.active = false
  end,
  initial = 48,
})
local activeBullets: Shared.Bullet[] = {}
local game = store({ score = 0, shots = 0, wave = 1 })
local flow = stateMachine({
  initial = "playing",
  events = {
    { name = "pause", from = "playing", to = "paused" },
    { name = "resume", from = "paused", to = "playing" },
    { name = "finish", from = { "playing", "paused" }, to = "gameover" },
    { name = "restart", from = "gameover", to = "playing" },
  },
})

return {
  game = game,
  flow = flow,
  bullets = bullets,
  activeBullets = activeBullets,
}
"#;
    let types = r#"export type Bullet = {
  x: number,
  y: number,
  dx: number,
  dy: number,
  active: boolean,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/global.tua", global);
    temp.write("src/types.tua", types);

    let uri = temp.uri_for("src/main.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
        hover_request(2, &uri, source, "flow ="),
        hover_request(3, &uri, source, "flow\nflow:pause"),
    ]);

    assert_hover_contains(&output, 3, "(field) Globals.flow: {");
    assert_hover_contains(&output, 2, "local flow:");
    assert_hover_contains(
        &output,
        2,
        "Events: `\"finish\" | \"pause\" | \"restart\" | \"resume\"`",
    );
    assert!(!hover_contents(&output, 2).contains("local flow: any"));
}

#[test]
fn hover_renders_module_export_docs_and_module_path() {
    let temp = TempProject::new("documented-module-hover");
    let source = r#"local Audio = require("audio")
local ok = Audio.play("theme")
"#;
    let audio = r#"--- Plays a source.
---@param source string audio source
---@return boolean success flag
local function play(source)
  return true
end

return {
  play = play,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/audio.lua", audio);

    let uri = temp.uri_for("src/main.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
        hover_request(2, &uri, source, "play("),
    ]);
    let contents = hover_contents(&output, 2);

    assert!(contents.contains("```tua\nfunction Audio.play(source: string): boolean\n```"));
    assert!(contents.contains("Plays a source.\naudio source\nsuccess flag"));
    assert!(contents.contains("- kind: function"));
    assert!(contents.contains("- module:"));
    assert!(contents.contains("audio.lua"));
}
#[test]
fn hover_and_diagnostics_use_project_type_database_after_edit() {
    let temp = TempProject::new("project-type-hover-after-edit");
    let source = r#"local Types = require("types")
local LuaDocTypes = require("lua_doc_types")

-- Feature: project-aware type database resolves aliases through require aliases.
local playerFromTuaModule: Types.Player = nil

-- Feature: Lua files with LuaDoc annotations can provide types to Tua files.
local enemyFromLuaDocModule: LuaDocTypes.Enemy = nil
"#;
    let edited = source.replace(
        "nil\n\n-- Feature",
        "nil\n\nlocal touched = true\n\n-- Feature",
    );
    let types = r#"export type Player = { hp: number }

return {}
"#;
    let lua_doc_types = r#"---@class Enemy
---@field hp number
---@field name string
---@field type 'friendly' | 'mad'
local Enemy = {}

return {
  Enemy = Enemy,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/types.tua", types);
    temp.write("src/lua_doc_types.lua", lua_doc_types);

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    server.send(did_change(&uri, &edited, 2));
    server.send(hover_request(3, &uri, &edited, "Player = nil"));
    server.send(hover_request(4, &uri, &edited, "Enemy = nil"));
    server.send(hover_request(
        5,
        &uri,
        &edited,
        "enemyFromLuaDocModule: LuaDocTypes",
    ));

    assert_eq!(last_published_diagnostics(server.output(), &uri).len(), 0);
    assert_hover_contains(server.output(), 3, "type Types.Player = { hp: number }");
    assert_hover_contains(
        server.output(),
        4,
        "type LuaDocTypes.Enemy = { hp: number, name: string, type: \"friendly\" | \"mad\" }",
    );
    assert_hover_contains(
        server.output(),
        5,
        "local enemyFromLuaDocModule: {\n  hp: number,\n  name: string,\n  type: \"friendly\" | \"mad\"\n}",
    );
}

#[test]
fn imported_event_parameter_hover_reaches_project_inference() {
    let temp = TempProject::new("signal-parameter-project-hover");
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/events.tua", "return { enemies = signal() }\n");
    temp.write(
        "src/combat.tua",
        r#"local Events = require "events"
local EnemyEvents = Events.enemies
EnemyEvents:emit("hit", vec2(1, 2))
"#,
    );
    let source = r#"local Events = require "events"
local EnemyEvents = Events.enemies
EnemyEvents:on("hit", function(position)
  print(position.x)
end)
"#;
    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();
    server.send(initialize_with_pull_diagnostics(1, temp.uri(), false));
    server.send(did_open(&uri, source));
    server.send(hover_request(2, &uri, source, "position"));
    assert_hover_contains(server.output(), 2, "parameter position: Vec2");
}

#[test]
fn imported_ecs_component_result_hover_keeps_schema_in_project() {
    let temp = TempProject::new("imported-ecs-component-result-hover");
    temp.write("tua.toml", "source = \"src\"\nout = \"build\"\n");
    temp.write(
        "src/components.tua",
        "local Position = ecs.component(\"Position\", { x = \"number\" })\nreturn { Position = Position }\n",
    );
    let source = r#"local C = require "components"
local world = ecs.world()
local entity = world:create()
local position = world:get(entity, C.Position)
print(position)
local unrelated: any = nil
"#;
    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();
    server.send(initialize_with_pull_diagnostics(1, temp.uri(), false));
    server.send(did_open(&uri, source));
    server.send(hover_request(2, &uri, source, "position)"));
    assert_hover_contains(server.output(), 2, "local position: { x: number } | nil");
    server.send(hover_request(3, &uri, source, "unrelated:"));
    assert_hover_contains(server.output(), 3, "local unrelated: any");
}

#[test]
fn imported_class_method_hover_reads_docs_from_the_project_snapshot() {
    let temp = TempProject::new("imported-class-method-docs");
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write(
        "src/dialog.tua",
        r#"final class Dialogue {
  --- Append words.
  ---@param word string A word.
  append(word: string): boolean { return true }
}
return Dialogue
"#,
    );
    let source =
        "local Dialogue = require(\"dialog\")\nlocal d = Dialogue.new()\nd:append(\"hello\")\n";
    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();
    server.send(initialize_with_pull_diagnostics(1, temp.uri(), false));
    server.send(did_open(&uri, source));
    server.send(hover_request(2, &uri, source, "append"));
    assert_hover_contains(server.output(), 2, "Append words.");
    assert_hover_contains(server.output(), 2, "**@param** word string A word.");
    server.send(definition_request(3, &uri, source, "append", 0));
    assert_eq!(
        response_with_id(server.output(), 3)["result"]["uri"],
        temp.uri_for("src/dialog.tua")
    );
}

#[test]
fn imported_no_return_class_method_has_nil_completion_hover_and_definition() {
    let temp = TempProject::new("imported-no-return-method");
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write(
        "src/dialog.tua",
        r#"open class Dialogue {
  insert(key: string) {
    print(key)
  }
}
export type DialogueInstance = Dialogue
return { Dialogue = Dialogue }
"#,
    );
    let source = r#"import type { DialogueInstance } from "dialog"
local DialogueModule = require("dialog")
local dialogue: DialogueInstance = DialogueModule.Dialogue.new()
dialogue:insert("Quack")
"#;
    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();
    server.send(initialize_with_pull_diagnostics(1, temp.uri(), false));
    server.send(did_open(&uri, source));
    server.send(completion_request(2, &uri, source, "dialogue:in"));
    assert_completion_labels(server.output(), 2, ["insert"]);
    server.send(hover_request(3, &uri, source, "insert(\"Quack\""));
    assert_hover_contains(server.output(), 3, "(method) insert: (key: string) -> nil");
    server.send(definition_request(4, &uri, source, "insert(\"Quack\"", 0));
    assert_eq!(
        response_with_id(server.output(), 4)["result"]["uri"],
        temp.uri_for("src/dialog.tua")
    );
}

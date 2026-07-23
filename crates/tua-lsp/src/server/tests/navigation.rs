use super::*;

#[test]
fn definition_resolves_metatable_method_and_self_field_through_semantic_index() {
    let uri = "file:///tmp/metatable-definition.tua";
    let source = r#"type Account = {
  name: string,
  balance: number,
}

local Account = {}
Account.__index = Account

function Account:new(name, balance)
  local instance = {
    name = name,
    balance = balance,
  }

  return setmetatable(instance, self)
end

function Account:deposit(amount: number): number
  self.balance = self.balance + amount

  return self.balance
end

local account = setmetatable({ name = "checking", balance = 50 }, Account)
account:deposit(25)
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        definition_request(2, uri, source, "deposit(25)", 0),
        definition_request(3, uri, source, "self.balance", "self.".len()),
        definition_request(4, uri, source, "self.balance", "self.".len()),
    ]);

    assert_definition_text(&output, 2, uri, source, "Account:deposit");
    assert_definition_text(&output, 3, uri, source, "balance");
    assert_eq!(
        response_with_id(&output, 3)["result"]["range"]["start"]["line"],
        json!(2)
    );
    assert_eq!(
        response_with_id(&output, 3)["result"],
        response_with_id(&output, 4)["result"]
    );
}

#[test]
fn class_symbols_support_definition_references_and_rename() {
    let uri = "file:///tmp/class-navigation.tua";
    let source = r#"open class Entity {
  health: number = 10

  damage(amount: number) {
    self.health -= amount
  }
}

local entity = Entity.new()
local typed: Entity = entity
print(entity.health)
entity:damage(1)
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        definition_request(2, uri, source, "entity.health", "entity.".len()),
        references_request(3, uri, source, "entity.health", "entity.".len(), true),
        prepare_rename_request(4, uri, source, "entity.health", "entity.".len()),
        rename_request(
            5,
            uri,
            source,
            "entity.health",
            "entity.".len(),
            "hitPoints",
        ),
        references_request(6, uri, source, "Entity.new", 0, true),
        references_request(7, uri, source, "entity:damage", "entity:".len(), true),
    ]);

    assert_definition_text(&output, 2, uri, source, "health");
    assert_reference_texts(&output, 3, source, ["health", "health", "health"]);
    assert_eq!(
        response_with_id(&output, 4)["result"]["placeholder"],
        "health"
    );
    assert_rename_edit_texts(
        &output,
        5,
        uri,
        source,
        ["health", "health", "health"],
        "hitPoints",
    );
    assert_reference_texts(&output, 6, source, ["Entity", "Entity", "Entity"]);
    assert_reference_texts(&output, 7, source, ["damage", "damage"]);
}

#[test]
fn signal_graph_returns_typed_calls_and_source_locations() {
    let uri = "file:///tmp/signal-graph.tua";
    let source = r#"local events = signal()
events:declare("player.hit", function(player: string, damage: number) end)
events:on("player.hit", function(player: string, damage: number) end)
events:emit("player.hit", "Ada", 12)
"#;
    let output = run_messages(vec![did_open(uri, source), signal_graph_request(2)]);
    let response = response_with_id(&output, 2);
    let event = &response["result"]["events"][0];

    assert_eq!(event["name"], "player.hit");
    assert!(event["id"].as_str().is_some_and(|id| !id.is_empty()));
    assert!(event["bus"]
        .as_str()
        .is_some_and(|bus| bus.contains("events")));
    assert_eq!(event["busUri"], uri);
    assert_eq!(event["schemaConflict"], false);
    assert_eq!(event["resolvedPayload"][0]["name"], "player");
    assert_eq!(event["resolvedPayload"][0]["type"], "string");
    assert_eq!(event["resolvedPayload"][1]["name"], "damage");
    assert_eq!(event["resolvedPayload"][1]["type"], "number");
    assert_eq!(event["calls"].as_array().map(Vec::len), Some(3));
    assert_eq!(event["calls"][0]["role"], "declaration");
    assert_eq!(event["calls"][0]["payload"][0]["name"], "player");
    assert_eq!(event["calls"][0]["payload"][0]["type"], "string");
    assert_eq!(event["calls"][2]["role"], "emit");
    assert_eq!(event["calls"][2]["uri"], uri);
    assert_eq!(event["calls"][2]["range"]["start"]["line"], 3);
    assert_eq!(event["calls"][2]["callRange"]["start"]["line"], 3);
    assert_eq!(event["calls"][2]["insertionPosition"]["line"], 3);
    assert_eq!(event["calls"][2]["insertionPosition"]["character"], 0);
    assert_eq!(event["calls"][2]["inferredPayload"][0]["name"], "value1");
    assert_eq!(event["calls"][2]["inferredPayload"][0]["type"], "string");
    assert_eq!(event["calls"][2]["inferredPayload"][1]["name"], "value2");
    assert_eq!(event["calls"][2]["inferredPayload"][1]["type"], "number");
}

#[test]
fn signal_graph_reports_conflicting_payload_schemas() {
    let uri = "file:///tmp/signal-graph-conflict.tua";
    let source = r#"local events = signal()
events:declare("player.hit", function(player: string) end)
events:on("player.hit", function(player: number) end)
"#;
    let output = run_messages(vec![did_open(uri, source), signal_graph_request(2)]);
    let response = response_with_id(&output, 2);
    let event = &response["result"]["events"][0];

    assert_eq!(event["name"], "player.hit");
    assert_eq!(event["schemaConflict"], true);
    assert!(event["resolvedPayload"].is_null());
    assert_eq!(event["calls"][0]["payload"][0]["type"], "string");
    assert_eq!(event["calls"][1]["payload"][0]["type"], "number");
    assert_eq!(event["calls"][0]["insertionPosition"]["line"], 1);
    assert_eq!(event["calls"][0]["insertionPosition"]["character"], 0);
}

#[test]
fn signal_event_definition_references_and_rename_share_bus_identity() {
    let uri = "file:///tmp/signal-navigation.tua";
    let source = r#"local events = signal()
local ui = signal()
events:declare("player.hit", function(player: string, damage: number) end)
events:emit("player.hit", "Ada", 12)
ui:declare("player.hit", function(message: string) end)
"#;
    let emit = "events:emit(\"player.hit";
    let event_offset = "events:emit(\"".len();
    let output = run_messages(vec![
        did_open(uri, source),
        definition_request(2, uri, source, emit, event_offset),
        references_request(3, uri, source, emit, event_offset, true),
        prepare_rename_request(4, uri, source, emit, event_offset),
        rename_request(5, uri, source, emit, event_offset, "player.damaged"),
    ]);
    let definition = response_with_id(&output, 2);
    let references = response_with_id(&output, 3);
    let prepare = response_with_id(&output, 4);

    assert_eq!(definition["result"]["uri"], uri);
    assert_eq!(
        range_text(source, &definition["result"]["range"]),
        "player.hit"
    );
    assert_eq!(references["result"].as_array().map(Vec::len), Some(2));
    assert!(references["result"]
        .as_array()
        .expect("references should be an array")
        .iter()
        .all(|reference| range_text(source, &reference["range"]) == "player.hit"));
    assert_eq!(prepare["result"]["placeholder"], "player.hit");
    assert_eq!(
        range_text(source, &prepare["result"]["range"]),
        "player.hit"
    );
    assert_rename_edit_texts(
        &output,
        5,
        uri,
        source,
        ["player.hit", "player.hit"],
        "player.damaged",
    );
}

#[test]
fn definition_returns_known_alias_function_local_and_table_field_locations() {
    let uri = "file:///tmp/definition.tua";
    let source = r#"type Vec2 = { x: number, y: number }
type Player = { pos: Vec2, hp: number, alive?: boolean }

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
        definition_request(2, uri, source, "player: Player", "player: ".len()),
        definition_request(3, uri, source, "damage(player", 0),
        definition_request(4, uri, source, "damage(player", "damage(".len()),
        definition_request(5, uri, source, "player.hp =", "player.".len()),
    ]);

    assert_definition_text(&output, 2, uri, source, "Player");
    assert_definition_text(&output, 3, uri, source, "damage");
    assert_definition_text(&output, 4, uri, source, "player");
    assert_definition_text(&output, 5, uri, source, "hp");
}
#[test]
fn definition_and_references_resolve_inferred_local_table_fields() {
    let uri = "file:///tmp/inferred-table-field.tua";
    let source = r#"local lookupObject = {
  count = 0,
  y = function(self: any, amount: number): number
    self.count = self.count + amount

    return self.count
  end,
}

lookupObject.count = 1
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        definition_request(
            2,
            uri,
            source,
            "lookupObject.count = 1",
            "lookupObject.".len(),
        ),
        references_request(
            3,
            uri,
            source,
            "lookupObject.count = 1",
            "lookupObject.".len(),
            true,
        ),
    ]);

    assert_definition_text(&output, 2, uri, source, "count");
    assert_reference_texts(&output, 3, source, ["count", "count"]);
}

#[test]
fn definition_and_references_resolve_asset_paths() {
    let project = TempProject::new("asset-navigation");
    let main = r#"local player = image "assets/player.png"
"#;
    let ui = r#"local portrait = love.graphics.newImage("assets/player.png")
"#;

    project.write("tua.toml", "source = \".\"\n");
    project.write("main.tua", main);
    project.write("ui.lua", ui);
    project.write("assets/player.png", "");

    let main_uri = project.uri_for("main.tua");
    let ui_uri = project.uri_for("ui.lua");
    let asset_uri = project.uri_for("assets/player.png");
    let output = run_messages(vec![
        initialize_with_root(1, project.uri()),
        did_open(&main_uri, main),
        definition_request(2, &main_uri, main, "assets/player.png", 0),
        references_request(3, &main_uri, main, "assets/player.png", 0, true),
    ]);
    let definition = &response_with_id(&output, 2)["result"];
    let references = response_with_id(&output, 3)["result"]
        .as_array()
        .expect("asset references should be an array");

    assert_eq!(definition["uri"], asset_uri);
    assert_eq!(definition["range"]["start"]["line"], 0);
    assert_eq!(definition["range"]["start"]["character"], 0);
    assert_eq!(references.len(), 2);
    assert_reference_texts_for_uri(&output, 3, &main_uri, main, ["\"assets/player.png\""]);
    assert_reference_texts_for_uri(&output, 3, &ui_uri, ui, ["\"assets/player.png\""]);
}

#[test]
fn definition_returns_global_function_location() {
    let uri = "file:///tmp/global-definition.tua";
    let source = r#"function test(a: boolean)
  return ""
end

test(false)
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        definition_request(2, uri, source, "test(false", 0),
    ]);

    assert_definition_text(&output, 2, uri, source, "test");
}
#[test]
fn definition_returns_self_index_instance_method_location() {
    let uri = "file:///tmp/metatable-method-definition.tua";
    let source = r#"local Account = {}
Account.__index = Account

function Account:deposit(amount)
  self.balance = self.balance + amount
end

local account = setmetatable({ name = "checking", balance = 50 }, Account)
account:deposit(25)
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        definition_request(2, uri, source, "deposit(25)", 0),
    ]);

    assert_definition_text(&output, 2, uri, source, "Account:deposit");
}
#[test]
fn definition_returns_virtual_love_builtin_locations() {
    let uri = "file:///tmp/love-builtin-definition.tua";
    let source = r#"local player = love.graphics.newImage("assets/player.png")
player:setFilter()
"#;
    let mut server = TestServer::new();

    server.send(did_open(uri, source));
    server.send(definition_request(2, uri, source, "newImage", 0));
    server.send(definition_request(3, uri, source, "setFilter", 0));

    let constructor = response_with_id(server.output(), 2)["result"].clone();
    let method = response_with_id(server.output(), 3)["result"].clone();

    assert_eq!(
        constructor["uri"],
        "tua:/builtins/love.graphics.newImage.tua"
    );
    assert_eq!(method["uri"], "tua:/builtins/Image.setFilter.tua");

    server.send(builtin_document_request(
        4,
        "tua:/builtins/Image.setFilter.tua",
    ));

    let document = response_with_id(server.output(), 4)["result"]
        .as_str()
        .expect("builtin document should be returned");

    assert!(document.contains("function Image:setFilter"));
    assert_eq!(range_text(document, &method["range"]), "setFilter");
}

#[test]
fn helper_configuration_hover_and_definition_share_semantic_schema_targets() {
    let uri = "file:///tmp/helper-navigation.tua";
    let source = r#"local game = store({ score = 0 })
game:watch("score", function(score: number)
  print(score)
end)

local Player = spriteSheet({
  image = "assets/player.png",
  frame = { width = 16, height = 16 },
  animations = {
    idle = {
      from = 0,
      to = 3,
      fps = 8,
      onLoop = function(animation, loops)
        print(loops)
      end,
    },
  },
})
"#;
    let mut server = TestServer::new();

    server.send(did_open(uri, source));
    server.send(hover_request(2, uri, source, "image ="));
    server.send(hover_request(3, uri, source, "width ="));
    server.send(hover_request(4, uri, source, "loops)\n      end"));
    server.send(hover_request(5, uri, source, "\"score\", function"));
    server.send(definition_request(6, uri, source, "image =", 0));
    server.send(definition_request(7, uri, source, "\"score\", function", 0));
    server.send(builtin_document_request(
        8,
        "tua:/builtins/SpriteSheetConfig.image.tua",
    ));

    assert_hover_contains(server.output(), 2, "(field) image: string");
    assert_hover_contains(server.output(), 3, "(field) width: number");
    assert_hover_contains(server.output(), 4, "local loops: number");
    assert_hover_contains(server.output(), 5, "(field) score: number");
    assert_eq!(
        response_with_id(server.output(), 6)["result"]["uri"],
        "tua:/builtins/SpriteSheetConfig.image.tua"
    );
    assert_definition_text(server.output(), 7, uri, source, "score");

    let document = response_with_id(server.output(), 8)["result"]
        .as_str()
        .expect("builtin schema document should be returned");
    assert!(document.contains("type SpriteSheetConfig"));
    assert!(document.contains("image: string"));
}

#[test]
fn definition_recovers_when_an_unrelated_line_is_incomplete() {
    let uri = "file:///tmp/recovered-definition.tua";
    let source = r#"type Player = { hp: number }
local player: Player = { hp = 10 }
player.

function test(a: boolean)
  return ""
end

test(false)
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        definition_request(2, uri, source, "test(false", 0),
    ]);

    assert_definition_text(&output, 2, uri, source, "test");
}
#[test]
fn definition_returns_cross_file_module_and_export_locations() {
    let fixture = Fixture::parse(
        r#"-- file: tua.toml
source = "src"
-- file: src/main.tua
local Player = require("<|module|>player")
local Legacy = require("legacy")

local player = Player.<|tua_export|>makePlayer()
local clamped = Legacy.<|lua_export|>clamp(10, 0, 20)
-- file: src/player.tua
local function makePlayer()
  return { hp = 10 }
end

return {
  makePlayer = makePlayer,
}
-- file: src/legacy.lua
return {
  clamp = function(value, min, max)
    return value
  end,
}
"#,
    );
    let temp = fixture
        .write_to_temp("cross-file-definition")
        .expect("fixture should write");
    let main = fixture.source("src/main.tua");
    let player = fixture.source("src/player.tua");
    let legacy = fixture.source("src/legacy.lua");

    let main_uri = path_uri(&temp.join("src/main.tua"));
    let player_uri = path_uri(&temp.join("src/player.tua"));
    let legacy_uri = path_uri(&temp.join("src/legacy.lua"));
    let output = run_messages(vec![
        initialize_with_root(1, path_uri(temp.path())),
        did_open(&main_uri, main),
        definition_request_at(2, &main_uri, fixture.position("module")),
        definition_request_at(3, &main_uri, fixture.position("tua_export")),
        definition_request_at(4, &main_uri, fixture.position("lua_export")),
    ]);

    assert_definition_text(&output, 2, &player_uri, player, "makePlayer");
    assert_definition_text(&output, 3, &player_uri, player, "makePlayer");
    assert_definition_text(&output, 4, &legacy_uri, legacy, "clamp");
}

#[test]
fn editor_regression_definition_keeps_disk_backed_required_module_after_close() {
    let fixture = Fixture::parse(
        r#"-- file: tua.toml
source = "src"
-- file: src/main.tua
local Player = require("<|module|>player")
local player = Player.<|export|>makePlayer()
-- file: src/player.tua
local function makePlayer()
  return { hp = 10 }
end

return {
  makePlayer = makePlayer,
}
"#,
    );
    let temp = fixture
        .write_to_temp("closed-required-module-definition")
        .expect("fixture should write");
    let main = fixture.source("src/main.tua");
    let player = fixture.source("src/player.tua");
    let main_uri = path_uri(&temp.join("src/main.tua"));
    let player_uri = path_uri(&temp.join("src/player.tua"));
    let output = run_messages(vec![
        initialize_with_root(1, path_uri(temp.path())),
        did_open(&player_uri, player),
        did_close(&player_uri),
        did_open(&main_uri, main),
        definition_request_at(2, &main_uri, fixture.position("module")),
        definition_request_at(3, &main_uri, fixture.position("export")),
    ]);

    assert_definition_text(&output, 2, &player_uri, player, "makePlayer");
    assert_definition_text(&output, 3, &player_uri, player, "makePlayer");
}

#[test]
fn definition_returns_imported_module_and_export_locations() {
    let fixture = Fixture::parse(
        r#"-- file: tua.toml
source = "src"
-- file: src/main.tua
local Player = import("<|module|>./player")

local player = Player.<|export|>makePlayer()
-- file: src/player.tua
local function makePlayer()
  return { hp = 10 }
end

return {
  makePlayer = makePlayer,
}
"#,
    );
    let temp = fixture
        .write_to_temp("cross-file-import-definition")
        .expect("fixture should write");
    let main = fixture.source("src/main.tua");
    let player = fixture.source("src/player.tua");

    let main_uri = path_uri(&temp.join("src/main.tua"));
    let player_uri = path_uri(&temp.join("src/player.tua"));
    let output = run_messages(vec![
        initialize_with_root(1, path_uri(temp.path())),
        did_open(&main_uri, main),
        definition_request_at(2, &main_uri, fixture.position("module")),
        definition_request_at(3, &main_uri, fixture.position("export")),
    ]);

    assert_definition_text(&output, 2, &player_uri, player, "makePlayer");
    assert_definition_text(&output, 3, &player_uri, player, "makePlayer");
}
#[test]
fn type_definition_returns_local_and_cross_file_type_locations() {
    let fixture = Fixture::parse(
        r#"-- file: tua.toml
source = "src"
-- file: src/main.tua
local Types = require("types")

type Player = {
  <|local_hp_def|>hp<|/local_hp_def|>: number,
}

local <|local_decl|>player: <|local_annotation|>Player = { hp = 10 }
local copied = <|local_usage|>player
player.<|local_hp_ref|>hp = 12
local imported: Types.<|remote_type|>Enemy = { hp = 20 }
-- file: src/types.tua
type Enemy = {
  hp: number,
}

return {}
"#,
    );
    let temp = fixture
        .write_to_temp("type-definition-lsp")
        .expect("fixture should write");
    let main = fixture.source("src/main.tua");
    let types = fixture.source("src/types.tua");
    let main_uri = path_uri(&temp.join("src/main.tua"));
    let types_uri = path_uri(&temp.join("src/types.tua"));
    let output = run_messages(vec![
        initialize_with_root(1, path_uri(temp.path())),
        did_open(&main_uri, main),
        type_definition_request_at(2, &main_uri, fixture.position("local_decl")),
        type_definition_request_at(3, &main_uri, fixture.position("local_annotation")),
        type_definition_request_at(4, &main_uri, fixture.position("local_usage")),
        type_definition_request_at(5, &main_uri, fixture.position("local_hp_ref")),
        type_definition_request_at(6, &main_uri, fixture.position("remote_type")),
    ]);

    assert_definition_text(&output, 2, &main_uri, main, "Player");
    assert_definition_text(&output, 3, &main_uri, main, "Player");
    assert_definition_text(&output, 4, &main_uri, main, "Player");
    assert_definition_text(&output, 5, &main_uri, main, "hp");
    assert_definition_text(&output, 6, &types_uri, types, "Enemy");
}
#[test]
fn implementation_returns_functions_methods_exports_and_empty_for_dynamic_cases() {
    let fixture = Fixture::parse(
        r#"-- file: tua.toml
source = "src"
-- file: src/main.tua
local Functions = require("functions")

type Account = { balance: number }

local function damage(player: Account)
  return player.balance
end

function Account:deposit(amount: number)
  self.balance = self.balance + amount
end

local account: Account = { balance = 1 }
<|local_call|>damage(account)
account:<|method_call|>deposit(1)
Functions.<|module_call|>makePlayer()

local payload: any = {}
payload.<|any_call|>missing()

local Meta = {}
Meta.__index = Meta
function Meta:onlyRuntime() end
local runtime = setmetatable({}, Meta)
runtime:<|meta_call|>onlyRuntime()

function run() end
function run() end
<|global_call|>run()
-- file: src/functions.tua
local function makePlayer()
  return {}
end

return {
  makePlayer = makePlayer,
}
"#,
    );
    let temp = fixture
        .write_to_temp("implementation-lsp")
        .expect("fixture should write");
    let main = fixture.source("src/main.tua");
    let functions = fixture.source("src/functions.tua");
    let main_uri = path_uri(&temp.join("src/main.tua"));
    let functions_uri = path_uri(&temp.join("src/functions.tua"));
    let output = run_messages(vec![
        initialize_with_root(1, path_uri(temp.path())),
        did_open(&main_uri, main),
        implementation_request_at(2, &main_uri, fixture.position("local_call")),
        implementation_request_at(3, &main_uri, fixture.position("method_call")),
        implementation_request_at(4, &main_uri, fixture.position("module_call")),
        implementation_request_at(5, &main_uri, fixture.position("any_call")),
        implementation_request_at(6, &main_uri, fixture.position("meta_call")),
        implementation_request_at(7, &main_uri, fixture.position("global_call")),
    ]);

    assert_first_location_text(&output, 2, &main_uri, main, "damage");
    assert_first_location_text(&output, 3, &main_uri, main, "Account:deposit");
    assert_first_location_text(&output, 4, &functions_uri, functions, "makePlayer");
    assert_empty_locations(&output, 5);
    assert_empty_locations(&output, 6);
    assert_empty_locations(&output, 7);
}
#[test]
fn call_hierarchy_returns_same_file_and_cross_file_callers() {
    let temp = TempProject::new("call-hierarchy-modules");
    let player = r#"type Player = { name: string }

local function makePlayer(name: string): Player
  return { name = name }
end

local function localCaller()
  return makePlayer("local")
end

return {
  makePlayer = makePlayer,
}
"#;
    let main = r#"local Player = require("player")

local function spawn()
  return Player.makePlayer("Ada")
end
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/player.tua", player);
    temp.write("src/main.tua", main);

    let player_uri = temp.uri_for("src/player.tua");
    let main_uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&player_uri, player));
    server.send(did_open(&main_uri, main));
    server.send(prepare_call_hierarchy_request(
        2,
        &player_uri,
        player,
        "makePlayer(name",
        0,
    ));

    let item = first_call_hierarchy_item(server.output(), 2);

    assert_eq!(item["name"], "makePlayer");
    assert_eq!(item["kind"], 12);
    assert_eq!(item["uri"], player_uri);
    assert_eq!(range_text(player, &item["selectionRange"]), "makePlayer");

    server.send(call_hierarchy_incoming_request(3, item.clone()));

    let incoming = response_with_id(server.output(), 3)["result"]
        .as_array()
        .expect("incoming calls should be an array");

    assert_eq!(
        call_hierarchy_from_names(incoming),
        vec!["spawn", "localCaller"]
    );
    assert_call_hierarchy_range_text(incoming, "spawn", main, "makePlayer");
    assert_call_hierarchy_range_text(incoming, "localCaller", player, "makePlayer");

    server.send(prepare_call_hierarchy_request(
        4, &main_uri, main, "spawn()", 0,
    ));
    let spawn = first_call_hierarchy_item(server.output(), 4);

    server.send(call_hierarchy_outgoing_request(5, spawn));

    let outgoing = response_with_id(server.output(), 5)["result"]
        .as_array()
        .expect("outgoing calls should be an array");

    assert_eq!(call_hierarchy_to_names(outgoing), vec!["makePlayer"]);
    assert_call_hierarchy_outgoing_range_text(outgoing, "makePlayer", main, "makePlayer");
}
#[test]
fn call_hierarchy_resolves_known_receiver_methods_and_ignores_any() {
    let uri = "file:///tmp/call-hierarchy-method.tua";
    let source = r#"type Player = { hp: number }

function Player:heal()
end

local function healPlayer(player: Player, payload: any)
  player:heal()
  payload:heal()
end
"#;
    let mut server = TestServer::new();

    server.send(did_open(uri, source));
    server.send(prepare_call_hierarchy_request(2, uri, source, "heal()", 0));

    let method = first_call_hierarchy_item(server.output(), 2);

    assert_eq!(method["name"], "Player:heal");
    assert_eq!(method["kind"], 6);

    server.send(call_hierarchy_incoming_request(3, method));

    let incoming = response_with_id(server.output(), 3)["result"]
        .as_array()
        .expect("incoming calls should be an array");

    assert_eq!(call_hierarchy_from_names(incoming), vec!["healPlayer"]);
    assert_call_hierarchy_range_text(incoming, "healPlayer", source, "heal");

    server.send(prepare_call_hierarchy_request(
        4,
        uri,
        source,
        "healPlayer(player",
        0,
    ));
    let caller = first_call_hierarchy_item(server.output(), 4);

    server.send(call_hierarchy_outgoing_request(5, caller));

    let outgoing = response_with_id(server.output(), 5)["result"]
        .as_array()
        .expect("outgoing calls should be an array");

    assert_eq!(call_hierarchy_to_names(outgoing), vec!["Player:heal"]);
    assert_call_hierarchy_outgoing_range_text(outgoing, "Player:heal", source, "heal");
}
#[test]
fn call_hierarchy_unknown_symbol_returns_empty_array() {
    let uri = "file:///tmp/missing-call-hierarchy.tua";
    let source = "missing()\n";
    let output = run_messages(vec![
        did_open(uri, source),
        prepare_call_hierarchy_request(2, uri, source, "missing", 0),
    ]);

    assert_eq!(response_with_id(&output, 2)["result"], json!([]));
}
#[test]
fn document_link_returns_resolved_require_targets() {
    let temp = TempProject::new("document-links");
    let main = r#"local Player = require("player")
local Legacy = require("legacy")
local Missing = require("missing")
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", main);
    temp.write("src/player.tua", "return {}\n");
    temp.write("src/legacy.lua", "return {}\n");

    let main_uri = temp.uri_for("src/main.tua");
    let player_uri = temp.uri_for("src/player.tua");
    let legacy_uri = temp.uri_for("src/legacy.lua");
    let output = run_messages(vec![
        did_open(&main_uri, main),
        document_link_request(2, &main_uri),
    ]);
    let response = response_with_id(&output, 2);
    let links = response["result"]
        .as_array()
        .expect("document links should be an array");

    assert_eq!(links.len(), 2);
    assert_eq!(links[0]["target"], player_uri);
    assert_eq!(range_text(main, &links[0]["range"]), "player");
    assert_eq!(links[1]["target"], legacy_uri);
    assert_eq!(range_text(main, &links[1]["range"]), "legacy");
}
#[test]
fn will_rename_files_updates_same_folder_require() {
    let temp = TempProject::new("file-rename-require");
    let main = r#"local Player = require("player")
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", main);
    temp.write("src/player.tua", "return {}\n");

    let main_uri = temp.uri_for("src/main.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        will_rename_files_request(
            2,
            [(
                temp.uri_for("src/player.tua"),
                temp.uri_for("src/actor.tua"),
            )],
        ),
    ]);
    let edits = workspace_edit_changes_for_uri(&output, 2, &main_uri);

    assert_eq!(edits.len(), 1);
    assert_eq!(range_text(main, &edits[0]["range"]), "player");
    assert_eq!(edits[0]["newText"], "actor");
}

#[test]
fn will_rename_files_updates_static_asset_references() {
    let temp = TempProject::new("file-rename-asset");
    let main = r#"local player = image "assets/player.png"
"#;

    temp.write("tua.toml", "source = \".\"\n");
    temp.write("main.tua", main);
    temp.write("assets/player.png", "");

    let main_uri = temp.uri_for("main.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        will_rename_files_request(
            2,
            [(
                temp.uri_for("assets/player.png"),
                temp.uri_for("assets/hero.png"),
            )],
        ),
    ]);
    let edits = workspace_edit_changes_for_uri(&output, 2, &main_uri);

    assert_eq!(edits.len(), 1);
    assert_eq!(
        range_text(main, &edits[0]["range"]),
        "\"assets/player.png\""
    );
    assert_eq!(edits[0]["newText"], "\"assets/hero.png\"");
}
#[test]
fn will_rename_files_updates_nested_child_import_to_project_import() {
    let temp = TempProject::new("file-rename-import");
    let combat = r#"local Enemy = import("./enemy")
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/systems/combat.tua", combat);
    temp.write("src/systems/enemy.tua", "return {}\n");

    let combat_uri = temp.uri_for("src/systems/combat.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        will_rename_files_request(
            2,
            [(
                temp.uri_for("src/systems/enemy.tua"),
                temp.uri_for("src/entities/enemy.tua"),
            )],
        ),
    ]);
    let edits = workspace_edit_changes_for_uri(&output, 2, &combat_uri);

    assert_eq!(edits.len(), 1);
    assert_eq!(range_text(combat, &edits[0]["range"]), "./enemy");
    assert_eq!(edits[0]["newText"], "entities.enemy");
}
#[test]
fn will_rename_files_preserves_configured_require_separator() {
    let temp = TempProject::new("file-rename-separator");
    let main = r#"local Player = require("entities/player")
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", main);
    temp.write("src/entities/player.tua", "return {}\n");

    let main_uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_change_configuration(json!({
        "requirePathSeparator": "/"
    })));
    server.send(will_rename_files_request(
        2,
        [(
            temp.uri_for("src/entities/player.tua"),
            temp.uri_for("src/actors/player.tua"),
        )],
    ));

    let edits = workspace_edit_changes_for_uri(server.output(), 2, &main_uri);

    assert_eq!(edits.len(), 1);
    assert_eq!(range_text(main, &edits[0]["range"]), "entities/player");
    assert_eq!(edits[0]["newText"], "actors/player");
}
#[test]
fn will_rename_files_skips_ignored_and_ambiguous_modules() {
    let ignored = TempProject::new("file-rename-ignored");
    let ignored_main = r#"local Player = require("generated.player")
"#;

    ignored.write("tua.toml", "source = \"src\"\n");
    ignored.write("src/main.tua", ignored_main);
    ignored.write("src/generated/player.tua", "return {}\n");

    let mut server = TestServer::new();

    server.send(initialize_with_root(1, ignored.uri()));
    server.send(did_change_configuration(json!({
        "ignoredPaths": ["generated/**"]
    })));
    server.send(will_rename_files_request(
        2,
        [(
            ignored.uri_for("src/generated/player.tua"),
            ignored.uri_for("src/generated/actor.tua"),
        )],
    ));

    assert_eq!(response_with_id(server.output(), 2)["result"], Value::Null);

    let ambiguous = TempProject::new("file-rename-ambiguous");
    let ambiguous_main = r#"local Player = require("player")
"#;

    ambiguous.write("tua.toml", "source = \"src\"\n");
    ambiguous.write("src/main.tua", ambiguous_main);
    ambiguous.write("src/player.tua", "return {}\n");
    ambiguous.write("src/player.lua", "return {}\n");

    let output = run_messages(vec![
        initialize_with_root(1, ambiguous.uri()),
        will_rename_files_request(
            2,
            [(
                ambiguous.uri_for("src/player.tua"),
                ambiguous.uri_for("src/actor.tua"),
            )],
        ),
    ]);

    assert_eq!(response_with_id(&output, 2)["result"], Value::Null);
}
#[test]
fn will_rename_files_skips_dynamic_module_specifiers() {
    let temp = TempProject::new("file-rename-dynamic");
    let main = r#"local module_name = "player"
local Player = require(module_name)
local Legacy = import(module_name)
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", main);
    temp.write("src/player.tua", "return {}\n");

    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        will_rename_files_request(
            2,
            [(
                temp.uri_for("src/player.tua"),
                temp.uri_for("src/actor.tua"),
            )],
        ),
    ]);

    assert_eq!(response_with_id(&output, 2)["result"], Value::Null);
}
#[test]
fn will_rename_files_only_edits_the_affected_workspace_root() {
    let game = TempProject::new("file-rename-game");
    let tools = TempProject::new("file-rename-tools");
    let main = r#"local Player = require("player")
"#;

    game.write("tua.toml", "source = \"src\"\n");
    game.write("src/main.tua", main);
    game.write("src/player.tua", "return {}\n");
    tools.write("tua.toml", "source = \"src\"\n");
    tools.write("src/main.tua", main);
    tools.write("src/player.tua", "return {}\n");

    let game_main_uri = game.uri_for("src/main.tua");
    let tools_main_uri = tools.uri_for("src/main.tua");
    let output = run_messages(vec![
        initialize_with_root(1, game.uri()),
        workspace_folders_changed([tools.uri()], []),
        will_rename_files_request(
            2,
            [(
                game.uri_for("src/player.tua"),
                game.uri_for("src/actor.tua"),
            )],
        ),
    ]);

    let edits = workspace_edit_changes_for_uri(&output, 2, &game_main_uri);

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["newText"], "actor");
    assert!(workspace_edit_changes_for_uri_optional(&output, 2, &tools_main_uri).is_none());
}
#[test]
fn document_highlight_returns_symbol_usages() {
    let uri = "file:///tmp/document-highlight.tua";
    let source = r#"type Player = { hp: number }

local player: Player = { hp = 10 }

local function damage(target: Player, amount: number): Player
  target.hp -= amount
  return target
end

damage(player, 2)
player.hp = player.hp - 1
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        document_highlight_request(2, uri, source, "damage(player", "damage(".len()),
    ]);

    assert_document_highlight_texts(&output, 2, source, ["player", "player", "player", "player"]);
}
#[test]
fn document_highlight_unknown_document_returns_empty_array() {
    let uri = "file:///tmp/missing-document-highlight.tua";
    let output = run_messages(vec![document_highlight_request(2, uri, "", "", 0)]);
    let response = response_with_id(&output, 2);

    assert_eq!(response["result"], json!([]));
}
#[test]
fn definition_unknown_symbol_returns_null_result() {
    let uri = "file:///tmp/unknown-definition.tua";
    let source = "local result = missing\n";
    let output = run_messages(vec![
        did_open(uri, source),
        definition_request(2, uri, source, "missing", 0),
    ]);

    let response = response_with_id(&output, 2);

    assert!(response["result"].is_null());
    assert!(response.get("error").is_none());
}
#[test]
fn references_returns_alias_function_local_and_table_field_locations() {
    let uri = "file:///tmp/references.tua";
    let source = r#"type Vec2 = { x: number, y: number }
type Player = { pos: Vec2, hp: number, alive?: boolean }

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

local payload: any = {}
local dynamic = payload.value
local unresolved = missing
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        references_request(2, uri, source, "player: Player", "player: ".len(), true),
        references_request(3, uri, source, "damage(player", 0, true),
        references_request(4, uri, source, "damage(player", "damage(".len(), true),
        references_request(5, uri, source, "target.hp", "target.".len(), true),
    ]);

    assert_reference_texts(&output, 2, source, ["Player", "Player", "Player", "Player"]);
    assert_reference_texts(&output, 3, source, ["damage", "damage"]);
    assert_reference_texts(&output, 4, source, ["player", "player", "player", "player"]);
    assert_reference_texts(&output, 5, source, ["hp", "hp", "hp", "hp"]);
}
#[test]
fn references_returns_require_alias_usages() {
    let uri = "file:///tmp/require-alias-references.tua";
    let source = r#"local Player = require("player")
Player.load()
Player.draw()
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        references_request(2, uri, source, "Player = require", 0, true),
        references_request(3, uri, source, "Player = require", 0, false),
    ]);

    assert_reference_texts(&output, 2, source, ["Player", "Player", "Player"]);
    assert_reference_texts(&output, 3, source, ["Player", "Player"]);
}
#[test]
fn references_can_exclude_declarations_and_returns_empty_for_unknowns() {
    let uri = "file:///tmp/quiet-references.tua";
    let source = r#"type Player = { hp: number }
local player: Player = { hp = 10 }
player.hp = player.hp - 1

local payload: any = {}
local dynamic = payload.value
local unresolved = missing
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        references_request(2, uri, source, "player: Player", "player: ".len(), false),
        references_request(3, uri, source, "payload.value", "payload.".len(), true),
        references_request(4, uri, source, "missing", 0, true),
    ]);

    assert_reference_texts(&output, 2, source, ["Player"]);
    assert_reference_texts(&output, 3, source, []);
    assert_reference_texts(&output, 4, source, []);
}
#[test]
fn references_project_wide_include_exported_module_callers() {
    let temp = TempProject::new("project-references-export");
    let player = r#"type Player = { name: string }

local function makePlayer(name: string): Player
  return { name = name }
end

return {
  makePlayer = makePlayer,
}
"#;
    let main = r#"local Player = require("player")

local made = Player.makePlayer("Ada")
"#;
    let enemy = r#"local Player = require("player")

local enemy = Player.makePlayer("Bea")
"#;
    let outside = r#"local Player = require("player")

local skipped = Player.makePlayer("Outside")
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/player.tua", player);
    temp.write("src/main.tua", main);
    temp.write("src/enemy.tua", enemy);
    temp.write("scratch/outside.tua", outside);

    let player_uri = temp.uri_for("src/player.tua");
    let main_uri = temp.uri_for("src/main.tua");
    let enemy_uri = temp.uri_for("src/enemy.tua");
    let outside_uri = temp.uri_for("scratch/outside.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&player_uri, player),
        references_request(2, &player_uri, player, "makePlayer(name", 0, true),
        references_request(3, &player_uri, player, "makePlayer(name", 0, false),
    ]);

    assert_reference_texts_for_uri(
        &output,
        2,
        &player_uri,
        player,
        ["makePlayer", "makePlayer", "makePlayer"],
    );
    assert_reference_texts_for_uri(&output, 2, &main_uri, main, ["makePlayer"]);
    assert_reference_texts_for_uri(&output, 2, &enemy_uri, enemy, ["makePlayer"]);
    assert_reference_texts_for_uri(&output, 2, &outside_uri, outside, []);

    assert_reference_texts_for_uri(
        &output,
        3,
        &player_uri,
        player,
        ["makePlayer", "makePlayer"],
    );
    assert_reference_texts_for_uri(&output, 3, &main_uri, main, ["makePlayer"]);
    assert_reference_texts_for_uri(&output, 3, &enemy_uri, enemy, ["makePlayer"]);
}
#[test]
fn references_project_wide_update_after_watched_file_changes() {
    let temp = TempProject::new("project-references-watch");
    let player = r#"type Player = { name: string }

local function makePlayer(name: string): Player
  return { name = name }
end

return {
  makePlayer = makePlayer,
}
"#;
    let main_before = r#"local Player = require("player")

local made = Player.makePlayer("Ada")
"#;
    let main_after = r#"local Player = require("player")

local made = Player.makePlayer("Ada")
local other = Player.makePlayer("Bea")
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/player.tua", player);
    temp.write("src/main.tua", main_before);

    let player_uri = temp.uri_for("src/player.tua");
    let main_uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&player_uri, player));
    server.send(references_request(
        2,
        &player_uri,
        player,
        "makePlayer(name",
        0,
        false,
    ));
    assert_reference_texts_for_uri(server.output(), 2, &main_uri, main_before, ["makePlayer"]);

    temp.write("src/main.tua", main_after);
    server.send(watched_files_changed([(&main_uri, 2)]));
    server.send(references_request(
        3,
        &player_uri,
        player,
        "makePlayer(name",
        0,
        false,
    ));

    assert_reference_texts_for_uri(
        server.output(),
        3,
        &main_uri,
        main_after,
        ["makePlayer", "makePlayer"],
    );
}
#[test]
fn references_project_wide_include_globals_without_local_shadows() {
    let temp = TempProject::new("project-references-globals");
    let globals = r#"function greet(name: string): string
  return name
end

local own = greet("Local")
"#;
    let main = r#"local message = greet("Ada")
"#;
    let shadow = r#"local function greet()
  return "shadow"
end

local value = greet()
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/globals.tua", globals);
    temp.write("src/main.tua", main);
    temp.write("src/shadow.tua", shadow);

    let globals_uri = temp.uri_for("src/globals.tua");
    let main_uri = temp.uri_for("src/main.tua");
    let shadow_uri = temp.uri_for("src/shadow.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&globals_uri, globals),
        references_request(2, &globals_uri, globals, "greet(name", 0, true),
        references_request(3, &globals_uri, globals, "greet(name", 0, false),
    ]);

    assert_reference_texts_for_uri(&output, 2, &globals_uri, globals, ["greet", "greet"]);
    assert_reference_texts_for_uri(&output, 2, &main_uri, main, ["greet"]);
    assert_reference_texts_for_uri(&output, 2, &shadow_uri, shadow, []);

    assert_reference_texts_for_uri(&output, 3, &globals_uri, globals, ["greet"]);
    assert_reference_texts_for_uri(&output, 3, &main_uri, main, ["greet"]);
}
#[test]
fn references_unknown_document_returns_empty_array() {
    let uri = "file:///tmp/missing-references.tua";
    let output = run_messages(vec![references_request(
        2,
        uri,
        "local value = missing\n",
        "missing",
        0,
        true,
    )]);
    let response = response_with_id(&output, 2);

    assert_eq!(response["result"], json!([]));
}
#[test]
fn prepare_rename_returns_alias_and_table_field_ranges_and_placeholders() {
    let uri = "file:///tmp/prepare-rename.tua";
    let source = r#"type Player = { hp: number }
local player: Player = { hp = 10 }
player.hp = player.hp - 1
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        prepare_rename_request(2, uri, source, "player: Player", "player: ".len()),
        prepare_rename_request(3, uri, source, "player.hp", "player.".len()),
    ]);
    let supported = response_with_id(&output, 2);
    let field = response_with_id(&output, 3);

    assert_eq!(supported["result"]["placeholder"], "Player");
    assert_eq!(range_text(source, &supported["result"]["range"]), "Player");
    assert_eq!(field["result"]["placeholder"], "hp");
    assert_eq!(range_text(source, &field["result"]["range"]), "hp");
}
#[test]
fn rename_returns_workspace_edits_for_alias_function_local_and_parameter() {
    let uri = "file:///tmp/rename.tua";
    let source = r#"type Vec2 = { x: number, y: number }
type Player = { pos: Vec2, hp: number, alive?: boolean }

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
        rename_request(2, uri, source, "player: Player", "player: ".len(), "Actor"),
        rename_request(3, uri, source, "damage(player", 0, "hurt"),
        rename_request(4, uri, source, "damage(player", "damage(".len(), "hero"),
        rename_request(5, uri, source, "return target", "return ".len(), "victim"),
    ]);

    assert_rename_edit_texts(
        &output,
        2,
        uri,
        source,
        ["Player", "Player", "Player", "Player"],
        "Actor",
    );
    assert_rename_edit_texts(&output, 3, uri, source, ["damage", "damage"], "hurt");
    assert_rename_edit_texts(
        &output,
        4,
        uri,
        source,
        ["player", "player", "player", "player"],
        "hero",
    );
    assert_rename_edit_texts(
        &output,
        5,
        uri,
        source,
        ["target", "target", "target"],
        "victim",
    );
}
#[test]
fn rename_project_wide_exported_function_from_caller() {
    let temp = TempProject::new("rename-exported-function");
    let player = r#"type Player = { name: string }

local function makePlayer(name: string): Player
  return { name = name }
end

return {
  makePlayer = makePlayer,
}
"#;
    let main = r#"local Player = require("player")

local made = Player.makePlayer("Ada")
"#;
    let enemy = r#"local Player = require("player")

local enemy = Player.makePlayer("Bea")
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/player.tua", player);
    temp.write("src/main.tua", main);
    temp.write("src/enemy.tua", enemy);

    let player_uri = temp.uri_for("src/player.tua");
    let main_uri = temp.uri_for("src/main.tua");
    let enemy_uri = temp.uri_for("src/enemy.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&main_uri, main),
        prepare_rename_request(2, &main_uri, main, "Player.makePlayer", "Player.".len()),
        rename_request(
            3,
            &main_uri,
            main,
            "Player.makePlayer",
            "Player.".len(),
            "createPlayer",
        ),
    ]);
    let prepared = response_with_id(&output, 2);

    assert_eq!(range_text(main, &prepared["result"]["range"]), "makePlayer");
    assert_eq!(prepared["result"]["placeholder"], "makePlayer");
    assert_rename_edit_texts(
        &output,
        3,
        &player_uri,
        player,
        ["makePlayer", "makePlayer", "makePlayer"],
        "createPlayer",
    );
    assert_rename_edit_texts(&output, 3, &main_uri, main, ["makePlayer"], "createPlayer");
    assert_rename_edit_texts(
        &output,
        3,
        &enemy_uri,
        enemy,
        ["makePlayer"],
        "createPlayer",
    );
}
#[test]
fn rename_project_wide_type_alias_references() {
    let temp = TempProject::new("rename-type-alias");
    let types = r#"type Player = { hp: number }

local localPlayer: Player = { hp = 10 }
"#;
    let main = r#"local player: Player = { hp = 10 }
"#;
    let enemy = r#"local enemy: Player | nil = nil
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/types.tua", types);
    temp.write("src/main.tua", main);
    temp.write("src/enemy.tua", enemy);

    let types_uri = temp.uri_for("src/types.tua");
    let main_uri = temp.uri_for("src/main.tua");
    let enemy_uri = temp.uri_for("src/enemy.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&types_uri, types),
        rename_request(2, &types_uri, types, "type Player", "type ".len(), "Actor"),
    ]);

    assert_rename_edit_texts(&output, 2, &types_uri, types, ["Player", "Player"], "Actor");
    assert_rename_edit_texts(&output, 2, &main_uri, main, ["Player"], "Actor");
    assert_rename_edit_texts(&output, 2, &enemy_uri, enemy, ["Player"], "Actor");
}
#[test]
fn rename_supports_table_fields_and_rejects_unknowns_and_invalid_names() {
    let uri = "file:///tmp/quiet-rename.tua";
    let source = r#"type Player = { hp: number }
local player: Player = { hp = 10 }
player.hp = player.hp - 1
local unresolved = missing
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        rename_request(2, uri, source, "player.hp", "player.".len(), "health"),
        rename_request(3, uri, source, "missing", 0, "known"),
        rename_request(
            4,
            uri,
            source,
            "player: Player",
            "player: ".len(),
            "not-valid",
        ),
        rename_request(5, uri, source, "player: Player", "player: ".len(), "while"),
    ]);

    assert_rename_edit_texts(&output, 2, uri, source, ["hp", "hp", "hp"], "health");
    assert!(response_with_id(&output, 3)["result"].is_null());
    assert!(response_with_id(&output, 4)["result"].is_null());
    assert!(response_with_id(&output, 5)["result"].is_null());
}
#[test]
fn rename_returns_null_for_ambiguous_project_globals() {
    let temp = TempProject::new("rename-ambiguous-globals");
    let first = r#"function greet(name: string): string
  return name
end
"#;
    let second = r#"function greet()
  return "shadow"
end
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/first.tua", first);
    temp.write("src/second.tua", second);

    let first_uri = temp.uri_for("src/first.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&first_uri, first),
        rename_request(2, &first_uri, first, "greet(name", 0, "salute"),
    ]);

    assert!(response_with_id(&output, 2)["result"].is_null());
}
#[test]
fn rename_unknown_document_returns_null() {
    let uri = "file:///tmp/missing-rename.tua";
    let output = run_messages(vec![rename_request(
        2,
        uri,
        "local value = missing\n",
        "missing",
        0,
        "known",
    )]);
    let response = response_with_id(&output, 2);

    assert!(response["result"].is_null());
}

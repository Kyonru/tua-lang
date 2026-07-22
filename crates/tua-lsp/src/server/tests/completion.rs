use super::*;

#[test]
fn completion_returns_class_members_for_self_inside_methods() {
    let uri = "file:///tmp/class-self-completion.tua";
    let source = r#"
final class Counter {
  value: number

  add(amount: number): number {
    return self.value + amount
  }

  reset() {
    self.
  }
}
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        completion_request(2, uri, source, "self."),
    ]);

    assert_completion_labels(&output, 2, ["value", "add", "reset"]);
}

#[test]
fn hover_and_completion_surface_love_asset_metadata() {
    let temp = TempProject::new("asset-metadata");
    let source = r#"
local player = image "assets/player.png"
local next = image "assets/p"
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/assets/player.png", "placeholder");
    temp.write("src/assets/jump.wav", "placeholder");

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    server.send(hover_request(2, &uri, source, "player.png"));
    server.send(completion_request(3, &uri, source, "assets/p"));

    assert_hover_contains(server.output(), 2, "asset `assets/player.png`");
    assert_hover_contains(server.output(), 2, "LOVE type: Image");
    assert_hover_contains(server.output(), 2, "status: valid");
    assert_completion_labels(server.output(), 3, ["assets/player.png"]);

    let completion = response_with_id(server.output(), 3)["result"][0].clone();

    server.send(completion_resolve_request(4, completion));

    let resolved = &response_with_id(server.output(), 4)["result"];

    assert_eq!(resolved["detail"], "image asset path -> Image");
    assert!(resolved["documentation"]["value"]
        .as_str()
        .expect("asset completion docs should be markdown")
        .contains("Supported image asset"));
}
#[test]
fn did_change_configuration_can_enable_completion_sequence_ranking() {
    let temp = TempProject::new("configuration-completion-sequence-ranking");
    let source = r#"type Sprite = { pause: number, play: number, stop: number }
local stop = 1
local stopAgain = stop
local sprite: Sprite = {}
local current = sprite.
"#;

    temp.write("tua.toml", "source = \"src\"\n");

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    server.send(completion_request(2, &uri, source, "sprite."));

    assert_completion_labels(server.output(), 2, ["pause", "play", "stop"]);

    server.send(did_change_configuration(json!({
        "completion": {
            "sequenceRanking": {
                "enabled": true,
                "timeoutMs": 4,
                "maxCandidates": 64
            }
        }
    })));
    server.send(completion_request(3, &uri, source, "sprite."));

    assert_completion_labels(server.output(), 3, ["stop", "pause", "play"]);
}
#[test]
fn did_change_configuration_can_enable_multi_token_completion() {
    let temp = TempProject::new("configuration-completion-multi-token");
    let source = "local velocity = \n";

    temp.write("tua.toml", "source = \"src\"\n");

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    server.send(completion_request(2, &uri, source, "local velocity = "));

    assert!(!completion_labels(server.output(), 2).contains(&"vec2(0, 0)"));

    server.send(did_change_configuration(json!({
        "completion": {
            "multiToken": {
                "enabled": true,
                "advanced": true,
                "allowModelIntegration": false,
                "maxSuggestions": 2
            }
        }
    })));
    server.send(completion_request(3, &uri, source, "local velocity = "));

    assert!(completion_labels(server.output(), 3).contains(&"vec2(0, 0)"));
}
#[test]
fn completion_returns_type_aliases_and_table_fields() {
    let uri = "file:///tmp/completion.tua";
    let source = r#"type Vec2 = { x: number, y: number }
type Player = { pos: Vec2, hp: number, alive?: boolean }

local player: Player = {
  pos = { x = 0, y = 0 },
  hp = 10,
}

local typed: Pl = nil
local field = player.hp
"#;
    let output = run_messages(vec![
        initialize_with_completion_label_details(1),
        did_open(uri, source),
        completion_request(2, uri, source, "typed: Pl"),
        completion_request(3, uri, source, "player."),
    ]);

    assert_completion_labels(&output, 2, ["Player"]);
    assert_eq!(
        completion_with_label(&output, 2, "Player")["labelDetails"]["description"],
        "type alias"
    );
    assert_completion_labels(&output, 3, ["hp", "pos", "alive"]);
    assert_eq!(
        completion_with_label(&output, 3, "hp")["labelDetails"]["description"],
        "field: number"
    );
}
#[test]
fn completion_uses_persisted_adaptive_stats_and_can_reset_them() {
    let temp = TempProject::new("adaptive-completion-stats");
    let source = r#"type Sprite = { pause: number, play: number, stop: number }
local sprite: Sprite = {}
local current = sprite.pause
"#;
    let uri = temp.uri_for("src/main.tua");
    let path = temp.path.join("src/main.tua");
    let file_scope_hash = CompletionAdaptiveStats::file_scope_hash(&path);
    let receiver_hash = CompletionAdaptiveStats::receiver_hash(&["sprite".to_owned()]);
    let stats_path = temp.path.join(".tua/completion-stats.json");
    let now_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_secs();
    let mut server = TestServer::new();

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write(
        ".tua/completion-stats.json",
        &format!(
            r#"{{
  "version": 1,
  "updated_at_epoch": {now_epoch},
  "entries": [
    {{
      "file_scope_hash": {file_scope_hash},
      "receiver_hash": {receiver_hash},
      "member": "stop",
      "access_count": 2,
      "call_count": 2,
      "assignment_rhs_count": 0,
      "last_seen_epoch": {now_epoch}
    }},
    {{
      "file_scope_hash": {file_scope_hash},
      "receiver_hash": {receiver_hash},
      "member": "play",
      "access_count": 1,
      "call_count": 1,
      "assignment_rhs_count": 0,
      "last_seen_epoch": {now_epoch}
    }}
  ]
}}
"#
        ),
    );

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    server.send(completion_request(2, &uri, source, "sprite."));

    assert_completion_labels(server.output(), 2, ["stop", "play", "pause"]);

    server.send(reset_completion_stats_request(3));

    assert_eq!(
        response_with_id(server.output(), 3)["result"]["removedFiles"],
        1
    );
    assert!(
        !stats_path.exists(),
        "workspace completion stats file should be removed"
    );

    server.send(completion_request(4, &uri, source, "sprite."));

    assert_completion_labels(server.output(), 4, ["pause", "play", "stop"]);
}
#[test]
fn completion_returns_module_label_details() {
    let temp = TempProject::new("module-label-details");
    let source = r#"local Module = require("pla")"#;
    let uri = temp.uri_for("src/main.tua");

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/player.tua", "return {}\n");

    let output = run_messages(vec![
        initialize_with_root_and_completion_label_details(1, temp.uri()),
        did_open(&uri, source),
        completion_request(2, &uri, source, "pla"),
    ]);
    let completion = completion_with_label(&output, 2, "player");

    assert_eq!(completion["kind"], 9);
    assert_eq!(completion["labelDetails"]["description"], "module");
    assert!(completion.get("detail").is_none());
}

#[test]
fn completion_indexes_module_paths_from_the_configured_output_root() {
    let temp = TempProject::new("output-module-completion");
    let source = r#"local ImportedRuntime = import("run")
local RequiredRuntime = require("run")"#;
    let uri = temp.uri_for("src/main.tua");

    temp.write("tua.toml", "source = \"src\"\nout = \"build\"\n");
    temp.write("src/main.tua", source);
    temp.write("build/runtime.lua", "return {}\n");

    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
        completion_request(2, &uri, source, "import(\"run"),
        completion_request(3, &uri, source, "require(\"run"),
    ]);

    assert_completion_labels(&output, 2, ["runtime"]);
    assert_completion_labels(&output, 3, ["runtime"]);
}

#[test]
fn watched_output_modules_update_snapshot_backed_completion_inventory() {
    let temp = TempProject::new("watched-output-module-completion");
    let source = r#"local Watched = import("wat")"#;
    let uri = temp.uri_for("src/main.tua");
    let module_uri = temp.uri_for("build/watched.lua");
    let mut server = TestServer::new();

    temp.write("tua.toml", "source = \"src\"\nout = \"build\"\n");
    temp.write("src/main.tua", source);
    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    server.send(completion_request(2, &uri, source, "wat"));

    assert_completion_labels(server.output(), 2, []);

    temp.write("build/watched.lua", "return {}\n");
    server.send(watched_files_changed([(&module_uri, 1)]));
    server.send(completion_request(3, &uri, source, "wat"));

    assert_completion_labels(server.output(), 3, ["watched"]);

    temp.remove("build/watched.lua");
    server.send(watched_files_changed([(&module_uri, 3)]));
    server.send(completion_request(4, &uri, source, "wat"));

    assert_completion_labels(server.output(), 4, []);
}

#[test]
fn project_import_completion_is_scoped_to_the_active_workspace() {
    let game = TempProject::new("module-completion-game");
    let tools = TempProject::new("module-completion-tools");
    let source = r#"local Player = import("")"#;
    let uri = game.uri_for("src/main.tua");

    game.write("tua.toml", "source = \"src\"\n");
    game.write("src/main.tua", source);
    game.write("src/player.tua", "return {}\n");
    tools.write("tua.toml", "source = \"src\"\n");
    tools.write("src/enemy.tua", "return {}\n");

    let output = run_messages(vec![
        initialize_with_workspace_folders(1, [game.uri(), tools.uri()]),
        did_open(&uri, source),
        completion_request(2, &uri, source, "import(\""),
    ]);

    assert_completion_labels(&output, 2, ["player"]);
}

#[test]
fn completion_indexes_sibling_modules_without_a_project_config() {
    let temp = TempProject::new("configless-module-completion");
    let source = r#"local Player = require("")"#;
    let uri = temp.uri_for("main.tua");

    temp.write("main.tua", source);
    temp.write("player.lua", "return {}\n");
    temp.write("vendor/deep.lua", "return {}\n");

    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
        completion_request(2, &uri, source, "require(\""),
    ]);

    assert_completion_labels(&output, 2, ["player"]);
}

#[test]
fn configless_completion_indexes_siblings_of_a_nested_open_document() {
    let temp = TempProject::new("configless-nested-module-completion");
    let source = r#"local Player = require("game.pla")"#;
    let uri = temp.uri_for("game/main.tua");
    let mut server = TestServer::new();

    temp.write("game/main.tua", source);
    temp.write("game/player.lua", "return {}\n");
    temp.write("other/deep.lua", "return {}\n");

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    server.send(completion_request(2, &uri, source, "game.pla"));

    assert_completion_labels(server.output(), 2, ["game.player"]);

    server.send(did_change_configuration(json!({})));
    server.send(completion_request(3, &uri, source, "game.pla"));

    assert_completion_labels(server.output(), 3, ["game.player"]);
}

#[test]
fn closing_an_unsaved_document_removes_its_module_completion() {
    let temp = TempProject::new("unsaved-module-close-completion");
    let main_source = r#"local Ghost = require("gho")"#;
    let main_uri = temp.uri_for("src/main.tua");
    let ghost_uri = temp.uri_for("src/ghost.tua");
    let mut server = TestServer::new();

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", main_source);

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&main_uri, main_source));
    server.send(did_open(&ghost_uri, "return {}\n"));
    server.send(completion_request(2, &main_uri, main_source, "gho"));

    assert_completion_labels(server.output(), 2, ["ghost"]);

    server.send(did_close(&ghost_uri));
    server.send(completion_request(3, &main_uri, main_source, "gho"));

    assert_completion_labels(server.output(), 3, []);
}

#[test]
fn identifier_completion_uses_open_document_fast_path() {
    let temp = TempProject::new("identifier-completion-fast-path");
    let uri = temp.uri_for("src/main.tua");
    let source = "local outside_function = 1\nprint(ou)\nprint(out)\n";
    let mut server = TestServer::new();

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/other.tua", "return { value = 1 }\n");
    server.send(initialize_with_pull_diagnostics(1, temp.uri(), false));
    server.send(did_open(&uri, source));
    server.send(cache_stats_request(2));
    server.send(invoked_completion_request(3, &uri, source, "print(o"));
    server.send(invoked_completion_request(4, &uri, source, "print(out"));
    server.send(cache_stats_request(5));

    let outside_function = completion_with_label(server.output(), 3, "outside_function");
    assert!(
        outside_function.get("filterText").is_none(),
        "stale request positions inside the current identifier should keep label-based client filtering"
    );
    let outside_function = completion_with_label(server.output(), 4, "outside_function");
    assert!(
        outside_function.get("filterText").is_none(),
        "longer plain prefix matches should keep label-based client filtering"
    );

    let before = &response_with_id(server.output(), 2)["result"];
    let after = &response_with_id(server.output(), 5)["result"];

    assert_eq!(
        after["parse"]["misses"].as_u64(),
        Some(before["parse"]["misses"].as_u64().unwrap_or_default() + 1),
        "plain identifier completion should parse only the active document once while typing"
    );
}

#[test]
fn triggered_field_completion_uses_open_document_fast_path() {
    let temp = TempProject::new("field-completion-fast-path");
    let uri = temp.uri_for("src/main.tua");
    let source = "local value = table.\n";
    let mut server = TestServer::new();

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/other.tua", "return { value = 1 }\n");
    server.send(initialize_with_pull_diagnostics(1, temp.uri(), false));
    server.send(did_open(&uri, source));
    server.send(cache_stats_request(2));
    server.send(trigger_character_completion_request(
        3, &uri, source, "table.", ".",
    ));
    server.send(cache_stats_request(4));

    for label in ["concat", "insert", "maxn", "remove", "sort"] {
        completion_with_label(server.output(), 3, label);
    }

    let before = &response_with_id(server.output(), 2)["result"];
    let after = &response_with_id(server.output(), 4)["result"];

    assert_eq!(
        after["parse"]["misses"].as_u64(),
        Some(before["parse"]["misses"].as_u64().unwrap_or_default() + 1),
        "known current-file receivers should parse only the active document"
    );
}
#[test]
fn completion_survives_incomplete_colon_after_valid_open() {
    let uri = "file:///tmp/recovered-completion.tua";
    let valid = r#"type Account = {
  balance: number,
}

local Account = {}
Account.__index = Account

function Account:deposit(amount: number): number
  self.balance = self.balance + amount

  return self.balance
end

local account = setmetatable({ balance = 50 }, Account)
"#;
    let invalid = format!("{valid}account:\n");
    let mut server = TestServer::new();

    server.send(did_open(uri, valid));
    server.send(did_change(uri, &invalid, 2));
    server.send(completion_request(3, uri, &invalid, "account:"));

    completion_with_label(server.output(), 3, "deposit");
}
#[test]
fn completion_returns_lua_doc_tags_types_and_snippets() {
    let uri = "file:///tmp/doc-completion.tua";
    let source = r#"type Player = { hp: number }
---
local function damage(player: Player, amount: number): Player
end
---@p
---@param player Pl
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        completion_request(2, uri, source, "---@p"),
        completion_request(3, uri, source, "---@param player Pl"),
        completion_request(4, uri, source, "---"),
    ]);

    assert_completion_labels(&output, 2, ["param", "type"]);
    assert_completion_labels(&output, 3, ["Player"]);
    assert_completion_labels(&output, 4, ["document function"]);

    let snippet = response_with_id(&output, 4)["result"][0].clone();

    assert_eq!(snippet["kind"], 15);
    assert_eq!(snippet["insertTextFormat"], 2);
    assert_eq!(
        snippet["insertText"],
        "@param player Player ${1:description}\n---@param amount number ${2:description}\n---@return Player ${3:description}"
    );
}
#[test]
fn lua_doc_annotation_completion_works_for_lua_documents() {
    let uri = "file:///tmp/doc-completion.lua";
    let source = r#"---@alias Player { hp: number }
---@return Pl
local function makePlayer()
end
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        completion_request(2, uri, source, "---@return Pl"),
    ]);

    assert_completion_labels(&output, 2, ["Player"]);
}
#[test]
fn completion_is_quiet_for_any_and_unknown_field_receivers() {
    let uri = "file:///tmp/quiet-completion.tua";
    let source = r#"local payload: any = {}
local any_field = payload.value
local unknown_field = missing.value
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        completion_request(2, uri, source, "payload."),
        completion_request(3, uri, source, "missing."),
    ]);

    assert_completion_labels(&output, 2, []);
    assert_completion_labels(&output, 3, []);
}
#[test]
fn completion_returns_lua_and_love_items() {
    let uri = "file:///tmp/lua-completion.tua";
    let source = r#"local player = image "assets/player.png"
local value = math.si
local draw = love.graphics.pr
player:setFilter("")
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        completion_request(2, uri, source, "math.si"),
        completion_request(3, uri, source, "love.graphics.pr"),
        completion_request(4, uri, source, "setFilter(\""),
    ]);

    assert_completion_labels(&output, 2, ["sin", "asin"]);
    assert_completion_labels(&output, 3, ["print", "printf", "present", "newSpriteBatch"]);
    assert_completion_labels(&output, 4, ["linear", "nearest"]);
    assert_eq!(
        completion_with_label(&output, 2, "asin")["filterText"],
        "si"
    );
    assert_eq!(
        completion_with_label(&output, 3, "newSpriteBatch")["filterText"],
        "pr"
    );
}
#[test]
fn completion_returns_love_function_call_snippets_from_signatures() {
    let uri = "file:///tmp/love-function-snippet.tua";
    let source = "love.graphics.cir\n";
    let mut server = TestServer::new();

    server.send(initialize_with_completion_label_details(1));
    server.send(did_open(uri, source));
    server.send(completion_request(2, uri, source, "love.graphics.cir"));

    assert_completion_labels(server.output(), 2, ["circle"]);
    let circle = completion_with_label(server.output(), 2, "circle");

    assert_eq!(circle["kind"], 3);
    assert_eq!(
        circle["labelDetails"]["description"],
        "function love.graphics.circle(mode: DrawMode, x: number, y: number, radius: number)"
    );
    assert_eq!(circle["insertTextFormat"], 2);
    assert_eq!(
        circle["insertText"],
        r#"circle("${1|fill,line|}", ${2:x}, ${3:y}, ${4:radius})$0"#
    );

    server.send(completion_resolve_request(3, circle));
    let resolved = &response_with_id(server.output(), 3)["result"];

    assert!(resolved["documentation"]["value"]
        .as_str()
        .expect("documentation should be a string")
        .contains("[Open LOVE wiki](https://love2d.org/wiki/love.graphics.circle)"));
}
#[test]
fn completion_recovers_love_enum_values_for_typoed_string_arguments() {
    let uri = "file:///tmp/love-draw-mode-completion.tua";
    let source = r#"function love.update(dt)
  love.graphics.circle(
    "fillter",
    100,
    100,
    2
  )
end
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        completion_request(2, uri, source, "\"fillter"),
    ]);

    assert_completion_labels(&output, 2, ["fill", "line"]);
    let fill = completion_with_label(&output, 2, "fill");
    assert_eq!(fill["filterText"], "fillter");
}
#[test]
fn completion_returns_signal_event_names_in_string_arguments() {
    let uri = "file:///tmp/signal-event-completion.tua";
    let source = r#"
local events = signal()
events:on("player.hit", function() end)
events:emit("pla")
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        completion_request(2, uri, source, "events:emit(\"pla"),
    ]);

    assert_completion_labels(&output, 2, ["player.hit"]);
}
#[test]
fn completion_returns_generated_love_api_enum_and_type_items() {
    let uri = "file:///tmp/generated-love-api-completion.tua";
    let source = r#"local file = love.filesystem.newFile("save.txt")
file.op
love.graphics.setBlendMode("scr")
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        completion_request(2, uri, source, "file.op"),
        completion_request(3, uri, source, "\"scr"),
    ]);

    assert_completion_labels(&output, 2, ["open", "isOpen"]);
    assert_completion_labels(&output, 3, ["screen"]);
}
#[test]
fn completion_sort_text_preserves_server_ranking() {
    let uri = "file:///tmp/ranked-completion.tua";
    let source = r#"local classFar = 1
local classNear = 2
local value = cl
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        invoked_completion_request(2, uri, source, "local value = cl"),
    ]);
    let items = response_with_id(&output, 2)["result"]
        .as_array()
        .expect("completion result should be an array");
    let labels = items
        .iter()
        .map(|item| item["label"].as_str().expect("label should be a string"))
        .collect::<Vec<_>>();
    let class = items
        .iter()
        .find(|item| item["label"] == "class")
        .expect("class snippet should complete");

    assert_eq!(labels[0], "classNear");
    assert_eq!(labels[1], "classFar");
    assert_eq!(items[0]["sortText"], "0000_classNear");
    assert_eq!(items[1]["sortText"], "0001_classFar");
    assert_eq!(class["kind"], 15);
    assert_eq!(class["insertTextFormat"], 2);
    assert_eq!(class["insertText"], "class ${1:Name} {\n  ${0}\n}");
}
#[test]
fn completion_returns_love_callback_snippet_and_conf_fields() {
    let callback_uri = "file:///tmp/love-callback-completion.tua";
    let callback_source = "function love.up\n";
    let generated_callback_uri = "file:///tmp/love-generated-callback-completion.tua";
    let generated_callback_source = "function love.file\n";
    let conf_uri = "file:///tmp/love-conf-completion.tua";
    let conf_source = r#"function love.conf(t)
  local title = t.window.ti
end
"#;
    let mut server = TestServer::new();
    for message in [
        did_open(callback_uri, callback_source),
        did_open(generated_callback_uri, generated_callback_source),
        did_open(conf_uri, conf_source),
        completion_request(2, callback_uri, callback_source, "love.up"),
        completion_request(
            3,
            generated_callback_uri,
            generated_callback_source,
            "love.file",
        ),
        completion_request(4, conf_uri, conf_source, "t.window.ti"),
    ] {
        server.send(message);
    }
    let callback = response_with_id(server.output(), 2)["result"]
        .as_array()
        .expect("callback completions should be an array")
        .iter()
        .find(|item| item["label"] == "update")
        .expect("update callback should complete")
        .clone();

    assert_eq!(callback["insertText"], "update(dt)\n  $0\nend");
    assert_eq!(callback["insertTextFormat"], 2);
    let filedropped = response_with_id(server.output(), 3)["result"]
        .as_array()
        .expect("generated callback completions should be an array")
        .iter()
        .find(|item| item["label"] == "filedropped")
        .expect("filedropped callback should complete")
        .clone();

    assert_eq!(filedropped["insertText"], "filedropped(file)\n  $0\nend");
    assert_eq!(filedropped["insertTextFormat"], 2);
    assert_completion_labels(server.output(), 4, ["title"]);

    server.send(completion_resolve_request(5, filedropped));
    assert!(
        response_with_id(server.output(), 5)["result"]["documentation"]["value"]
            .as_str()
            .unwrap_or_default()
            .contains("The unopened File object")
    );
}
#[test]
fn completion_items_can_be_resolved_for_documentation() {
    let uri = "file:///tmp/resolve-completion.tua";
    let source = "local value = math.si\n";
    let mut server = TestServer::new();

    server.send(initialize_with_completion_label_details(1));
    server.send(did_open(uri, source));
    server.send(completion_request(2, uri, source, "math.si"));

    let completions = response_with_id(server.output(), 2)["result"]
        .as_array()
        .expect("completion result should be an array");
    let sin = completions
        .iter()
        .find(|completion| completion["label"] == "sin")
        .expect("math.sin completion should exist")
        .clone();

    assert_eq!(sin["data"]["uri"], uri);
    assert_eq!(sin["data"]["label"], "sin");
    assert_eq!(sin["data"]["kind"], "function");
    assert_eq!(
        sin["labelDetails"]["description"],
        "function math.sin(x: number): number"
    );
    assert!(sin.get("detail").is_none());
    assert!(sin.get("documentation").is_none());
    assert!(sin["data"].get("detail").is_none());
    assert!(sin["data"].get("documentation").is_none());

    server.send(completion_resolve_request(3, sin));
    let response = response_with_id(server.output(), 3);

    assert_eq!(response["result"]["label"], "sin");
    assert_eq!(
        response["result"]["detail"],
        "function math.sin(x: number): number"
    );
    assert_eq!(response["result"]["documentation"]["kind"], "markdown");
    assert!(response["result"]["documentation"]["value"]
        .as_str()
        .expect("documentation should be a string")
        .contains("Returns the sine."));
    assert_eq!(response["result"]["data"]["symbolKind"], "function");
    assert_eq!(
        response["result"]["data"]["typeLabel"],
        "function math.sin(x: number): number"
    );
}
#[test]
fn completion_resolve_uses_latest_snapshot_after_edits() {
    let uri = "file:///tmp/resolve-after-edit.tua";
    let source = "local value = math.si\n";
    let edited = "local value = math.si\nlocal other = 1\n";
    let mut server = TestServer::new();

    server.send(did_open(uri, source));
    server.send(completion_request(2, uri, source, "math.si"));

    let sin = response_with_id(server.output(), 2)["result"][0].clone();

    server.send(did_change(uri, edited, 2));
    server.send(completion_resolve_request(3, sin));

    let response = response_with_id(server.output(), 3);

    assert_eq!(
        response["result"]["detail"],
        "function math.sin(x: number): number"
    );
}
#[test]
fn completion_resolve_enriches_project_symbols_and_snippets() {
    let uri = "file:///tmp/resolve-project-symbols.tua";
    let source = r#"type Player = { hp: number }
local score = 1

local function damage(player: Player): Player
  return player
end

local typed: Pl = nil
local value = sco
local call = dam
---
local function heal(player: Player): Player
  return player
end
"#;
    let mut server = TestServer::new();

    server.send(initialize_with_completion_label_details(1));
    server.send(did_open(uri, source));
    server.send(completion_request(2, uri, source, "typed: Pl"));
    server.send(completion_request(3, uri, source, "value = sco"));
    server.send(completion_request(4, uri, source, "call = dam"));
    server.send(completion_request(5, uri, source, "---"));

    let alias = response_with_id(server.output(), 2)["result"][0].clone();
    let local = completion_with_label(server.output(), 3, "score");
    let function = completion_with_label(server.output(), 4, "damage");
    let snippet = completion_with_label(server.output(), 5, "document function");

    assert_eq!(alias["labelDetails"]["description"], "type alias");
    assert_eq!(local["labelDetails"]["description"], "local");
    assert_eq!(function["labelDetails"]["description"], "local function");
    assert_eq!(
        snippet["labelDetails"]["description"],
        "LuaDoc function documentation"
    );
    assert!(alias.get("detail").is_none());
    assert!(local.get("detail").is_none());
    assert!(function.get("detail").is_none());
    assert!(snippet.get("detail").is_none());

    server.send(completion_resolve_request(6, alias));
    server.send(completion_resolve_request(7, local));
    server.send(completion_resolve_request(8, function));
    server.send(completion_resolve_request(9, snippet));

    let alias = response_with_id(server.output(), 6);
    let local = response_with_id(server.output(), 7);
    let function = response_with_id(server.output(), 8);
    let snippet = response_with_id(server.output(), 9);

    assert_eq!(alias["result"]["detail"], "type alias");
    assert_eq!(alias["result"]["data"]["symbolKind"], "typeAlias");
    assert_eq!(local["result"]["detail"], "local");
    assert_eq!(local["result"]["data"]["symbolKind"], "variable");
    assert_eq!(function["result"]["detail"], "local function");
    assert_eq!(function["result"]["data"]["symbolKind"], "function");
    assert_eq!(snippet["result"]["detail"], "LuaDoc function documentation");
    assert_eq!(snippet["result"]["data"]["symbolKind"], "snippet");
    assert_eq!(snippet["result"]["insertTextFormat"], 2);
    assert!(snippet["result"]["insertText"]
        .as_str()
        .expect("snippet insert text should be a string")
        .contains("@param player Player"));
}
#[test]
fn completion_returns_auto_imported_exported_functions() {
    let temp = TempProject::new("auto-import-completion");
    let main = "local made = make\n";
    let player = r#"local function makePlayer(name: string)
  return { name = name }
end

return {
  makePlayer = makePlayer,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/player.tua", player);

    let main_uri = temp.uri_for("src/main.tua");
    let module_path = temp.path.join("src/player.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root_and_completion_label_details(
        1,
        temp.uri(),
    ));
    server.send(did_open(&main_uri, main));
    server.send(completion_request(2, &main_uri, main, "make"));

    let completion = completion_with_label(server.output(), 2, "makePlayer");

    assert_eq!(completion["kind"], 3);
    assert_eq!(completion["insertText"], "Player.makePlayer");
    assert_eq!(completion["data"]["sourceModule"], "player");
    assert_eq!(
        completion["data"]["modulePath"],
        module_path.to_string_lossy().as_ref()
    );
    assert_eq!(
        completion["labelDetails"]["description"],
        "auto import from player: (name: string) -> { name: string }"
    );
    assert!(completion.get("detail").is_none());
}
#[test]
fn completion_skips_auto_imports_for_single_character_prefixes() {
    let temp = TempProject::new("auto-import-short-prefix");
    let main = "local made = m\n";
    let player = r#"local function makePlayer(name: string)
  return { name = name }
end

return {
  makePlayer = makePlayer,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/player.tua", player);

    let main_uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root_and_completion_label_details(
        1,
        temp.uri(),
    ));
    server.send(did_open(&main_uri, main));
    server.send(completion_request(2, &main_uri, main, "m"));

    let labels = response_with_id(server.output(), 2)["result"]
        .as_array()
        .expect("completion result should be an array")
        .iter()
        .filter_map(|completion| completion["label"].as_str())
        .collect::<Vec<_>>();

    assert!(
        !labels.contains(&"makePlayer"),
        "single-character prefixes should not include auto-import candidates: {labels:?}"
    );
}
#[test]
fn completion_resolve_adds_child_import_once() {
    let temp = TempProject::new("auto-import-resolve");
    let main = "local made = make\n";
    let player = r#"---Creates a player.
local function makePlayer(name: string)
  return { name = name }
end

return {
  makePlayer = makePlayer,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/player.tua", player);

    let main_uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&main_uri, main));
    server.send(completion_request(2, &main_uri, main, "make"));

    let completion = completion_with_label(server.output(), 2, "makePlayer");

    server.send(completion_resolve_request(3, completion));

    let resolved = &response_with_id(server.output(), 3)["result"];

    assert_eq!(resolved["insertText"], "Player.makePlayer");
    assert_eq!(
        resolved["detail"],
        "auto import from player: (name: string) -> { name: string }"
    );
    assert_eq!(resolved["additionalTextEdits"].as_array().unwrap().len(), 1);
    assert_eq!(
        resolved["additionalTextEdits"][0]["newText"],
        "local Player = import(\"./player\")\n"
    );
    assert_eq!(
        resolved["additionalTextEdits"][0]["range"],
        json!({
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 0 }
        })
    );
}
#[test]
fn completion_resolve_uses_project_import_for_parent_modules() {
    let temp = TempProject::new("auto-import-parent-module");
    let main = "local made = make\n";
    let player = r#"local function makePlayer(name: string)
  return { name = name }
end

return {
  makePlayer = makePlayer,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/player.tua", player);

    let main_uri = temp.uri_for("src/systems/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&main_uri, main));
    server.send(completion_request(2, &main_uri, main, "make"));

    let completion = completion_with_label(server.output(), 2, "makePlayer");

    server.send(completion_resolve_request(3, completion));

    let resolved = &response_with_id(server.output(), 3)["result"];

    assert_eq!(
        resolved["additionalTextEdits"][0]["newText"],
        "local Player = import(\"player\")\n"
    );
}
#[test]
fn completion_reuses_existing_require_alias_for_auto_import() {
    let temp = TempProject::new("auto-import-existing-alias");
    let main = r#"local Functions = require("player")

local made = make
"#;
    let player = r#"local function makePlayer(name: string)
  return { name = name }
end

return {
  makePlayer = makePlayer,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/player.tua", player);

    let main_uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&main_uri, main));
    server.send(completion_request(2, &main_uri, main, "make"));

    let completion = completion_with_label(server.output(), 2, "makePlayer");

    assert_eq!(completion["insertText"], "Functions.makePlayer");

    server.send(completion_resolve_request(3, completion));

    let resolved = &response_with_id(server.output(), 3)["result"];

    assert_eq!(resolved["insertText"], "Functions.makePlayer");
    assert!(resolved.get("additionalTextEdits").is_none());
}
#[test]
fn completion_reuses_existing_import_alias_for_auto_import() {
    let temp = TempProject::new("auto-import-existing-import-alias");
    let main = r#"local Functions = import("./player")

local made = make
"#;
    let player = r#"local function makePlayer(name: string)
  return { name = name }
end

return {
  makePlayer = makePlayer,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/player.tua", player);

    let main_uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&main_uri, main));
    server.send(completion_request(2, &main_uri, main, "make"));

    let completion = completion_with_label(server.output(), 2, "makePlayer");

    assert_eq!(completion["insertText"], "Functions.makePlayer");

    server.send(completion_resolve_request(3, completion));

    let resolved = &response_with_id(server.output(), 3)["result"];

    assert_eq!(resolved["insertText"], "Functions.makePlayer");
    assert!(resolved.get("additionalTextEdits").is_none());
}
#[test]
fn completion_returns_fields_from_required_modules() {
    let temp = TempProject::new("required-module-completion");
    let source = r#"
local audio: any = require("audio")
local value = audio.play
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write(
        "src/audio.lua",
        r#"---@deprecated use newPlay
---@param source string audio source
---@return boolean success flag
local function play(source)
  return true
end

return {
  play = play,
  volume = 1,
}
"#,
    );

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(did_open(&uri, source));
    server.send(completion_request(2, &uri, source, "audio."));

    assert_completion_labels(server.output(), 2, ["play", "volume"]);

    let play = response_with_id(server.output(), 2)["result"]
        .as_array()
        .expect("completion result should be an array")
        .iter()
        .find(|completion| completion["label"] == "play")
        .expect("play completion should exist")
        .clone();

    assert!(play.get("detail").is_none());
    assert!(play.get("documentation").is_none());
    assert_eq!(play["tags"], json!([1]));

    server.send(completion_resolve_request(3, play));

    let response = response_with_id(server.output(), 3);

    assert_eq!(
        response["result"]["detail"],
        "field: (source: string) -> boolean"
    );
    assert!(response["result"]["documentation"]["value"]
        .as_str()
        .expect("documentation should be a string")
        .contains("audio source\nsuccess flag"));
    assert_eq!(response["result"]["data"]["symbolKind"], "function");
    assert_eq!(response["result"]["tags"], json!([1]));
    assert_eq!(
        response["result"]["data"]["typeLabel"],
        "field: (source: string) -> boolean"
    );
    assert_eq!(
        response["result"]["data"]["modulePath"],
        temp.path
            .join("src/audio.lua")
            .to_string_lossy()
            .to_string()
    );
}

#[test]
fn triggered_required_module_completion_uses_open_export_snapshot() {
    let temp = TempProject::new("open-required-module-completion");
    let main = "local audio = require(\"audio\")\nlocal value = audio.\n";
    let disk_audio = "return { play = function() end }\n";
    let open_audio = "return { play = function() end, pause = function() end }\n";
    let main_uri = temp.uri_for("src/main.tua");
    let audio_uri = temp.uri_for("src/audio.tua");
    let mut server = TestServer::new();

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", main);
    temp.write("src/audio.tua", disk_audio);
    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&audio_uri, open_audio));
    server.send(did_open(&main_uri, main));
    server.send(trigger_character_completion_request(
        2, &main_uri, main, "audio.", ".",
    ));

    assert_completion_labels(server.output(), 2, ["pause", "play"]);
}

#[test]
fn completion_and_local_hover_use_project_types_in_incomplete_documents() {
    let temp = TempProject::new("project-type-completion-incomplete");
    let source = r#"local LuaDocTypes = require("lua_doc_types")

local enemyFromLuaDocModule: LuaDocTypes.Enemy = {
  h
}

local a = enemyFromLuaDocModule
a.
local copied = enemyFromLuaDocModule
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
    temp.write("src/lua_doc_types.lua", lua_doc_types);

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    server.send(completion_request(2, &uri, source, "  h"));
    server.send(completion_request(3, &uri, source, "a."));
    server.send(hover_request(4, &uri, source, "enemyFromLuaDocModule\n"));

    assert_completion_labels(server.output(), 2, ["hp"]);
    assert_completion_labels(server.output(), 3, ["hp", "name", "type"]);
    assert_hover_contains(
        server.output(),
        4,
        "local enemyFromLuaDocModule: {\n  hp: number,\n  name: string,\n  type: \"friendly\" | \"mad\"\n}",
    );
}
#[test]
fn completion_and_definition_use_imported_function_return_alias_fields() {
    let fixture = Fixture::parse(
        r#"-- file: tua.toml
source = "src"
-- file: src/main.tua
local Functions = require("functions")

local makePlayer = Functions.makePlayer
local player = makePlayer("static import")
local direct = Functions.makePlayer("direct import")
local hp = player.h
local directHp = direct.h
local usage = player.hp
-- file: src/functions.tua
type Player = {
  name: string,
  hp: number,
  alive?: boolean,
}

local function makePlayer(name: string): Player
  return {
    name = name,
    hp = 100,
    alive = true,
  }
end

return {
  makePlayer = makePlayer,
}
"#,
    );
    let temp = fixture
        .write_to_temp("imported-return-alias-fields")
        .expect("fixture should write");
    let main = fixture.source("src/main.tua");
    let functions = fixture.source("src/functions.tua");

    let main_uri = path_uri(&temp.join("src/main.tua"));
    let functions_uri = path_uri(&temp.join("src/functions.tua"));
    let output = run_messages(vec![
        initialize_with_root(1, path_uri(temp.path())),
        did_open(&main_uri, main),
        completion_request(2, &main_uri, main, "player."),
        completion_request(3, &main_uri, main, "direct."),
        definition_request(4, &main_uri, main, "player.hp", "player.".len()),
    ]);

    completion_with_label(&output, 2, "hp");
    completion_with_label(&output, 2, "name");
    completion_with_label(&output, 3, "hp");
    assert_definition_text(&output, 4, &functions_uri, functions, "hp");
}
#[test]
fn completion_recovers_imported_function_return_alias_fields_while_typing() {
    let fixture = Fixture::parse(
        r#"-- file: tua.toml
source = "src"
-- file: src/main.tua
local Functions = require("functions")

local makePlayer = Functions.makePlayer
local player = makePlayer("static import")
local hp = player.
-- file: src/functions.tua
type Player = {
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
"#,
    );
    let temp = fixture
        .write_to_temp("imported-return-alias-fields-recovery")
        .expect("fixture should write");
    let main = fixture.source("src/main.tua");

    let main_uri = path_uri(&temp.join("src/main.tua"));
    let output = run_messages(vec![
        initialize_with_root(1, path_uri(temp.path())),
        did_open(&main_uri, main),
        completion_request(2, &main_uri, main, "player."),
    ]);

    completion_with_label(&output, 2, "hp");
    completion_with_label(&output, 2, "name");
}

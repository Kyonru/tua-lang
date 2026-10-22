use super::*;

#[test]
fn initialized_registers_watched_files_when_client_supports_dynamic_registration() {
    let output = run_messages(vec![
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "capabilities": {
                    "workspace": {
                        "didChangeWatchedFiles": {
                            "dynamicRegistration": true
                        }
                    }
                }
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }),
    ]);
    let registration = output
        .iter()
        .find(|message| message["method"] == "client/registerCapability")
        .expect("server should dynamically register watched files");

    assert_eq!(
        registration["params"]["registrations"][0]["method"],
        "workspace/didChangeWatchedFiles"
    );
    assert_eq!(
        registration["params"]["registrations"][0]["registerOptions"]["watchers"][0]["globPattern"],
        "**/*.tua"
    );
    let watchers = registration["params"]["registrations"][0]["registerOptions"]["watchers"]
        .as_array()
        .expect("watchers should be an array");
    assert!(
        watchers
            .iter()
            .any(|watcher| watcher["globPattern"] == "**/*.png"),
        "asset files should be watched: {watchers:?}"
    );
}
#[test]
fn workspace_symbol_returns_project_modules_aliases_and_functions() {
    let temp = TempProject::new("workspace-symbols");

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write(
        "src/player.tua",
        r#"type Player = { hp: number }

local function makePlayer(): Player
  return { hp = 10 }
end
"#,
    );
    temp.write(
        "src/systems/combat.tua",
        r#"type CombatEvent = { amount: number }

local function damage(player: Player, amount: number): Player
  return player
end
"#,
    );

    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        workspace_symbol_request(2, "plr"),
        workspace_symbol_request(3, "syscmb"),
    ]);
    let player_symbols = response_with_id(&output, 2)["result"]
        .as_array()
        .expect("workspace symbols should be an array");
    let combat_symbols = response_with_id(&output, 3)["result"]
        .as_array()
        .expect("workspace symbols should be an array");

    assert_eq!(
        workspace_symbol_names(player_symbols),
        vec!["player", "Player", "makePlayer"]
    );
    assert_eq!(player_symbols[0]["kind"], 2);
    assert_eq!(player_symbols[1]["kind"], 23);
    assert_eq!(player_symbols[2]["kind"], 12);
    assert_eq!(player_symbols[1]["containerName"], "player");
    assert_eq!(player_symbols[2]["containerName"], "player");
    assert_eq!(
        player_symbols[1]["location"]["uri"],
        temp.uri_for("src/player.tua")
    );

    assert_eq!(
        workspace_symbol_names(combat_symbols),
        vec!["systems.combat", "CombatEvent", "damage"]
    );
    assert_eq!(combat_symbols[0]["kind"], 2);
    assert_eq!(combat_symbols[1]["kind"], 23);
    assert_eq!(combat_symbols[2]["kind"], 12);
    assert_eq!(combat_symbols[1]["containerName"], "systems.combat");
    assert_eq!(combat_symbols[2]["containerName"], "systems.combat");
}

#[test]
fn configured_type_library_drives_expected_table_completion() {
    let temp = TempProject::new("type-library-completion");
    let main = r#"local feel = require("lib.feel")

feel.define("button.press", {
  { dummy },
})
"#;

    temp.write(
        "tua.toml",
        r#"source = "src"

[[type_libraries]]
path = "lib/feel/types"
declaration_prefix = "feel"
runtime_prefix = "lib.feel"
"#,
    );
    temp.write("src/main.tua", main);
    temp.write(
        "lib/feel/types/feel.lua",
        r#"---@meta feel
---@class AnimateStep
---@field kind "animate"
---@field duration number
---@class EmitStep
---@field kind "emit"
---@field event string
---@alias Step AnimateStep|EmitStep
---@param name string
---@param steps Step[]
local function define(name, steps) end
return { define = define }
"#,
    );

    let uri = temp.uri_for("src/main.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, main),
        completion_request(2, &uri, main, "  { "),
    ]);

    assert_completion_labels(&output, 2, ["duration", "event", "kind"]);
}

#[test]
fn recursive_type_library_aliases_are_safe_across_editor_queries() {
    let temp = TempProject::new("recursive-type-library");
    let main = r#"local feel = require("lib.feel")

local sequence = feel.define("button.press", {
  { dummy },
})
"#;

    temp.write(
        "tua.toml",
        r#"source = "src"

[[type_libraries]]
path = "lib/feel/types"
declaration_prefix = "feel"
runtime_prefix = "lib.feel"
"#,
    );
    temp.write("src/main.tua", main);
    temp.write(
        "lib/feel/types/feel.lua",
        r#"---@meta feel
---@class FeelAnimateStep
---@field kind "animate"
---@field duration number
---@class FeelParallelStep
---@field kind "parallel"
---@field steps FeelSequenceInput[]
---@alias FeelStep FeelAnimateStep|FeelParallelStep
---@alias FeelSequenceInput FeelStep|FeelStep[]
---@param name string
---@param steps FeelSequenceInput
---@return FeelStep[]
local function define(name, steps) end
return { define = define }
"#,
    );

    let uri = temp.uri_for("src/main.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, main),
        completion_request(2, &uri, main, "  { "),
        inlay_hint_request(3, &uri, 0, 0, 20, 0),
        hover_request(4, &uri, main, "sequence ="),
    ]);

    assert_completion_labels(&output, 2, ["duration", "kind", "steps"]);
    assert!(response_with_id(&output, 3)["result"].is_array());
    assert!(response_with_id(&output, 4)["result"].is_object());
}

#[test]
fn workspace_symbol_searches_all_workspace_folders() {
    let game = TempProject::new("workspace-folder-game");
    let tools = TempProject::new("workspace-folder-tools");

    game.write("tua.toml", "source = \"src\"\n");
    game.write(
        "src/player.tua",
        r#"type Player = { hp: number }

local function makePlayer(): Player
  return { hp = 10 }
end
"#,
    );
    tools.write("tua.toml", "source = \"src\"\n");
    tools.write(
        "src/enemy.tua",
        r#"type Enemy = { hp: number }

local function makeEnemy(): Enemy
  return { hp = 5 }
end
"#,
    );

    let output = run_messages(vec![
        initialize_with_workspace_folders(1, [game.uri(), tools.uri()]),
        workspace_symbol_request(2, "Player"),
        workspace_symbol_request(3, "Enemy"),
    ]);
    let player_symbols = response_with_id(&output, 2)["result"]
        .as_array()
        .expect("player symbols should be an array");
    let enemy_symbols = response_with_id(&output, 3)["result"]
        .as_array()
        .expect("enemy symbols should be an array");

    assert!(workspace_symbol_names(player_symbols).contains(&"Player"));
    assert!(workspace_symbol_names(enemy_symbols).contains(&"Enemy"));
    assert_eq!(
        enemy_symbols[0]["location"]["uri"],
        tools.uri_for("src/enemy.tua")
    );
}
#[test]
fn workspace_folder_changes_update_workspace_symbol_roots() {
    let game = TempProject::new("workspace-folder-change-game");
    let tools = TempProject::new("workspace-folder-change-tools");

    game.write("tua.toml", "source = \"src\"\n");
    game.write("src/player.tua", "type Player = { hp: number }\n");
    tools.write("tua.toml", "source = \"src\"\n");
    tools.write("src/enemy.tua", "type Enemy = { hp: number }\n");

    let output = run_messages(vec![
        initialize_with_workspace_folders(1, [game.uri()]),
        workspace_symbol_request(2, "Enemy"),
        workspace_folders_changed([tools.uri()], []),
        workspace_symbol_request(3, "Enemy"),
        workspace_folders_changed([], [tools.uri()]),
        workspace_symbol_request(4, "Enemy"),
    ]);

    assert_eq!(response_with_id(&output, 2)["result"], json!([]));
    assert!(workspace_symbol_names(
        response_with_id(&output, 3)["result"]
            .as_array()
            .expect("added folder symbols should be an array")
    )
    .contains(&"Enemy"));
    assert_eq!(response_with_id(&output, 4)["result"], json!([]));
}
#[test]
fn workspace_symbol_empty_or_unavailable_workspace_returns_empty_array() {
    let temp = TempProject::new("empty-workspace-symbols");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        workspace_symbol_request(2, "missing"),
    ]);
    let response = response_with_id(&output, 2);

    assert_eq!(response["result"], json!([]));
}
#[test]
fn did_open_accepts_love_assets_from_configured_output_root() {
    let temp = TempProject::new("configured-output-asset");
    let source = r#"
local player = image "assets/player.png"
return player
"#;

    temp.write("tua.toml", "source = \"src\"\nout = \"game\"\n");
    temp.write("src/assets.tua", source);
    temp.write("game/assets/player.png", "");

    let uri = temp.uri_for("src/assets.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
    ]);
    let diagnostics = last_published_diagnostics(&output, &uri);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}
#[test]
fn watched_asset_changes_refresh_asset_index_and_republish_open_diagnostics() {
    let temp = TempProject::new("watched-asset-index");
    let source = r#"
local player = image "assets/player.png"
return player
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);

    let uri = temp.uri_for("src/main.tua");
    let asset_uri = temp.uri_for("assets/player.png");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    assert_eq!(
        last_published_diagnostics(server.output(), &uri)[0]["code"],
        "TL2004"
    );

    temp.write("assets/player.png", "placeholder");
    server.send(watched_files_changed([(&asset_uri, 1)]));
    server.send(completion_request(2, &uri, source, "assets/p"));
    let diagnostics = last_published_diagnostics(server.output(), &uri);

    assert!(
        diagnostics.is_empty(),
        "asset refresh should clear missing-asset diagnostics: {diagnostics:#?}"
    );
    assert_completion_labels(server.output(), 2, ["assets/player.png"]);
}
#[test]
fn did_change_uses_warmed_asset_index_until_watched_asset_refresh() {
    let temp = TempProject::new("asset-index-no-request-scan");
    let source = r#"
local player = image "assets/player.png"
return player
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("assets/player.png", "placeholder");

    let uri = temp.uri_for("src/main.tua");
    let asset_uri = temp.uri_for("assets/player.png");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    assert!(
        last_published_diagnostics(server.output(), &uri).is_empty(),
        "indexed asset should be valid on open"
    );

    temp.remove("assets/player.png");
    let edited = format!("{source}-- touched\n");

    server.send(did_change(&uri, &edited, 2));
    assert!(
        last_published_diagnostics(server.output(), &uri).is_empty(),
        "ordinary edits should reuse the warmed asset index instead of rescanning disk"
    );

    server.send(completion_request(2, &uri, &edited, "assets/p"));
    assert_completion_labels(server.output(), 2, ["assets/player.png"]);

    server.send(watched_files_changed([(&asset_uri, 3)]));
    let diagnostics = last_published_diagnostics(server.output(), &uri);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["code"] == "TL2004"),
        "watched asset deletion should refresh the asset index: {diagnostics:#?}"
    );

    server.send(completion_request(3, &uri, &edited, "assets/p"));
    assert_completion_labels(server.output(), 3, []);
}
#[test]
fn did_open_resolves_imports_from_nested_project_config() {
    let temp = TempProject::new("nested-import-config");

    temp.write(
        "examples/features/tua.toml",
        r#"source = "src"

[diagnostics]
missing_requires = true
"#,
    );
    temp.write(
        "examples/features/src/imports.tua",
        "local Functions = import(\"./functions\")\nreturn Functions\n",
    );
    temp.write("examples/features/src/functions.tua", "return {}\n");

    let uri = temp.uri_for("examples/features/src/imports.tua");
    let source = fs::read_to_string(temp.path.join("examples/features/src/imports.tua"))
        .expect("fixture source should exist");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, &source));
    let diagnostics = last_published_diagnostics(server.output(), &uri);

    assert_eq!(diagnostics, Vec::<Value>::new());
}
#[test]
fn did_open_resolves_imports_from_nearest_config_without_workspace_root() {
    let temp = TempProject::new("open-file-import-config");

    temp.write(
        "examples/features/tua.toml",
        r#"source = "src"

[diagnostics]
missing_requires = true
"#,
    );
    temp.write(
        "examples/features/src/imports.tua",
        "local Functions = import(\"./functions\")\nreturn Functions\n",
    );
    temp.write("examples/features/src/functions.tua", "return {}\n");

    let uri = temp.uri_for("examples/features/src/imports.tua");
    let source = fs::read_to_string(temp.path.join("examples/features/src/imports.tua"))
        .expect("fixture source should exist");
    let mut server = TestServer::new();

    server.send(did_open(&uri, &source));
    let diagnostics = last_published_diagnostics(server.output(), &uri);

    assert_eq!(diagnostics, Vec::<Value>::new());
}
#[test]
fn watched_child_import_create_clears_missing_import_diagnostic() {
    let temp = TempProject::new("watched-child-import-create");
    let main = "local Functions = import(\"./functions\")\nreturn Functions\n";

    temp.write(
        "tua.toml",
        r#"source = "src"

[diagnostics]
missing_requires = true
"#,
    );
    temp.write("src/imports.tua", main);

    let imports_uri = temp.uri_for("src/imports.tua");
    let functions_uri = temp.uri_for("src/functions.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&imports_uri, main));

    let initial = last_published_diagnostics(server.output(), &imports_uri);
    assert!(initial
        .iter()
        .any(|diagnostic| diagnostic["code"] == "TL4002"));

    temp.write("src/functions.tua", "return {}\n");
    server.send(watched_files_changed([(&functions_uri, 1)]));

    assert_eq!(
        last_published_diagnostics(server.output(), &imports_uri),
        Vec::<Value>::new()
    );
}
#[test]
fn pull_diagnostics_refresh_after_configuration_change() {
    let temp = TempProject::new("pull-configuration-refresh");
    let source = "local unused = 1\n";

    temp.write(
        "tua.toml",
        r#"source = "src"

[diagnostics]
rules = false
unused_locals = false
"#,
    );

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_pull_diagnostics(1, temp.uri(), true));
    server.send(did_open(&uri, source));
    server.send(text_document_diagnostic_request(2, &uri));

    assert!(document_diagnostic_items(server.output(), 2).is_empty());

    server.send(did_change_configuration(json!({
        "diagnostics": {
            "unusedLocals": true
        }
    })));

    assert_eq!(diagnostic_refresh_requests(server.output()).len(), 1);

    server.send(text_document_diagnostic_request(3, &uri));

    assert_eq!(
        diagnostic_codes(&document_diagnostic_items(server.output(), 3)),
        vec!["TL4005"]
    );
}
#[test]
fn pull_diagnostics_refresh_after_watched_dependency_change() {
    let temp = TempProject::new("pull-watched-dependency-refresh");
    let main = r#"local Player = require("player")

local made = Player.makePlayer("Ada")
return made
"#;
    let player = r#"local function makePlayer(name)
  return { name = name }
end

return {
  makePlayer = makePlayer,
}
"#;

    temp.write(
        "tua.toml",
        r#"source = "src"

[diagnostics]
missing_requires = true
"#,
    );
    temp.write("src/main.tua", main);

    let main_uri = temp.uri_for("src/main.tua");
    let player_uri = temp.uri_for("src/player.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_pull_diagnostics(1, temp.uri(), true));
    server.send(did_open(&main_uri, main));
    server.send(text_document_diagnostic_request(2, &main_uri));

    assert_eq!(
        diagnostic_codes(&document_diagnostic_items(server.output(), 2)),
        vec!["TL4002"]
    );

    temp.write("src/player.tua", player);
    server.send(watched_files_changed([(&player_uri, 1)]));

    assert_eq!(diagnostic_refresh_requests(server.output()).len(), 1);

    server.send(text_document_diagnostic_request(3, &main_uri));

    assert!(document_diagnostic_items(server.output(), 3).is_empty());
}
#[test]
fn watched_file_changes_republish_open_document_diagnostics() {
    let uri = "file:///tmp/watched.tua";
    let output = run_messages(vec![
        did_open(uri, "local hp: number = \"full\"\n"),
        json!({
            "jsonrpc": "2.0",
            "method": "workspace/didChangeWatchedFiles",
            "params": {
                "changes": [
                    {
                        "uri": uri,
                        "type": 2
                    }
                ]
            }
        }),
    ]);
    let publications = diagnostics_publications(&output, uri);

    assert_eq!(publications.len(), 2);
    assert_eq!(
        publications[1]["params"]["diagnostics"][0]["code"],
        "TL1003"
    );
}
#[test]
fn watched_file_changes_publish_closed_tua_file_diagnostics() {
    let temp = TempProject::new("watched-closed-file");

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/new_module.tua", "local hp: number = \"full\"\n");

    let uri = temp.uri_for("src/new_module.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        watched_files_changed([(&uri, 1)]),
    ]);
    let publications = diagnostics_publications(&output, &uri);

    assert_eq!(publications.len(), 1);
    assert_eq!(
        publications[0]["params"]["diagnostics"][0]["code"],
        "TL1003"
    );
}
#[test]
fn watched_file_delete_clears_diagnostics_and_removes_open_document() {
    let uri = "file:///tmp/deleted-watched.tua";
    let source = "local hp: number = \"full\"\n";
    let output = run_messages(vec![
        did_open(uri, source),
        watched_files_changed([(uri, 3)]),
        hover_request(2, uri, source, "hp"),
    ]);
    let publications = diagnostics_publications(&output, uri);

    assert_eq!(publications.len(), 2);
    assert_eq!(
        publications[1]["params"]["diagnostics"]
            .as_array()
            .expect("delete diagnostics should be an array")
            .len(),
        0
    );
    assert!(response_with_id(&output, 2)["result"].is_null());
}
#[test]
fn watched_required_module_create_updates_intelligence_and_diagnostics() {
    let temp = TempProject::new("watched-module-create");
    let main = r#"local Player = require("player")

local made = Player.makePlayer("Ada")
local hp = Player.defaultHp
return { made = made, hp = hp }
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

    temp.write(
        "tua.toml",
        r#"source = "src"

[diagnostics]
missing_requires = true
"#,
    );
    temp.write("src/main.tua", main);

    let main_uri = temp.uri_for("src/main.tua");
    let player_uri = temp.uri_for("src/player.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&main_uri, main));
    assert_eq!(
        last_published_diagnostics(server.output(), &main_uri)[0]["code"],
        "TL4002"
    );

    temp.write("src/player.tua", player);
    server.send(watched_files_changed([(&player_uri, 1)]));
    assert_eq!(
        last_published_diagnostics(server.output(), &main_uri).len(),
        0
    );

    server.send(completion_request(2, &main_uri, main, "Player."));
    server.send(hover_request(3, &main_uri, main, "makePlayer("));
    server.send(definition_request(
        4,
        &main_uri,
        main,
        "Player.makePlayer",
        "Player.".len(),
    ));

    assert_completion_labels(server.output(), 2, ["defaultHp", "makePlayer"]);
    assert_hover_contains(
        server.output(),
        3,
        "function Player.makePlayer(name: string): Player",
    );
    assert_definition_text(server.output(), 4, &player_uri, player, "makePlayer");
}
#[test]
fn snapshot_backed_requests_work_after_edit_and_watched_file_change() {
    let temp = TempProject::new("snapshot-backed-requests");
    let main = r#"local Player = require("player")

local player = Player.makePlayer("Ada")

local function spawn(name: string)
  return Player.makePlayer(name)
end

local again = spawn("Bea")
local pending = Player.make

return {
  spawn = spawn,
}
"#;
    let edited = r#"local Player = require("player")

local player = Player.makePlayer("Ada")

local function spawn(name: string)
  return Player.makePlayer(name)
end

local again = spawn("Bea")
local edited = Player.makePlayer("Cal")
local pending = Player.make

return {
  spawn = spawn,
}
"#;
    let player = r#"type Player = { name: string, hp: number }

local function makePlayer(name: string): Player
  return { name = name, hp = 10 }
end

return {
  makePlayer = makePlayer,
}
"#;
    let changed_player = r#"type Player = { name: string, hp: number }

local function makePlayer(name: string): Player
  return { name = name, hp = 10 }
end

local function makeEnemy(name: string): Player
  return { name = name, hp = 3 }
end

return {
  makeEnemy = makeEnemy,
  makePlayer = makePlayer,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", main);
    temp.write("src/player.tua", player);

    let main_uri = temp.uri_for("src/main.tua");
    let player_uri = temp.uri_for("src/player.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&main_uri, main));
    server.send(did_change(&main_uri, edited, 2));
    temp.write("src/player.tua", changed_player);
    server.send(watched_files_changed([(&player_uri, 2)]));

    server.send(hover_request(3, &main_uri, edited, "makePlayer(\"Cal\")"));
    server.send(completion_request(4, &main_uri, edited, "Player.make"));
    let completion = completion_with_label(server.output(), 4, "makeEnemy");
    server.send(completion_resolve_request(5, completion));
    server.send(signature_help_request(
        6,
        &main_uri,
        edited,
        "makePlayer(\"Cal",
    ));
    server.send(definition_request(
        7,
        &main_uri,
        edited,
        "Player.makePlayer",
        "Player.".len(),
    ));
    server.send(document_link_request(8, &main_uri));
    server.send(document_highlight_request(
        9,
        &main_uri,
        edited,
        "spawn(name",
        0,
    ));
    server.send(document_symbol_request(10, &main_uri));
    server.send(references_request(
        11,
        &main_uri,
        edited,
        "spawn(name",
        0,
        true,
    ));
    server.send(prepare_rename_request(
        12,
        &main_uri,
        edited,
        "spawn(name",
        0,
    ));
    server.send(rename_request(
        13,
        &main_uri,
        edited,
        "spawn(name",
        0,
        "spawnPlayer",
    ));
    server.send(inlay_hint_request(14, &main_uri, 0, 0, 20, 0));
    server.send(semantic_tokens_request(15, &main_uri));
    server.send(workspace_symbol_request(16, "makeEnemy"));

    assert_hover_contains(
        server.output(),
        3,
        "function Player.makePlayer(name: string): Player",
    );
    assert_completion_labels(server.output(), 4, ["makeEnemy", "makePlayer"]);
    assert_eq!(
        response_with_id(server.output(), 5)["result"]["label"],
        "makeEnemy"
    );
    assert!(response_with_id(server.output(), 6)["result"]["signatures"].is_array());
    assert_definition_text(
        server.output(),
        7,
        &player_uri,
        changed_player,
        "makePlayer",
    );
    assert!(!response_with_id(server.output(), 8)["result"]
        .as_array()
        .expect("document links should be an array")
        .is_empty());
    assert!(!response_with_id(server.output(), 9)["result"]
        .as_array()
        .expect("document highlights should be an array")
        .is_empty());
    assert!(workspace_symbol_names(
        response_with_id(server.output(), 10)["result"]
            .as_array()
            .expect("document symbols should be an array")
    )
    .contains(&"spawn"));
    assert!(!response_with_id(server.output(), 11)["result"]
        .as_array()
        .expect("references should be an array")
        .is_empty());
    assert_eq!(
        response_with_id(server.output(), 12)["result"]["placeholder"],
        "spawn"
    );
    assert!(!response_with_id(server.output(), 13)["result"]["changes"]
        .as_object()
        .expect("rename should return workspace changes")
        .is_empty());
    assert!(!response_with_id(server.output(), 14)["result"]
        .as_array()
        .expect("inlay hints should be an array")
        .is_empty());
    assert!(!response_with_id(server.output(), 15)["result"]["data"]
        .as_array()
        .expect("semantic tokens should have data")
        .is_empty());
    assert!(workspace_symbol_names(
        response_with_id(server.output(), 16)["result"]
            .as_array()
            .expect("workspace symbols should be an array")
    )
    .contains(&"makeEnemy"));
}
#[test]
fn watched_required_module_delete_clears_exports_and_reports_missing_require() {
    let temp = TempProject::new("watched-module-delete");
    let main = r#"local Player = require("player")

local made = Player.makePlayer("Ada")
return made
"#;
    let player = r#"local function makePlayer(name)
  return { name = name }
end

return {
  makePlayer = makePlayer,
}
"#;

    temp.write(
        "tua.toml",
        r#"source = "src"

[diagnostics]
missing_requires = true
"#,
    );
    temp.write("src/main.tua", main);
    temp.write("src/player.tua", player);

    let main_uri = temp.uri_for("src/main.tua");
    let player_uri = temp.uri_for("src/player.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&main_uri, main));
    assert_eq!(
        last_published_diagnostics(server.output(), &main_uri).len(),
        0
    );

    temp.remove("src/player.tua");
    server.send(watched_files_changed([(&player_uri, 3)]));
    let diagnostics = last_published_diagnostics(server.output(), &main_uri);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["code"], "TL4002");

    server.send(completion_request(2, &main_uri, main, "Player."));
    server.send(hover_request(3, &main_uri, main, "makePlayer("));
    server.send(definition_request(
        4,
        &main_uri,
        main,
        "Player.makePlayer",
        "Player.".len(),
    ));

    assert_completion_labels(server.output(), 2, []);
    assert!(response_with_id(server.output(), 3)["result"].is_null());
    assert!(response_with_id(server.output(), 4)["result"].is_null());
}
#[test]
fn watched_config_change_republishes_open_diagnostics_with_new_settings() {
    let temp = TempProject::new("watched-config-change");
    let main = r#"local Missing = require("missing")
return Missing
"#;

    temp.write(
        "tua.toml",
        "source = \"src\"\n\n[diagnostics]\nmissing_requires = false\n",
    );
    temp.write("src/main.tua", main);

    let config_uri = temp.uri_for("tua.toml");
    let main_uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&main_uri, main));
    let initial_diagnostics = last_published_diagnostics(server.output(), &main_uri);
    assert_eq!(initial_diagnostics.len(), 0, "{initial_diagnostics:#?}");

    temp.write(
        "tua.toml",
        r#"source = "src"

[diagnostics]
missing_requires = true
"#,
    );
    server.send(watched_files_changed([(&config_uri, 2)]));

    let diagnostics = last_published_diagnostics(server.output(), &main_uri);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["code"], "TL4002");
}
#[test]
fn did_change_configuration_republishes_diagnostic_overrides() {
    let temp = TempProject::new("configuration-diagnostic-toggle");
    let source = "local unused = 1\n";

    temp.write(
        "tua.toml",
        r#"source = "src"

[diagnostics]
rules = false
unused_locals = false
"#,
    );

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    let diagnostics = last_published_diagnostics(server.output(), &uri);

    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    server.send(did_change_configuration(json!({
        "diagnostics": {
            "unusedLocals": true
        }
    })));

    let diagnostics = last_published_diagnostics(server.output(), &uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["code"], "TL4005");
}

#[test]
fn did_change_configuration_can_disable_default_rules() {
    let temp = TempProject::new("configuration-lint-rules");
    let source = "local first, first = 1, 2\nreturn first\n";

    temp.write("tua.toml", "source = \"src\"\n");

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    assert!(last_published_diagnostics(server.output(), &uri)
        .iter()
        .any(|diagnostic| diagnostic["code"] == "TL4043"));

    server.send(did_change_configuration(json!({
        "diagnostics": {
            "rules": false
        }
    })));

    let diagnostics = last_published_diagnostics(server.output(), &uri);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn did_change_configuration_can_enable_luahelper_parity_lints() {
    let temp = TempProject::new("configuration-luahelper-parity-lints");
    let source = r#"
local a, b = 1
a = 1, 2

local assigned = 1
assigned = 2

local bad = 1 + "hp"

local function callback()
end

local function complex(first: boolean, second: boolean)
  if first and second then
    return 1
  end
  return 0
end

local divided = 1 / 0
local left, right = 1, 2
left = right
right = left
local mixed = { "player", hp = 10 }
local game: { score: number } = { score = 0 }
game.socre = 10

print(callback)
print(complex(true, false), divided, left, right, mixed)
"#;

    temp.write(
        "tua.toml",
        "source = \"src\"\n\n[diagnostics]\nrules = false\n",
    );

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    assert_eq!(last_published_diagnostics(server.output(), &uri).len(), 0);

    server.send(did_change_configuration(json!({
        "diagnostics": {
            "assignmentArity": true,
            "localDeclarationArity": true,
            "unusedAssignments": true,
            "binaryOperatorTypeMismatches": true,
            "uncalledLocalFunctions": true,
            "maxCyclomaticComplexity": 2,
            "divideByZero": true,
            "almostSwapped": true,
            "mixedTables": true,
            "unknownRecordFields": true
        }
    })));

    let diagnostics = last_published_diagnostics(server.output(), &uri);
    let codes = diagnostics
        .iter()
        .map(|diagnostic| diagnostic["code"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();

    assert!(codes.contains(&"TL4027"));
    assert!(codes.contains(&"TL4028"));
    assert!(codes.contains(&"TL4029"));
    assert!(codes.contains(&"TL4030"));
    assert!(codes.contains(&"TL4031"));
    assert!(codes.contains(&"TL4070"));
    assert!(codes.contains(&"TL4071"));
    assert!(codes.contains(&"TL4072"));
    assert!(codes.contains(&"TL4073"));
    assert!(codes.contains(&"TL4074"));
}
#[test]
fn did_change_configuration_can_enable_jit_guidance() {
    let temp = TempProject::new("configuration-jit-guidance");
    let source = r#"
local player = { hp = 10 }
player.name = "late"
"#;

    temp.write("tua.toml", "source = \"src\"\n");

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    assert_eq!(last_published_diagnostics(server.output(), &uri).len(), 0);

    server.send(did_change_configuration(json!({
        "jit": {
            "enabled": true
        }
    })));

    let diagnostics = last_published_diagnostics(server.output(), &uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["code"], "TL4033");
}
#[test]
fn did_change_configuration_can_disable_ecs_domain_support() {
    let temp = TempProject::new("configuration-ecs-domain");
    let source = r#"local Position = ecs.component("Position", { x = "number", y = "number" })
local world = ecs.world()
local entity = world:create()
world:add(entity, Position, { x = 1 })
"#;

    temp.write("tua.toml", "source = \"src\"\n");

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    let initial = last_published_diagnostics(server.output(), &uri);

    assert_eq!(initial.len(), 1);
    assert_eq!(initial[0]["code"], "TL5002");

    server.send(did_change_configuration(json!({
        "ecs": {
            "enabled": false
        }
    })));

    let diagnostics = last_published_diagnostics(server.output(), &uri);

    assert_eq!(diagnostics.len(), 0);
}
#[test]
fn did_change_configuration_can_disable_inline_ignores() {
    let temp = TempProject::new("configuration-inline-ignores");
    let source = r#"-- tua-ignore-next-line TL1003
local value: number = "legacy"
print(value)
"#;

    temp.write("tua.toml", "source = \"src\"\n");

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    assert_eq!(last_published_diagnostics(server.output(), &uri).len(), 0);

    server.send(did_change_configuration(json!({
        "diagnostics": {
            "inlineIgnores": false
        }
    })));

    let diagnostics = last_published_diagnostics(server.output(), &uri);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0]["code"], "TL1003");
}
#[test]
fn did_change_configuration_null_diagnostics_defer_to_project_config() {
    let temp = TempProject::new("configuration-diagnostic-precedence");
    let source = "local unused = 1\n";

    temp.write(
        "tua.toml",
        r#"source = "src"

[diagnostics]
rules = false
unused_locals = true
"#,
    );

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    assert_eq!(last_published_diagnostics(server.output(), &uri).len(), 1);

    server.send(did_change_configuration(json!({
        "diagnostics": {
            "unusedLocals": null
        }
    })));

    let diagnostics = last_published_diagnostics(server.output(), &uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["code"], "TL4005");

    server.send(did_change_configuration(json!({
        "diagnostics": {
            "unusedLocals": false
        }
    })));

    assert_eq!(last_published_diagnostics(server.output(), &uri).len(), 0);
}
#[test]
fn did_change_configuration_file_associations_index_lua_interop_sources() {
    let temp = TempProject::new("configuration-file-associations");
    let main = "local value = cla\nlocal Legacy = import(\"leg\")\n";
    let legacy = r#"local function clamp(value)
  return value
end

return {
  clamp = clamp,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", main);
    temp.write("src/legacy.script", legacy);

    let main_uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&main_uri, main));
    server.send(completion_request(2, &main_uri, main, "cla"));
    let initial_labels = response_with_id(server.output(), 2)["result"]
        .as_array()
        .expect("completion result should be an array")
        .iter()
        .filter_map(|item| item["label"].as_str())
        .collect::<Vec<_>>();

    assert!(initial_labels.contains(&"class"), "{initial_labels:?}");
    assert!(!initial_labels.contains(&"clamp"), "{initial_labels:?}");

    server.send(did_change_configuration(json!({
        "fileAssociations": {
            "*.script": "lua"
        }
    })));
    server.send(completion_request(3, &main_uri, main, "cla"));

    let completion = completion_with_label(server.output(), 3, "clamp");

    assert_eq!(completion["insertText"], "Legacy.clamp");
    assert_eq!(completion["data"]["sourceModule"], "legacy");

    server.send(completion_request(4, &main_uri, main, "import(\"leg"));

    assert_completion_labels(server.output(), 4, ["legacy"]);
}
#[test]
fn did_change_configuration_ignored_paths_skip_indexing_and_diagnostics() {
    let temp = TempProject::new("configuration-ignored-paths");
    let ignored = r#"local unused = 1

local function makeIgnored()
  return {}
end

return {
  makeIgnored = makeIgnored,
}
"#;
    let main = "local value = make\n";

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/generated/ignored.tua", ignored);
    temp.write("src/main.tua", main);

    let ignored_uri = temp.uri_for("src/generated/ignored.tua");
    let main_uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_change_configuration(json!({
        "diagnostics": {
            "unusedLocals": true
        },
        "ignoredPaths": [
            "src/generated/**"
        ]
    })));
    server.send(did_open(&ignored_uri, ignored));
    server.send(did_open(&main_uri, main));
    server.send(completion_request(2, &main_uri, main, "make"));

    assert_eq!(
        last_published_diagnostics(server.output(), &ignored_uri).len(),
        0
    );
    assert_completion_labels(server.output(), 2, []);
}
#[test]
fn nested_project_type_aliases_resolve_when_outer_workspace_is_open() {
    let temp = TempProject::new("nested-project-type-aliases");
    let source = r#"local Types = require("types")

local playerFromTuaModule: Types.Player = nil
"#;
    let types = r#"export type Player = { hp: number }

return {}
"#;

    temp.write("examples/features/tua.toml", "source = \"src\"\n");
    temp.write("examples/features/src/main.tua", source);
    temp.write("examples/features/src/types.tua", types);

    let uri = temp.uri_for("examples/features/src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    server.send(hover_request(2, &uri, source, "Player = nil"));

    let diagnostics = last_published_diagnostics(server.output(), &uri);

    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {diagnostics:#?}"
    );
    assert_hover_contains(server.output(), 2, "type Types.Player = { hp: number }");
}

use super::*;
use serde_json::{json, Value};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};
use tua_core::{Fixture, SourcePosition};

#[test]
fn exposes_server_name() {
    assert_eq!(server_name(), "tua-lsp");
}

#[test]
fn describes_tua_language_server() {
    assert!(server_description().contains("language server"));
}

#[test]
fn initialize_returns_server_capabilities() {
    let output = run_messages(vec![json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    })]);

    assert_eq!(output.len(), 1);
    assert_eq!(output[0]["id"], 1);
    assert_eq!(output[0]["result"]["serverInfo"]["name"], "tua-lsp");
    assert_eq!(
        output[0]["result"]["capabilities"]["textDocumentSync"]["openClose"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["textDocumentSync"]["change"],
        2
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["textDocumentSync"]["save"]["includeText"],
        true
    );
    assert_eq!(output[0]["result"]["capabilities"]["hoverProvider"], true);
    assert_eq!(
        output[0]["result"]["capabilities"]["completionProvider"]["triggerCharacters"],
        json!([":", ".", "\"", "'", "@", "-"])
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["completionProvider"]["resolveProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["signatureHelpProvider"]["triggerCharacters"],
        json!(["(", ","])
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["definitionProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["callHierarchyProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["documentLinkProvider"]["resolveProvider"],
        false
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["documentHighlightProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["documentSymbolProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["codeActionProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["referencesProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["workspaceSymbolProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["renameProvider"]["prepareProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["inlayHintProvider"]["resolveProvider"],
        false
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["semanticTokensProvider"]["legend"]["tokenTypes"],
        json!([
            "typeAlias",
            "function",
            "variable",
            "parameter",
            "property",
            "builtinType",
            "unresolvedReference",
            "global",
            "module",
            "method",
            "annotationTag"
        ])
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["semanticTokensProvider"]["legend"]["tokenModifiers"],
        json!(["declaration", "readonly", "deprecated"])
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["semanticTokensProvider"]["full"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["workspace"]["workspaceFolders"]["supported"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["workspace"]["workspaceFolders"]["changeNotifications"],
        true
    );
}

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
fn did_open_publishes_matching_core_diagnostics() {
    let uri = "file:///tmp/player.tua";
    let output = run_messages(vec![
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "tua",
                    "version": 1,
                    "text": "local hp: number = \"full\"\n"
                }
            }
        }),
    ]);

    let diagnostics = published_diagnostics(&output, uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["severity"], 1);
    assert_eq!(diagnostics[0]["code"], "TL1003");
    assert_eq!(diagnostics[0]["source"], "tua");
    assert_eq!(diagnostics[0]["range"]["start"]["line"], 0);
    assert_eq!(diagnostics[0]["range"]["start"]["character"], 19);
    assert!(diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message should be a string")
        .contains("type mismatch: expected number, found string"));
}

#[test]
fn did_open_publishes_love_method_argument_diagnostics() {
    let uri = "file:///tmp/assets.tua";
    let source = r#"
local player: Image = love.graphics.newImage("assets/player.png")

player:setFilter(123)
"#;
    let output = run_messages(vec![did_open(uri, source)]);
    let diagnostics = published_diagnostics(&output, uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["severity"], 1);
    assert_eq!(diagnostics[0]["code"], "TL1003");
    assert!(diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message should be a string")
        .contains("type mismatch: expected FilterMode, found number"));
}

#[test]
fn did_open_publishes_love_asset_path_diagnostics() {
    let temp = TempProject::new("missing-asset-diagnostics");
    let source = r#"
local player = image "assets/missing.png"
"#;

    temp.write("tua.toml", "source = \"src\"\nout = \"game\"\n");
    temp.write("src/assets.tua", source);

    let uri = temp.uri_for("src/assets.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
    ]);
    let diagnostics = published_diagnostics(&output, &uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["severity"], 1);
    assert_eq!(diagnostics[0]["code"], "TL2004");
    assert!(diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message should be a string")
        .contains("image asset not found: assets/missing.png"));
}

#[test]
fn did_open_accepts_love_assets_from_configured_output_root() {
    let temp = TempProject::new("configured-output-asset");
    let source = r#"
local player = image "assets/player.png"
"#;

    temp.write("tua.toml", "source = \"src\"\nout = \"game\"\n");
    temp.write("src/assets.tua", source);
    temp.write("game/assets/player.png", "");

    let uri = temp.uri_for("src/assets.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
    ]);
    let diagnostics = published_diagnostics(&output, &uri);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn did_open_publishes_impossible_comparison_warnings() {
    let uri = "file:///tmp/comparison.tua";
    let source = r#"local function recursive(a: string)
  if a == 1 then
    return 1
  end
end
"#;
    let output = run_messages(vec![did_open(uri, source)]);
    let diagnostics = published_diagnostics(&output, uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["severity"], 2);
    assert_eq!(diagnostics[0]["code"], "TL1005");
    assert!(diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message should be a string")
        .contains("comparison is always false: string and number do not overlap"));
}

#[test]
fn did_open_uses_project_diagnostics_config() {
    let temp = TempProject::new("diagnostics-config");

    temp.write(
        "tua.toml",
        r#"
source = "src"

[diagnostics]
duplicate_keys = true
undefined_variables = true
"#,
    );

    let uri = temp.uri_for("src/main.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, "local config = { hp = 10, hp = 20 }\nmissing()\n"),
    ]);
    let diagnostics = published_diagnostics(&output, &uri);

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic["severity"] == 2
            && diagnostic["code"] == "TL4001"
            && diagnostic["message"]
                .as_str()
                .expect("diagnostic message should be a string")
                .contains("duplicate table key `hp`")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic["severity"] == 2
            && diagnostic["code"] == "TL4004"
            && diagnostic["message"]
                .as_str()
                .expect("diagnostic message should be a string")
                .contains("undefined variable `missing`")
    }));
}

#[test]
fn did_change_republishes_empty_diagnostics_for_clean_document() {
    let uri = "file:///tmp/player.tua";
    let output = run_messages(vec![
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "tua",
                    "version": 1,
                    "text": "local hp: number = \"full\"\n"
                }
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "version": 2
                },
                "contentChanges": [
                    {
                        "text": "local hp: number = 10\n"
                    }
                ]
            }
        }),
    ]);

    let publications = output
        .iter()
        .filter(|message| message["method"] == "textDocument/publishDiagnostics")
        .collect::<Vec<_>>();

    assert_eq!(publications.len(), 2);
    assert_eq!(
        publications[1]["params"]["diagnostics"]
            .as_array()
            .expect("diagnostics should be an array")
            .len(),
        0
    );
}

#[test]
fn did_change_applies_incremental_edits_and_republishes_diagnostics() {
    let uri = "file:///tmp/incremental-change.tua";
    let source = "local hp: number = 10\n";
    let output = run_messages(vec![
        did_open(uri, source),
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "version": 2
                },
                "contentChanges": [
                    {
                        "range": {
                            "start": {
                                "line": 0,
                                "character": "local hp: number = ".len()
                            },
                            "end": {
                                "line": 0,
                                "character": "local hp: number = 10".len()
                            }
                        },
                        "rangeLength": 2,
                        "text": "\"full\""
                    }
                ]
            }
        }),
    ]);
    let publications = diagnostics_publications(&output, uri);
    let change_diagnostics = publications[1]["params"]["diagnostics"]
        .as_array()
        .expect("change diagnostics should be an array");

    assert_eq!(publications.len(), 2);
    assert_eq!(change_diagnostics.len(), 1);
    assert_eq!(change_diagnostics[0]["code"], "TL1003");
}

#[test]
fn did_save_accepts_text_and_republishes_diagnostics() {
    let uri = "file:///tmp/save.tua";
    let output = run_messages(vec![
        did_open(uri, "local hp: number = 10\n"),
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didSave",
            "params": {
                "textDocument": {
                    "uri": uri
                },
                "text": "local hp: number = \"full\"\n"
            }
        }),
    ]);
    let publications = diagnostics_publications(&output, uri);
    let save_diagnostics = publications[1]["params"]["diagnostics"]
        .as_array()
        .expect("save diagnostics should be an array");

    assert_eq!(publications.len(), 2);
    assert_eq!(save_diagnostics.len(), 1);
    assert_eq!(save_diagnostics[0]["code"], "TL1003");
}

#[test]
fn did_close_removes_document_and_clears_diagnostics() {
    let uri = "file:///tmp/close.tua";
    let output = run_messages(vec![
        did_open(uri, "local hp: number = \"full\"\n"),
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didClose",
            "params": {
                "textDocument": {
                    "uri": uri
                }
            }
        }),
        hover_request(2, uri, "local hp: number = \"full\"\n", "hp"),
    ]);
    let publications = diagnostics_publications(&output, uri);

    assert_eq!(publications.len(), 2);
    assert_eq!(
        publications[1]["params"]["diagnostics"]
            .as_array()
            .expect("close diagnostics should be an array")
            .len(),
        0
    );
    assert!(response_with_id(&output, 2)["result"].is_null());
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
        "function Player.makePlayer(string): Player",
    );
    assert_definition_text(server.output(), 4, &player_uri, player, "makePlayer");
}

#[test]
fn watched_required_module_delete_clears_exports_and_reports_missing_require() {
    let temp = TempProject::new("watched-module-delete");
    let main = r#"local Player = require("player")

local made = Player.makePlayer("Ada")
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
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", main);

    let config_uri = temp.uri_for("tua.toml");
    let main_uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&main_uri, main));
    assert_eq!(
        last_published_diagnostics(server.output(), &main_uri).len(),
        0
    );

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
unused_locals = false
"#,
    );

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    assert_eq!(last_published_diagnostics(server.output(), &uri).len(), 0);

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
fn did_change_configuration_null_diagnostics_defer_to_project_config() {
    let temp = TempProject::new("configuration-diagnostic-precedence");
    let source = "local unused = 1\n";

    temp.write(
        "tua.toml",
        r#"source = "src"

[diagnostics]
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
    let main = "local value = cla\n";
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
    assert_completion_labels(server.output(), 2, []);

    server.send(did_change_configuration(json!({
        "fileAssociations": {
            "*.script": "lua"
        }
    })));
    server.send(completion_request(3, &main_uri, main, "cla"));

    let completion = completion_with_label(server.output(), 3, "clamp");

    assert_eq!(completion["insertText"], "Legacy.clamp");
    assert_eq!(completion["data"]["sourceModule"], "legacy");
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
    assert!(contents.contains("- declared: /tmp/documented-hover.tua:"));
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
        did_open(uri, source),
        completion_request(2, uri, source, "typed: Pl"),
        completion_request(3, uri, source, "player."),
    ]);

    assert_completion_labels(&output, 2, ["Player"]);
    assert_completion_labels(&output, 3, ["alive", "hp", "pos"]);
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

    assert_completion_labels(&output, 2, ["param"]);
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

    assert_completion_labels(&output, 2, ["sin"]);
    assert_completion_labels(&output, 3, ["present", "print", "printf"]);
    assert_completion_labels(&output, 4, ["linear", "nearest"]);
}

#[test]
fn completion_items_can_be_resolved_for_documentation() {
    let uri = "file:///tmp/resolve-completion.tua";
    let source = "local value = math.si\n";
    let mut server = TestServer::new();

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

    server.send(did_open(uri, source));
    server.send(completion_request(2, uri, source, "typed: Pl"));
    server.send(completion_request(3, uri, source, "value = sco"));
    server.send(completion_request(4, uri, source, "call = dam"));
    server.send(completion_request(5, uri, source, "---"));

    let alias = response_with_id(server.output(), 2)["result"][0].clone();
    let local = response_with_id(server.output(), 3)["result"][0].clone();
    let function = response_with_id(server.output(), 4)["result"][0].clone();
    let snippet = response_with_id(server.output(), 5)["result"][0].clone();

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
    assert_eq!(function["result"]["detail"], "function");
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

    server.send(initialize_with_root(1, temp.uri()));
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
    assert!(completion.get("detail").is_none());
}

#[test]
fn completion_resolve_adds_auto_import_require_once() {
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
        "auto import from player: function(string): { name: string }"
    );
    assert_eq!(resolved["additionalTextEdits"].as_array().unwrap().len(), 1);
    assert_eq!(
        resolved["additionalTextEdits"][0]["newText"],
        "local Player = require(\"player\")\n"
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
fn signature_help_returns_known_function_signature() {
    let uri = "file:///tmp/signature-help.tua";
    let source = r#"type Player = { hp: number }

local player: Player = { hp = 10 }

---@param target Player player receiving damage
---@param amount number damage amount
---@return Player updated player
local function damage(target, amount)
  return target
end

damage(player, 
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        signature_help_request(2, uri, source, "damage(player, "),
    ]);
    let response = response_with_id(&output, 2);

    assert_eq!(
        response["result"]["signatures"][0]["label"],
        "function damage(target: Player, amount: number): Player"
    );
    assert_eq!(response["result"]["activeSignature"], 0);
    assert_eq!(response["result"]["activeParameter"], 1);
    assert_eq!(
        response["result"]["signatures"][0]["parameters"][0]["label"],
        "target: Player"
    );
    assert_eq!(
        response["result"]["signatures"][0]["parameters"][1]["label"],
        "amount: number"
    );
    assert_eq!(
        response["result"]["signatures"][0]["parameters"][0]["documentation"]["value"],
        "player receiving damage"
    );
    assert_eq!(
        response["result"]["signatures"][0]["parameters"][1]["documentation"]["value"],
        "damage amount"
    );
}

#[test]
fn signature_help_returns_lua_and_required_module_signatures() {
    let temp = TempProject::new("signature-help-modules");
    let source = r#"local Player = require("player")

local value = math.sin(1)
local player = Player.makePlayer(
"#;
    let player = r#"type Player = { name: string, hp: number }

---@param name string player name
---@return Player created player
local function makePlayer(name)
  return { name = name, hp = 10 }
end

return {
  makePlayer = makePlayer,
}
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write("src/player.tua", player);

    let uri = temp.uri_for("src/main.tua");
    let output = run_messages(vec![
        did_open(&uri, source),
        signature_help_request(2, &uri, source, "math.sin("),
        signature_help_request(3, &uri, source, "Player.makePlayer("),
    ]);

    assert_signature_label(&output, 2, "function math.sin(x: number): number");
    assert_signature_label(&output, 3, "function Player.makePlayer(string): Player");

    let lua_response = response_with_id(&output, 2);
    assert_eq!(
        lua_response["result"]["signatures"][0]["parameters"][0]["label"],
        "x: number"
    );
    assert!(lua_response["result"]["signatures"][0]["parameters"][0]["documentation"].is_null());

    let module_response = response_with_id(&output, 3);
    assert_eq!(
        module_response["result"]["signatures"][0]["parameters"][0]["documentation"]["value"],
        "player name"
    );
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
        r#"---@param source string audio source
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

    server.send(completion_resolve_request(3, play));

    let response = response_with_id(server.output(), 3);

    assert_eq!(
        response["result"]["detail"],
        "field: function(string): boolean"
    );
    assert!(response["result"]["documentation"]["value"]
        .as_str()
        .expect("documentation should be a string")
        .contains("audio source\nsuccess flag"));
    assert_eq!(response["result"]["data"]["symbolKind"], "function");
    assert_eq!(
        response["result"]["data"]["typeLabel"],
        "field: function(string): boolean"
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
        did_open(&uri, source),
        hover_request(2, &uri, source, "player\")"),
        hover_request(3, &uri, source, "Player.makePlayer"),
        hover_request(4, &uri, source, "makePlayer("),
        hover_request(5, &uri, source, "defaultHp"),
        hover_request(6, &uri, source, "describe("),
    ]);

    assert_hover_contains(&output, 2, "(module) player ->");
    assert_hover_contains(&output, 3, "(module) Player = require(\"player\") ->");
    assert_hover_contains(&output, 4, "function Player.makePlayer(string): Player");
    assert_hover_contains(&output, 5, "(field) Player.defaultHp: number");
    assert_hover_contains(&output, 6, "function Legacy.describe(any, any): any");
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
        did_open(&uri, source),
        hover_request(2, &uri, source, "play("),
    ]);
    let contents = hover_contents(&output, 2);

    assert!(contents.contains("```tua\nfunction Audio.play(string): boolean\n```"));
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
    let types = r#"type Player = { hp: number }

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
        "type LuaDocTypes.Enemy = { hp: number, name: string, type: string }",
    );
    assert_hover_contains(
        server.output(),
        5,
        "local enemyFromLuaDocModule: { hp: number, name: string, type: string }",
    );
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
        "local enemyFromLuaDocModule: { hp: number, name: string, type: string }",
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
fn document_symbol_returns_file_outline_tree() {
    let uri = "file:///tmp/document-symbol.tua";
    let source = r#"type Vec2 = { x: number, y: number }
type Player = { pos: Vec2, hp: number, alive?: boolean }

local player: Player = nil

local function damage(target: Player, amount: number): Player
  return target
end

return {
  damage = damage,
  player = player,
  ["ignored"] = player,
  [player] = player,
  player,
}
"#;
    let output = run_messages(vec![did_open(uri, source), document_symbol_request(2, uri)]);
    let response = response_with_id(&output, 2);
    let symbols = response["result"]
        .as_array()
        .expect("document symbols should be an array");

    assert_eq!(
        symbol_names(symbols),
        vec!["Vec2", "Player", "player", "damage", "exports"]
    );
    assert_eq!(symbols[1]["kind"], 23);
    assert_eq!(
        symbols[1]["detail"],
        "type { pos: Vec2, hp: number, alive?: boolean }"
    );
    assert_eq!(
        symbol_names(
            symbols[1]["children"]
                .as_array()
                .expect("fields should exist")
        ),
        vec!["pos", "hp", "alive"]
    );
    assert_eq!(symbols[1]["children"][0]["kind"], 8);

    assert_eq!(symbols[2]["kind"], 13);
    assert_eq!(symbols[2]["detail"], "Player");

    assert_eq!(symbols[3]["kind"], 12);
    assert_eq!(symbols[3]["detail"], "returns Player");
    assert_eq!(
        symbol_names(
            symbols[3]["children"]
                .as_array()
                .expect("parameters should exist")
        ),
        vec!["target", "amount"]
    );
    assert_eq!(symbols[3]["children"][0]["kind"], 13);

    assert_eq!(symbols[4]["kind"], 19);
    assert_eq!(
        symbol_names(
            symbols[4]["children"]
                .as_array()
                .expect("exports should exist")
        ),
        vec!["damage", "player"]
    );
    assert!(symbols[0]["range"]["start"]["line"].is_number());
    assert!(symbols[0]["selectionRange"]["start"]["character"].is_number());
    for symbol in symbols {
        assert_lsp_document_symbol_ranges_contain_selection(symbol);
    }
}

#[test]
fn document_symbol_unknown_document_returns_empty_array() {
    let uri = "file:///tmp/missing-document-symbol.tua";
    let output = run_messages(vec![document_symbol_request(2, uri)]);
    let response = response_with_id(&output, 2);

    assert_eq!(response["result"], json!([]));
}

#[test]
fn code_action_creates_missing_require_target() {
    let temp = TempProject::new("code-action-missing-require");
    let source = "local Player = require(\"player\")\n";

    temp.write(
        "tua.toml",
        r#"source = "src"

[diagnostics]
missing_requires = true
"#,
    );

    let uri = temp.uri_for("src/main.tua");
    let player_uri = temp.uri_for("src/player.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));

    let diagnostics = last_published_diagnostics(server.output(), &uri);

    server.send(code_action_request(2, &uri, 0, 0, 0, 40, diagnostics));

    let action = code_action_result(server.output(), 2, 0);

    assert_eq!(action["title"], "Create module `player`");
    assert_eq!(action["kind"], "quickfix");
    assert_eq!(action["edit"]["documentChanges"][0]["kind"], "create");
    assert_eq!(action["edit"]["documentChanges"][0]["uri"], player_uri);
    assert_eq!(
        action["edit"]["documentChanges"][0]["options"]["ignoreIfExists"],
        true
    );
    assert!(action["edit"].get("changes").is_none());
    assert_eq!(
        action["edit"]["documentChanges"][1]["edits"][0]["newText"],
        "return {}\n"
    );
}

#[test]
fn code_action_adds_missing_local_annotation() {
    let uri = "file:///tmp/code-action-annotation.tua";
    let source = "local hp = 10\n";
    let output = run_messages(vec![
        did_open(uri, source),
        code_action_request(2, uri, 0, 0, 0, 20, Vec::new()),
    ]);
    let action = code_action_result(&output, 2, 0);

    assert_eq!(action["title"], "Add type annotation `: number`");
    assert_eq!(action["kind"], "quickfix");
    assert_eq!(action["edit"]["changes"][uri][0]["newText"], ": number");
    assert_eq!(
        range_text(source, &action["edit"]["changes"][uri][0]["range"]),
        ""
    );
}

#[test]
fn code_action_removes_unused_local() {
    let temp = TempProject::new("code-action-unused-local");
    let source = "local unused = 1\nlocal used = 2\nprint(used)\n";

    temp.write(
        "tua.toml",
        r#"source = "src"

[diagnostics]
unused_locals = true
"#,
    );

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));

    let diagnostics = last_published_diagnostics(server.output(), &uri)
        .into_iter()
        .filter(|diagnostic| diagnostic["code"] == "TL4005")
        .collect::<Vec<_>>();

    server.send(code_action_request(2, &uri, 0, 0, 0, 30, diagnostics));

    let action = code_action_result(server.output(), 2, 0);

    assert_eq!(action["title"], "Remove unused local");
    assert_eq!(action["edit"]["changes"][&uri][0]["newText"], "");
    assert_eq!(
        action["edit"]["changes"][&uri][0]["range"],
        json!({
            "start": { "line": 0, "character": 0 },
            "end": { "line": 1, "character": 0 }
        })
    );
}

#[test]
fn code_action_renames_duplicate_table_key() {
    let temp = TempProject::new("code-action-duplicate-key");
    let source = "local config = { hp = 10, hp = 20 }\n";

    temp.write(
        "tua.toml",
        r#"source = "src"

[diagnostics]
duplicate_keys = true
"#,
    );

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));

    let diagnostics = last_published_diagnostics(server.output(), &uri);

    server.send(code_action_request(2, &uri, 0, 0, 0, 60, diagnostics));

    let action = code_action_result(server.output(), 2, 0);

    assert_eq!(action["title"], "Rename duplicate key to `hp2`");
    assert_eq!(action["edit"]["changes"][&uri][0]["newText"], "hp2");
    assert_eq!(
        range_text(source, &action["edit"]["changes"][&uri][0]["range"]),
        "hp"
    );
}

#[test]
fn code_action_removes_self_assignment() {
    let temp = TempProject::new("code-action-self-assignment");
    let source = "hp = hp\nlocal ok = 1\n";

    temp.write(
        "tua.toml",
        r#"source = "src"

[diagnostics]
self_assignments = true
"#,
    );

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));

    let diagnostics = last_published_diagnostics(server.output(), &uri);

    server.send(code_action_request(2, &uri, 0, 0, 0, 20, diagnostics));

    let action = code_action_result(server.output(), 2, 0);

    assert_eq!(action["title"], "Remove self-assignment");
    assert_eq!(action["edit"]["changes"][&uri][0]["newText"], "");
    assert_eq!(
        action["edit"]["changes"][&uri][0]["range"],
        json!({
            "start": { "line": 0, "character": 0 },
            "end": { "line": 1, "character": 0 }
        })
    );
}

#[test]
fn code_action_is_absent_for_unsafe_edits() {
    let temp = TempProject::new("code-action-unsafe");
    let source = r#"local unused, other = 1, 2
local config = { ["hp"] = 10, ["hp"] = 20 }
print(config)
"#;

    temp.write(
        "tua.toml",
        r#"source = "src"

[diagnostics]
unused_locals = true
duplicate_keys = true
"#,
    );

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));

    let diagnostics = last_published_diagnostics(server.output(), &uri);

    server.send(code_action_request(2, &uri, 0, 0, 1, 80, diagnostics));

    assert_eq!(response_with_id(server.output(), 2)["result"], json!([]));
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
fn prepare_rename_returns_supported_symbol_range_and_placeholder() {
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
    let unsupported = response_with_id(&output, 3);

    assert_eq!(supported["result"]["placeholder"], "Player");
    assert_eq!(range_text(source, &supported["result"]["range"]), "Player");
    assert!(unsupported["result"].is_null());
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
fn rename_returns_null_for_unsupported_unknown_and_invalid_names() {
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

    assert!(response_with_id(&output, 2)["result"].is_null());
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

#[test]
fn inlay_hint_returns_local_type_and_parameter_hints() {
    let uri = "file:///tmp/inlay-hints.tua";
    let source = r#"type Player = { hp: number }

local function makePlayer(): Player
  return { hp = 10 }
end

local function damage(target: Player, amount: number): Player
  target.hp -= amount
  return target
end

local player = makePlayer()
local total = 10
damage(player, 10)
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        inlay_hint_request(2, uri, 0, 0, 20, 0),
    ]);
    let response = response_with_id(&output, 2);
    let hints = response["result"]
        .as_array()
        .expect("inlay hints should be an array");

    assert_eq!(
        inlay_hint_labels(hints),
        vec![": Player", ": number", "target:", "amount:"]
    );
    assert_eq!(hints[0]["kind"], 1);
    assert_eq!(hints[1]["kind"], 1);
    assert_eq!(hints[2]["kind"], 2);
    assert_eq!(hints[2]["paddingRight"], true);
    assert_eq!(hints[0]["position"], json!({ "line": 11, "character": 12 }));
    assert_eq!(hints[2]["position"], json!({ "line": 13, "character": 7 }));
}

#[test]
fn inlay_hint_filters_to_requested_range() {
    let uri = "file:///tmp/ranged-inlay-hints.tua";
    let source = r#"type Player = { hp: number }

local function makePlayer(): Player
  return { hp = 10 }
end

local function damage(target: Player, amount: number): Player
  target.hp -= amount
  return target
end

local player = makePlayer()
local total = 10
damage(player, 10)
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        inlay_hint_request(2, uri, 11, 0, 12, 20),
    ]);
    let response = response_with_id(&output, 2);
    let hints = response["result"]
        .as_array()
        .expect("inlay hints should be an array");

    assert_eq!(inlay_hint_labels(hints), vec![": Player", ": number"]);
}

#[test]
fn inlay_hint_unknown_document_returns_empty_array() {
    let uri = "file:///tmp/missing-inlay-hints.tua";
    let output = run_messages(vec![inlay_hint_request(2, uri, 0, 0, 20, 0)]);
    let response = response_with_id(&output, 2);

    assert_eq!(response["result"], json!([]));
}

#[test]
fn semantic_tokens_return_encoded_token_classes() {
    let uri = "file:///tmp/semantic-tokens.tua";
    let source = r#"---@class Player playable character
---@field armor number
---@deprecated
type Player = {
  hp: number,
  name: string,
}

local function makePlayer(): Player
  return { hp = 10, name = "Ada" }
end

local function damage(player: Player, amount: number): Player
  player.hp -= amount
  player:heal()
  return player
end

function Player:heal()
end

local MAX_HP = 10
local hero = makePlayer()
damage(hero, 10)
missing(hero)
require("systems.player")
return {
  makePlayer = makePlayer,
}
"#;
    let output = run_messages(vec![did_open(uri, source), semantic_tokens_request(2, uri)]);
    let response = response_with_id(&output, 2);
    let tokens = decoded_semantic_tokens(source, response);

    assert_semantic_token(&tokens, "Player", 0, true);
    assert_semantic_token(&tokens, "makePlayer", 1, true);
    assert_semantic_token(&tokens, "hero", 2, true);
    assert_semantic_token(&tokens, "player", 3, true);
    assert_semantic_token(&tokens, "hp", 4, true);
    assert_semantic_token(&tokens, "number", 5, false);
    assert_semantic_token(&tokens, "missing", 6, false);
    assert_semantic_token(&tokens, "require", 7, false);
    assert_semantic_token(&tokens, "systems.player", 8, false);
    assert_semantic_token(&tokens, "heal", 9, true);
    assert_semantic_token(&tokens, "heal", 9, false);
    assert_semantic_token(&tokens, "@class", 10, false);
    assert_semantic_token(&tokens, "@field", 10, false);
    assert_semantic_token(&tokens, "@deprecated", 10, false);
    assert!(tokens
        .iter()
        .any(|token| token.text == "damage" && token.token_type == 1 && !token.declaration));
    assert!(!tokens
        .iter()
        .any(|token| token.text == "require" && token.token_type == 6));
    assert!(tokens
        .iter()
        .any(|token| token.text == "makePlayer" && token.token_type == 4 && token.declaration));
    assert!(tokens
        .iter()
        .any(|token| token.text == "MAX_HP" && token.token_type == 2 && token.readonly));
    assert!(tokens
        .iter()
        .any(|token| token.text == "Player" && token.token_type == 0 && token.deprecated));
    assert!(tokens
        .iter()
        .any(|token| token.text == "@deprecated" && token.token_type == 10 && token.deprecated));
}

#[test]
fn semantic_tokens_fall_back_when_source_does_not_parse() {
    let uri = "file:///tmp/semantic-invalid.tua";
    let source = r#"type Player = {
  hp: number,
}

local player: Player =
function player.damage(amount: number)
  player.
"#;
    let output = run_messages(vec![did_open(uri, source), semantic_tokens_request(2, uri)]);
    let response = response_with_id(&output, 2);
    let tokens = decoded_semantic_tokens(source, response);

    assert_semantic_token(&tokens, "Player", 0, true);
    assert_semantic_token(&tokens, "hp", 4, false);
    assert_semantic_token(&tokens, "number", 5, false);
    assert_semantic_token(&tokens, "player", 2, true);
    assert_semantic_token(&tokens, "damage", 1, true);
}

#[test]
fn semantic_tokens_unknown_document_returns_empty_data() {
    let uri = "file:///tmp/missing-semantic-tokens.tua";
    let output = run_messages(vec![semantic_tokens_request(2, uri)]);
    let response = response_with_id(&output, 2);

    assert_eq!(response["result"], json!({ "data": [] }));
}

fn run_messages(messages: Vec<Value>) -> Vec<Value> {
    let mut input = Vec::new();

    for message in messages {
        let body = serde_json::to_vec(&message).expect("message should serialize");

        input.extend_from_slice(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
        input.extend_from_slice(&body);
    }

    let mut output = Vec::new();

    run(Cursor::new(input), &mut output).expect("server should run");

    read_output_messages(&output)
}

struct TestServer {
    server: LspServer,
    output: Vec<Value>,
}

impl TestServer {
    fn new() -> Self {
        Self {
            server: LspServer::default(),
            output: Vec::new(),
        }
    }

    fn send(&mut self, message: Value) {
        if let Some(messages) = self.server.handle_message(message) {
            self.output.extend(messages);
        }
    }

    fn output(&self) -> &[Value] {
        &self.output
    }
}

fn did_open(uri: &str, source: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": uri,
                "languageId": "tua",
                "version": 1,
                "text": source
            }
        }
    })
}

fn did_change(uri: &str, source: &str, version: u64) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didChange",
        "params": {
            "textDocument": {
                "uri": uri,
                "version": version
            },
            "contentChanges": [
                {
                    "text": source
                }
            ]
        }
    })
}

fn initialize_with_root(id: u64, root_uri: String) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "rootUri": root_uri
        }
    })
}

fn initialize_with_workspace_folders<const N: usize>(id: u64, roots: [String; N]) -> Value {
    let folders = roots
        .into_iter()
        .enumerate()
        .map(|(index, uri)| {
            json!({
                "uri": uri,
                "name": format!("workspace-{index}")
            })
        })
        .collect::<Vec<_>>();

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "workspaceFolders": folders
        }
    })
}

fn workspace_folders_changed<const A: usize, const R: usize>(
    added: [String; A],
    removed: [String; R],
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "workspace/didChangeWorkspaceFolders",
        "params": {
            "event": {
                "added": workspace_folder_items(added),
                "removed": workspace_folder_items(removed)
            }
        }
    })
}

fn watched_files_changed<const N: usize>(changes: [(&str, u8); N]) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "workspace/didChangeWatchedFiles",
        "params": {
            "changes": changes
                .into_iter()
                .map(|(uri, kind)| {
                    json!({
                        "uri": uri,
                        "type": kind
                    })
                })
                .collect::<Vec<_>>()
        }
    })
}

fn did_change_configuration(settings: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "workspace/didChangeConfiguration",
        "params": {
            "settings": {
                "tua": settings
            }
        }
    })
}

fn workspace_folder_items<const N: usize>(roots: [String; N]) -> Vec<Value> {
    roots
        .into_iter()
        .enumerate()
        .map(|(index, uri)| {
            json!({
                "uri": uri,
                "name": format!("workspace-{index}")
            })
        })
        .collect()
}

fn hover_request(id: u64, uri: &str, source: &str, needle: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/hover",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_of(source, needle)
        }
    })
}

fn completion_request(id: u64, uri: &str, source: &str, needle: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/completion",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_after(source, needle)
        }
    })
}

fn completion_resolve_request(id: u64, item: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "completionItem/resolve",
        "params": item
    })
}

fn signature_help_request(id: u64, uri: &str, source: &str, needle: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/signatureHelp",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_after(source, needle)
        }
    })
}

fn definition_request(id: u64, uri: &str, source: &str, needle: &str, offset: usize) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/definition",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_with_offset(source, needle, offset)
        }
    })
}

fn definition_request_at(id: u64, uri: &str, position: SourcePosition) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/definition",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position(position)
        }
    })
}

fn builtin_document_request(id: u64, uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tua/builtinDocument",
        "params": {
            "uri": uri
        }
    })
}

fn prepare_call_hierarchy_request(
    id: u64,
    uri: &str,
    source: &str,
    needle: &str,
    offset: usize,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/prepareCallHierarchy",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_with_offset(source, needle, offset)
        }
    })
}

fn call_hierarchy_incoming_request(id: u64, item: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "callHierarchy/incomingCalls",
        "params": {
            "item": item
        }
    })
}

fn call_hierarchy_outgoing_request(id: u64, item: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "callHierarchy/outgoingCalls",
        "params": {
            "item": item
        }
    })
}

fn document_symbol_request(id: u64, uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/documentSymbol",
        "params": {
            "textDocument": {
                "uri": uri
            }
        }
    })
}

fn document_link_request(id: u64, uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/documentLink",
        "params": {
            "textDocument": {
                "uri": uri
            }
        }
    })
}

fn document_highlight_request(
    id: u64,
    uri: &str,
    source: &str,
    needle: &str,
    offset: usize,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/documentHighlight",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_with_offset(source, needle, offset)
        }
    })
}

fn code_action_request(
    id: u64,
    uri: &str,
    start_line: usize,
    start_character: usize,
    end_line: usize,
    end_character: usize,
    diagnostics: Vec<Value>,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/codeAction",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "range": {
                "start": {
                    "line": start_line,
                    "character": start_character
                },
                "end": {
                    "line": end_line,
                    "character": end_character
                }
            },
            "context": {
                "diagnostics": diagnostics
            }
        }
    })
}

fn references_request(
    id: u64,
    uri: &str,
    source: &str,
    needle: &str,
    offset: usize,
    include_declaration: bool,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/references",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_with_offset(source, needle, offset),
            "context": {
                "includeDeclaration": include_declaration
            }
        }
    })
}

fn prepare_rename_request(id: u64, uri: &str, source: &str, needle: &str, offset: usize) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/prepareRename",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_with_offset(source, needle, offset)
        }
    })
}

fn rename_request(
    id: u64,
    uri: &str,
    source: &str,
    needle: &str,
    offset: usize,
    new_name: &str,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/rename",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "position": lsp_position_with_offset(source, needle, offset),
            "newName": new_name
        }
    })
}

fn inlay_hint_request(
    id: u64,
    uri: &str,
    start_line: usize,
    start_character: usize,
    end_line: usize,
    end_character: usize,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/inlayHint",
        "params": {
            "textDocument": {
                "uri": uri
            },
            "range": {
                "start": {
                    "line": start_line,
                    "character": start_character
                },
                "end": {
                    "line": end_line,
                    "character": end_character
                }
            }
        }
    })
}

fn semantic_tokens_request(id: u64, uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/semanticTokens/full",
        "params": {
            "textDocument": {
                "uri": uri
            }
        }
    })
}

fn workspace_symbol_request(id: u64, query: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "workspace/symbol",
        "params": {
            "query": query
        }
    })
}

fn lsp_position_of(source: &str, needle: &str) -> Value {
    let index = source.find(needle).expect("needle should exist in source");
    let before = &source[..index];
    let line = before.bytes().filter(|byte| *byte == b'\n').count();
    let character = before
        .rsplit_once('\n')
        .map(|(_, line)| line.len())
        .unwrap_or(before.len());

    json!({
        "line": line,
        "character": character
    })
}

fn lsp_position_with_offset(source: &str, needle: &str, offset: usize) -> Value {
    let mut position = lsp_position_of(source, needle);
    position["character"] = json!(
        position["character"]
            .as_u64()
            .expect("character should be numeric")
            + offset as u64
    );

    position
}

fn lsp_position_after(source: &str, needle: &str) -> Value {
    let mut position = lsp_position_of(source, needle);
    position["character"] = json!(
        position["character"]
            .as_u64()
            .expect("character should be numeric")
            + needle.chars().count() as u64
    );

    position
}

fn lsp_position(position: SourcePosition) -> Value {
    json!({
        "line": position.line.saturating_sub(1),
        "character": position.column.saturating_sub(1)
    })
}

fn read_output_messages(mut output: &[u8]) -> Vec<Value> {
    let mut messages = Vec::new();

    while !output.is_empty() {
        let header_end = output
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("output should contain a header terminator");
        let header = std::str::from_utf8(&output[..header_end]).expect("header should be UTF-8");
        let content_length = header
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;

                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().expect("valid content length"))
            })
            .expect("content length should exist");
        let body_start = header_end + 4;
        let body_end = body_start + content_length;
        let message =
            serde_json::from_slice(&output[body_start..body_end]).expect("valid JSON response");

        messages.push(message);
        output = &output[body_end..];
    }

    messages
}

fn published_diagnostics(output: &[Value], uri: &str) -> Vec<Value> {
    output
        .iter()
        .find(|message| {
            message["method"] == "textDocument/publishDiagnostics"
                && message["params"]["uri"] == uri
        })
        .and_then(|message| message["params"]["diagnostics"].as_array())
        .cloned()
        .expect("diagnostics publication should exist")
}

fn diagnostics_publications<'a>(output: &'a [Value], uri: &str) -> Vec<&'a Value> {
    output
        .iter()
        .filter(|message| {
            message["method"] == "textDocument/publishDiagnostics"
                && message["params"]["uri"] == uri
        })
        .collect()
}

fn last_published_diagnostics(output: &[Value], uri: &str) -> Vec<Value> {
    diagnostics_publications(output, uri)
        .last()
        .and_then(|message| message["params"]["diagnostics"].as_array())
        .cloned()
        .expect("diagnostics publication should exist")
}

fn assert_hover_contains(output: &[Value], id: u64, expected: &str) {
    let contents = hover_contents(output, id);

    assert!(
        contents.contains(expected),
        "expected hover contents to contain {expected:?}, got {contents:?}"
    );
}

fn hover_contents(output: &[Value], id: u64) -> &str {
    let response = response_with_id(output, id);

    response["result"]["contents"]["value"]
        .as_str()
        .expect("hover contents should be a string")
}

fn assert_completion_labels<const N: usize>(output: &[Value], id: u64, expected: [&str; N]) {
    let response = response_with_id(output, id);
    let labels = response["result"]
        .as_array()
        .expect("completion result should be an array")
        .iter()
        .map(|item| {
            item["label"]
                .as_str()
                .expect("completion label should be a string")
        })
        .collect::<Vec<_>>();

    assert_eq!(labels, expected);
}

fn completion_with_label(output: &[Value], id: u64, label: &str) -> Value {
    response_with_id(output, id)["result"]
        .as_array()
        .expect("completion result should be an array")
        .iter()
        .find(|item| item["label"] == label)
        .unwrap_or_else(|| panic!("completion {label:?} should exist"))
        .clone()
}

fn assert_signature_label(output: &[Value], id: u64, expected: &str) {
    let response = response_with_id(output, id);

    assert_eq!(response["result"]["signatures"][0]["label"], expected);
}

fn assert_definition_text(output: &[Value], id: u64, uri: &str, source: &str, expected: &str) {
    let response = response_with_id(output, id);

    assert_eq!(response["result"]["uri"], uri);

    let range = &response["result"]["range"];
    let line = range["start"]["line"]
        .as_u64()
        .expect("definition start line should be numeric") as usize;
    let start = range["start"]["character"]
        .as_u64()
        .expect("definition start character should be numeric") as usize;
    let end = range["end"]["character"]
        .as_u64()
        .expect("definition end character should be numeric") as usize;
    let text = source
        .split('\n')
        .nth(line)
        .expect("definition line should exist");

    assert_eq!(&text[start..end], expected);
}

fn first_call_hierarchy_item(output: &[Value], id: u64) -> Value {
    response_with_id(output, id)["result"]
        .as_array()
        .expect("call hierarchy prepare result should be an array")
        .first()
        .expect("call hierarchy prepare should return an item")
        .clone()
}

fn call_hierarchy_from_names(calls: &[Value]) -> Vec<&str> {
    calls
        .iter()
        .map(|call| {
            call["from"]["name"]
                .as_str()
                .expect("incoming caller name should be a string")
        })
        .collect()
}

fn call_hierarchy_to_names(calls: &[Value]) -> Vec<&str> {
    calls
        .iter()
        .map(|call| {
            call["to"]["name"]
                .as_str()
                .expect("outgoing callee name should be a string")
        })
        .collect()
}

fn assert_call_hierarchy_range_text(
    calls: &[Value],
    caller_name: &str,
    source: &str,
    expected: &str,
) {
    let call = calls
        .iter()
        .find(|call| call["from"]["name"] == caller_name)
        .unwrap_or_else(|| panic!("incoming call from {caller_name:?} should exist"));
    let range = call["fromRanges"]
        .as_array()
        .expect("incoming call ranges should be an array")
        .first()
        .expect("incoming call should have a range");

    assert_eq!(range_text(source, range), expected);
}

fn assert_call_hierarchy_outgoing_range_text(
    calls: &[Value],
    callee_name: &str,
    source: &str,
    expected: &str,
) {
    let call = calls
        .iter()
        .find(|call| call["to"]["name"] == callee_name)
        .unwrap_or_else(|| panic!("outgoing call to {callee_name:?} should exist"));
    let range = call["fromRanges"]
        .as_array()
        .expect("outgoing call ranges should be an array")
        .first()
        .expect("outgoing call should have a range");

    assert_eq!(range_text(source, range), expected);
}

fn assert_lsp_document_symbol_ranges_contain_selection(symbol: &Value) {
    assert!(
        lsp_range_contains(&symbol["range"], &symbol["selectionRange"]),
        "{} selectionRange {:?} should be contained in range {:?}",
        symbol["name"],
        symbol["selectionRange"],
        symbol["range"]
    );

    if let Some(children) = symbol["children"].as_array() {
        for child in children {
            assert_lsp_document_symbol_ranges_contain_selection(child);
        }
    }
}

fn lsp_range_contains(outer: &Value, inner: &Value) -> bool {
    lsp_position_key(&outer["start"]) <= lsp_position_key(&inner["start"])
        && lsp_position_key(&inner["end"]) <= lsp_position_key(&outer["end"])
}

fn lsp_position_key(position: &Value) -> (u64, u64) {
    (
        position["line"].as_u64().expect("line should be numeric"),
        position["character"]
            .as_u64()
            .expect("character should be numeric"),
    )
}

fn assert_reference_texts<const N: usize>(
    output: &[Value],
    id: u64,
    source: &str,
    expected: [&str; N],
) {
    let response = response_with_id(output, id);
    let texts = response["result"]
        .as_array()
        .expect("references result should be an array")
        .iter()
        .map(|location| range_text(source, &location["range"]))
        .collect::<Vec<_>>();

    assert_eq!(texts, expected);
}

fn assert_reference_texts_for_uri<const N: usize>(
    output: &[Value],
    id: u64,
    uri: &str,
    source: &str,
    expected: [&str; N],
) {
    let response = response_with_id(output, id);
    let texts = response["result"]
        .as_array()
        .expect("references result should be an array")
        .iter()
        .filter(|location| location["uri"] == uri)
        .map(|location| range_text(source, &location["range"]))
        .collect::<Vec<_>>();

    assert_eq!(texts, expected);
}

fn assert_document_highlight_texts<const N: usize>(
    output: &[Value],
    id: u64,
    source: &str,
    expected: [&str; N],
) {
    let response = response_with_id(output, id);
    let highlights = response["result"]
        .as_array()
        .expect("document highlight result should be an array");
    let texts = highlights
        .iter()
        .map(|highlight| range_text(source, &highlight["range"]))
        .collect::<Vec<_>>();

    assert_eq!(texts, expected);
    assert!(highlights.iter().all(|highlight| highlight["kind"] == 1));
}

fn code_action_result(output: &[Value], id: u64, index: usize) -> &Value {
    &response_with_id(output, id)["result"]
        .as_array()
        .expect("code actions should be an array")[index]
}

fn assert_rename_edit_texts<const N: usize>(
    output: &[Value],
    id: u64,
    uri: &str,
    source: &str,
    expected_originals: [&str; N],
    expected_new_text: &str,
) {
    let response = response_with_id(output, id);
    let edits = response["result"]["changes"][uri]
        .as_array()
        .expect("rename edits should be an array");
    let originals = edits
        .iter()
        .map(|edit| range_text(source, &edit["range"]))
        .collect::<Vec<_>>();
    let new_texts = edits
        .iter()
        .map(|edit| {
            edit["newText"]
                .as_str()
                .expect("rename edit text should be a string")
        })
        .collect::<Vec<_>>();

    assert_eq!(originals, expected_originals);
    assert!(new_texts
        .iter()
        .all(|new_text| *new_text == expected_new_text));
}

fn inlay_hint_labels(hints: &[Value]) -> Vec<&str> {
    hints
        .iter()
        .map(|hint| {
            hint["label"]
                .as_str()
                .expect("inlay hint label should be a string")
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedSemanticToken<'a> {
    text: &'a str,
    token_type: usize,
    declaration: bool,
    readonly: bool,
    deprecated: bool,
}

fn decoded_semantic_tokens<'a>(source: &'a str, response: &Value) -> Vec<DecodedSemanticToken<'a>> {
    let data = response["result"]["data"]
        .as_array()
        .expect("semantic token data should be an array");
    let mut line = 0usize;
    let mut character = 0usize;
    let mut tokens = Vec::new();

    for chunk in data.chunks(5) {
        let delta_line = chunk[0].as_u64().expect("delta line should be numeric") as usize;
        let delta_start = chunk[1].as_u64().expect("delta start should be numeric") as usize;
        let length = chunk[2].as_u64().expect("length should be numeric") as usize;
        let token_type = chunk[3].as_u64().expect("type should be numeric") as usize;
        let modifiers = chunk[4].as_u64().expect("modifiers should be numeric") as usize;

        if delta_line == 0 {
            character += delta_start;
        } else {
            line += delta_line;
            character = delta_start;
        }

        let text = source
            .split('\n')
            .nth(line)
            .and_then(|line_text| line_text.get(character..character + length))
            .expect("semantic token range should exist");

        tokens.push(DecodedSemanticToken {
            text,
            token_type,
            declaration: modifiers & 1 == 1,
            readonly: modifiers & 2 == 2,
            deprecated: modifiers & 4 == 4,
        });
    }

    tokens
}

fn assert_semantic_token(
    tokens: &[DecodedSemanticToken<'_>],
    text: &str,
    token_type: usize,
    declaration: bool,
) {
    assert!(
        tokens.iter().any(|token| {
            token.text == text
                && token.token_type == token_type
                && token.declaration == declaration
        }),
        "expected semantic token `{text}` type {token_type} declaration {declaration}, got {tokens:?}"
    );
}

fn range_text<'a>(source: &'a str, range: &Value) -> &'a str {
    let line = range["start"]["line"]
        .as_u64()
        .expect("range start line should be numeric") as usize;
    let start = range["start"]["character"]
        .as_u64()
        .expect("range start character should be numeric") as usize;
    let end = range["end"]["character"]
        .as_u64()
        .expect("range end character should be numeric") as usize;
    let text = source
        .split('\n')
        .nth(line)
        .expect("reference line should exist");

    &text[start..end]
}

fn symbol_names(symbols: &[Value]) -> Vec<&str> {
    symbols
        .iter()
        .map(|symbol| {
            symbol["name"]
                .as_str()
                .expect("symbol name should be a string")
        })
        .collect()
}

fn workspace_symbol_names(symbols: &[Value]) -> Vec<&str> {
    symbols
        .iter()
        .map(|symbol| {
            symbol["name"]
                .as_str()
                .expect("workspace symbol name should be a string")
        })
        .collect()
}

fn response_with_id(output: &[Value], id: u64) -> &Value {
    output
        .iter()
        .find(|message| message["id"] == id)
        .expect("response should exist")
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
        let path = std::env::temp_dir().join(format!("tua-lsp-{name}-{}-{unique}", process::id()));

        fs::create_dir_all(&path).expect("temp project should be created");

        Self { path }
    }

    fn uri(&self) -> String {
        path_uri(&self.path)
    }

    fn uri_for(&self, relative: &str) -> String {
        path_uri(&self.path.join(relative))
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.path.join(relative);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent directory should be created");
        }

        fs::write(path, contents).expect("fixture should be written");
    }

    fn remove(&self, relative: &str) {
        fs::remove_file(self.path.join(relative)).expect("fixture should be removed");
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn path_uri(path: &Path) -> String {
    format!("file://{}", path.display())
}

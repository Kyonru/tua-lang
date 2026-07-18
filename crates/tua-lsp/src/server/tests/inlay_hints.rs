use super::*;

#[test]
fn inlay_hints_ignore_generated_class_nodes_and_keep_source_positions() {
    let uri = "file:///tmp/class-inlay-hints.tua";
    let source = r#"open class Entity {
  x: number
  y: number
}

local particle = Entity.new()

return {
  particle = particle,
}
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        inlay_hint_request(2, uri, 0, 0, 20, 0),
    ]);
    let hints = response_with_id(&output, 2)["result"]
        .as_array()
        .expect("inlay hints should be an array");

    assert_eq!(inlay_hint_labels(hints), vec![": Entity"]);
    assert_eq!(hints[0]["position"], json!({ "line": 5, "character": 14 }));
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
fn inlay_hints_survive_an_incomplete_edit_without_waiting_for_a_snapshot() {
    let uri = "file:///tmp/inlay-hints-incomplete-edit.tua";
    let before = r#"type Player = { hp: number }

local function makePlayer(): Player
  return { hp = 10 }
end

local player = makePlayer()
"#;
    let after = format!("{before}\nplayer");
    let output = run_messages(vec![
        did_open(uri, before),
        did_change(uri, &after, 2),
        inlay_hint_request(2, uri, 0, 0, 20, 0),
    ]);
    let hints = response_with_id(&output, 2)["result"]
        .as_array()
        .expect("inlay hints should be an array");

    assert!(
        inlay_hint_labels(hints).contains(&": Player"),
        "the last incomplete line must not hide valid hints above it: {hints:#?}"
    );
}

#[test]
fn incomplete_edit_keeps_project_enriched_inlay_hints_from_the_warm_snapshot() {
    let temp = TempProject::new("project-inlay-hints-incomplete-edit");
    let before = r#"local Player = require("player")

local player = Player.new("Ada")
"#;
    let after = format!("{before}\nplayer");

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", before);
    temp.write(
        "src/player.tua",
        r#"local M = {}

function M.new(name: string): { name: string }
  return { name = name }
end

return M
"#,
    );

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, before));
    server.send(inlay_hint_request(2, &uri, 0, 0, 10, 0));
    server.send(did_change(&uri, &after, 2));
    server.send(inlay_hint_request(3, &uri, 0, 0, 10, 0));

    let output = server.output();
    let before_hints = response_with_id(output, 2)["result"]
        .as_array()
        .expect("warm inlay hints should be an array");
    let after_hints = response_with_id(output, 3)["result"]
        .as_array()
        .expect("edited inlay hints should be an array");

    assert_eq!(
        inlay_hint_labels(after_hints),
        inlay_hint_labels(before_hints)
    );
    assert!(inlay_hint_labels(after_hints).contains(&": { name: string }"));
}
#[test]
fn inlay_hint_returns_required_module_parameter_hints() {
    let temp = TempProject::new("required-module-inlay-hints");
    let source = r#"local Player = require("entities.player")
local Legacy = require("legacy")

local player = Player.new("Ada")
local label = Legacy.label(player.name)
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write(
        "src/entities/player.tua",
        r#"type Player = {
  name: string,
  hp: number,
}

local M = {}

function M.new(name: string): Player
  return {
    name = name,
    hp = 100,
  }
end

return M
"#,
    );
    temp.write(
        "src/legacy.lua",
        r#"local M = {}

---@param name number
---@return string
function M.label(name)
  return "player:" .. name
end

return M
"#,
    );

    let uri = temp.uri_for("src/main.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
        inlay_hint_request(2, &uri, 0, 0, 10, 0),
    ]);
    let response = response_with_id(&output, 2);
    let hints = response["result"]
        .as_array()
        .expect("inlay hints should be an array");
    let parameter_hints = hints
        .iter()
        .filter(|hint| hint["kind"] == 2)
        .filter(|hint| {
            hint["position"]["line"]
                .as_u64()
                .is_some_and(|line| line == 3 || line == 4)
        })
        .map(|hint| hint["label"].as_str().expect("label should be a string"))
        .collect::<Vec<_>>();

    assert_eq!(parameter_hints, vec!["name:", "name:"]);
    assert!(hints.iter().any(|hint| {
        hint["label"] == "name:" && hint["position"] == json!({ "line": 3, "character": 26 })
    }));
    assert!(hints.iter().any(|hint| {
        hint["label"] == "name:" && hint["position"] == json!({ "line": 4, "character": 27 })
    }));
}
#[test]
fn inlay_hint_returns_required_module_alias_and_lua_export_parameter_hints() {
    let temp = TempProject::new("required-module-alias-inlay-hints");
    let source = r#"local Functions = require("functions")
local Legacy = require("legacy")

local makePlayer = Functions.makePlayer
local player = makePlayer("require graph")
local description = Legacy.describe("hp", player.hp)
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);
    temp.write(
        "src/functions.tua",
        r#"type Player = {
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
    temp.write(
        "src/legacy.lua",
        r#"return {
  describe = function(label, value)
    return label
  end,
}
"#,
    );

    let uri = temp.uri_for("src/main.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
        inlay_hint_request(2, &uri, 0, 0, 10, 0),
    ]);
    let response = response_with_id(&output, 2);
    let hints = response["result"]
        .as_array()
        .expect("inlay hints should be an array");
    let target_hints = hints
        .iter()
        .filter(|hint| hint["kind"] == 2)
        .filter(|hint| {
            hint["position"]["line"]
                .as_u64()
                .is_some_and(|line| line == 4 || line == 5)
        })
        .map(|hint| hint["label"].as_str().expect("label should be a string"))
        .collect::<Vec<_>>();

    assert_eq!(target_hints, vec!["name:", "label:", "value:"]);
    assert!(hints.iter().any(|hint| {
        hint["label"] == "name:" && hint["position"] == json!({ "line": 4, "character": 26 })
    }));
    assert!(hints.iter().any(|hint| {
        hint["label"] == "label:" && hint["position"] == json!({ "line": 5, "character": 36 })
    }));
    assert!(hints.iter().any(|hint| {
        hint["label"] == "value:" && hint["position"] == json!({ "line": 5, "character": 42 })
    }));
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

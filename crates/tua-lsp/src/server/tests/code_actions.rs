use super::*;

#[test]
fn code_action_adds_exact_code_suppressions() {
    let uri = "file:///tmp/code-action-suppress-diagnostic.tua";
    let source = "local value = flag and false or true\n";
    let mut server = TestServer::new();

    server.send(did_open(uri, source));
    let diagnostic = last_published_diagnostics(server.output(), uri)
        .into_iter()
        .find(|diagnostic| diagnostic["code"] == "TL4048")
        .expect("misleading and-or diagnostic should be published");

    server.send(code_action_request(
        2,
        uri,
        0,
        0,
        0,
        source.trim_end().len(),
        vec![diagnostic],
    ));

    let actions = response_with_id(server.output(), 2)["result"]
        .as_array()
        .expect("code actions should be an array");
    let line = actions
        .iter()
        .find(|action| action["title"] == "Suppress TL4048 for this line")
        .expect("line suppression should be present");
    let file = actions
        .iter()
        .find(|action| action["title"] == "Suppress TL4048 for this file")
        .expect("file suppression should be present");

    assert_eq!(line["kind"], "quickfix");
    assert_eq!(
        line["edit"]["changes"][uri][0]["newText"],
        " -- tua-ignore-line TL4048"
    );
    assert_eq!(file["kind"], "quickfix");
    assert_eq!(
        file["edit"]["changes"][uri][0]["newText"],
        "-- tua-ignore-file TL4048\n"
    );
}

#[test]
fn code_action_does_not_offer_suppressions_when_inline_ignores_are_disabled() {
    let temp = TempProject::new("code-action-suppression-disabled");
    let source = "local value = flag and false or true\n";

    temp.write(
        "tua.toml",
        r#"source = "src"

[diagnostics]
inline_ignores = false
"#,
    );

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    let diagnostic = last_published_diagnostics(server.output(), &uri)
        .into_iter()
        .find(|diagnostic| diagnostic["code"] == "TL4048")
        .expect("misleading and-or diagnostic should be published");

    server.send(code_action_request(
        2,
        &uri,
        0,
        0,
        0,
        source.trim_end().len(),
        vec![diagnostic],
    ));

    let actions = response_with_id(server.output(), 2)["result"]
        .as_array()
        .expect("code actions should be an array");

    assert!(actions.iter().all(|action| action["title"]
        .as_str()
        .is_none_or(|title| !title.starts_with("Suppress TL"))));
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
fn code_action_imports_unknown_exported_type() {
    let temp = TempProject::new("code-action-import-type");
    let source = "local player: Player = nil\n";

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/types.d.tua", "export type Player = { name: string }\n");

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();
    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    let diagnostic = last_published_diagnostics(server.output(), &uri)
        .into_iter()
        .find(|diagnostic| diagnostic["code"] == "TL1001")
        .expect("unknown type diagnostic should be published");

    server.send(code_action_request(
        2,
        &uri,
        0,
        0,
        0,
        source.trim_end().len(),
        vec![diagnostic],
    ));

    let action = response_with_id(server.output(), 2)["result"]
        .as_array()
        .expect("code actions should be an array")
        .iter()
        .find(|action| action["title"] == "Import type `Player` from `types`")
        .expect("type import quick fix should be present");
    assert_eq!(action["kind"], "quickfix");
    assert_eq!(
        action["edit"]["changes"][&uri][0]["newText"],
        "import type { Player } from \"./types\"\n"
    );
}

#[test]
fn code_action_reuses_existing_type_namespace() {
    let temp = TempProject::new("code-action-type-namespace");
    let source = r#"import type Shared from "./types"
local player: Player = nil
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/types.d.tua", "export type Player = { name: string }\n");

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();
    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    let diagnostic = last_published_diagnostics(server.output(), &uri)
        .into_iter()
        .find(|diagnostic| diagnostic["code"] == "TL1001")
        .expect("unknown type diagnostic should be published");

    server.send(code_action_request(
        2,
        &uri,
        0,
        0,
        1,
        source.lines().nth(1).unwrap().len(),
        vec![diagnostic],
    ));

    let action = response_with_id(server.output(), 2)["result"]
        .as_array()
        .expect("code actions should be an array")
        .iter()
        .find(|action| action["title"] == "Use `Shared.Player` from `types`")
        .expect("namespace reuse quick fix should be present");
    let edits = action["edit"]["changes"][&uri]
        .as_array()
        .expect("quick fix should edit the current document");
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["newText"], "Shared.Player");
    assert_eq!(range_text(source, &edits[0]["range"]), "Player");
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
fn code_action_generates_type_from_table_literal() {
    let uri = "file:///tmp/code-action-table-type.tua";
    let source = r#"local player = {
  name = "Ada",
  hp = 100,
  alive = true,
  position = { x = 0, y = 0 },
}
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        code_action_request(2, uri, 0, 0, 0, 20, Vec::new()),
    ]);
    let action = response_with_id(&output, 2)["result"]
        .as_array()
        .expect("code actions should be an array")
        .iter()
        .find(|action| action["title"] == "Generate type `Player` from table literal")
        .expect("table type action should be present");
    let edits = action["edit"]["changes"][uri]
        .as_array()
        .expect("table type action should contain text edits");

    assert_eq!(action["kind"], "quickfix");
    assert_eq!(edits.len(), 2);
    assert_eq!(
        edits[0]["newText"],
        r#"type Player = {
  name: string,
  hp: number,
  alive: boolean,
  position: {
    x: number,
    y: number,
  },
}

"#
    );
    assert_eq!(edits[1]["newText"], ": Player");
    assert_eq!(range_text(source, &edits[1]["range"]), "");
}

#[test]
fn code_action_extracts_selected_expression() {
    let uri = "file:///tmp/code-action-extract-expression.tua";
    let source = "local score = 10\nlocal bonus = 2\nlocal total = score + bonus\n";
    let output = run_messages(vec![
        did_open(uri, source),
        code_action_request(2, uri, 2, 14, 2, 27, Vec::new()),
    ]);
    let actions = response_with_id(&output, 2)["result"]
        .as_array()
        .expect("code actions should be an array");
    let local = actions
        .iter()
        .find(|action| action["title"] == "Extract local `extracted`")
        .expect("extract local action should be present");
    let function = actions
        .iter()
        .find(|action| action["title"] == "Extract function `extractedFunction`")
        .expect("extract function action should be present");
    let local_edits = local["edit"]["changes"][uri]
        .as_array()
        .expect("extract local should contain edits");
    let function_edits = function["edit"]["changes"][uri]
        .as_array()
        .expect("extract function should contain edits");

    assert_eq!(local["kind"], "quickfix");
    assert_eq!(
        local_edits[0]["newText"],
        "local extracted = score + bonus\n"
    );
    assert_eq!(local_edits[1]["newText"], "extracted");
    assert_eq!(
        range_text(source, &local_edits[1]["range"]),
        "score + bonus"
    );

    assert_eq!(function["kind"], "quickfix");
    assert_eq!(
        function_edits[0]["newText"],
        "local function extractedFunction(score, bonus)\n  return score + bonus\nend\n\n"
    );
    assert_eq!(
        function_edits[1]["newText"],
        "extractedFunction(score, bonus)"
    );
}

#[test]
fn code_action_inlines_simple_local() {
    let uri = "file:///tmp/code-action-inline-local.tua";
    let source = "local total = score + bonus\nprint(total)\nprint(total)\n";
    let output = run_messages(vec![
        did_open(uri, source),
        code_action_request(2, uri, 0, 6, 0, 11, Vec::new()),
    ]);
    let action = response_with_id(&output, 2)["result"]
        .as_array()
        .expect("code actions should be an array")
        .iter()
        .find(|action| action["title"] == "Inline local `total`")
        .expect("inline local action should be present");
    let edits = action["edit"]["changes"][uri]
        .as_array()
        .expect("inline local should contain edits");

    assert_eq!(action["kind"], "quickfix");
    assert_eq!(edits.len(), 3);
    assert_eq!(edits[0]["newText"], "");
    assert_eq!(edits[0]["range"]["start"]["line"], 0);
    assert_eq!(edits[0]["range"]["start"]["character"], 0);
    assert_eq!(edits[0]["range"]["end"]["line"], 1);
    assert_eq!(edits[0]["range"]["end"]["character"], 0);
    assert_eq!(edits[1]["newText"], "(score + bonus)");
    assert_eq!(range_text(source, &edits[1]["range"]), "total");
    assert_eq!(edits[2]["newText"], "(score + bonus)");
    assert_eq!(range_text(source, &edits[2]["range"]), "total");
}

#[test]
fn code_action_rewrites_love_dot_call_and_constructor_helpers() {
    let uri = "file:///tmp/code-action-love-devex.tua";
    let source = r#"local player = image "assets/player.png"
player.setFilter("nearest")
local raw = love.graphics.newImage("assets/player.png")
"#;
    let mut server = TestServer::new();

    server.send(did_open(uri, source));

    let diagnostics = last_published_diagnostics(server.output(), uri);

    server.send(code_action_request(2, uri, 1, 0, 1, 32, diagnostics));
    server.send(code_action_request(3, uri, 2, 0, 2, 60, Vec::new()));

    let colon_action = response_with_id(server.output(), 2)["result"]
        .as_array()
        .expect("code actions should be an array")
        .iter()
        .find(|action| action["title"] == "Use colon method call")
        .expect("colon-call action should be present");

    assert_eq!(colon_action["title"], "Use colon method call");
    assert_eq!(colon_action["edit"]["changes"][uri][0]["newText"], ":");
    assert_eq!(
        range_text(source, &colon_action["edit"]["changes"][uri][0]["range"]),
        "."
    );

    let helper_actions = response_with_id(server.output(), 3)["result"]
        .as_array()
        .expect("constructor code actions should be an array");
    let helper_action = helper_actions
        .iter()
        .find(|action| action["title"] == "Rewrite as `image` helper")
        .expect("image helper action should be present");

    assert_eq!(
        helper_action["edit"]["changes"][uri][0]["newText"],
        "image \"assets/player.png\""
    );
    assert_eq!(
        range_text(source, &helper_action["edit"]["changes"][uri][0]["range"]),
        "love.graphics.newImage(\"assets/player.png\")"
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
fn code_action_removes_unused_function() {
    let temp = TempProject::new("code-action-unused-function");
    let source = "local function unused()\nend\nlocal function used()\nend\nused()\n";

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

    let diagnostics = last_published_diagnostics(server.output(), &uri)
        .into_iter()
        .filter(|diagnostic| diagnostic["code"] == "TL4055")
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 1);
    server.send(code_action_request(2, &uri, 0, 0, 1, 3, diagnostics));

    let action = code_action_result(server.output(), 2, 0);

    assert_eq!(action["title"], "Remove unused function");
    assert_eq!(action["edit"]["changes"][&uri][0]["newText"], "");
    assert_eq!(
        action["edit"]["changes"][&uri][0]["range"],
        json!({
            "start": { "line": 0, "character": 0 },
            "end": { "line": 2, "character": 0 }
        })
    );
}

#[test]
fn code_action_renames_duplicate_table_key() {
    let temp = TempProject::new("code-action-duplicate-key");
    let source = "local config = { hp = 10, hp = 20 }\nreturn config\n";

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
fn code_action_only_offers_suppression_for_unsafe_edits() {
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

    let actions = response_with_id(server.output(), 2)["result"]
        .as_array()
        .expect("code actions should be an array");

    assert!(!actions.is_empty());
    assert!(actions.iter().all(|action| action["title"]
        .as_str()
        .is_some_and(|title| title.starts_with("Suppress TL"))));
}
#[test]
fn code_action_source_actions_sort_remove_and_organize_imports() {
    let uri = "file:///tmp/code-action-source-imports.tua";
    let source = r#"local Z = require("z")
local A = require("a")
local Unused = import("unused")
print(A, Z)
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        code_action_request_with_only(2, uri, &["source.sortImports"], Vec::new()),
        code_action_request_with_only(3, uri, &["source.removeUnusedImports"], Vec::new()),
        code_action_request_with_only(4, uri, &["source.organizeImports"], Vec::new()),
    ]);
    let sort = code_action_result(&output, 2, 0);
    let remove = code_action_result(&output, 3, 0);
    let organize = code_action_result(&output, 4, 0);

    assert_eq!(sort["kind"], "source.sortImports");
    assert_eq!(sort["title"], "Sort imports");
    assert_eq!(
        sort["edit"]["changes"][uri][0]["newText"],
        r#"local A = require("a")
local Unused = import("unused")
local Z = require("z")
print(A, Z)
"#
    );
    assert_eq!(remove["kind"], "source.removeUnusedImports");
    assert_eq!(remove["title"], "Remove unused imports");
    assert_eq!(
        remove["edit"]["changes"][uri][0]["newText"],
        r#"local Z = require("z")
local A = require("a")
print(A, Z)
"#
    );
    assert_eq!(organize["kind"], "source.organizeImports");
    assert_eq!(organize["title"], "Organize imports");
    assert_eq!(
        organize["edit"]["changes"][uri][0]["newText"],
        r#"local A = require("a")
local Z = require("z")
print(A, Z)
"#
    );
}

#[test]
fn import_source_actions_preserve_shadowed_require_calls() {
    let uri = "file:///tmp/code-action-shadowed-require.tua";
    let source = r#"local function require(name)
  return { name = name }
end
local Custom = require("custom")
local Unused = import("unused")
"#;
    let expected = r#"local function require(name)
  return { name = name }
end
local Custom = require("custom")
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        code_action_request_with_only(2, uri, &["source.removeUnusedImports"], Vec::new()),
        code_action_request_with_only(3, uri, &["source.organizeImports"], Vec::new()),
    ]);
    let remove = code_action_result(&output, 2, 0);
    let organize = code_action_result(&output, 3, 0);

    assert_eq!(remove["edit"]["changes"][uri][0]["newText"], expected);
    assert_eq!(organize["edit"]["changes"][uri][0]["newText"], expected);
}

#[test]
fn code_action_renames_misspelled_love_callback() {
    let uri = "file:///tmp/code-action-love-callback-typo.tua";
    let source = "function love.udpate(dt: number)\nend\n";
    let mut server = TestServer::new();

    server.send(did_open(uri, source));
    let diagnostics = last_published_diagnostics(server.output(), uri);
    server.send(code_action_request(2, uri, 0, 14, 0, 20, diagnostics));

    let action = code_action_result(server.output(), 2, 0);

    assert_eq!(action["title"], "Rename to `love.update`");
    assert_eq!(action["kind"], "quickfix");
    assert_eq!(action["edit"]["changes"][uri][0]["newText"], "update");
    assert_eq!(
        range_text(source, &action["edit"]["changes"][uri][0]["range"]),
        "udpate"
    );
}

#[test]
fn code_action_creates_missing_love_callbacks() {
    let uri = "file:///tmp/code-action-love-callbacks.tua";
    let source = "function love.update(dt: number)\nend\n";
    let output = run_messages(vec![
        did_open(uri, source),
        code_action_request_with_only(
            2,
            uri,
            &["source.tua.createMissingLoveCallbacks"],
            Vec::new(),
        ),
    ]);
    let action = code_action_result(&output, 2, 0);

    assert_eq!(action["title"], "Create missing LOVE callbacks");
    assert_eq!(action["kind"], "source.tua.createMissingLoveCallbacks");
    assert_eq!(
        action["edit"]["changes"][uri][0]["newText"],
        "\nfunction love.load(arg, unfilteredArg)\nend\n\nfunction love.draw()\nend\n"
    );
}

#[test]
fn code_action_implements_missing_imported_protocol_methods_for_inheriting_class() {
    let temp = TempProject::new("code-action-protocol-stubs");
    let source = r#"import type { Updatable } from "./contracts"
open class Base {
  update(dt: number): nil {}
}
final class Actor extends Base implements Updatable {
}
"#;
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write(
        "src/contracts.d.tua",
        "export protocol Updatable {\n  update(dt: number): nil\n  --- draws the actor to the screen\n  draw(): nil\n}\n",
    );

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();
    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    let diagnostics = last_published_diagnostics(server.output(), &uri);
    let actor_diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic["message"].as_str().is_some_and(|message| {
                message.contains("class `Actor`") && message.contains("missing method `draw`")
            })
        })
        .cloned()
        .unwrap_or_else(|| panic!("the inheriting class should receive its unimplemented protocol requirement: {diagnostics:#?}"));

    server.send(code_action_request_with_only(
        2,
        &uri,
        &["quickfix"],
        vec![actor_diagnostic],
    ));
    let actions = response_with_id(server.output(), 2)["result"]
        .as_array()
        .expect("code actions should be an array");
    let action = actions
        .iter()
        .find(|action| action["title"] == "Implement missing protocol methods")
        .expect("the protocol stub quick fix should be available");

    assert_eq!(action["kind"], "quickfix");
    let edit = &action["edit"]["changes"][&uri][0];
    assert_eq!(range_text(source, &edit["range"]), "");
    assert_eq!(
        edit["newText"],
        "  --- draws the actor to the screen\n  draw(): nil {\n    error(\"TODO: implement Actor.draw\")\n  }\n"
    );

    let mut implemented_source = source.to_owned();
    implemented_source.insert_str(
        implemented_source
            .rfind('}')
            .expect("class closing brace exists"),
        edit["newText"].as_str().expect("stub edit is text"),
    );
    server.send(did_change(&uri, &implemented_source, 2));
    assert!(
        last_published_diagnostics(server.output(), &uri)
            .iter()
            .all(|diagnostic| diagnostic["code"] != "TL1008"),
        "the TODO-throwing stub should satisfy the imported protocol signature"
    );
}

#[test]
fn protocol_stub_action_generates_multiple_methods_but_skips_conflicting_obligations() {
    let uri = "file:///tmp/code-action-protocol-conflict.tua";
    let source = r#"protocol TextActions {
  act(value: string): nil
  ready(): nil
}
protocol NumberActions {
  act(value: number): nil
  done(): nil
}
final class Actor implements TextActions, NumberActions {
}
"#;
    let mut server = TestServer::new();
    server.send(did_open(uri, source));
    let diagnostics = last_published_diagnostics(server.output(), uri);
    assert!(
        diagnostics.len() >= 4,
        "each protocol requirement should produce an actionable missing-method diagnostic: {diagnostics:#?}"
    );
    server.send(code_action_request_with_only(
        2,
        uri,
        &["quickfix"],
        diagnostics,
    ));
    let actions = response_with_id(server.output(), 2)["result"]
        .as_array()
        .expect("code actions should be an array");
    let action = actions
        .iter()
        .find(|action| action["title"] == "Implement missing protocol methods")
        .expect("unambiguous protocol requirements should still be generated");
    let new_text = action["edit"]["changes"][uri][0]["newText"]
        .as_str()
        .expect("protocol stubs should be inserted as source text");

    assert!(
        new_text.contains("ready(): nil"),
        "generated source: {new_text:?}"
    );
    assert!(
        new_text.contains("done(): nil"),
        "generated source: {new_text:?}"
    );
    assert!(
        !new_text.contains("act("),
        "conflicting signatures must not be guessed"
    );

    let mut partially_implemented = source.to_owned();
    partially_implemented.insert_str(
        partially_implemented
            .rfind('}')
            .expect("class closing brace exists"),
        new_text,
    );
    server.send(did_change(uri, &partially_implemented, 2));
    let remaining = last_published_diagnostics(server.output(), uri);
    assert_eq!(
        remaining.len(),
        2,
        "both conflicting `act` requirements should remain visible: {remaining:#?}"
    );
    assert!(
        remaining
            .iter()
            .filter_map(|diagnostic| diagnostic["message"].as_str())
            .all(|message| message.contains("missing method `act`")),
        "only the genuinely ambiguous requirement should remain: {remaining:#?}"
    );
}

#[test]
fn protocol_stub_action_preserves_crlf_line_endings() {
    let uri = "file:///tmp/code-action-protocol-crlf.tua";
    let source = "protocol Renderable {\r\n  render(): nil\r\n}\r\nfinal class Panel implements Renderable {\r\n}\r\n";
    let mut server = TestServer::new();
    server.send(did_open(uri, source));
    let diagnostics = last_published_diagnostics(server.output(), uri);
    server.send(code_action_request_with_only(
        2,
        uri,
        &["quickfix"],
        diagnostics,
    ));
    let actions = response_with_id(server.output(), 2)["result"]
        .as_array()
        .expect("code actions should be an array");
    let action = actions
        .iter()
        .find(|action| action["title"] == "Implement missing protocol methods")
        .expect("the protocol stub quick fix should be available");
    let new_text = action["edit"]["changes"][uri][0]["newText"]
        .as_str()
        .expect("protocol stubs should be inserted as source text");

    assert!(new_text.contains("\r\n"), "generated text: {new_text:?}");
    assert!(
        !new_text.replace("\r\n", "").contains('\n'),
        "generated text must not mix LF into a CRLF document: {new_text:?}"
    );
}

#[test]
fn code_action_creates_object_pool_for_repeated_constructor_call() {
    let uri = "file:///tmp/code-action-object-pool.tua";
    let source = r#"local first = Bullet.new()
local second = Bullet.new()
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        code_action_request(2, uri, 0, 14, 0, 26, Vec::new()),
    ]);
    let action = response_with_id(&output, 2)["result"]
        .as_array()
        .expect("code actions should be an array")
        .iter()
        .find(|action| {
            action["title"]
                .as_str()
                .is_some_and(|title| title.contains("objectPool"))
        })
        .expect("objectPool constructor action should be present");
    let edits = action["edit"]["changes"][uri]
        .as_array()
        .expect("objectPool action should contain text edits");

    assert_eq!(action["kind"], "quickfix");
    assert_eq!(edits.len(), 2);
    assert!(edits[0]["newText"]
        .as_str()
        .is_some_and(|text| text.contains("local bulletPool = objectPool({")));
    assert_eq!(edits[1]["newText"], "bulletPool:acquire()");
    assert_eq!(range_text(source, &edits[1]["range"]), "Bullet.new()");
}
#[test]
fn code_action_source_fix_all_uses_current_safe_diagnostics() {
    let temp = TempProject::new("code-action-fix-all");
    let source = "local unused = 1\nhp = hp\nlocal used = 2\nprint(used)\n";

    temp.write(
        "tua.toml",
        r#"source = "src"

[diagnostics]
unused_locals = true
self_assignments = true
"#,
    );

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    server.send(code_action_request_with_only(
        2,
        &uri,
        &["source.fixAll"],
        Vec::new(),
    ));

    let action = code_action_result(server.output(), 2, 0);
    let edits = action["edit"]["changes"][&uri]
        .as_array()
        .expect("fix-all should return edits");

    assert_eq!(action["kind"], "source.fixAll");
    assert_eq!(action["title"], "Fix all safe diagnostics");
    assert_eq!(edits.len(), 2);
    assert!(edits.iter().all(|edit| edit["newText"] == ""));

    let missing = TempProject::new("code-action-fix-all-unsafe");
    let missing_source = "local Player = require(\"player\")\n";

    missing.write(
        "tua.toml",
        r#"source = "src"

[diagnostics]
missing_requires = true
"#,
    );

    let missing_uri = missing.uri_for("src/main.tua");
    let mut unsafe_server = TestServer::new();

    unsafe_server.send(initialize_with_root(1, missing.uri()));
    unsafe_server.send(did_open(&missing_uri, missing_source));
    unsafe_server.send(code_action_request_with_only(
        2,
        &missing_uri,
        &["source.fixAll"],
        Vec::new(),
    ));

    assert_eq!(
        response_with_id(unsafe_server.output(), 2)["result"],
        json!([])
    );
}

#[test]
fn class_instance_export_action_feeds_type_auto_import_completion() {
    let fixture = Fixture::parse(
        r#"-- file: tua.toml
source = "src"
-- file: src/dialogue.tua
final class Dialogue {
  text: string = "Hi"
}
return { Dialogue = Dialogue }
-- file: src/main.tua
local dialogue: Dialog = nil
"#,
    );
    let temp = fixture.write_to_temp("class-instance-export").unwrap();
    let provider = fixture.source("src/dialogue.tua");
    let provider_uri = path_uri(&temp.join("src/dialogue.tua"));
    let main = fixture.source("src/main.tua");
    let uri = path_uri(&temp.join("src/main.tua"));
    let mut server = TestServer::new();
    server.send(initialize_with_root(1, path_uri(temp.path())));
    server.send(did_open(&provider_uri, provider));
    server.send(code_action_request(
        2,
        &provider_uri,
        0,
        15,
        0,
        15,
        Vec::new(),
    ));
    let actions = response_with_id(server.output(), 2)["result"]
        .as_array()
        .unwrap();
    let action = actions
        .iter()
        .find(|item| item["title"] == "Export instance type `DialogueInstance`")
        .unwrap();
    let edit = &action["edit"]["changes"][&provider_uri][0];
    assert_eq!(edit["range"]["start"], json!({"line": 2, "character": 1}));
    let text = edit["newText"].as_str().unwrap();
    let edited = provider.replacen("}\nreturn", &format!("}}{text}\nreturn"), 1);
    server.send(did_change(&provider_uri, &edited, 2));
    server.send(did_open(&uri, main));
    server.send(completion_request(3, &uri, main, "dialogue: Dialog"));
    assert_completion_labels(server.output(), 3, ["DialogueInstance"]);
    let completions = response_with_id(server.output(), 3)["result"]
        .as_array()
        .unwrap();
    let completion = completions
        .iter()
        .find(|item| item["label"] == "DialogueInstance")
        .unwrap();
    let completion = completion.clone();
    server.send(completion_resolve_request(4, completion));
    let resolved = &response_with_id(server.output(), 4)["result"];
    assert_eq!(
        resolved["additionalTextEdits"][0]["newText"],
        "import type { DialogueInstance } from \"./dialogue\"\n"
    );
}

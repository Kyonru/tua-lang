use super::*;

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

    let colon_action = code_action_result(server.output(), 2, 0);

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

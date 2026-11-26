use super::*;

#[test]
fn inspector_returns_source_linked_behavior_tree_nodes() {
    let temp = TempProject::new("behavior-tree-inspector");
    let source = r#"local tree = BehaviorTree.new()
tree:setRoot(BehaviorTree.selector({
  BehaviorTree.condition(function(actor) return actor.ready end),
  BehaviorTree.sequence({ BehaviorTree.wait(0.2), fallback }),
}))
"#;
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/ai.tua", source);
    let uri = temp.uri_for("src/ai.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
        inspector_request(2, &uri, None),
        inspector_request(3, &uri, Some(lsp_position_after(source, "local tr"))),
    ]);
    let tree = &response_with_id(&output, 2)["result"]["items"][0];
    assert_eq!(tree["kind"], "behaviorTree");
    assert_eq!(tree["name"], "tree");
    assert_eq!(tree["truncated"], false);
    assert_eq!(tree["root"]["kind"], "selector");
    assert_eq!(tree["root"]["children"][0]["kind"], "condition");
    assert_eq!(tree["root"]["children"][1]["kind"], "sequence");
    assert_eq!(
        tree["root"]["children"][1]["children"][1]["kind"],
        "dynamic"
    );
    assert_eq!(tree["root"]["children"][0]["range"]["start"]["line"], 2);
    assert_eq!(
        response_with_id(&output, 3)["result"]["items"][0]["name"],
        "tree"
    );
}

#[test]
fn inspector_returns_compiler_backed_sprite_metadata_and_diagnostics() {
    let temp = TempProject::new("sprite-inspector");
    let source = r#"local Player = spriteSheet({
  image = "assets/player.png",
  frame = { width = 16, height = 16, left = 2, top = 4, border = 1 },
  origin = { x = 8, y = 16 },
  animations = {
    idle = {
      frames = {
        { from = 0, to = 1, row = 1 },
        { from = 0, to = 1, row = 2 },
      },
      origin = { x = 7, y = 15 },
      fps = 8,
      markers = { { frame = 1, name = "step" } },
      metadata = {
        { origin = { x = 7, y = 15 }, sourceWidth = 16, sourceHeight = 16 },
        { origin = { x = 8, y = 14 }, trimX = 1, trimY = 2, sourceWidth = 18, sourceHeight = 20 },
        { origin = { x = 7, y = 15 }, sourceWidth = 16, sourceHeight = 16 },
        { origin = { x = 7, y = 15 }, sourceWidth = 16, sourceHeight = 16 },
      },
    },
    idle = {
      from = 4,
      to = 2,
      frame = { width = 8, height = 12, left = 6, top = 10, border = 2 },
      durations = 0.25,
      onLoop = "pauseAtEnd",
    },
  },
})
local selected = Player
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/sprites.tua", source);
    temp.write("src/assets/player.png", "placeholder");

    let uri = temp.uri_for("src/sprites.tua");
    let image_uri = temp.uri_for("src/assets/player.png");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
        inspector_request(2, &uri, None),
        inspector_request(3, &uri, Some(lsp_position_after(source, "selected = Pla"))),
    ]);
    let response = response_with_id(&output, 2);
    let sprite = &response["result"]["items"][0];

    assert_eq!(response["result"]["documentVersion"], 1);
    assert!(response["result"]["analysisRevision"].as_u64().is_some());
    assert_eq!(sprite["kind"], "spriteSheet");
    assert_eq!(sprite["name"], "Player");
    assert_eq!(sprite["image"]["path"], "assets/player.png");
    assert_eq!(sprite["image"]["uri"], image_uri);
    assert_eq!(sprite["image"]["status"], "valid");
    assert_eq!(sprite["frame"]["width"], 16);
    assert_eq!(sprite["frame"]["height"], 16);
    assert_eq!(sprite["frame"]["left"], 2);
    assert_eq!(sprite["frame"]["top"], 4);
    assert_eq!(sprite["frame"]["border"], 1);
    assert_eq!(sprite["origin"]["x"], 8);
    assert_eq!(sprite["origin"]["y"], 16);
    assert_eq!(sprite["animations"][0]["name"], "idle");
    assert_eq!(sprite["animations"][0]["fps"], 8.0);
    assert_eq!(sprite["animations"][0]["row"], 1);
    assert_eq!(
        sprite["animations"][0]["frames"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(sprite["animations"][0]["frames"][1]["row"], 2);
    assert_eq!(sprite["animations"][0]["origin"]["x"], 7);
    assert_eq!(sprite["animations"][0]["durationValues"][0], 0.125);
    assert_eq!(sprite["animations"][0]["markers"][0]["name"], "step");
    assert_eq!(sprite["animations"][0]["metadata"][1]["trimX"], 1);
    assert_eq!(sprite["animations"][0]["metadata"][1]["trimY"], 2);
    assert_eq!(sprite["animations"][0]["metadata"][1]["sourceWidth"], 18);
    assert_eq!(sprite["animations"][1]["frame"]["width"], 8);
    assert_eq!(sprite["animations"][1]["frame"]["left"], 6);
    assert_eq!(sprite["animations"][1]["durations"], "0.25");
    assert_eq!(sprite["animations"][1]["onLoop"], "pauseAtEnd");
    assert_eq!(sprite["animations"][1]["loop"], false);

    let messages = sprite["diagnostics"]
        .as_array()
        .expect("sprite diagnostics should be an array")
        .iter()
        .filter_map(|diagnostic| diagnostic["message"].as_str())
        .collect::<Vec<_>>();

    assert!(messages
        .iter()
        .any(|message| message.contains("defined more than once")));
    assert!(messages
        .iter()
        .any(|message| message.contains("invalid frame range")));
    assert_eq!(
        response_with_id(&output, 3)["result"]["items"][0]["name"],
        "Player"
    );
}

#[test]
fn inspector_returns_source_linked_state_machine_graph_metadata() {
    let temp = TempProject::new("state-machine-inspector");
    let source = r#"local flow = stateMachine({
  initial = "idle",
  events = {
    { name = "start", from = "idle", to = "running" },
    { name = "reset", from = { "running", "paused" }, to = "idle" },
    { name = "stop", from = "*", to = "idle" },
  },
})
local selected = flow
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/flow.tua", source);

    let uri = temp.uri_for("src/flow.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
        inspector_request(2, &uri, None),
        inspector_request(3, &uri, Some(lsp_position_after(source, "selected = flo"))),
    ]);
    let response = response_with_id(&output, 2);
    let machine = response["result"]["items"]
        .as_array()
        .and_then(|items| items.iter().find(|item| item["kind"] == "stateMachine"))
        .expect("state machine inspection");

    assert_eq!(response["result"]["documentVersion"], 1);
    assert_eq!(machine["name"], "flow");
    assert_eq!(machine["initial"], "idle");
    assert_eq!(machine["states"][0]["name"], "idle");
    assert_eq!(machine["states"][0]["initial"], true);
    assert_eq!(machine["transitions"][0]["event"], "start");
    assert_eq!(machine["transitions"][0]["from"][0], "idle");
    assert_eq!(machine["transitions"][0]["to"], "running");
    assert_eq!(machine["transitions"][1]["from"][1], "paused");
    assert_eq!(machine["transitions"][2]["wildcardFrom"], true);
    assert!(machine["transitions"][0]["selectionRange"]["start"]["line"]
        .as_u64()
        .is_some());
    assert_eq!(
        response_with_id(&output, 3)["result"]["items"][0]["name"],
        "flow"
    );
}

#[test]
fn inspector_can_fall_back_to_all_state_machines_in_one_request() {
    let temp = TempProject::new("state-machine-inspector-fallback");
    let source = r#"local sprite = spriteSheet({})
local flow = stateMachine({ initial = "idle", events = {} })
local selected = flow
local unrelated = 42
"#;
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/flow.tua", source);
    let uri = temp.uri_for("src/flow.tua");
    let mut unrelated =
        inspector_request(2, &uri, Some(lsp_position_after(source, "unrelated = 4")));
    unrelated["params"]["kind"] = json!("stateMachine");
    unrelated["params"]["fallbackToAll"] = json!(true);
    let mut selected =
        inspector_request(3, &uri, Some(lsp_position_after(source, "selected = flo")));
    selected["params"]["kind"] = json!("stateMachine");
    selected["params"]["fallbackToAll"] = json!(true);

    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
        unrelated,
        selected,
    ]);
    let fallback = &response_with_id(&output, 2)["result"];
    assert_eq!(fallback["fallbackApplied"], true);
    assert_eq!(fallback["items"].as_array().unwrap().len(), 1);
    assert_eq!(fallback["items"][0]["name"], "flow");
    let focused = &response_with_id(&output, 3)["result"];
    assert_eq!(focused["fallbackApplied"], false);
    assert_eq!(focused["items"][0]["name"], "flow");
}

#[test]
fn dialogue_preview_uses_current_overlay_and_utf16_navigation() {
    let temp = TempProject::new("dialogue-preview");
    temp.write("tua.toml", "source = 'src'\n");
    let source = "@dialogue 1\n[start]\n// @id greeting\n: Hello %{name}\n// @id proceed\n-> 😀 Go [target:done]\n[done]\n// @id bye\n: Bye (end)\n";
    // Use plain authored text: placeholders require a declared binding.
    let source = source.replace("%{name}", "reader");
    temp.write("src/scene.tdialogue", &source);
    let uri = temp.uri_for("src/scene.tdialogue");
    let mut server = TestServer::new();
    server.send(initialize_with_root(1, temp.uri()));
    let mut open = did_open(&uri, &source);
    open["params"]["textDocument"]["languageId"] = json!("tdialogue");
    server.send(open);
    let request = |id| json!({"jsonrpc":"2.0", "id":id, "method":"tua/dialoguePreview", "params":{"textDocument":{"uri":uri}}});
    server.send(request(2));
    let result = &response_with_id(server.output(), 2)["result"];
    assert_eq!(result["documentVersion"], 1);
    assert_eq!(
        result["preview"]["sections"][0]["lines"][0]["text"],
        "Hello reader"
    );
    assert_eq!(result["preview"]["routes"][0]["target"], "done");
    assert_eq!(
        result["preview"]["routes"][0]["range"]["start"],
        json!({"line":5,"character":17})
    );
    server.send(did_change(&uri, "@dialogue 1\n[broken", 2));
    server.send(request(3));
    let result = &response_with_id(server.output(), 3)["result"];
    assert_eq!(result["documentVersion"], 2);
    assert!(result["preview"].is_null());
    server.send(did_change(&uri, &source.replace("Hello", "Welcome"), 3));
    server.send(request(4));
    let result = &response_with_id(server.output(), 4)["result"];
    assert_eq!(result["documentVersion"], 3);
    assert_eq!(
        result["preview"]["sections"][0]["lines"][0]["text"],
        "Welcome reader"
    );
}

#[test]
fn dialogue_preview_mock_is_opt_in_and_invalid_input_clears_results() {
    let temp = TempProject::new("dialogue-preview-mock");
    temp.write("tua.toml", "source = 'src'\n");
    let source = "@dialogue 1\n@context \"context\"\n[gate]\n@branch context.coins > 0 [target:gate] [else:gate]\n";
    temp.write("src/scene.tdialogue", source);
    let uri = temp.uri_for("src/scene.tdialogue");
    let mut server = TestServer::new();
    server.send(initialize_with_root(1, temp.uri()));
    let mut open = did_open(&uri, source);
    open["params"]["textDocument"]["languageId"] = json!("tdialogue");
    server.send(open);
    let request = |id, mock: Option<Value>| {
        let mut params = json!({"textDocument":{"uri":uri}});
        if let Some(mock) = mock {
            params["mockContext"] = mock;
        }
        json!({"jsonrpc":"2.0", "id":id, "method":"tua/dialoguePreview", "params":params})
    };
    server.send(request(2, None));
    assert!(response_with_id(server.output(), 2)["result"]["preview"]["mock"].is_null());
    server.send(request(3, Some(json!(r#"{"context":{"coins":2}}"#))));
    let result = &response_with_id(server.output(), 3)["result"];
    assert_eq!(result["preview"]["mock"]["conditions"][0]["value"], true);
    assert_eq!(result["preview"]["routes"].as_array().unwrap().len(), 2);
    server.send(request(4, Some(json!(r#"{"context":{"coins":"two"}}"#))));
    let result = &response_with_id(server.output(), 4)["result"];
    assert_eq!(
        result["preview"]["mock"]["conditions"][0]["error"],
        "ordered comparison needs numbers"
    );
    assert_eq!(
        result["preview"]["mock"]["conditions"][0]["range"]["start"]["line"],
        3
    );
    for (id, input) in [
        (5, json!("{")),
        (6, json!({})),
        (7, json!(" ".repeat(65_537))),
    ] {
        server.send(request(id, Some(input)));
        let result = &response_with_id(server.output(), id)["result"];
        assert!(result["mockError"].is_string());
        assert!(result["preview"]["mock"].is_null());
        assert_eq!(result["preview"]["routes"].as_array().unwrap().len(), 2);
    }
    server.send(request(8, None));
    assert!(response_with_id(server.output(), 8)["result"]["preview"]["mock"].is_null());
}

#[test]
fn dialogue_preview_mock_text_refreshes_and_preserves_authored_source() {
    let temp = TempProject::new("dialogue-preview-mock-text");
    temp.write("tua.toml", "source = 'src'\n");
    let source = "@dialogue 1\n[start]\n// @id hello\n: Hello %{name} (end)\n";
    temp.write("src/scene.tdialogue", source);
    let uri = temp.uri_for("src/scene.tdialogue");
    let mut server = TestServer::new();
    server.send(initialize_with_root(1, temp.uri()));
    let mut open = did_open(&uri, source);
    open["params"]["textDocument"]["languageId"] = json!("tdialogue");
    server.send(open);
    let request = |id, input| json!({"jsonrpc":"2.0", "id":id, "method":"tua/dialoguePreview", "params":{"textDocument":{"uri":uri},"mockContext":input}});
    server.send(request(2, r#"{"args":{"name":"Ada"}}"#));
    let result = &response_with_id(server.output(), 2)["result"];
    assert_eq!(
        result["preview"]["mock"]["text"]["texts"][0]["value"],
        "Hello Ada"
    );
    assert_eq!(
        result["preview"]["sections"][0]["lines"][0]["text"],
        "Hello %{name}"
    );
    server.send(did_change(&uri, &source.replace("Hello", "Welcome"), 2));
    server.send(request(3, r#"{"args":{"name":"<script>"}}"#));
    let result = &response_with_id(server.output(), 3)["result"];
    assert_eq!(result["documentVersion"], 2);
    assert_eq!(
        result["preview"]["mock"]["text"]["texts"][0]["value"],
        "Welcome <script>"
    );
    server.send(request(4, "{}"));
    let result = &response_with_id(server.output(), 4)["result"];
    assert!(result["preview"]["mock"]["text"]["texts"][0]["value"].is_null());
    assert_eq!(
        result["preview"]["mock"]["text"]["texts"][0]["error"],
        "missing or invalid dialogue argument"
    );
    let bound = source.replace(
        "[start]",
        "@context \"context\"\n@bind name context.name\n[start]",
    );
    server.send(did_change(&uri, &bound, 3));
    server.send(request(5, "{}"));
    let result = &response_with_id(server.output(), 5)["result"];
    assert_eq!(
        result["preview"]["mock"]["text"]["bindingError"]["message"],
        "missing dialogue binding"
    );
    assert_eq!(
        result["preview"]["mock"]["text"]["bindingError"]["range"]["start"]["line"],
        2
    );
}

#[test]
fn dialogue_preview_keeps_overlay_versions_after_disk_deletion_and_recreation() {
    let temp = TempProject::new("dialogue-preview-disk-lifecycle");
    temp.write("tua.toml", "source = 'src'\n");
    let source = "@dialogue 1\n[start]\n// @id greeting\n: Original (end)\n";
    temp.write("src/scene.tdialogue", source);
    let uri = temp.uri_for("src/scene.tdialogue");
    let mut server = TestServer::new();
    server.send(initialize_with_root(1, temp.uri()));
    let mut open = did_open(&uri, source);
    open["params"]["textDocument"]["languageId"] = json!("tdialogue");
    server.send(open);
    let request = |id| json!({"jsonrpc":"2.0", "id":id, "method":"tua/dialoguePreview", "params":{"textDocument":{"uri":uri}}});
    temp.remove("src/scene.tdialogue");
    server.send(watched_files_changed([(&*uri, 3)]));
    server.send(did_change(&uri, &source.replace("Original", "Unsaved"), 2));
    server.send(request(2));
    let result = &response_with_id(server.output(), 2)["result"];
    assert_eq!(result["documentVersion"], 2);
    assert_eq!(
        result["preview"]["sections"][0]["lines"][0]["text"],
        "Unsaved"
    );
    temp.write(
        "src/scene.tdialogue",
        &source.replace("Original", "Recreated"),
    );
    server.send(watched_files_changed([(&*uri, 1)]));
    server.send(did_change(
        &uri,
        &source.replace("Original", "Still unsaved"),
        3,
    ));
    server.send(request(3));
    let result = &response_with_id(server.output(), 3)["result"];
    assert_eq!(result["documentVersion"], 3);
    assert_eq!(
        result["preview"]["sections"][0]["lines"][0]["text"],
        "Still unsaved"
    );
    server.send(did_close(&uri));
    server.send(request(4));
    let result = &response_with_id(server.output(), 4)["result"];
    assert!(result["documentVersion"].is_null());
    assert!(result["preview"].is_null());
    let recreated = std::fs::read_to_string(temp.path.join("src/scene.tdialogue")).unwrap();
    let mut reopen = did_open(&uri, &recreated);
    reopen["params"]["textDocument"]["languageId"] = json!("tdialogue");
    server.send(reopen);
    server.send(request(5));
    let result = &response_with_id(server.output(), 5)["result"];
    assert_eq!(result["documentVersion"], 1);
    assert_eq!(
        result["preview"]["sections"][0]["lines"][0]["text"],
        "Recreated"
    );
}

#[test]
fn dialogue_preview_character_literals_expressions_and_rich_mock_use_overlays() {
    let temp = TempProject::new("dialogue-preview-media");
    temp.write("nested/tua.toml", "source = 'src'\n");
    let cast = "local Guide = { name = 'Mara', portrait = 'assets/mara.png', expressions = { happy = { portrait = 'assets/happy.png' } } }\nreturn { Guide = Guide, Courier = { name = 'Ivo', portrait = 'ivo' } }\n";
    temp.write("nested/src/characters.tua", cast);
    let source = "@dialogue 1\n@text rich\n@characters Cast \"characters\"\n[start]\n// @id welcome\nCast.Guide: [em]Hello %{name}[/em][pause=0.2] (end)\n@expression happy\n";
    temp.write("nested/src/scene.tdialogue", source);
    let uri = temp.uri_for("nested/src/scene.tdialogue");
    let mut server = TestServer::new();
    server.send(initialize_with_root(1, temp.uri()));
    let mut open = did_open(&uri, source);
    open["params"]["textDocument"]["languageId"] = json!("tdialogue");
    server.send(open);
    let request = |id| json!({"jsonrpc":"2.0", "id":id, "method":"tua/dialoguePreview", "params":{"textDocument":{"uri":uri},"mockContext":r#"{"args":{"name":"[style=warning]<img>[/style]"}}"#}});
    server.send(request(2));
    let result = &response_with_id(server.output(), 2)["result"]["preview"];
    assert_eq!(
        result["sections"][0]["lines"][0]["character"]["name"], "Mara",
        "{result}"
    );
    assert_eq!(
        result["sections"][0]["lines"][0]["character"]["portrait"],
        "assets/happy.png"
    );
    assert_eq!(
        result["mock"]["text"]["texts"][0]["rich"][1]["text"],
        "[style=warning]<img>[/style]"
    );
    assert_eq!(
        result["mock"]["text"]["texts"][0]["rich"][1]["marks"],
        json!(["em"])
    );
    assert_eq!(
        result["sections"][0]["lines"][0]["rich"][2]["annotation"],
        "Pause 0.2s"
    );
    let cast_uri = temp.uri_for("nested/src/characters.tua");
    server.send(did_open(&cast_uri, cast));
    server.send(did_change(
        &cast_uri,
        &cast
            .replace("Mara", "Marina")
            .replace("assets/happy.png", "new.png"),
        2,
    ));
    server.send(request(3));
    let result = &response_with_id(server.output(), 3)["result"]["preview"];
    assert_eq!(
        result["sections"][0]["lines"][0]["character"]["name"],
        "Marina"
    );
    assert_eq!(
        result["sections"][0]["lines"][0]["character"]["portrait"],
        "new.png"
    );
    server.send(did_change(
        &uri,
        &source
            .replace("Cast.Guide", "Cast.Courier")
            .replace("@expression happy\n", ""),
        2,
    ));
    server.send(request(4));
    let result = &response_with_id(server.output(), 4)["result"]["preview"];
    assert_eq!(
        result["sections"][0]["lines"][0]["character"]["name"],
        "Ivo"
    );
    assert_eq!(
        result["sections"][0]["lines"][0]["character"]["portrait"],
        "ivo"
    );
}

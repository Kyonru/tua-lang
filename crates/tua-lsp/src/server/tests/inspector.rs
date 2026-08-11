use super::*;

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

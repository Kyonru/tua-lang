use super::*;

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
fn signature_help_returns_signal_event_payload_signature() {
    let uri = "file:///tmp/signal-signature-help.tua";
    let source = r#"type Player = {
  name: string,
  hp: number,
}

local events = signal()
events:on("player.hit", function(player: Player, damage: number)
  player.hp -= damage
end)

events:emit("player.hit",
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        signature_help_request(2, uri, source, "events:emit(\"player.hit\","),
    ]);
    let response = response_with_id(&output, 2);

    assert_eq!(
        response["result"]["signatures"][0]["label"],
        "function events:emit(\"player.hit\", player: Player, damage: number): nil"
    );
    assert_eq!(response["result"]["activeSignature"], 0);
    assert_eq!(response["result"]["activeParameter"], 1);
    assert_eq!(
        response["result"]["signatures"][0]["parameters"][1]["label"],
        "player: Player"
    );
}
#[test]
fn signature_help_accepts_trigger_context() {
    let uri = "file:///tmp/signature-help-trigger.tua";
    let source = r#"local function damage(target: number, amount: number): number
  return target - amount
end

local result = damage(
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        signature_help_trigger_request(2, uri, source, "result = damage(", "("),
    ]);
    let response = response_with_id(&output, 2);

    assert_eq!(
        response["result"]["signatures"][0]["label"],
        "function damage(target: number, amount: number): number"
    );
    assert_eq!(response["result"]["activeSignature"], 0);
    assert_eq!(response["result"]["activeParameter"], 0);
}
#[test]
fn signature_help_accepts_retrigger_context() {
    let uri = "file:///tmp/signature-help-retrigger.tua";
    let source = r#"local function damage(target: number, amount: number): number
  return target - amount
end

damage(10,
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        signature_help_retrigger_request(2, uri, source, "damage(10,", ")"),
    ]);
    let response = response_with_id(&output, 2);

    assert_eq!(
        response["result"]["signatures"][0]["label"],
        "function damage(target: number, amount: number): number"
    );
    assert_eq!(response["result"]["activeParameter"], 1);
}
#[test]
fn signature_help_retrigger_recomputes_outer_call_after_nested_close() {
    let uri = "file:///tmp/signature-help-retrigger-nested-close.tua";
    let source = r#"local function load(path: string): string
  return path
end

---@overload fun(value: string): string
---@overload fun(value: string, amount: number): string
local function wrap(value, amount)
  return value
end

wrap(load("asset.png")
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        signature_help_retrigger_request(2, uri, source, r#"load("asset.png")"#, ")"),
    ]);
    let response = response_with_id(&output, 2);

    assert_eq!(
        response["result"]["signatures"][0]["label"],
        "function wrap(value: string): string"
    );
    assert_eq!(
        response["result"]["signatures"][1]["label"],
        "function wrap(value: string, amount: number): string"
    );
    assert_eq!(response["result"]["activeSignature"], 0);
    assert_eq!(response["result"]["activeParameter"], 0);
}
#[test]
fn signature_help_returns_lua_doc_overloads() {
    let uri = "file:///tmp/signature-help-overload.tua";
    let source = r#"---@overload fun(path: string): Image
---@overload fun(path: string, mode: "stream"): Source
local function load(path, mode)
  return nil
end

load("assets/jump.wav",
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        signature_help_request(2, uri, source, "load(\"assets/jump.wav\","),
    ]);
    let response = response_with_id(&output, 2);

    assert_eq!(
        response["result"]["signatures"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        response["result"]["signatures"][0]["label"],
        "function load(path: string): Image"
    );
    assert_eq!(
        response["result"]["signatures"][1]["label"],
        "function load(path: string, mode: \"stream\"): Source"
    );
    assert_eq!(response["result"]["activeSignature"], 1);
    assert_eq!(response["result"]["activeParameter"], 1);
}
#[test]
fn signature_help_returns_love_overloads_and_parameter_docs() {
    let uri = "file:///tmp/signature-help-love-overloads.tua";
    let source = "love.graphics.print(\"hi\", \n";
    let output = run_messages(vec![
        did_open(uri, source),
        signature_help_request(2, uri, source, "love.graphics.print(\"hi\", "),
    ]);
    let response = response_with_id(&output, 2);

    assert_eq!(
        response["result"]["signatures"].as_array().unwrap().len(),
        6
    );
    assert_eq!(
        response["result"]["signatures"][0]["label"],
        "function love.graphics.print(text: string, x?: number, y?: number, r?: number, sx?: number, sy?: number, ox?: number, oy?: number, kx?: number, ky?: number)"
    );
    assert_eq!(
        response["result"]["signatures"][2]["label"],
        "function love.graphics.print(text: string, transform: Transform)"
    );
    assert_eq!(response["result"]["activeParameter"], 1);
    assert!(
        response["result"]["signatures"][0]["documentation"]["value"]
            .as_str()
            .expect("signature documentation should be a string")
            .contains("Draws text on screen")
    );
    assert_eq!(
        response["result"]["signatures"][0]["parameters"][1]["documentation"]["value"],
        "The position to draw the object (x-axis).\n\nDefault: `0`."
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

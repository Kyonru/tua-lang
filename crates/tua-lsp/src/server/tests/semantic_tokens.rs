use super::*;

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
local x = hero.stats.position.x
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
    assert_semantic_token(&tokens, "stats", 4, false);
    assert_semantic_token(&tokens, "position", 4, false);
    assert_semantic_token(&tokens, "x", 4, false);
    assert_semantic_token(&tokens, "player", 3, true);
    assert_semantic_token(&tokens, "hp", 4, true);
    assert_semantic_token(&tokens, "number", 0, false);
    assert_semantic_token(&tokens, "missing", 2, false);
    assert_semantic_token(&tokens, "require", 1, false);
    assert_semantic_token(&tokens, "systems.player", 9, false);
    assert_semantic_token(&tokens, "heal", 5, true);
    assert_semantic_token(&tokens, "heal", 5, false);
    assert_semantic_token(&tokens, "@class", 12, false);
    assert_semantic_token(&tokens, "@field", 12, false);
    assert_semantic_token(&tokens, "@deprecated", 12, false);
    assert!(tokens
        .iter()
        .any(|token| token.text == "damage" && token.token_type == 1 && !token.declaration));
    assert!(tokens
        .iter()
        .any(|token| token.text == "require" && token.default_library));
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
        .any(|token| token.text == "@deprecated" && token.token_type == 12 && token.deprecated));
    assert!(tokens
        .iter()
        .any(|token| token.text == "hp" && token.token_type == 4 && token.modification));
}
#[test]
fn semantic_tokens_range_filters_to_requested_span() {
    let uri = "file:///tmp/semantic-token-range.tua";
    let source = r#"type Player = {
  hp: number,
}

local function makePlayer(): Player
  return { hp = 10 }
end

local function damage(player: Player, amount: number): Player
  player.hp -= amount
  player:heal()
  return player
end

local MAX_HP = 10
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        semantic_tokens_range_request(
            2,
            uri,
            lsp_position_of(source, "local function damage"),
            lsp_position_of(source, "local MAX_HP"),
        ),
    ]);
    let response = response_with_id(&output, 2);
    let tokens = decoded_semantic_tokens(source, response);

    assert_semantic_token(&tokens, "damage", 1, true);
    assert_semantic_token(&tokens, "player", 3, true);
    assert_semantic_token(&tokens, "hp", 4, false);
    assert!(!tokens.iter().any(|token| token.text == "makePlayer"));
    assert!(!tokens.iter().any(|token| token.text == "MAX_HP"));
}
#[test]
fn semantic_tokens_delta_returns_edits_matching_next_full_result() {
    let uri = "file:///tmp/semantic-token-delta.tua";
    let before = r#"type Player = { hp: number }

local player: Player = { hp = 10 }
player.hp = player.hp - 1
"#;
    let after = r#"type Player = { hp: number }

local player: Player = { hp = 10 }
local damage: number = 1
player.hp = player.hp - damage
"#;
    let mut server = TestServer::new();

    server.send(did_open(uri, before));
    server.send(semantic_tokens_request(2, uri));

    let first = &response_with_id(server.output(), 2)["result"];
    let first_result_id = semantic_tokens_result_id(first);
    let first_data = semantic_token_data(first);

    server.send(did_change(uri, after, 2));
    server.send(semantic_tokens_delta_request(3, uri, &first_result_id));
    server.send(semantic_tokens_request(4, uri));

    let delta = &response_with_id(server.output(), 3)["result"];
    let next_full = &response_with_id(server.output(), 4)["result"];
    let patched_data = apply_semantic_token_edits(first_data, semantic_token_edits(delta));

    assert_ne!(delta["resultId"], first_result_id);
    assert_eq!(patched_data, semantic_token_data(next_full));
}
#[test]
fn semantic_tokens_delta_falls_back_to_full_for_unknown_result_id() {
    let uri = "file:///tmp/semantic-token-delta-fallback.tua";
    let source = r#"type Player = { hp: number }

local player: Player = { hp = 10 }
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        semantic_tokens_delta_request(2, uri, "missing-result"),
    ]);
    let response = &response_with_id(&output, 2)["result"];

    assert!(response["resultId"].is_string());
    assert!(response["data"]
        .as_array()
        .is_some_and(|data| !data.is_empty()));
    assert!(response.get("edits").is_none());
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
    assert_semantic_token(&tokens, "number", 0, false);
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

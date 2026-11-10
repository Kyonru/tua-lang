use super::*;

#[test]
fn semantic_features_survive_incomplete_field_chain_statement() {
    let uri = "file:///tmp/recovered-love-conf.tua";
    let source = r#"function love.conf(t)
  local bad: number = "oops"
  local title = t.window.title
  local identity = t.identity -- Window itself can now be nil.
  t.modules.font
end
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        hover_request(2, uri, source, "bad: number"),
        completion_request(3, uri, source, "t.modules.font"),
        inlay_hint_request(4, uri, 0, 0, 6, 0),
    ]);
    let diagnostics = last_published_diagnostics(&output, uri);
    let hints = response_with_id(&output, 4)["result"]
        .as_array()
        .expect("inlay hints should be an array");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["code"] == "TL1003"),
        "{diagnostics:?}"
    );
    assert_hover_contains(&output, 2, "local bad: number");
    assert_completion_labels(&output, 3, ["font"]);
    assert_eq!(
        inlay_hint_labels(hints),
        vec![": string | nil", ": string | nil"]
    );
}
#[test]
fn semantic_features_use_last_good_snapshot_during_unrecoverable_edits() {
    let uri = "file:///tmp/last-good-semantics.tua";
    let valid = r#"type Player = {
  hp: number,
}

local player: Player = { hp = 100 }
local health = player.hp
"#;
    let invalid = format!("{valid}local unfinished = (\nlocal nextPlayer: Pl\n");
    let mut server = TestServer::new();

    server.send(did_open(uri, valid));
    server.send(did_change(uri, &invalid, 2));
    server.send(hover_request(3, uri, &invalid, "player: Player"));
    server.send(completion_request(4, uri, &invalid, "nextPlayer: Pl"));
    server.send(inlay_hint_request(5, uri, 0, 0, 8, 0));

    assert_hover_contains(server.output(), 3, "local player: Player");
    assert_completion_labels(server.output(), 4, ["Player", "SpritePlayer"]);

    let hints = response_with_id(server.output(), 5)["result"]
        .as_array()
        .expect("inlay hints should be an array");

    assert!(inlay_hint_labels(hints).contains(&": number"));
}

#[test]
fn incomplete_protocol_member_keeps_hover_and_completion_available() {
    let uri = "file:///tmp/incomplete-protocol-member.tua";
    let prefix = r#"local label = "ready"
local function draw(value: Display): nil
  value:
end
protocol Display {
  render(): string
"#;

    for partial_member in [
        "r",
        "render(",
        "render(value",
        "render(value:",
        "render(value: number",
        "render(value: {",
        "render(value: { text",
        "render(value: { text: string",
        "render(value: number):",
    ] {
        let source = format!("{prefix}  {partial_member}\n");
        let output = run_messages(vec![
            did_open(uri, &source),
            completion_request(2, uri, &source, "value:"),
            hover_request(3, uri, &source, "label ="),
        ]);
        let diagnostics = last_published_diagnostics(&output, uri);

        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic["code"] == "TL0001"
                    || diagnostic["message"]
                        .as_str()
                        .is_some_and(|message| message.contains("unclosed protocol `Display`"))
            }),
            "the original syntax error should remain visible while editor queries recover for `{partial_member}`: {diagnostics:#?}"
        );
        assert_completion_labels(&output, 2, ["render"]);
        assert_hover_contains(&output, 3, "local label: string");
    }
}

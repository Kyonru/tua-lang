use super::*;

#[test]
fn semantic_features_survive_incomplete_field_chain_statement() {
    let uri = "file:///tmp/recovered-love-conf.tua";
    let source = r#"function love.conf(t)
  local bad: number = "oops"
  local title = t.window.title
  t.modules.font
end
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        hover_request(2, uri, source, "bad: number"),
        completion_request(3, uri, source, "t.modules.font"),
        inlay_hint_request(4, uri, 0, 0, 5, 0),
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
    assert!(inlay_hint_labels(hints).contains(&": string"));
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
    assert_completion_labels(server.output(), 4, ["Player"]);

    let hints = response_with_id(server.output(), 5)["result"]
        .as_array()
        .expect("inlay hints should be an array");

    assert!(inlay_hint_labels(hints).contains(&": number"));
}

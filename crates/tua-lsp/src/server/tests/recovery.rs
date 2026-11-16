use super::*;

#[test]
fn unclosed_comment_does_not_expose_following_text_as_code() {
    let uri = "file:///tmp/tua-unclosed-comment-scope.tua";
    let source = "--[[ unfinished\nlocal invented = { secret = 99 }\nprint(invented.secret)\n";
    let output = run_messages(vec![
        did_open(uri, source),
        completion_request(2, uri, source, "invented."),
        definition_request(3, uri, source, "invented.secret", 0),
    ]);
    assert_completion_labels(&output, 2, []);
    assert!(response_with_id(&output, 3)["result"].is_null());
}

#[test]
fn incomplete_function_header_does_not_promote_body_local_to_top_level() {
    let uri = "file:///tmp/tua-incomplete-function-scope.tua";
    let source = "local function broken(\n  local leaked = { secret = 99 }\nlocal healthy = { hp = 1 }\nprint(leaked.secret)\nprint(healthy.hp)\n";
    let output = run_messages(vec![
        did_open(uri, source),
        completion_request(2, uri, source, "leaked."),
        definition_request(3, uri, source, "leaked.secret", 0),
    ]);
    assert_completion_labels(&output, 2, []);
    assert!(response_with_id(&output, 3)["result"].is_null());
}

#[test]
fn cold_malformed_line_keeps_later_completion_hover_and_signature() {
    let prefix = "type Actor = { hp: number, name: string }\nlocal actor: Actor = { hp = 10, name = \"Ada\" }\n";
    let suffix = "local laterActor: Actor = actor\nlocal read = laterActor.hp\nlocal function damage(target: Actor, amount: number): number return target.hp - amount end\nlocal result = damage(laterActor, 2)\n";
    for (name, middle) in [
        ("unclosed-call", "local broken = damage(actor,\n"),
        ("unclosed-table", "local broken = { hp =\n"),
        ("stray-closer", "local broken = )\n"),
        ("broken-string", "local broken = \"unterminated\n"),
        ("incomplete-params", "local function unfinished(value: \n"),
    ] {
        let uri = format!("file:///tmp/tua-cold-recovery-{name}.tua");
        let source = format!("{prefix}{middle}{suffix}");
        let output = run_messages(vec![
            did_open(&uri, &source),
            completion_request(2, &uri, &source, "laterActor."),
            hover_request(3, &uri, &source, "read ="),
            signature_help_request(4, &uri, &source, "damage(laterActor, "),
        ]);
        assert_completion_labels(&output, 2, ["hp", "name"]);
        assert_hover_contains(&output, 3, "read: number");
        let signature = &response_with_id(&output, 4)["result"]["signatures"][0]["label"];
        assert!(
            signature
                .as_str()
                .is_some_and(|label| label.contains("damage(target: Actor, amount: number)")),
            "{name}: {signature:?}"
        );
    }
}

#[test]
fn malformed_consumer_does_not_report_false_imported_self_mismatch() {
    let temp = TempProject::new("malformed-imported-self");
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/agent.tua", "class Agent {\n  health: number = 100\n  takeDamage(amount: number): nil { self.health -= amount }\n}\nexport type AgentInstance = Agent\nreturn { Agent = Agent }\n");
    temp.write("src/factory.tua", "import type { AgentInstance } from \"./agent\"\nlocal Agent = require(\"agent\").Agent\nlocal function spawn(): AgentInstance\n  return Agent.new()\nend\nreturn { spawn = spawn }\n");
    let valid =
        "local factory = require \"factory\"\nlocal agent = factory.spawn()\nagent:takeDamage(1)\n";
    let invalid = format!("local broken = (\n{valid}");
    temp.write("src/main.tua", &invalid);
    let uri = temp.uri_for("src/main.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, &invalid),
        text_document_diagnostic_request(2, &uri),
    ]);
    let malformed = response_with_id(&output, 2)["result"]["items"]
        .as_array()
        .unwrap();
    assert!(
        malformed.iter().any(|item| item["code"] == "TL0001"),
        "{malformed:?}"
    );
    assert!(
        !malformed.iter().any(|item| item["code"] == "TL1003"),
        "{malformed:?}"
    );
    let repaired = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, valid),
        text_document_diagnostic_request(3, &uri),
    ]);
    let repaired = response_with_id(&repaired, 3)["result"]["items"]
        .as_array()
        .unwrap();
    assert!(
        !repaired.iter().any(|item| item["code"] == "TL1003"),
        "{repaired:?}"
    );

    let wrong_argument = invalid.replace("takeDamage(1)", "takeDamage(\"bad\")");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, &wrong_argument),
        text_document_diagnostic_request(4, &uri),
    ]);
    let diagnostics = response_with_id(&output, 4)["result"]["items"]
        .as_array()
        .unwrap();
    assert!(
        diagnostics.iter().any(|item| {
            item["code"] == "TL1003" && !item["message"].as_str().unwrap_or("").contains("at self")
        }),
        "{diagnostics:?}"
    );
}

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
export -- { contract visibility }
protocol -- { behavior comment }
Display -- { contract body }
{
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

#[test]
fn missing_protocol_closer_does_not_hide_following_protocol_members() {
    let uri = "file:///tmp/missing-protocol-closer-before-next-declaration.tua";
    let source = r#"export protocol Updatable {
  update(dt: number): nil
  r
export -- public drawable contract
protocol -- name on the next line
Drawable -- body follows separately
{
  draw(): nil
}
local function render(value: Drawable): nil
  value:
end
"#;
    let output = run_messages(vec![
        did_open(uri, source),
        completion_request(2, uri, source, "  value:"),
    ]);
    let diagnostics = last_published_diagnostics(&output, uri);

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic["message"]
            .as_str()
            .is_some_and(|message| message.contains("unclosed protocol `Updatable`"))),
        "the incomplete first declaration must remain diagnosed: {diagnostics:#?}"
    );
    assert!(response_with_id(&output, 2)["result"]
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item["label"] == "draw")));
}

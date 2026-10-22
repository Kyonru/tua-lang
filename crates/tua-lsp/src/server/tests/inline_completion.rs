use super::*;

fn local_inline_request(id: u64, uri: &str, source: &str, needle: &str, version: i64) -> Value {
    let offset = source
        .rfind(needle)
        .map(|offset| offset + needle.len())
        .expect("needle should exist");
    let line_start = source[..offset].rfind('\n').map_or(0, |index| index + 1);
    let position = json!({
        "line": source[..offset].bytes().filter(|byte| *byte == b'\n').count(),
        "character": source[line_start..offset].encode_utf16().count()
    });
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tua/localInlineCompletion",
        "params": {
            "textDocument": {"uri": uri},
            "position": position,
            "documentVersion": version
        }
    })
}

fn standard_inline_request(id: u64, uri: &str, source: &str, needle: &str) -> Value {
    let mut request = local_inline_request(id, uri, source, needle, 0);
    request["method"] = json!("textDocument/inlineCompletion");
    request["params"]
        .as_object_mut()
        .expect("inline completion params should be an object")
        .remove("documentVersion");
    request["params"]["context"] = json!({"triggerKind": 1});
    request
}

fn enable_local_inline_completion(server: &mut TestServer) {
    server.send(did_change_configuration(json!({
        "completion": {
            "inline": {"enabled": true}
        }
    })));
}

#[test]
fn local_inline_completion_returns_current_file_recall() {
    let uri = "file:///tmp/local-inline.tua";
    let source = "draw(player, score)\ndraw(player, ";
    let mut server = TestServer::new();
    server.send(did_open(uri, source));
    enable_local_inline_completion(&mut server);
    server.send(local_inline_request(1, uri, source, "draw(player, ", 1));

    let result = &response_with_id(server.output(), 1)["result"];
    assert_eq!(result["documentVersion"], 1);
    assert_eq!(result["items"][0]["insertText"], "score");
    assert_eq!(result["items"][0]["source"], "currentFile");
    assert_eq!(result["items"][0]["isSnippet"], false);
}

#[test]
fn standard_inline_completion_reuses_the_production_query() {
    let uri = "file:///tmp/standard-inline.tua";
    let source = "draw(player, score)\ndraw(player, ";
    let mut server = TestServer::new();
    server.send(did_open(uri, source));
    enable_local_inline_completion(&mut server);
    server.send(standard_inline_request(1, uri, source, "draw(player, "));

    let result = &response_with_id(server.output(), 1)["result"];
    assert_eq!(result[0]["insertText"], "score");
    assert!(result[0].get("source").is_none());
    assert!(result[0].get("score").is_none());
    assert!(result[0].get("patternId").is_none());
}

#[test]
fn standard_inline_completion_is_empty_when_disabled() {
    let uri = "file:///tmp/standard-inline-disabled.tua";
    let source = "draw(player, score)\ndraw(player, ";
    let mut server = TestServer::new();
    server.send(did_open(uri, source));
    server.send(standard_inline_request(1, uri, source, "draw(player, "));

    assert_eq!(response_with_id(server.output(), 1)["result"], json!([]));
}

#[test]
fn standard_inline_completion_encodes_snippet_insert_text() {
    let uri = "file:///tmp/standard-inline-snippet.tua";
    let source = "whi";
    let mut server = TestServer::new();
    server.send(did_open(uri, source));
    enable_local_inline_completion(&mut server);
    server.send(standard_inline_request(1, uri, source, source));

    let insert_text = &response_with_id(server.output(), 1)["result"][0]["insertText"];
    assert_eq!(insert_text["kind"], "snippet");
    assert_eq!(insert_text["value"], "le ${1:condition} do\n  ${0}\nend");
}

#[test]
fn standard_inline_completion_honors_selected_completion_info() {
    let uri = "file:///tmp/standard-inline-selected.tua";
    let source = "print(player.name)\nprint(player.name)\nprint(pla";
    let mut server = TestServer::new();
    server.send(did_open(uri, source));
    enable_local_inline_completion(&mut server);
    let mut request = standard_inline_request(1, uri, source, "print(pla");
    request["params"]["context"]["selectedCompletionInfo"] = json!({
        "text": "player",
        "range": {
            "start": {"line": 2, "character": 6},
            "end": {"line": 2, "character": 9}
        }
    });
    server.send(request);

    let item = &response_with_id(server.output(), 1)["result"][0];
    assert_eq!(item["insertText"], "player.name)");
    assert_eq!(item["range"]["start"], json!({"line": 2, "character": 6}));
    assert_eq!(item["range"]["end"], json!({"line": 2, "character": 9}));
}

#[test]
fn local_inline_completion_reads_only_the_adopted_project_corpus() {
    let temp = TempProject::new("inline-project-corpus");
    let source = "local Pattern = require('one.pattern')\ndraw(player,";
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/game/main.tua", source);
    temp.write(
        "src/game/sibling.tua",
        "draw(player, wrong)\ndraw(player, wrong)\n",
    );
    temp.write(
        "src/one/pattern.tua",
        "draw(player,score)\ndraw(player,score)\n",
    );
    temp.write("src/two/pattern.tua", "draw(player,score)\n");
    let uri = temp.uri_for("src/game/main.tua");
    let output = run_chunked_messages(vec![
        (
            Duration::ZERO,
            vec![
                initialize_with_root(1, temp.uri()),
                did_open(&uri, source),
                did_change_configuration(json!({
                    "completion": {
                        "inline": {
                            "enabled": true,
                            "sources": {
                                "semantic": false,
                                "currentFile": false,
                                "project": true
                            }
                        }
                    }
                })),
            ],
        ),
        (
            Duration::from_millis(750),
            vec![local_inline_request(2, &uri, source, source, 1)],
        ),
    ]);
    let result = &response_with_id(&output, 2)["result"];

    assert_eq!(result["items"][0]["insertText"], "score)");
    assert_eq!(result["items"][0]["source"], "project");
}

#[test]
fn statistical_completion_uses_normal_warm_adoption_and_refreshes_after_edits() {
    let temp = TempProject::new("inline-statistical-warm");
    let initial = "local point = 0\npoint =1";
    let shifted = "-- café\nlocal target = 0\ntarget =1";
    let incomplete = "-- café\nlocal target = 0\ntarget =";
    let training = |name: &str| format!("local {name} = 0\n{}", format!("{name} = 1\n").repeat(40));
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", initial);
    temp.write("src/train-a.tua", &training("position"));
    temp.write("src/train-b.tua", &training("velocity"));
    let uri = temp.uri_for("src/main.tua");
    let output = run_chunked_messages(vec![
        (
            Duration::ZERO,
            vec![
                initialize_with_root(1, temp.uri()),
                did_open(&uri, initial),
                did_change_configuration(json!({
                    "completion": {
                        "inline": {
                            "enabled": true,
                            "sources": {
                                "semantic": false,
                                "currentFile": false,
                                "project": false,
                                "statistical": true
                            }
                        }
                    }
                })),
            ],
        ),
        (
            Duration::from_millis(750),
            vec![did_change(&uri, shifted, 2)],
        ),
        (
            Duration::from_millis(1500),
            vec![did_change(&uri, incomplete, 3)],
        ),
        (
            Duration::from_millis(2250),
            vec![local_inline_request(2, &uri, incomplete, incomplete, 3)],
        ),
    ]);
    let result = &response_with_id(&output, 2)["result"];

    assert_eq!(result["items"][0]["insertText"], " 1");
    assert_eq!(result["items"][0]["source"], "projectStatistical");
}

#[test]
fn local_inline_completion_is_empty_when_disabled_or_stale() {
    let uri = "file:///tmp/local-inline-stale.tua";
    let source = "draw(player, score)\ndraw(player, ";
    let mut server = TestServer::new();
    server.send(did_open(uri, source));
    server.send(local_inline_request(1, uri, source, "draw(player, ", 1));
    assert_eq!(
        response_with_id(server.output(), 1)["result"]["items"],
        json!([])
    );

    enable_local_inline_completion(&mut server);
    server.send(local_inline_request(2, uri, source, "draw(player, ", 0));
    assert_eq!(
        response_with_id(server.output(), 2)["result"]["items"],
        json!([])
    );
}

#[test]
fn selected_completion_uses_its_range_and_only_returns_an_extension() {
    let uri = "file:///tmp/local-inline-selected.tua";
    let source = "print(player.name)\nprint(player.name)\nprint(pla";
    let mut server = TestServer::new();
    server.send(did_open(uri, source));
    enable_local_inline_completion(&mut server);
    let mut request = local_inline_request(1, uri, source, "print(pla", 1);
    request["params"]["selectedCompletion"] = json!({
        "text": "player",
        "range": {
            "start": {"line": 2, "character": 6},
            "end": {"line": 2, "character": 9}
        }
    });
    server.send(request);

    let item = &response_with_id(server.output(), 1)["result"]["items"][0];
    assert_eq!(item["insertText"], "player.name)");
    assert_eq!(item["range"]["start"], json!({"line": 2, "character": 6}));
    assert_eq!(item["range"]["end"], json!({"line": 2, "character": 9}));
}

#[test]
fn selected_completion_rejects_a_range_that_ends_after_the_cursor() {
    let uri = "file:///tmp/local-inline-selected-wide.tua";
    let source = "print(player.name)\nprint(player.name)\nprint(pla)";
    let mut server = TestServer::new();
    server.send(did_open(uri, source));
    enable_local_inline_completion(&mut server);
    let mut request = local_inline_request(1, uri, source, "print(pla", 1);
    request["params"]["selectedCompletion"] = json!({
        "text": "player",
        "range": {
            "start": {"line": 2, "character": 6},
            "end": {"line": 2, "character": 10}
        }
    });
    server.send(request);

    assert_eq!(
        response_with_id(server.output(), 1)["result"]["items"],
        json!([])
    );
}

#[test]
fn selected_completion_uses_utf16_range_coordinates() {
    let uri = "file:///tmp/local-inline-selected-utf16.tua";
    let completed = "print(\"😀\", player.name)";
    let prefix = "print(\"😀\", ";
    let partial = format!("{prefix}pla");
    let source = format!("{completed}\n{completed}\n{partial}");
    let start_character = prefix.encode_utf16().count();
    let cursor_character = partial.encode_utf16().count();
    let mut server = TestServer::new();
    server.send(did_open(uri, &source));
    enable_local_inline_completion(&mut server);
    let mut request = local_inline_request(1, uri, &source, &partial, 1);
    request["params"]["selectedCompletion"] = json!({
        "text": "player",
        "range": {
            "start": {"line": 2, "character": start_character},
            "end": {"line": 2, "character": cursor_character}
        }
    });
    server.send(request);

    let item = &response_with_id(server.output(), 1)["result"]["items"][0];
    assert_eq!(item["insertText"], "player.name)");
    assert_eq!(
        item["range"],
        json!({
            "start": {"line": 2, "character": start_character},
            "end": {"line": 2, "character": cursor_character}
        })
    );
}

#[test]
fn local_inline_completion_preserves_utf16_cursor_ranges() {
    let uri = "file:///tmp/local-inline-utf16.tua";
    let prefix = "print(\"😀\", player, ";
    let source = format!("{prefix}score)\n{prefix}");
    let mut server = TestServer::new();
    server.send(did_open(uri, &source));
    enable_local_inline_completion(&mut server);
    server.send(local_inline_request(1, uri, &source, prefix, 1));

    let range = &response_with_id(server.output(), 1)["result"]["items"][0]["range"];
    assert_eq!(range["start"]["line"], 1);
    assert_eq!(range["start"]["character"], prefix.encode_utf16().count());
    assert_eq!(range["end"], range["start"]);
}

#[test]
fn cache_stats_and_health_report_inline_completion_work() {
    let uri = "file:///tmp/local-inline-observability.tua";
    let source = "print(player.name)\nprint(player.name)\nprint(pla";
    let mut server = TestServer::new();
    server.send(did_open(uri, source));
    enable_local_inline_completion(&mut server);
    server.send(local_inline_request(1, uri, source, "print(pla", 1));
    server.send(cache_stats_request(2));
    server.send(health_request(3));

    let stats = &response_with_id(server.output(), 2)["result"]["inlineCompletion"];
    assert_eq!(stats["requests"].as_u64(), Some(1));
    assert!(
        stats["cachePeek"]["hits"].as_u64().unwrap_or(0)
            + stats["cachePeek"]["misses"].as_u64().unwrap_or(0)
            >= 1
    );
    assert!(stats["candidateAttempts"].as_u64().unwrap_or(0) >= 1);
    assert!(stats["candidateParses"].as_u64().unwrap_or(0) >= 1);
    assert!(stats["candidateParseDistribution"]["one"].is_number());
    assert!(stats["candidateParseDistribution"]["two"].is_number());
    assert_eq!(stats["returnedParseDistribution"]["zero"].as_u64(), Some(0));
    assert_eq!(stats["returnedParseDistribution"]["one"].as_u64(), Some(1));
    assert_eq!(stats["returnedParseDistribution"]["two"].as_u64(), Some(0));
    assert!(stats["parsesPerRequest"].is_number());

    let health = &response_with_id(server.output(), 3)["result"];
    assert_eq!(health["inlineCompletion"]["requests"].as_u64(), Some(1));
    assert!(health["inlineCompletion"]["candidateParseDistribution"].is_object());
    assert!(health["inlineCompletion"]["returnedParseDistribution"].is_object());
    assert!(health["cache"]["inlineCurrentFile"]["hits"].is_number());
    assert!(health["cache"]["inlineCurrentFile"]["misses"].is_number());
}

use super::*;

#[test]
fn diagnostic_severities_round_trip_through_lsp() {
    for (severity, expected) in [
        (DiagnosticSeverity::Error, 1),
        (DiagnosticSeverity::Warning, 2),
        (DiagnosticSeverity::Info, 3),
        (DiagnosticSeverity::Hint, 4),
    ] {
        let diagnostic = Diagnostic::new("src/main.tua", 3, 5, severity, "sample");
        let lsp = protocol::lsp_diagnostic_from_core(diagnostic);

        assert_eq!(lsp["severity"], expected);

        let round_tripped = protocol::core_diagnostic_from_lsp(Path::new("src/main.tua"), &lsp)
            .expect("diagnostic should round-trip");

        assert_eq!(round_tripped.severity, severity);
    }
}
#[test]
fn did_open_publishes_matching_core_diagnostics() {
    let uri = "file:///tmp/player.tua";
    let output = run_messages(vec![
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "tua",
                    "version": 1,
                    "text": "local hp: number = \"full\"\n"
                }
            }
        }),
    ]);

    let diagnostics = published_diagnostics(&output, uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["severity"], 1);
    assert_eq!(diagnostics[0]["code"], "TL1003");
    assert_eq!(diagnostics[0]["source"], "tua");
    assert_eq!(diagnostics[0]["range"]["start"]["line"], 0);
    assert_eq!(diagnostics[0]["range"]["start"]["character"], 19);
    assert!(diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message should be a string")
        .contains("type mismatch: expected number, found string"));
}

#[test]
fn did_open_publishes_conflicting_signal_event_schemas() {
    let uri = "file:///tmp/signal-schema-diagnostics.tua";
    let source = r#"local events = signal()
events:declare("player.hit", function(player: string, damage: number) end)
events:on("player.hit", function(player: string, damage: string) end)
"#;
    let output = run_messages(vec![did_open(uri, source)]);
    let diagnostics = published_diagnostics(&output, uri)
        .into_iter()
        .filter(|diagnostic| diagnostic["code"] == "TL5005")
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().all(|diagnostic| diagnostic["message"]
        .as_str()
        .is_some_and(|message| message.contains("player.hit"))));
}

#[test]
fn did_open_publishes_love_method_argument_diagnostics() {
    let temp = TempProject::new("love-method-argument-diagnostics");
    let source = r#"
local player: Image = love.graphics.newImage("assets/player.png")

player:setFilter(123)
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/assets.tua", source);
    temp.write("assets/player.png", "");

    let uri = temp.uri_for("src/assets.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
    ]);
    let diagnostics = last_published_diagnostics(&output, &uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["severity"], 1);
    assert_eq!(diagnostics[0]["code"], "TL1003");
    assert!(diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message should be a string")
        .contains("type mismatch: expected FilterMode, found number"));
}
#[test]
fn did_open_publishes_love_enum_string_argument_diagnostics() {
    let uri = "file:///tmp/draw.tua";
    let source = r#"
function love.update(dt)
  love.graphics.circle("fillter", 100, 100, 2)
end
"#;
    let output = run_messages(vec![did_open(uri, source)]);
    let diagnostics = published_diagnostics(&output, uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["severity"], 1);
    assert_eq!(diagnostics[0]["code"], "TL1003");
    assert!(diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message should be a string")
        .contains(r#"type mismatch: expected DrawMode, found "fillter""#));
}
#[test]
fn did_open_publishes_love_conf_field_assignment_diagnostics() {
    let uri = "file:///tmp/conf.tua";
    let source = r#"
function love.conf(t)
  t.window.title = "Tua LOVE minimal"
  t.window.width = 320
  t.window.msaa = "12"
end
"#;
    let output = run_messages(vec![did_open(uri, source)]);
    let diagnostics = published_diagnostics(&output, uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["severity"], 1);
    assert_eq!(diagnostics[0]["code"], "TL1003");
    assert!(diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message should be a string")
        .contains("type mismatch: expected number, found string"));
}
#[test]
fn did_open_publishes_love_asset_path_diagnostics() {
    let temp = TempProject::new("missing-asset-diagnostics");
    let source = r#"
local player = image "assets/missing.png"
"#;

    temp.write("tua.toml", "source = \"src\"\nout = \"game\"\n");
    temp.write("src/assets.tua", source);

    let uri = temp.uri_for("src/assets.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
    ]);
    let diagnostics = last_published_diagnostics(&output, &uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["severity"], 1);
    assert_eq!(diagnostics[0]["code"], "TL2004");
    assert!(diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message should be a string")
        .contains("image asset not found: assets/missing.png"));
}
#[test]
fn did_open_publishes_impossible_comparison_warnings() {
    let uri = "file:///tmp/comparison.tua";
    let source = r#"local function recursive(a: string)
  if a == 1 then
    return 1
  end
end
"#;
    let output = run_messages(vec![did_open(uri, source)]);
    let diagnostics = last_published_diagnostics(&output, uri);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["severity"], 2);
    assert_eq!(diagnostics[0]["code"], "TL1005");
    assert!(diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message should be a string")
        .contains("comparison is always false: string and number do not overlap"));
}
#[test]
fn did_open_uses_project_diagnostics_config() {
    let temp = TempProject::new("diagnostics-config");

    temp.write(
        "tua.toml",
        r#"
source = "src"

[diagnostics]
duplicate_keys = true
undefined_variables = true
"#,
    );

    let uri = temp.uri_for("src/main.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, "local config = { hp = 10, hp = 20 }\nmissing()\n"),
    ]);
    let diagnostics = last_published_diagnostics(&output, &uri);

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic["severity"] == 2
            && diagnostic["code"] == "TL4001"
            && diagnostic["message"]
                .as_str()
                .expect("diagnostic message should be a string")
                .contains("duplicate table key `hp`")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic["severity"] == 2
            && diagnostic["code"] == "TL4004"
            && diagnostic["message"]
                .as_str()
                .expect("diagnostic message should be a string")
                .contains("undefined variable `missing`")
    }));
}
#[test]
fn did_change_republishes_empty_diagnostics_for_clean_document() {
    let uri = "file:///tmp/player.tua";
    let output = run_messages(vec![
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "tua",
                    "version": 1,
                    "text": "local hp: number = \"full\"\n"
                }
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "version": 2
                },
                "contentChanges": [
                    {
                        "text": "local hp: number = 10\n"
                    }
                ]
            }
        }),
    ]);

    let publications = output
        .iter()
        .filter(|message| message["method"] == "textDocument/publishDiagnostics")
        .collect::<Vec<_>>();

    assert_eq!(publications.len(), 2);
    assert_eq!(
        publications[1]["params"]["diagnostics"]
            .as_array()
            .expect("diagnostics should be an array")
            .len(),
        0
    );
}
#[test]
fn text_document_diagnostic_returns_same_codes_as_push_diagnostics() {
    let uri = "file:///tmp/pull-diagnostics.tua";
    let source = "local hp: number = \"full\"\n";
    let push_output = run_messages(vec![did_open(uri, source)]);
    let push_codes = diagnostic_codes(&last_published_diagnostics(&push_output, uri));
    let mut server = TestServer::new();

    server.send(initialize_with_pull_diagnostics(
        1,
        "file:///tmp".to_owned(),
        true,
    ));
    server.send(did_open(uri, source));

    assert!(
        diagnostics_publications(server.output(), uri).is_empty(),
        "pull-capable clients should not receive push diagnostics"
    );

    server.send(text_document_diagnostic_request(2, uri));

    assert_eq!(
        diagnostic_codes(&document_diagnostic_items(server.output(), 2)),
        push_codes
    );
}
#[test]
fn pull_diagnostics_use_fast_current_file_results_during_edit_window() {
    let temp = TempProject::new("pull-fast-edit-diagnostics");

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", "local ready = true\n");

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_pull_diagnostics(1, temp.uri(), true));
    server.send(did_open(&uri, "local ready = true\n"));
    server.send(did_change(
        &uri,
        "local sprite = image \"missing.png\"\n",
        2,
    ));
    server.send(text_document_diagnostic_request(2, &uri));

    assert!(
        diagnostics_publications(server.output(), &uri).is_empty(),
        "pull-capable clients should not receive push diagnostics"
    );
    assert!(
        document_diagnostic_items(server.output(), 2).is_empty(),
        "active typing should use fast diagnostics and skip slow asset validation"
    );

    let due = server
        .next_slow_diagnostics_due()
        .expect("pull diagnostics should queue an idle refresh");
    server.drain_due_diagnostics(due - Duration::from_millis(1));
    assert_eq!(diagnostic_refresh_requests(server.output()).len(), 0);

    server.drain_due_diagnostics(due);
    assert_eq!(diagnostic_refresh_requests(server.output()).len(), 1);

    server.send(text_document_diagnostic_request(3, &uri));

    assert_eq!(
        diagnostic_codes(&document_diagnostic_items(server.output(), 3)),
        vec!["TL2004"]
    );
}
#[test]
fn did_change_applies_incremental_edits_and_republishes_diagnostics() {
    let uri = "file:///tmp/incremental-change.tua";
    let source = "local hp: number = 10\n";
    let output = run_messages(vec![
        did_open(uri, source),
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "version": 2
                },
                "contentChanges": [
                    {
                        "range": {
                            "start": {
                                "line": 0,
                                "character": "local hp: number = ".len()
                            },
                            "end": {
                                "line": 0,
                                "character": "local hp: number = 10".len()
                            }
                        },
                        "rangeLength": 2,
                        "text": "\"full\""
                    }
                ]
            }
        }),
    ]);
    let publications = diagnostics_publications(&output, uri);
    let change_diagnostics = publications[1]["params"]["diagnostics"]
        .as_array()
        .expect("change diagnostics should be an array");

    assert_eq!(publications.len(), 2);
    assert_eq!(change_diagnostics.len(), 1);
    assert_eq!(change_diagnostics[0]["code"], "TL1003");
}
#[test]
fn did_change_publishes_fast_diagnostics_before_slow_diagnostics() {
    let temp = TempProject::new("fast-slow-diagnostics");

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", "local ready = true\n");

    let uri = temp.uri_for("src/main.tua");
    let edited = "local sprite = image \"missing.png\"\n";
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, "local ready = true\n"),
        did_change(&uri, edited, 2),
    ]);
    let publications = diagnostics_publications(&output, &uri);

    assert_eq!(publications.len(), 3);
    assert_eq!(
        publications[1]["params"]["diagnostics"]
            .as_array()
            .expect("fast diagnostics should be an array")
            .len(),
        0
    );
    assert_eq!(
        publications[2]["params"]["diagnostics"][0]["code"],
        "TL2004"
    );
}
#[test]
fn did_change_debounces_slow_diagnostics_until_idle_window() {
    let temp = TempProject::new("debounced-slow-diagnostics");

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", "local ready = true\n");

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, "local ready = true\n"));
    server.send_without_drain(did_change(
        &uri,
        "local sprite = image \"missing.png\"\n",
        2,
    ));

    let publications = diagnostics_publications(server.output(), &uri);
    assert_eq!(publications.len(), 2);
    assert_eq!(
        publications[1]["params"]["diagnostics"]
            .as_array()
            .expect("fast diagnostics should be an array")
            .len(),
        0
    );

    let due = server
        .next_slow_diagnostics_due()
        .expect("slow diagnostics should be queued");
    server.drain_due_diagnostics(due - Duration::from_millis(1));
    assert_eq!(diagnostics_publications(server.output(), &uri).len(), 2);

    server.drain_due_diagnostics(due);
    let publications = diagnostics_publications(server.output(), &uri);
    assert_eq!(publications.len(), 3);
    assert_eq!(
        publications[2]["params"]["diagnostics"][0]["code"],
        "TL2004"
    );
}
#[test]
fn did_change_cancels_stale_slow_diagnostics_for_newer_versions() {
    let temp = TempProject::new("cancel-stale-slow-diagnostics");

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", "local ready = true\n");

    let uri = temp.uri_for("src/main.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, "local ready = true\n"),
        did_change(&uri, "local sprite = image \"missing.png\"\n", 2),
        did_change(&uri, "local ready = true\n", 3),
    ]);
    let publications = diagnostics_publications(&output, &uri);
    let published_codes = publications
        .iter()
        .flat_map(|publication| {
            publication["params"]["diagnostics"]
                .as_array()
                .into_iter()
                .flat_map(|diagnostics| diagnostics.iter())
        })
        .filter_map(|diagnostic| diagnostic["code"].as_str())
        .collect::<Vec<_>>();

    assert_eq!(publications.len(), 3);
    assert!(
        !published_codes.contains(&"TL2004"),
        "stale missing-asset diagnostic should not publish: {published_codes:?}"
    );
}
#[test]
fn did_save_accepts_text_and_republishes_diagnostics() {
    let uri = "file:///tmp/save.tua";
    let output = run_messages(vec![
        did_open(uri, "local hp: number = 10\n"),
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didSave",
            "params": {
                "textDocument": {
                    "uri": uri
                },
                "text": "local hp: number = \"full\"\n"
            }
        }),
    ]);
    let publications = diagnostics_publications(&output, uri);
    let save_diagnostics = publications[1]["params"]["diagnostics"]
        .as_array()
        .expect("save diagnostics should be an array");

    assert_eq!(publications.len(), 2);
    assert_eq!(save_diagnostics.len(), 1);
    assert_eq!(save_diagnostics[0]["code"], "TL1003");
}
#[test]
fn did_close_removes_document_and_clears_diagnostics() {
    let uri = "file:///tmp/close.tua";
    let output = run_messages(vec![
        did_open(uri, "local hp: number = \"full\"\n"),
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didClose",
            "params": {
                "textDocument": {
                    "uri": uri
                }
            }
        }),
        hover_request(2, uri, "local hp: number = \"full\"\n", "hp"),
    ]);
    let publications = diagnostics_publications(&output, uri);

    assert_eq!(publications.len(), 2);
    assert_eq!(
        publications[1]["params"]["diagnostics"]
            .as_array()
            .expect("close diagnostics should be an array")
            .len(),
        0
    );
    assert!(response_with_id(&output, 2)["result"].is_null());
}

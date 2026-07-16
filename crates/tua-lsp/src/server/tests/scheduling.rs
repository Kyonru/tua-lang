use super::*;

#[test]
fn input_backlog_prioritizes_typing_and_completion_messages() {
    let uri = "file:///tmp/backlog-priority.tua";
    let before = "local outside_function = 1\nprint()\n";
    let after = "local outside_function = 1\nprint(o)\n";
    let mut messages = std::collections::VecDeque::from([
        document_symbol_request(2, uri),
        code_lens_request(3, uri),
        hover_request(6, uri, before, "outside_function"),
        document_highlight_request(7, uri, before, "outside_function", 0),
        did_change(uri, after, 2),
        invoked_completion_request(4, uri, after, "print(o"),
        folding_range_request(5, uri),
    ]);

    let first = pop_next_pending_message(&mut messages).expect("first message");
    let second = pop_next_pending_message(&mut messages).expect("second message");
    let third = pop_next_pending_message(&mut messages).expect("third message");

    assert_eq!(first["method"], "textDocument/didChange");
    assert_eq!(second["method"], "textDocument/completion");
    assert_eq!(third["method"], "textDocument/hover");
    assert_eq!(
        first["params"]["contentChanges"][0]["text"], after,
        "the prioritized didChange should carry the latest edited text"
    );
    assert_ne!(
        before, after,
        "the fixture should model a real edit instead of identical text"
    );
}
#[test]
fn shutdown_does_not_overtake_an_earlier_interactive_request() {
    let uri = "file:///tmp/shutdown-order.tua";
    let source = "local hp: number = 10\n";
    let mut messages = std::collections::VecDeque::from([
        hover_request(3, uri, source, "hp"),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "shutdown" }),
        json!({ "jsonrpc": "2.0", "method": "exit" }),
    ]);

    let first = pop_next_pending_message(&mut messages).expect("first message");
    let second = pop_next_pending_message(&mut messages).expect("second message");

    assert_eq!(first["method"], "textDocument/hover");
    assert_eq!(second["method"], "shutdown");
}
#[test]
fn typing_and_completion_preempt_a_running_background_request() {
    let uri = "file:///tmp/preemptible-completion.tua";
    let before = "local outside_function = 1\n";
    let after = "local outside_function = 1\no";
    let output = run_chunked_messages(vec![
        (
            Duration::ZERO,
            vec![
                json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
                did_open(uri, before),
                slow_background_request(2, None, 300),
            ],
        ),
        (
            Duration::from_millis(50),
            vec![
                did_change(uri, after, 2),
                invoked_completion_request(3, uri, after, "o"),
            ],
        ),
    ]);
    let completion_index = response_index_with_id(&output, 3);
    let slow_index = response_index_with_id(&output, 2);

    assert!(
        completion_index < slow_index,
        "completion should finish before an already-running background request: {output:#?}"
    );
    assert!(response_with_id(&output, 3)["result"]
        .as_array()
        .is_some_and(|items| !items.is_empty()));
}
#[test]
fn did_change_discards_a_stale_background_result() {
    let uri = "file:///tmp/stale-background.tua";
    let before = "local value = 1\n";
    let after = "local value = 2\n";
    let output = run_chunked_messages(vec![
        (
            Duration::ZERO,
            vec![
                json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
                did_open(uri, before),
                slow_background_request(2, Some(uri), 250),
            ],
        ),
        (Duration::from_millis(40), vec![did_change(uri, after, 2)]),
    ]);

    assert_eq!(response_with_id(&output, 2)["error"]["code"], -32801);
    assert!(!output
        .iter()
        .any(|message| message["id"] == 2 && message.get("result").is_some()));
}
#[test]
fn cancel_request_cancels_a_running_analysis_request() {
    let output = run_chunked_messages(vec![
        (
            Duration::ZERO,
            vec![
                json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
                slow_background_request(2, None, 250),
            ],
        ),
        (
            Duration::from_millis(40),
            vec![json!({
                "jsonrpc": "2.0",
                "method": "$/cancelRequest",
                "params": { "id": 2 }
            })],
        ),
    ]);

    assert_eq!(response_with_id(&output, 2)["error"]["code"], -32800);
    assert!(!output
        .iter()
        .any(|message| message["id"] == 2 && message.get("result").is_some()));
}
#[test]
fn duplicate_background_requests_share_one_worker_job() {
    let uri = "file:///tmp/coalesced-background.tua";
    let output = run_messages(vec![
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
        did_open(uri, "local value = 1\n"),
        slow_background_request(2, Some(uri), 40),
        slow_background_request(3, Some(uri), 40),
    ]);

    let first = &response_with_id(&output, 2)["result"];
    let second = &response_with_id(&output, 3)["result"];

    assert_eq!(first["slow"], true);
    assert_eq!(
        first, second,
        "coalesced requests should share one execution"
    );
}
#[test]
fn a_new_duplicate_does_not_join_a_cancelled_worker_job() {
    let uri = "file:///tmp/retry-cancelled-background.tua";
    let output = run_chunked_messages(vec![
        (
            Duration::ZERO,
            vec![
                json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
                did_open(uri, "local value = 1\n"),
                slow_background_request(2, Some(uri), 100),
            ],
        ),
        (
            Duration::from_millis(20),
            vec![
                json!({
                    "jsonrpc": "2.0",
                    "method": "$/cancelRequest",
                    "params": { "id": 2 }
                }),
                slow_background_request(3, Some(uri), 100),
            ],
        ),
    ]);

    assert_eq!(response_with_id(&output, 2)["error"]["code"], -32800);
    assert_eq!(response_with_id(&output, 3)["result"]["slow"], true);
}
#[test]
fn active_typing_backlog_uses_fast_background_responses() {
    let mut server = TestServer::new();
    let uri = "file:///tmp/background-fast-response.tua";
    let before = "local value = 1\nprint(value)\n";
    let after = "local value = 1\nprint(value)\nlocal next = value\n";

    server.send(initialize_with_trace_options(
        1,
        json!({
            "trace": {
                "server": "messages"
            },
            "logVerbosity": "debug"
        }),
    ));
    server.send(did_open(uri, before));
    server.send(did_change(uri, after, 2));

    server.server.input_backlog_active = true;
    server.send_without_drain(document_symbol_request(3, uri));
    server.server.input_backlog_active = false;

    assert_eq!(response_with_id(server.output(), 3)["result"], json!([]));
    assert!(server.output().iter().any(|message| {
        message["method"] == "window/logMessage"
            && message["params"]["message"]
                .as_str()
                .is_some_and(|message| {
                    message.contains("tua background request fast response during input backlog")
                        && message.contains("method=textDocument/documentSymbol")
                        && message.contains("sinceChangeMs=")
                })
    }));
}
#[test]
fn active_typing_backlog_keeps_inlay_hint_responses() {
    let mut server = TestServer::new();
    let uri = "file:///tmp/background-inlay-hints.tua";
    let before = r#"type Player = { hp: number }

local function makePlayer(): Player
  return { hp = 10 }
end
"#;
    let after = r#"type Player = { hp: number }

local function makePlayer(): Player
  return { hp = 10 }
end

local player = makePlayer()
"#;

    server.send(initialize_with_trace_options(
        1,
        json!({
            "trace": {
                "server": "messages"
            },
            "logVerbosity": "debug"
        }),
    ));
    server.send(did_open(uri, before));
    server.send(did_change(uri, after, 2));

    server.server.input_backlog_active = true;
    server.send_without_drain(inlay_hint_request(3, uri, 0, 0, 10, 0));
    server.server.input_backlog_active = false;

    let hints = response_with_id(server.output(), 3)["result"]
        .as_array()
        .expect("inlay hints should be an array");

    assert!(
        inlay_hint_labels(hints).contains(&": Player"),
        "inlay hints should not be replaced by the active-edit empty response: {hints:#?}"
    );
    assert!(!server.output().iter().any(|message| {
        message["method"] == "window/logMessage"
            && message["params"]["message"]
                .as_str()
                .is_some_and(|message| {
                    message.contains("tua background request fast response during input backlog")
                        && message.contains("method=textDocument/inlayHint")
                })
    }));
}
#[test]
fn current_file_requests_do_not_force_project_snapshot() {
    let main_uri = "file:///tmp/current-file-main.tua";
    let other_uri = "file:///tmp/current-file-other.tua";
    let main = r#"type Player = { hp: number }
local function spawn()
end
"#;
    let other = "local other: number = 2\n";
    let mut server = TestServer::new();

    server.send(did_open(main_uri, main));
    server.send(did_open(other_uri, other));
    server.send(cache_stats_request(2));
    server.send(document_symbol_request(3, main_uri));
    server.send(semantic_tokens_request(4, main_uri));
    server.send(cache_stats_request(5));

    let before = &response_with_id(server.output(), 2)["result"];
    let after = &response_with_id(server.output(), 5)["result"];

    assert_eq!(
        after["parse"]["misses"].as_u64(),
        before["parse"]["misses"].as_u64()
    );
    assert_eq!(
        after["hir"]["misses"].as_u64(),
        before["hir"]["misses"].as_u64()
    );
    assert_eq!(
        after["parse"]["hits"].as_u64(),
        before["parse"]["hits"].as_u64().map(|hits| hits + 2)
    );
    assert_eq!(
        after["hir"]["hits"].as_u64(),
        before["hir"]["hits"].as_u64().map(|hits| hits + 2)
    );
}
#[test]
fn did_open_queues_slow_diagnostics_without_forcing_project_snapshot() {
    let main_uri = "file:///tmp/did-open-fast-main.tua";
    let other_uri = "file:///tmp/did-open-fast-other.tua";
    let mut server = TestServer::new();

    server.send_without_drain(did_open(main_uri, "local main: number = 1\n"));
    server.send_without_drain(did_open(other_uri, "local other: number = 2\n"));
    server.send_without_drain(cache_stats_request(2));

    let before = &response_with_id(server.output(), 2)["result"];

    assert_eq!(before["parse"]["misses"].as_u64(), Some(0));
    assert!(server.next_slow_diagnostics_due().is_some());

    server.drain_due_diagnostics(
        server
            .next_slow_diagnostics_due()
            .expect("slow diagnostics should be queued"),
    );
    server.send_without_drain(cache_stats_request(3));

    let after = &response_with_id(server.output(), 3)["result"];

    assert_eq!(after["parse"]["misses"].as_u64(), Some(2));
}

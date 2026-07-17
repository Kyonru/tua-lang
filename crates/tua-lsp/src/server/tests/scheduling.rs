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
fn asset_inventory_yields_to_typing_and_completion() {
    let uri = "file:///tmp/asset-inventory-priority.tua";
    let after = "local player = image \"assets/player.png\"\np";
    let mut messages = std::collections::VecDeque::from([
        asset_inventory_request(2),
        did_change(uri, after, 2),
        invoked_completion_request(3, uri, after, "p"),
    ]);

    let first = pop_next_pending_message(&mut messages).expect("first message");
    let second = pop_next_pending_message(&mut messages).expect("second message");
    let third = pop_next_pending_message(&mut messages).expect("third message");

    assert_eq!(first["method"], "textDocument/didChange");
    assert_eq!(second["method"], "textDocument/completion");
    assert_eq!(third["method"], "tua/assetInventory");
}

#[test]
fn module_graph_yields_to_typing_and_completion() {
    let uri = "file:///tmp/module-graph-priority.tua";
    let after = "local Player = require(\"player\")\nP";
    let mut messages = std::collections::VecDeque::from([
        module_graph_request(2),
        did_change(uri, after, 2),
        invoked_completion_request(3, uri, after, "P"),
    ]);

    let first = pop_next_pending_message(&mut messages).expect("first message");
    let second = pop_next_pending_message(&mut messages).expect("second message");
    let third = pop_next_pending_message(&mut messages).expect("third message");

    assert_eq!(first["method"], "textDocument/didChange");
    assert_eq!(second["method"], "textDocument/completion");
    assert_eq!(third["method"], "tua/moduleGraph");
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
fn editor_regression_fresh_inlay_hints_preempt_background_after_typing() {
    let uri = "file:///tmp/preemptible-inlay-hints.tua";
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
                inlay_hint_request(3, uri, 0, 0, 10, 0),
            ],
        ),
    ]);
    let inlay_index = response_index_with_id(&output, 3);
    let slow_index = response_index_with_id(&output, 2);
    let hints = response_with_id(&output, 3)["result"]
        .as_array()
        .expect("inlay hints should be an array");

    assert!(
        inlay_index < slow_index,
        "fresh inlay hints should not wait for background refreshes: {output:#?}"
    );
    assert!(
        inlay_hint_labels(hints).contains(&": Player"),
        "inlay hints should describe the latest document version: {hints:#?}"
    );
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

#[test]
fn workspace_warm_queue_prioritizes_an_open_file() {
    let temp = TempProject::new("warm-open-priority");
    let main_source = "local main: number = 1\n";

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", main_source);
    temp.write("src/other.tua", "local other: number = 2\n");

    let main_uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&main_uri, main_source));

    let work = server
        .server
        .take_next_workspace_warm_work()
        .expect("open file should be queued for warming");

    match work {
        WorkspaceWarmWork::File { path, priority, .. } => {
            assert_eq!(path, temp.path.join("src/main.tua"));
            assert_eq!(priority, tua_core::AnalysisWarmPriority::High);
        }
        WorkspaceWarmWork::Snapshot { .. } => panic!("files must warm before the snapshot"),
    }
}

#[test]
fn idle_workspace_warm_adopts_files_and_prepares_the_project_snapshot() {
    let temp = TempProject::new("idle-workspace-warm");

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", "local main: number = 1\n");
    temp.write("src/other.lua", "local other = 2\nreturn other\n");

    let mut initialize = initialize_with_root(1, temp.uri());
    initialize["params"]["capabilities"] = json!({
        "workspace": {
            "inlayHint": { "refreshSupport": true },
            "semanticTokens": { "refreshSupport": true }
        }
    });
    let output = run_chunked_messages(vec![
        (Duration::ZERO, vec![initialize]),
        (Duration::from_millis(300), vec![cache_stats_request(100)]),
    ]);
    let warm = &response_with_id(&output, 100)["result"]["workspaceWarm"];

    assert_eq!(warm["discovered"].as_u64(), Some(2));
    assert_eq!(warm["warmed"].as_u64(), Some(2));
    assert_eq!(warm["queuedHigh"].as_u64(), Some(0));
    assert_eq!(warm["queuedLow"].as_u64(), Some(0));
    assert!(warm["snapshotReadyRevision"].is_number());
    assert!(output
        .iter()
        .any(|message| message["method"] == "workspace/inlayHint/refresh"));
    assert!(output
        .iter()
        .any(|message| message["method"] == "workspace/semanticTokens/refresh"));
}

#[test]
fn edit_rejects_and_requeues_a_stale_workspace_warm_result() {
    let uri = "file:///tmp/stale-workspace-warm.tua";
    let before = "local value = 1\n";
    let after = "local value = 2\n";
    let mut server = TestServer::new();

    server.send_without_drain(did_open(uri, before));
    let work = server
        .server
        .take_next_workspace_warm_work()
        .expect("opened file should be queued");
    let warmed = server.server.analysis.clone();
    let WorkspaceWarmWork::File { path, priority, .. } = &work else {
        panic!("opened file should warm before the snapshot");
    };

    assert!(warmed.warm_file(path, *priority));
    server.send_without_drain(did_change(uri, after, 2));
    server
        .server
        .finish_workspace_warm(&work, Some(&warmed), true);

    assert_eq!(server.server.workspace_warm.stale_results, 1);
    let retried = server
        .server
        .take_next_workspace_warm_work()
        .expect("stale file should be requeued");
    assert!(matches!(
        retried,
        WorkspaceWarmWork::File {
            priority: tua_core::AnalysisWarmPriority::High,
            ..
        }
    ));
}

#[test]
fn shutdown_abandons_internal_workspace_warm_jobs() {
    let uri = "file:///tmp/shutdown-workspace-warm.tua";
    let mut server = LspServer::default();
    let (event_tx, _event_rx) = std::sync::mpsc::channel();
    let mut scheduler = RequestScheduler::new(event_tx, SharedReceivedInputState::default());

    server.handle_message(did_open(uri, "local value = 1\n"));

    assert!(scheduler.dispatch_next_workspace_warm(&mut server));
    assert!(scheduler.has_in_flight());

    scheduler.abandon_workspace_warm();

    assert!(!scheduler.has_in_flight());
}

#[test]
fn received_document_version_rejects_a_result_before_did_change_is_processed() {
    let uri = "file:///tmp/transport-version-watermark.tua";
    let mut server = LspServer::default();
    let received_input = SharedReceivedInputState::default();
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let mut scheduler = RequestScheduler::new(event_tx, std::sync::Arc::clone(&received_input));

    server.handle_message(did_open(uri, "local value = 1\n"));
    received_input
        .document_versions
        .lock()
        .expect("received document version lock")
        .insert(uri.to_owned(), Some(1));
    scheduler.dispatch(
        &mut server,
        slow_background_request(2, Some(uri), 10),
        Some(1),
    );

    received_input
        .document_versions
        .lock()
        .expect("received document version lock")
        .insert(uri.to_owned(), Some(2));
    let result = loop {
        match event_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("worker result")
        {
            ServerEvent::RequestFinished(result) => break result,
            ServerEvent::Input(_) => continue,
        }
    };
    let output = scheduler.finish(&mut server, *result);

    assert_eq!(response_with_id(&output, 2)["error"]["code"], -32801);
    assert!(!output
        .iter()
        .any(|message| message["id"] == 2 && message.get("result").is_some()));
}

#[test]
fn did_change_immediately_cancels_stale_foreground_inlay_work() {
    let uri = "file:///tmp/cancel-stale-inlay.tua";
    let mut server = LspServer::default();
    let received_input = SharedReceivedInputState::default();
    let (event_tx, _event_rx) = std::sync::mpsc::channel();
    let mut scheduler = RequestScheduler::new(event_tx, received_input);

    server.handle_message(did_open(uri, "local value = 1\n"));
    scheduler.dispatch(&mut server, inlay_hint_request(2, uri, 0, 0, 1, 0), Some(1));
    server.handle_message(did_change(uri, "local value = 2\n", 2));

    let output = scheduler.cancel_stale_document_requests(
        uri,
        server.document_generations.get(uri).copied(),
        server.document_versions.get(uri).copied(),
    );

    assert_eq!(response_with_id(&output, 2)["error"]["code"], -32801);
}

#[test]
fn reader_preserves_the_document_version_seen_by_each_request() {
    let uri = "file:///tmp/receive-time-version.tua";
    let input = ReceivedInputState::default();
    let old_request = inlay_hint_request(2, uri, 0, 0, 1, 0);
    let new_request = inlay_hint_request(3, uri, 0, 0, 1, 0);

    record_received_input(&did_open(uri, "local value = 1\n"), &input);
    record_received_input(&old_request, &input);
    record_received_input(&did_change(uri, "local value = 2\n", 2), &input);
    record_received_input(&new_request, &input);

    assert_eq!(
        take_received_request_version(
            &json!({ "jsonrpc": "2.0", "id": 2, "result": null }),
            &input,
        ),
        None,
        "a client response in the opposite JSON-RPC direction must not consume the watermark"
    );
    assert_eq!(take_received_request_version(&old_request, &input), Some(1));
    assert_eq!(take_received_request_version(&new_request, &input), Some(2));
}

use super::*;

fn enable_scheduled_local_inline_completion(server: &mut LspServer) {
    let _ = server.handle_message(did_change_configuration(json!({
        "completion": {"inline": {"enabled": true}}
    })));
}

fn enable_project_pattern_corpus(server: &mut LspServer) {
    let _ = server.handle_message(did_change_configuration(json!({
        "completion": {
            "inline": {
                "enabled": true,
                "sources": {"project": true}
            }
        }
    })));
}

fn finish_pattern_work(server: &mut LspServer, work: PatternCorpusWork) {
    let started = Instant::now();
    let PatternCorpusWork {
        generation,
        paths,
        operation,
    } = work;
    let PatternCorpusOperation::Files {
        inputs,
        project_model,
        retrieval_limits,
    } = operation
    else {
        panic!("test helper expected file build work");
    };
    let entries = inputs
        .into_iter()
        .map(|input| {
            tua_core::PatternCorpusBatchEntry::File(input.build(&project_model, retrieval_limits))
        })
        .collect();
    server.finish_pattern_corpus(scheduler::PatternCorpusWorkerResult {
        generation,
        paths,
        entries,
        remerge: None,
        unfinished_paths: Vec::new(),
        cancelled: false,
        elapsed_ms: started.elapsed().as_millis() as u64,
        disposed: false,
    });
}

fn finish_remerge_work(server: &mut LspServer, work: PatternCorpusWork) {
    let PatternCorpusWork {
        generation,
        paths,
        operation,
    } = work;
    let PatternCorpusOperation::Remerge(prepared) = operation else {
        panic!("test helper expected remerge work");
    };
    server.finish_pattern_corpus(scheduler::PatternCorpusWorkerResult {
        generation,
        paths,
        entries: Vec::new(),
        remerge: Some(prepared.build()),
        unfinished_paths: Vec::new(),
        cancelled: false,
        elapsed_ms: 0,
        disposed: false,
    });
}

fn wait_for_speculative_results(
    scheduler: &RequestScheduler,
    expected: usize,
) -> Vec<scheduler::WorkerResult> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut results = Vec::new();
    loop {
        results.extend(scheduler.take_speculative_results());
        if results.len() >= expected {
            return results;
        }
        assert!(
            Instant::now() < deadline,
            "speculative worker should return before the test deadline"
        );
        std::thread::yield_now();
    }
}

#[test]
fn scheduler_rejects_requests_beyond_the_in_flight_job_limit() {
    let mut server = LspServer::default();
    let (event_tx, _event_rx) = std::sync::mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
    let mut scheduler = RequestScheduler::new(event_tx, SharedReceivedInputState::default());

    for id in 0..crate::server::scheduler::MAX_IN_FLIGHT_JOBS {
        let output = scheduler.dispatch(
            &mut server,
            json!({"jsonrpc":"2.0", "id":id, "method":"tua/unknown", "params":{}}),
            None,
        );
        assert!(output.iter().all(|message| message.get("error").is_none()));
    }

    let output = scheduler.dispatch(
        &mut server,
        json!({"jsonrpc":"2.0", "id":crate::server::scheduler::MAX_IN_FLIGHT_JOBS, "method":"tua/unknown", "params":{}}),
        None,
    );
    assert_eq!(
        response_with_id(&output, crate::server::scheduler::MAX_IN_FLIGHT_JOBS as u64,)["error"]
            ["code"],
        -32000
    );
}

#[test]
fn slow_diagnostics_reserve_capacity_for_client_requests() {
    let (event_tx, _event_rx) = std::sync::mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
    let mut scheduler = RequestScheduler::new(event_tx, SharedReceivedInputState::default());
    let mut server = LspServer::default();

    for index in 0..=crate::server::scheduler::MAX_IN_FLIGHT_JOBS {
        let uri = format!("file:///tmp/diagnostic-capacity-{index}.tua");
        let _ = server.handle_message(did_open(&uri, "local value = 1\n"));
    }
    let due = server
        .next_slow_diagnostics_due()
        .expect("slow diagnostics should be queued")
        + Duration::from_secs(1);

    for _ in 0..(crate::server::scheduler::MAX_IN_FLIGHT_JOBS / 2) {
        assert!(scheduler.dispatch_due_slow_diagnostic(&mut server, due));
    }
    assert!(!scheduler.dispatch_due_slow_diagnostic(&mut server, due));

    let output = scheduler.dispatch(
        &mut server,
        document_link_request(1, "file:///tmp/diagnostic-capacity-0.tua"),
        None,
    );
    assert!(output.iter().all(|message| message.get("error").is_none()));
}

#[test]
fn slow_diagnostics_do_not_consume_the_last_client_request_slot() {
    let (event_tx, _event_rx) = std::sync::mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
    let mut scheduler = RequestScheduler::new(event_tx, SharedReceivedInputState::default());
    let mut server = LspServer::default();
    let uri = "file:///tmp/diagnostic-follow-up-capacity.tua";
    let _ = server.handle_message(did_open(uri, "local value = 1\n"));
    let due = server
        .next_slow_diagnostics_due()
        .expect("slow diagnostics should be queued")
        + Duration::from_secs(1);

    for id in 0..(crate::server::scheduler::MAX_IN_FLIGHT_JOBS - 1) {
        let output = scheduler.dispatch(
            &mut server,
            json!({"jsonrpc":"2.0", "id":id, "method":"tua/unknown", "params":{}}),
            None,
        );
        assert!(output.iter().all(|message| message.get("error").is_none()));
    }

    assert!(
        !scheduler.dispatch_due_slow_diagnostic(&mut server, due),
        "internal diagnostics must leave capacity for a follow-up editor request"
    );
    let output = scheduler.dispatch(
        &mut server,
        json!({
            "jsonrpc":"2.0",
            "id":crate::server::scheduler::MAX_IN_FLIGHT_JOBS,
            "method":"tua/unknown",
            "params":{}
        }),
        None,
    );
    assert!(output.iter().all(|message| message.get("error").is_none()));
}

#[test]
fn non_interactive_bursts_reserve_capacity_for_interactive_requests() {
    let (event_tx, _event_rx) = std::sync::mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
    let mut scheduler = RequestScheduler::new(event_tx, SharedReceivedInputState::default());
    let mut server = LspServer::default();
    let source = "local profile_primary = { label = \"ready\" }\nprofile_primary.la\n";
    let uri = "file:///tmp/background-capacity-completion.tua";
    let _ = server.handle_message(did_open(uri, source));

    for id in 0..(crate::server::scheduler::MAX_IN_FLIGHT_JOBS * 3 / 4) {
        let output = scheduler.dispatch(
            &mut server,
            inlay_hint_request(id as u64, uri, id, 0, id + 1, 0),
            None,
        );
        assert!(output.iter().all(|message| message.get("error").is_none()));
    }

    let rejected_id = crate::server::scheduler::MAX_IN_FLIGHT_JOBS as u64;
    let output = scheduler.dispatch(
        &mut server,
        inlay_hint_request(rejected_id, uri, rejected_id as usize, 0, 100, 0),
        None,
    );
    assert_eq!(
        response_with_id(&output, rejected_id)["error"]["code"],
        -32000
    );

    let output = scheduler.dispatch(
        &mut server,
        invoked_completion_request(rejected_id + 1, uri, source, "profile_primary.la"),
        None,
    );
    assert!(output.iter().all(|message| message.get("error").is_none()));
}

#[test]
fn newer_local_inline_request_cancels_the_older_request_for_the_same_document() {
    let uri = "file:///tmp/newest-inline.tua";
    let mut server = LspServer::default();
    let _ = server.handle_message(did_open(uri, "local value = 1\n"));
    enable_scheduled_local_inline_completion(&mut server);
    let (event_tx, _event_rx) = std::sync::mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
    let mut scheduler = RequestScheduler::new(event_tx, SharedReceivedInputState::default());
    let request = |id, delay_ms| {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tua/localInlineCompletion",
            "params": {
                "textDocument": {"uri": uri},
                "position": {"line": 0, "character": 15},
                "documentVersion": 1,
                "testDelayMs": delay_ms
            }
        })
    };

    assert!(scheduler
        .dispatch(&mut server, request(1, 100), Some(1))
        .iter()
        .all(|message| message.get("error").is_none()));
    let deadline = Instant::now() + Duration::from_millis(100);
    while scheduler.speculative_pending_count() != 0 {
        assert!(Instant::now() < deadline, "the first request should start");
        std::thread::yield_now();
    }
    let output = scheduler.dispatch(&mut server, request(2, 0), Some(1));

    assert_eq!(response_with_id(&output, 1)["error"]["code"], -32800);
    assert_eq!(scheduler.in_flight_count(), 1);

    let mut results = wait_for_speculative_results(&scheduler, 2).into_iter();
    let old = results.next().expect("superseded physical result");
    assert!(
        scheduler.finish(&mut server, old).is_empty(),
        "the superseded worker result must stay suppressed"
    );
    let latest = results.next().expect("latest physical result");
    let output = scheduler.finish(&mut server, latest);
    assert!(response_with_id(&output, 2).get("error").is_none());
    assert_eq!(scheduler.in_flight_count(), 0);
}

#[test]
fn standard_and_custom_inline_requests_share_newest_wins_cancellation() {
    let uri = "file:///tmp/newest-inline-protocols.tua";
    let mut server = LspServer::default();
    let _ = server.handle_message(did_open(uri, "local value = 1\n"));
    enable_scheduled_local_inline_completion(&mut server);
    let (event_tx, _event_rx) = std::sync::mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
    let mut scheduler = RequestScheduler::new(event_tx, SharedReceivedInputState::default());
    let custom = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tua/localInlineCompletion",
        "params": {
            "textDocument": {"uri": uri},
            "position": {"line": 0, "character": 15},
            "documentVersion": 1,
            "testDelayMs": 100
        }
    });
    let standard = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "textDocument/inlineCompletion",
        "params": {
            "textDocument": {"uri": uri},
            "position": {"line": 0, "character": 15},
            "context": {"triggerKind": 1}
        }
    });

    assert!(scheduler
        .dispatch(&mut server, custom, Some(1))
        .iter()
        .all(|message| message.get("error").is_none()));
    let deadline = Instant::now() + Duration::from_millis(100);
    while scheduler.speculative_pending_count() != 0 {
        assert!(Instant::now() < deadline, "the custom request should start");
        std::thread::yield_now();
    }
    let output = scheduler.dispatch(&mut server, standard, Some(1));

    assert_eq!(response_with_id(&output, 1)["error"]["code"], -32800);
    let mut results = wait_for_speculative_results(&scheduler, 2).into_iter();
    assert!(scheduler
        .finish(&mut server, results.next().expect("superseded result"))
        .is_empty());
    let output = scheduler.finish(&mut server, results.next().expect("standard result"));
    assert!(response_with_id(&output, 2).get("error").is_none());
}

#[test]
fn standard_inline_request_rejects_a_result_after_the_document_changes() {
    let uri = "file:///tmp/stale-standard-inline.tua";
    let mut server = LspServer::default();
    let _ = server.handle_message(did_open(uri, "local value = 1\n"));
    enable_scheduled_local_inline_completion(&mut server);
    let (event_tx, _event_rx) = std::sync::mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
    let mut scheduler = RequestScheduler::new(event_tx, SharedReceivedInputState::default());
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "textDocument/inlineCompletion",
        "params": {
            "textDocument": {"uri": uri},
            "position": {"line": 0, "character": 15},
            "context": {"triggerKind": 1},
            "testDelayMs": 100
        }
    });

    assert!(scheduler
        .dispatch(&mut server, request, Some(1))
        .iter()
        .all(|message| message.get("error").is_none()));
    let deadline = Instant::now() + Duration::from_millis(100);
    while scheduler.speculative_pending_count() != 0 {
        assert!(
            Instant::now() < deadline,
            "the standard request should start"
        );
        std::thread::yield_now();
    }

    let _ = server.handle_message(did_change(uri, "local value = 2\n", 2));
    let output = scheduler.cancel_stale_document_requests(
        uri,
        server.document_generations.get(uri).copied(),
        server.document_versions.get(uri).copied(),
    );

    assert_eq!(response_with_id(&output, 1)["error"]["code"], -32801);
    let result = wait_for_speculative_results(&scheduler, 1)
        .into_iter()
        .next()
        .expect("cancelled standard request result");
    assert!(scheduler.finish(&mut server, result).is_empty());
}

#[test]
fn malformed_local_inline_request_returns_invalid_params_without_queuing() {
    let mut server = LspServer::default();
    enable_scheduled_local_inline_completion(&mut server);
    let (event_tx, _event_rx) = std::sync::mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
    let mut scheduler = RequestScheduler::new(event_tx, SharedReceivedInputState::default());
    let output = scheduler.dispatch(
        &mut server,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tua/localInlineCompletion",
            "params": {"position": {"line": 0, "character": 0}}
        }),
        None,
    );

    assert_eq!(response_with_id(&output, 1)["error"]["code"], -32602);
    assert_eq!(scheduler.in_flight_count(), 0);
    assert_eq!(scheduler.speculative_pending_count(), 0);
}

#[test]
fn disabled_local_inline_request_returns_without_queuing() {
    let uri = "file:///tmp/disabled-inline.tua";
    let mut server = LspServer::default();
    let _ = server.handle_message(did_open(uri, "local value = 1\n"));
    let (event_tx, _event_rx) = std::sync::mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
    let mut scheduler = RequestScheduler::new(event_tx, SharedReceivedInputState::default());
    let output = scheduler.dispatch(
        &mut server,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tua/localInlineCompletion",
            "params": {
                "textDocument": {"uri": uri},
                "position": {"line": 0, "character": 15},
                "documentVersion": 1
            }
        }),
        Some(1),
    );

    assert_eq!(response_with_id(&output, 1)["result"]["items"], json!([]));
    assert_eq!(scheduler.in_flight_count(), 0);
    assert_eq!(scheduler.speculative_pending_count(), 0);
}

#[test]
fn speculative_worker_survives_a_panicking_request() {
    let uri = "file:///tmp/panicking-inline.tua";
    let mut server = LspServer::default();
    let _ = server.handle_message(did_open(uri, "local value = 1\n"));
    enable_scheduled_local_inline_completion(&mut server);
    let (event_tx, _event_rx) = std::sync::mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
    let mut scheduler = RequestScheduler::new(event_tx, SharedReceivedInputState::default());
    let request = |id, test_panic| {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tua/localInlineCompletion",
            "params": {
                "textDocument": {"uri": uri},
                "position": {"line": 0, "character": 15},
                "documentVersion": 1,
                "testPanic": test_panic
            }
        })
    };

    assert!(scheduler
        .dispatch(&mut server, request(1, true), Some(1))
        .iter()
        .all(|message| message.get("error").is_none()));
    let panicked = wait_for_speculative_results(&scheduler, 1)
        .pop()
        .expect("panicking request result");
    let output = scheduler.finish(&mut server, panicked);
    assert_eq!(response_with_id(&output, 1)["error"]["code"], -32603);

    assert!(scheduler
        .dispatch(&mut server, request(2, false), Some(1))
        .iter()
        .all(|message| message.get("error").is_none()));
    let survived = wait_for_speculative_results(&scheduler, 1)
        .pop()
        .expect("surviving request result");
    let output = scheduler.finish(&mut server, survived);
    assert!(response_with_id(&output, 2).get("error").is_none());
}

#[test]
fn newest_inline_bursts_remain_bounded_until_cancelled_workers_finish() {
    let uri = "file:///tmp/newest-inline-burst.tua";
    let mut server = LspServer::default();
    let _ = server.handle_message(did_open(uri, "local value = 1\n"));
    enable_scheduled_local_inline_completion(&mut server);
    let (event_tx, _event_rx) = std::sync::mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
    let mut scheduler = RequestScheduler::new(event_tx, SharedReceivedInputState::default());
    let request = |id| {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tua/localInlineCompletion",
            "params": {
                "textDocument": {"uri": uri},
                "position": {"line": 0, "character": 15},
                "documentVersion": 1,
                "testDelayMs": 100
            }
        })
    };
    let burst = crate::server::scheduler::MAX_IN_FLIGHT_JOBS * 3;

    for id in 0..burst {
        let output = scheduler.dispatch(&mut server, request(id), Some(1));
        assert!(output.iter().all(|message| {
            message.get("error").is_none() || message["error"]["code"] == -32800
        }));
        assert!(
            scheduler.in_flight_count() <= 1,
            "only the latest speculative request should remain accounted, got {}",
            scheduler.in_flight_count()
        );
        assert!(scheduler.speculative_pending_count() <= 1);
    }
}

#[test]
fn local_inline_requests_respect_non_interactive_capacity() {
    let mut server = LspServer::default();
    enable_scheduled_local_inline_completion(&mut server);
    let (event_tx, _event_rx) = std::sync::mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
    let mut scheduler = RequestScheduler::new(event_tx, SharedReceivedInputState::default());
    let request = |id: usize| {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tua/localInlineCompletion",
            "params": {
                "textDocument": {"uri": format!("file:///tmp/inline-capacity-{id}.tua")},
                "position": {"line": 0, "character": 0},
                "documentVersion": 1
            }
        })
    };

    for id in 0..(crate::server::scheduler::MAX_IN_FLIGHT_JOBS * 3 / 4) {
        assert!(scheduler
            .dispatch(&mut server, request(id), Some(1))
            .iter()
            .all(|message| message.get("error").is_none()));
    }
    let rejected_id = crate::server::scheduler::MAX_IN_FLIGHT_JOBS;
    let output = scheduler.dispatch(&mut server, request(rejected_id), Some(1));
    assert_eq!(
        response_with_id(&output, rejected_id as u64)["error"]["code"],
        -32000
    );
}

#[test]
fn local_inline_completion_yields_to_menu_completion() {
    let uri = "file:///tmp/inline-priority.tua";
    let source = "local value = 1\n";
    let inline = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tua/localInlineCompletion",
        "params": {
            "textDocument": {"uri": uri},
            "position": {"line": 0, "character": 15},
            "documentVersion": 1
        }
    });
    let mut messages = std::collections::VecDeque::from([
        inline,
        invoked_completion_request(3, uri, source, "value"),
    ]);

    assert_eq!(
        pop_next_pending_message(&mut messages).unwrap()["method"],
        "textDocument/completion"
    );
    assert_eq!(
        pop_next_pending_message(&mut messages).unwrap()["method"],
        "tua/localInlineCompletion"
    );
}

#[test]
fn speculative_result_barrier_never_holds_edits_or_menu_completion() {
    let uri = "file:///tmp/speculative-priority.tua";
    let source = "local value = 1\n";

    assert!(message_can_wait_for_speculative_result(
        &inlay_hint_request(1, uri, 0, 0, 1, 0,)
    ));
    assert!(!message_can_wait_for_speculative_result(&did_change(
        uri, source, 2,
    )));
    assert!(!message_can_wait_for_speculative_result(
        &invoked_completion_request(2, uri, source, "value")
    ));
    assert!(!message_can_wait_for_speculative_result(&json!({
        "jsonrpc": "2.0",
        "method": "$/cancelRequest",
        "params": {"id": 2}
    })));
}

#[test]
fn speculative_result_barrier_expires_without_a_worker_result() {
    let uri = "file:///tmp/speculative-timeout.tua";
    let mut server = LspServer::default();
    let _ = server.handle_message(did_open(uri, "local value = 1\n"));
    enable_scheduled_local_inline_completion(&mut server);
    let (event_tx, _event_rx) = std::sync::mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
    let mut scheduler = RequestScheduler::new(event_tx, SharedReceivedInputState::default());
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tua/localInlineCompletion",
        "params": {
            "textDocument": {"uri": uri},
            "position": {"line": 0, "character": 15},
            "documentVersion": 1,
            "testDelayMs": 100
        }
    });

    assert!(scheduler
        .dispatch(&mut server, request, Some(1))
        .iter()
        .all(|message| message.get("error").is_none()));
    assert!(scheduler.speculative_has_priority(Instant::now()));
    std::thread::sleep(Duration::from_millis(15));
    assert!(!scheduler.speculative_has_priority(Instant::now()));
}

#[test]
fn speculative_barrier_defers_only_workspace_warm_adoption() {
    let mut deferred = VecDeque::new();
    let request =
        ServerEvent::RequestFinished(Box::new(scheduler::WorkerResult::test_result(1, false)));
    assert!(defer_background_adoption(request, &mut deferred).is_some());
    assert!(
        deferred.is_empty(),
        "ordinary request results stay responsive"
    );

    let warm =
        ServerEvent::RequestFinished(Box::new(scheduler::WorkerResult::test_result(2, true)));
    assert!(defer_background_adoption(warm, &mut deferred).is_none());
    assert_eq!(deferred.len(), 1);
    assert!(take_deferred_background_adoption(&mut deferred, true, true).is_none());
    assert_eq!(
        deferred.len(),
        1,
        "the bounded priority window keeps warm adoption deferred"
    );
    assert!(take_deferred_background_adoption(&mut deferred, false, true).is_some());
    assert!(
        deferred.is_empty(),
        "idle coordinator drains deferred warm adoption"
    );
}

#[test]
fn speculative_result_mailbox_bypasses_a_saturated_event_queue() {
    let uri = "file:///tmp/speculative-saturated-events.tua";
    let mut server = LspServer::default();
    let _ = server.handle_message(did_open(uri, "local value = 1\n"));
    enable_scheduled_local_inline_completion(&mut server);
    let (event_tx, event_rx) = std::sync::mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
    let event_fill = event_tx.clone();
    let mut scheduler = RequestScheduler::new(event_tx, SharedReceivedInputState::default());
    for id in 0..EVENT_QUEUE_CAPACITY {
        event_fill
            .try_send(ServerEvent::Input(Ok(Some(json!({
                "jsonrpc": "2.0",
                "id": 1000 + id,
                "method": "tua/unknown"
            })))))
            .expect("fixture should fill the event queue exactly");
    }
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tua/localInlineCompletion",
        "params": {
            "textDocument": {"uri": uri},
            "position": {"line": 0, "character": 15},
            "documentVersion": 1
        }
    });
    assert!(scheduler
        .dispatch(&mut server, request, Some(1))
        .iter()
        .all(|message| message.get("error").is_none()));

    let deadline = Instant::now() + Duration::from_millis(100);
    let result = loop {
        if let Some(result) = scheduler.take_speculative_results().into_iter().next() {
            break result;
        }
        assert!(
            Instant::now() < deadline,
            "speculative result was not published"
        );
        std::thread::yield_now();
    };
    assert_eq!(scheduler.finish(&mut server, result)[0]["id"], 1);
    drop(event_rx);
}

#[test]
fn cancelling_a_coalesced_waiter_restores_request_capacity() {
    let uri = "file:///tmp/coalesced-capacity.tua";
    let mut server = LspServer::default();
    let (event_tx, _event_rx) = std::sync::mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
    let mut scheduler = RequestScheduler::new(event_tx, SharedReceivedInputState::default());

    for id in 0..crate::server::scheduler::MAX_PENDING_REQUESTS {
        let output = scheduler.dispatch(
            &mut server,
            slow_background_request(id as u64, Some(uri), 1_000),
            None,
        );
        assert!(output.iter().all(|message| message.get("error").is_none()));
    }
    let rejected_id = crate::server::scheduler::MAX_PENDING_REQUESTS as u64;
    let output = scheduler.dispatch(
        &mut server,
        slow_background_request(rejected_id, Some(uri), 1_000),
        None,
    );
    assert_eq!(
        response_with_id(&output, rejected_id)["error"]["code"],
        -32000
    );

    assert_eq!(scheduler.cancel(&json!(0))[0]["error"]["code"], -32800);
    let accepted_id = rejected_id + 1;
    let output = scheduler.dispatch(
        &mut server,
        slow_background_request(accepted_id, Some(uri), 1_000),
        None,
    );
    assert!(output.iter().all(|message| message.get("error").is_none()));
}

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
fn ecs_graph_yields_to_typing_and_completion() {
    let uri = "file:///tmp/ecs-graph-priority.tua";
    let after = "local Position = ecs.component(\"Position\", {})\nP";
    let mut messages = std::collections::VecDeque::from([
        ecs_graph_request(2),
        did_change(uri, after, 2),
        invoked_completion_request(3, uri, after, "P"),
    ]);

    let first = pop_next_pending_message(&mut messages).expect("first message");
    let second = pop_next_pending_message(&mut messages).expect("second message");
    let third = pop_next_pending_message(&mut messages).expect("third message");

    assert_eq!(first["method"], "textDocument/didChange");
    assert_eq!(second["method"], "textDocument/completion");
    assert_eq!(third["method"], "tua/ecsGraph");
}

#[test]
fn object_pool_graph_yields_to_typing_and_completion() {
    let uri = "file:///tmp/object-pool-graph-priority.tua";
    let after = "local bullets = objectPool({ create = function() return {} end })\nb";
    let mut messages = std::collections::VecDeque::from([
        object_pool_graph_request(2),
        did_change(uri, after, 2),
        invoked_completion_request(3, uri, after, "b"),
    ]);

    let first = pop_next_pending_message(&mut messages).expect("first message");
    let second = pop_next_pending_message(&mut messages).expect("second message");
    let third = pop_next_pending_message(&mut messages).expect("third message");

    assert_eq!(first["method"], "textDocument/didChange");
    assert_eq!(second["method"], "textDocument/completion");
    assert_eq!(third["method"], "tua/objectPoolGraph");
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
fn request_worker_survives_a_large_incomplete_lua_block() {
    let output = std::thread::Builder::new()
        .name("tua-large-incomplete-test-coordinator".to_owned())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let uri = "file:///tmp/large-incomplete-block.tua";
            let mut source = String::from(
                "type Player = { hp: number }\n\
                 local player: Player = { hp = 10 }\n\
                 function hurtPlayer(player: Player, damage: number)\n\
                   if player.hp <= 0 then\n",
            );
            for index in 0..900 {
                source.push_str(&format!("local value{index} = {index}\n"));
            }

            let mut server = LspServer::default();
            server.handle_message(did_open(uri, &source));
            server.recent_document_changes.clear();

            let received_input = SharedReceivedInputState::default();
            let (event_tx, event_rx) = std::sync::mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
            let mut scheduler = RequestScheduler::new(event_tx, received_input);
            scheduler.dispatch(
                &mut server,
                inlay_hint_request(2, uri, 0, 0, 1_000, 0),
                Some(1),
            );

            let result = loop {
                match event_rx
                    .recv_timeout(Duration::from_secs(10))
                    .expect("large incomplete request should finish")
                {
                    ServerEvent::RequestFinished(result) => break result,
                    ServerEvent::Input(_) | ServerEvent::SpeculativeReady => continue,
                }
            };

            scheduler.finish(&mut server, *result)
        })
        .expect("test coordinator should start")
        .join()
        .expect("large incomplete request should not overflow");

    assert!(
        response_with_id(&output, 2)["result"].is_array(),
        "inlay hints should return a response for a large incomplete block: {output:#?}"
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
        WorkspaceWarmWork::Files { files, .. } => {
            assert_eq!(files[0].0, temp.path.join("src/main.tua"));
            assert_eq!(files[0].1, tua_core::AnalysisWarmPriority::High);
        }
        WorkspaceWarmWork::Snapshot { .. } => panic!("files must warm before the snapshot"),
    }
}

#[test]
fn project_pattern_corpus_is_disabled_until_explicitly_enabled() {
    let temp = TempProject::new("pattern-disabled");
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", "print(value)\n");
    let mut server = TestServer::new();

    server.send_without_drain(initialize_with_root(1, temp.uri()));

    assert!(!server.server.pattern_corpus.enabled);
    assert!(server.server.pattern_corpus.pending.is_empty());
    assert!(server
        .server
        .take_next_pattern_corpus_work(Instant::now() + Duration::from_secs(1))
        .is_none());
}

#[test]
fn disabling_project_patterns_publishes_empty_and_queues_storage_for_background_drop() {
    let temp = TempProject::new("pattern-disable-clears");
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", "draw(player,score)\n");
    let mut server = TestServer::new();
    server.send_without_drain(initialize_with_root(1, temp.uri()));
    enable_project_pattern_corpus(&mut server.server);
    let work = server
        .server
        .take_next_pattern_corpus_work(Instant::now() + Duration::from_secs(1))
        .expect("enabled corpus should queue a file build");
    finish_pattern_work(&mut server.server, work);
    assert_eq!(
        server.server.analysis.project_pattern_corpus_stats().files,
        1
    );
    let populated_bytes = server
        .server
        .analysis
        .project_pattern_corpus_stats()
        .approximate_bytes;

    server.send_without_drain(did_change_configuration(json!({
        "completion": {
            "inline": {
                "enabled": true,
                "sources": {"project": false}
            }
        }
    })));

    assert_eq!(
        server.server.analysis.project_pattern_corpus_stats().files,
        0,
        "disabling must publish an empty corpus immediately"
    );
    server.send(cache_stats_request(88));
    let stats = &response_with_id(server.output(), 88)["result"];
    assert!(
        stats["patternCorpus"]["approximateBytes"]
            .as_u64()
            .is_some_and(|bytes| bytes < populated_bytes as u64),
        "only the fixed empty-corpus overhead should remain published"
    );
    assert_eq!(stats["patternCorpus"]["disposalsQueued"], 1);
    let disposal = server
        .server
        .take_next_pattern_corpus_work(Instant::now())
        .expect("retired storage should be queued for a background drop");
    assert!(matches!(
        disposal.operation,
        PatternCorpusOperation::Dispose(_)
    ));
}

#[test]
fn project_pattern_edits_are_debounced_and_coalesced_without_building() {
    let temp = TempProject::new("pattern-coalesce");
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", "print(first)\n");
    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();
    server.send_without_drain(initialize_with_root(1, temp.uri()));
    enable_project_pattern_corpus(&mut server.server);
    server.server.pattern_corpus.pending.clear();

    let first = Instant::now();
    server.send_without_drain(did_open(&uri, "print(first)\n"));
    let first_due = server
        .server
        .next_pattern_corpus_due()
        .expect("queued open");
    assert!(first_due >= first + PATTERN_CORPUS_REBUILD_DEBOUNCE);

    server.send_without_drain(did_change(&uri, "print(second)\n", 2));
    let second_due = server
        .server
        .next_pattern_corpus_due()
        .expect("queued edit");
    assert!(second_due >= first_due);
    assert_eq!(server.server.pattern_corpus.pending.len(), 1);
    assert!(server
        .server
        .take_next_pattern_corpus_work(second_due - Duration::from_millis(1))
        .is_none());
    assert_eq!(
        server.server.analysis.project_pattern_corpus_stats().files,
        0
    );
    assert!(server
        .server
        .pattern_corpus
        .pending
        .contains_key(&temp.path.join("src/main.tua")));
}

#[test]
fn project_pattern_adoption_rejects_an_edited_file_and_keeps_fresh_files() {
    let temp = TempProject::new("pattern-stale-partial");
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", "print(main_value)\n");
    temp.write("src/other.tua", "print(other_value)\n");
    let main_uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();
    server.send_without_drain(initialize_with_root(1, temp.uri()));
    enable_project_pattern_corpus(&mut server.server);

    let work = server
        .server
        .take_next_pattern_corpus_work(Instant::now() + Duration::from_secs(1))
        .expect("initial corpus work");
    server.send_without_drain(did_open(&main_uri, "print(changed_value)\n"));
    finish_pattern_work(&mut server.server, work);

    let stats = server.server.analysis.project_pattern_corpus_stats();
    assert_eq!(server.server.pattern_corpus.stale, 1);
    assert_eq!(stats.files, 1, "the unchanged file should still be adopted");
    assert!(server
        .server
        .pattern_corpus
        .pending
        .contains_key(&temp.path.join("src/main.tua")));
}

#[test]
fn project_pattern_generation_change_rejects_the_whole_old_batch() {
    let temp = TempProject::new("pattern-generation");
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", "print(value)\n");
    let mut server = TestServer::new();
    server.send_without_drain(initialize_with_root(1, temp.uri()));
    enable_project_pattern_corpus(&mut server.server);
    let old = server
        .server
        .take_next_pattern_corpus_work(Instant::now() + Duration::from_secs(1))
        .expect("initial corpus work");

    server.server.reset_pattern_corpus_queue();
    finish_pattern_work(&mut server.server, old);

    assert_eq!(server.server.pattern_corpus.generation_rejections, 1);
    assert_eq!(
        server.server.analysis.project_pattern_corpus_stats().files,
        0
    );
}

#[test]
fn project_pattern_remerge_builds_in_background_and_adopts_when_current() {
    let temp = TempProject::new("pattern-remerge");
    temp.write("tua.toml", "source = \"src\"\n");
    for index in 0..32 {
        temp.write(
            &format!("src/module_{index:02}.tua"),
            &format!("print(value_{index})\n"),
        );
    }
    let mut server = TestServer::new();
    server.send_without_drain(initialize_with_root(1, temp.uri()));
    enable_project_pattern_corpus(&mut server.server);
    let due = Instant::now() + Duration::from_secs(1);
    for _ in 0..2 {
        let work = server
            .server
            .take_next_pattern_corpus_work(due)
            .expect("file build batch");
        finish_pattern_work(&mut server.server, work);
    }

    assert!(server.server.pattern_corpus.remerge_pending);
    assert_eq!(
        server
            .server
            .analysis
            .project_pattern_corpus_stats()
            .overlay_files,
        32
    );
    let remerge = server
        .server
        .take_next_pattern_corpus_work(due)
        .expect("background remerge work");
    finish_remerge_work(&mut server.server, remerge);

    let stats = server.server.analysis.project_pattern_corpus_stats();
    assert_eq!(server.server.pattern_corpus.remerges, 1);
    assert_eq!(stats.base_files, 32);
    assert_eq!(stats.overlay_files, 0);
}

#[test]
fn project_pattern_remerge_rejects_a_source_changed_during_build() {
    let temp = TempProject::new("pattern-remerge-stale");
    temp.write("tua.toml", "source = \"src\"\n");
    for index in 0..32 {
        temp.write(
            &format!("src/module_{index:02}.tua"),
            &format!("print(value_{index})\n"),
        );
    }
    let changed_uri = temp.uri_for("src/module_00.tua");
    let mut server = TestServer::new();
    server.send_without_drain(initialize_with_root(1, temp.uri()));
    enable_project_pattern_corpus(&mut server.server);
    let due = Instant::now() + Duration::from_secs(1);
    for _ in 0..2 {
        let work = server
            .server
            .take_next_pattern_corpus_work(due)
            .expect("file build batch");
        finish_pattern_work(&mut server.server, work);
    }
    let remerge = server
        .server
        .take_next_pattern_corpus_work(due)
        .expect("background remerge work");
    server.send_without_drain(did_open(&changed_uri, "print(changed)\n"));
    finish_remerge_work(&mut server.server, remerge);

    assert_eq!(server.server.pattern_corpus.remerges, 0);
    assert_eq!(server.server.pattern_corpus.remerge_rejections, 1);
    assert_eq!(
        server
            .server
            .analysis
            .project_pattern_corpus_stats()
            .overlay_files,
        32
    );
}

#[test]
fn deleting_a_project_pattern_source_publishes_a_tombstone() {
    let temp = TempProject::new("pattern-delete");
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", "print(value)\n");
    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();
    server.send_without_drain(initialize_with_root(1, temp.uri()));
    enable_project_pattern_corpus(&mut server.server);
    let work = server
        .server
        .take_next_pattern_corpus_work(Instant::now() + Duration::from_secs(1))
        .expect("initial corpus work");
    finish_pattern_work(&mut server.server, work);
    assert_eq!(
        server.server.analysis.project_pattern_corpus_stats().files,
        1
    );

    temp.remove("src/main.tua");
    server.send_without_drain(watched_files_changed([(&uri, 3)]));

    let stats = server.server.analysis.project_pattern_corpus_stats();
    assert_eq!(stats.files, 0);
    assert_eq!(stats.tombstones, 1);
}

#[test]
fn project_pattern_worker_does_not_consume_client_request_capacity() {
    let temp = TempProject::new("pattern-contention");
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", "local value = 1\nvalue\n");
    let uri = temp.uri_for("src/main.tua");
    let mut server = LspServer::default();
    let _ = server.handle_message(initialize_with_root(1, temp.uri()));
    enable_project_pattern_corpus(&mut server);
    let (event_tx, _event_rx) = std::sync::mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
    let mut scheduler = RequestScheduler::new(event_tx, SharedReceivedInputState::default());

    assert!(
        scheduler.dispatch_due_pattern_corpus(&mut server, Instant::now() + Duration::from_secs(1))
    );
    let output = scheduler.dispatch(
        &mut server,
        invoked_completion_request(99, &uri, "local value = 1\nvalue\n", "value"),
        None,
    );

    assert!(output.iter().all(|message| message.get("error").is_none()));
}

#[test]
fn project_pattern_generation_change_cancels_background_work_between_files() {
    let temp = TempProject::new("pattern-cancel-generation");
    temp.write("tua.toml", "source = \"src\"\n");
    for index in 0..PATTERN_CORPUS_BATCH_SIZE {
        temp.write(
            &format!("src/module_{index}.tua"),
            &format!("print(value_{index})\n"),
        );
    }
    let mut server = LspServer::default();
    let _ = server.handle_message(initialize_with_root(1, temp.uri()));
    enable_project_pattern_corpus(&mut server);
    let generation = server.pattern_corpus.generation;
    let (event_tx, _event_rx) = std::sync::mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
    let mut scheduler = RequestScheduler::new(event_tx, SharedReceivedInputState::default());
    assert!(
        scheduler.dispatch_due_pattern_corpus(&mut server, Instant::now() + Duration::from_secs(1))
    );

    server.reset_pattern_corpus_queue();
    scheduler.cancel_stale_pattern_corpus(server.pattern_corpus.generation);

    assert!(scheduler.pattern_corpus_job_is_cancelled(generation));
    scheduler.abandon_workspace_warm();
}

#[test]
fn idle_server_builds_and_reports_the_project_pattern_corpus() {
    let temp = TempProject::new("pattern-idle-integration");
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", "print(first)\nprint(sec");
    temp.write("src/other.lua", "print(other)\n");

    let output = run_chunked_messages(vec![
        (
            Duration::ZERO,
            vec![
                initialize_with_root(1, temp.uri()),
                did_change_configuration(json!({
                    "completion": {
                        "inline": {
                            "enabled": true,
                            "sources": {"project": true}
                        }
                    }
                })),
            ],
        ),
        (Duration::from_millis(750), vec![cache_stats_request(100)]),
    ]);
    let corpus = &response_with_id(&output, 100)["result"]["patternCorpus"];

    assert_eq!(corpus["enabled"], true);
    assert_eq!(corpus["queued"].as_u64(), Some(0));
    assert_eq!(corpus["inFlight"].as_u64(), Some(0));
    assert_eq!(corpus["filesBuilt"].as_u64(), Some(2));
    assert_eq!(corpus["adopted"].as_u64(), Some(2));
    assert_eq!(corpus["files"].as_u64(), Some(2));
    assert!(corpus["approximateBytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
}

#[test]
fn workspace_warm_batches_files_without_starving_open_file_priority() {
    let temp = TempProject::new("warm-file-batches");
    let main_source = "local main: number = 1\n";

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", main_source);
    for index in 0..40 {
        temp.write(
            &format!("src/module_{index:02}.tua"),
            &format!("return {{ value = {index} }}\n"),
        );
    }

    let main_uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&main_uri, main_source));

    let first = server
        .server
        .take_next_workspace_warm_work()
        .expect("the first warm batch should exist");
    let WorkspaceWarmWork::Files { files, .. } = first else {
        panic!("files must warm before the snapshot");
    };

    assert_eq!(files.len(), WORKSPACE_WARM_BATCH_SIZE);
    assert_eq!(files[0].0, temp.path.join("src/main.tua"));
    assert_eq!(files[0].1, tua_core::AnalysisWarmPriority::High);

    let second = server
        .server
        .take_next_workspace_warm_work()
        .expect("the remaining warm batch should exist");
    let WorkspaceWarmWork::Files { files, .. } = second else {
        panic!("remaining files must warm before the snapshot");
    };
    assert_eq!(files.len(), 9);
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
fn cache_stats_reports_a_terminal_workspace_snapshot_failure() {
    let mut server = TestServer::new();
    server.server.workspace_warm.enabled = true;
    let revision = server.server.analysis.revision();
    let work = server
        .server
        .take_next_workspace_warm_work()
        .expect("the workspace snapshot should be queued");

    assert!(matches!(
        work,
        WorkspaceWarmWork::Snapshot {
            revision: work_revision
        } if work_revision == revision
    ));
    server.server.finish_workspace_warm(&work, None, false);
    server.send(cache_stats_request(100));

    let warm = &response_with_id(server.output(), 100)["result"]["workspaceWarm"];
    assert_eq!(warm["snapshotReadyRevision"], Value::Null);
    assert_eq!(warm["snapshotFailedRevision"].as_u64(), Some(revision));
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
    let WorkspaceWarmWork::Files { files, .. } = &work else {
        panic!("opened file should warm before the snapshot");
    };
    let (path, priority) = &files[0];

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
        WorkspaceWarmWork::Files { ref files, .. }
            if files.iter().any(|(_, priority)| *priority == tua_core::AnalysisWarmPriority::High)
    ));
}

#[test]
fn shutdown_abandons_internal_workspace_warm_jobs() {
    let uri = "file:///tmp/shutdown-workspace-warm.tua";
    let mut server = LspServer::default();
    let (event_tx, _event_rx) = std::sync::mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
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
    let (event_tx, event_rx) = std::sync::mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
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
            ServerEvent::Input(_) | ServerEvent::SpeculativeReady => continue,
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
    let (event_tx, _event_rx) = std::sync::mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
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

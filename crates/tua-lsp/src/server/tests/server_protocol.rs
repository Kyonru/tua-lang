use super::*;

#[test]
fn exposes_server_name() {
    assert_eq!(server_name(), "tua-lsp");
}
#[test]
fn describes_tua_language_server() {
    assert!(server_description().contains("language server"));
}
#[test]
fn initialize_returns_server_capabilities() {
    let output = run_messages(vec![json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    })]);

    assert_eq!(output.len(), 1);
    assert_eq!(output[0]["id"], 1);
    assert_eq!(output[0]["result"]["serverInfo"]["name"], "tua-lsp");
    assert_eq!(
        output[0]["result"]["capabilities"]["textDocumentSync"]["openClose"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["textDocumentSync"]["change"],
        2
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["textDocumentSync"]["save"]["includeText"],
        true
    );
    assert_eq!(output[0]["result"]["capabilities"]["hoverProvider"], true);
    let completion_triggers = output[0]["result"]["capabilities"]["completionProvider"]
        ["triggerCharacters"]
        .as_array()
        .expect("completion trigger characters should be an array");
    for trigger in [":", ".", "\"", "'", "@", "-", "_", "a", "Z", "0"] {
        assert!(
            completion_triggers
                .iter()
                .any(|character| character == trigger),
            "completion trigger characters should include {trigger:?}"
        );
    }
    assert_eq!(
        output[0]["result"]["capabilities"]["completionProvider"]["resolveProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["completionProvider"]["completionItem"],
        Value::Null
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["signatureHelpProvider"]["triggerCharacters"],
        json!(["(", ","])
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["signatureHelpProvider"]["retriggerCharacters"],
        json!([")"])
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["definitionProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["typeDefinitionProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["implementationProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["callHierarchyProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["documentLinkProvider"]["resolveProvider"],
        false
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["linkedEditingRangeProvider"],
        Value::Null
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["documentHighlightProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["documentSymbolProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["selectionRangeProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["foldingRangeProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["codeActionProvider"]["codeActionKinds"],
        json!([
            "quickfix",
            "source.organizeImports",
            "source.sortImports",
            "source.removeUnusedImports",
            "source.fixAll"
        ])
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["codeLensProvider"]["resolveProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["documentFormattingProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["documentRangeFormattingProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["documentOnTypeFormattingProvider"]
            ["firstTriggerCharacter"],
        "\n"
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["documentOnTypeFormattingProvider"]
            ["moreTriggerCharacter"],
        json!(["}", ")", "]", ",", "d"])
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["referencesProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["workspaceSymbolProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["renameProvider"]["prepareProvider"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["inlayHintProvider"]["resolveProvider"],
        false
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["semanticTokensProvider"]["legend"]["tokenTypes"],
        json!([
            "typeAlias",
            "function",
            "variable",
            "parameter",
            "property",
            "builtinType",
            "unresolvedReference",
            "global",
            "module",
            "method",
            "annotationTag"
        ])
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["semanticTokensProvider"]["legend"]["tokenModifiers"],
        json!(["declaration", "readonly", "deprecated"])
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["semanticTokensProvider"]["full"]["delta"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["semanticTokensProvider"]["range"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["workspace"]["workspaceFolders"]["supported"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["workspace"]["workspaceFolders"]["changeNotifications"],
        true
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["workspace"]["fileOperations"]["willRename"]["filters"]
            [0]["pattern"]["glob"],
        "**/*"
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["diagnosticProvider"],
        Value::Null
    );
}
#[test]
fn initialize_advertises_completion_label_details_when_supported() {
    let output = run_messages(vec![initialize_with_completion_label_details(1)]);

    assert_eq!(
        output[0]["result"]["capabilities"]["completionProvider"]["completionItem"]
            ["labelDetailsSupport"],
        true
    );
}
#[test]
fn initialize_advertises_pull_diagnostics_when_supported() {
    let output = run_messages(vec![initialize_with_pull_diagnostics(
        1,
        "file:///tmp".to_owned(),
        true,
    )]);
    let provider = &output[0]["result"]["capabilities"]["diagnosticProvider"];

    assert_eq!(provider["interFileDependencies"], true);
    assert_eq!(provider["workspaceDiagnostics"], false);
}
#[test]
fn cache_stats_request_reports_analysis_cache_reuse() {
    let uri = "file:///tmp/cache-stats.tua";
    let source = r#"type Player = { hp: number }
local player: Player = { hp = 10 }
player.hp = player.hp - 1
"#;
    let mut server = TestServer::new();

    server.send(did_open(uri, source));
    server.send(hover_request(2, uri, source, "Player ="));
    server.send(completion_request(3, uri, source, "player."));
    server.send(cache_stats_request(4));

    let stats = &response_with_id(server.output(), 4)["result"];

    assert!(stats["parse"]["misses"].as_u64().unwrap_or(0) >= 1);
    assert!(stats["parse"]["hits"].is_number());
    assert!(stats["hir"]["hits"].is_number());
    assert!(stats["scope"]["hits"].is_number());
    assert!(stats["symbolIndex"]["hits"].is_number());
    assert!(stats["usageIndex"]["hits"].is_number());
    assert!(stats["usageIndex"]["misses"].as_u64().unwrap_or(0) >= 1);
    assert!(stats["completionContext"]["hits"].is_number());
    assert!(stats["completionContext"]["misses"].as_u64().unwrap_or(0) >= 1);
    assert!(stats["invalidations"].is_number());
}
#[test]
fn trace_initialization_options_emit_lsp_log_messages() {
    let mut server = TestServer::new();

    server.send(initialize_with_trace_options(
        1,
        json!({
            "trace": {
                "server": "messages"
            },
            "logVerbosity": "off"
        }),
    ));
    server.send(cache_stats_request(2));

    assert!(server.output().iter().any(|message| {
        message["method"] == "window/logMessage"
            && message["params"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("tua/cacheStats"))
    }));
}
#[test]
fn trace_notification_updates_logging_without_restart() {
    let mut server = TestServer::new();

    server.send(initialize_with_trace_options(
        1,
        json!({
            "trace": {
                "server": "off"
            },
            "logVerbosity": "off"
        }),
    ));
    server.send(cache_stats_request(2));

    assert!(!server.output().iter().any(|message| {
        message["method"] == "window/logMessage"
            && message["params"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("tua/cacheStats"))
    }));

    server.send(set_trace_notification(json!({
        "trace": {
            "server": "off"
        },
        "logVerbosity": "info"
    })));
    server.send(cache_stats_request(3));

    assert!(server.output().iter().any(|message| {
        message["method"] == "window/logMessage"
            && message["params"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("trace updated"))
    }));
    assert!(server.output().iter().any(|message| {
        message["method"] == "window/logMessage"
            && message["params"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("tua/cacheStats"))
    }));
}
#[test]
fn trace_summarizes_noisy_editor_background_requests() {
    let mut server = TestServer::new();
    let uri = "file:///tmp/trace-summary.tua";
    let source = "local value = 1\nprint(value)\n";

    server.send(initialize_with_trace_options(
        1,
        json!({
            "trace": {
                "server": "messages"
            },
            "logVerbosity": "debug"
        }),
    ));
    server.send(did_open(uri, source));
    server.send(text_document_diagnostic_request(2, uri));
    server.send(code_action_request(3, uri, 0, 0, 0, 16, Vec::new()));
    server.send(document_link_request(4, uri));
    server.send(inlay_hint_request(5, uri, 0, 0, 3, 0));
    server.send(folding_range_request(6, uri));
    server.send(document_symbol_request(7, uri));
    server.send(code_lens_request(8, uri));
    server.send(json!({
        "jsonrpc": "2.0",
        "method": "$/cancelRequest",
        "params": {
            "id": 8
        }
    }));
    server.send(hover_request(9, uri, source, "value"));

    let log_messages = server
        .output()
        .iter()
        .filter(|message| message["method"] == "window/logMessage")
        .filter_map(|message| message["params"]["message"].as_str())
        .collect::<Vec<_>>();
    let summaries = log_messages
        .iter()
        .filter(|message| message.contains("tua lsp trace summary"))
        .copied()
        .collect::<Vec<_>>();
    let combined = summaries.join("\n");

    assert_eq!(summaries.len(), 2, "{log_messages:#?}");
    assert!(summaries[0].contains("total=3"));
    assert!(summaries[1].contains("total=4"));
    assert!(combined.contains("textDocument/codeAction=1"));
    assert!(combined.contains("textDocument/codeLens=1"));
    assert!(combined.contains("$/cancelRequest=1"));
    assert!(
        log_messages
            .iter()
            .any(|message| message.contains("tua lsp request: textDocument/inlayHint")),
        "interactive inlay requests should remain visible in the trace: {log_messages:#?}"
    );
}
#[test]
fn debug_trace_logs_typing_and_completion_state() {
    let mut server = TestServer::new();
    let uri = "file:///tmp/debug-trace.tua";
    let source = "local outside_function = { x = 1 }\nprint(ou)\nprint(outside_function.x)\n";

    server.send(initialize_with_trace_options(
        1,
        json!({
            "trace": {
                "server": "messages"
            },
            "logVerbosity": "debug"
        }),
    ));
    server.send(did_open(uri, source));
    server.send(did_change(uri, source, 2));
    server.send(invoked_completion_request(2, uri, source, "print(o"));
    server.send(hover_request(4, uri, source, "outside_function"));
    server.send(definition_request(
        3,
        uri,
        source,
        "outside_function.x",
        "outside_function.".len(),
    ));

    let log_messages = server
        .output()
        .iter()
        .filter(|message| message["method"] == "window/logMessage")
        .filter_map(|message| message["params"]["message"].as_str())
        .collect::<Vec<_>>();

    assert!(
        log_messages.iter().all(|message| {
            message.starts_with("[lsp ")
                && message.get(28..31).is_some_and(|suffix| suffix == "Z] ")
        }),
        "LSP log messages should include UTC timestamps: {log_messages:#?}"
    );

    assert!(
        log_messages.iter().any(|message| message.contains(
            "tua document opened: uri=file:///tmp/debug-trace.tua languageId=tua version=1"
        )),
        "debug trace should log document open state: {log_messages:#?}"
    );
    assert!(
        log_messages.iter().any(|message| message.contains(
            "tua document changed: uri=file:///tmp/debug-trace.tua version=2 changes=[range=full"
        )),
        "debug trace should log document change state: {log_messages:#?}"
    );
    assert!(
        log_messages.iter().any(|message| {
            message.contains("tua completion response: id=2 uri=file:///tmp/debug-trace.tua")
                && message.contains("docVersion=2")
                && message.contains("prefix=\"o\"")
                && message.contains("line=\"print(ou)\"")
                && message.contains("effectivePrefix=\"ou\"")
                && message.contains("items=")
                && message.contains("labels=")
                && message.contains("outside_function")
                && message.contains("elapsedMs=")
        }),
        "debug trace should log completion response state: {log_messages:#?}"
    );
    assert!(
        log_messages.iter().any(|message| {
            message.contains("tua definition response: id=3 uri=file:///tmp/debug-trace.tua")
                && message.contains("result=file:///tmp/debug-trace.tua")
                && message.contains("elapsedMs=")
        }),
        "debug trace should log definition response state: {log_messages:#?}"
    );
    assert!(
        log_messages.iter().any(|message| {
            message.contains("tua hover response: id=4 uri=file:///tmp/debug-trace.tua")
                && message.contains("docVersion=2")
                && message.contains("result=hover")
                && message.contains("outside_function")
                && message.contains("elapsedMs=")
        }),
        "debug trace should log hover response state: {log_messages:#?}"
    );
}

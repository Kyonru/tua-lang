use super::*;

#[test]
fn inline_context_and_validation_are_versioned_and_stale_safe() {
    let uri = "file:///tmp/model-completion.tua";
    let source = "local value: number = ";
    let mut server = TestServer::new();
    server.send(did_open(uri, source));
    server.send(inline_context_request(
        1,
        uri,
        source,
        "local value: number = ",
        1,
    ));

    let context = response_with_id(server.output(), 1)["result"].clone();
    assert_eq!(context["documentVersion"], 1);
    assert!(context["analysisRevision"].as_u64().is_some());
    assert!(context["sourceFingerprint"].as_str().is_some());
    assert!(context["prompt"]
        .as_str()
        .is_some_and(|prompt| prompt.contains("Tua language contract")));

    server.send(validate_request(2, uri, &context, "1", 1));
    let accepted = &response_with_id(server.output(), 2)["result"];
    assert_eq!(accepted["status"], "accepted");
    assert_eq!(accepted["insertText"], "1");

    server.send(did_change(uri, "local value: number = 2", 2));
    server.send(validate_request(3, uri, &context, "1", 1));
    let stale = &response_with_id(server.output(), 3)["result"];
    assert_eq!(stale["status"], "rejected");
    assert_eq!(stale["reason"], "stale");
}

#[test]
fn inline_context_skips_comments_and_converts_utf16_positions() {
    let uri = "file:///tmp/model-completion-utf16.tua";
    let prefix = "local icon = \"😀\"; local value = ";
    let source = format!("{prefix}\n-- no completion");
    let mut server = TestServer::new();
    server.send(did_open(uri, &source));

    server.send(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tua/inlineCompletionContext",
        "params": {
            "textDocument": {"uri": uri},
            "position": {"line": 0, "character": prefix.encode_utf16().count()},
            "documentVersion": 1,
            "contextBytes": 4096
        }
    }));
    let context = &response_with_id(server.output(), 1)["result"];
    assert!(context["prompt"]
        .as_str()
        .is_some_and(|prompt| prompt.ends_with(prefix)));
    assert_eq!(
        context["range"]["start"]["character"],
        prefix.encode_utf16().count()
    );

    server.send(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tua/inlineCompletionContext",
        "params": {
            "textDocument": {"uri": uri},
            "position": {"line": 1, "character": 8},
            "documentVersion": 1
        }
    }));
    assert!(response_with_id(server.output(), 2)["result"].is_null());
}

fn inline_context_request(id: u64, uri: &str, source: &str, needle: &str, version: u64) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tua/inlineCompletionContext",
        "params": {
            "textDocument": {"uri": uri},
            "position": lsp_position_after(source, needle),
            "documentVersion": version,
            "contextBytes": 4096
        }
    })
}

fn validate_request(id: u64, uri: &str, context: &Value, candidate: &str, version: u64) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tua/validateInlineCompletion",
        "params": {
            "textDocument": {"uri": uri},
            "position": context["position"],
            "documentVersion": version,
            "contextId": context["contextId"],
            "analysisRevision": context["analysisRevision"],
            "contextBytes": context["contextBytes"],
            "sourceFingerprint": context["sourceFingerprint"],
            "candidate": candidate
        }
    })
}

use super::*;

#[test]
fn formatting_returns_full_document_edit() {
    let temp = TempProject::new("formatting-edit");
    let source = "local value=1\n";
    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    temp.write("tua.toml", "source = \"src\"\n");

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    server.send(formatting_request(2, &uri));

    let result = response_with_id(server.output(), 2)["result"]
        .as_array()
        .expect("formatting result should be an array");

    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["range"]["start"]["line"], 0);
    assert_eq!(result[0]["range"]["start"]["character"], 0);
    assert_eq!(result[0]["range"]["end"]["line"], 1);
    assert_eq!(result[0]["range"]["end"]["character"], 0);
    assert_eq!(result[0]["newText"], "local value = 1\n");
}
#[test]
fn formatting_returns_empty_edits_when_unchanged() {
    let temp = TempProject::new("formatting-unchanged");
    let source = "local value = 1\n";
    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    temp.write("tua.toml", "source = \"src\"\n");

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    server.send(formatting_request(2, &uri));

    let result = response_with_id(server.output(), 2)["result"]
        .as_array()
        .expect("formatting result should be an array");

    assert!(result.is_empty());
}
#[test]
fn formatting_returns_null_for_parse_errors() {
    let temp = TempProject::new("formatting-parse-error");
    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    temp.write("tua.toml", "source = \"src\"\n");

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, "local value = \n"));
    server.send(formatting_request(2, &uri));

    assert_eq!(response_with_id(server.output(), 2)["result"], Value::Null);
}
#[test]
fn range_formatting_returns_local_edits_inside_selection() {
    let temp = TempProject::new("range-formatting-edit");
    let source = r#"local before=1
function draw()
local value=call(1,2)
local after=3
end
"#;
    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    temp.write("tua.toml", "source = \"src\"\n");

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    server.send(range_formatting_request(
        2,
        &uri,
        lsp_position_of(source, "local value"),
        lsp_position_of(source, "local after"),
    ));

    let result = response_with_id(server.output(), 2)["result"]
        .as_array()
        .expect("range formatting result should be an array");

    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["range"]["start"]["line"], 2);
    assert_eq!(result[0]["range"]["start"]["character"], 0);
    assert_eq!(result[0]["range"]["end"]["line"], 2);
    assert_eq!(result[0]["newText"], "  local value = call(1, ");
}
#[test]
fn range_formatting_returns_null_for_parse_errors() {
    let temp = TempProject::new("range-formatting-parse-error");
    let source = "local value = \n";
    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    temp.write("tua.toml", "source = \"src\"\n");

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    server.send(range_formatting_request(
        2,
        &uri,
        lsp_position_of(source, "local value"),
        lsp_position_after(source, "local value = "),
    ));

    assert_eq!(response_with_id(server.output(), 2)["result"], Value::Null);
}
#[test]
fn on_type_formatting_returns_local_edits_for_trigger_line() {
    let temp = TempProject::new("on-type-formatting-edit");
    let source = r#"local before=1
function draw()
local value=call(1,2)
local after=3
end
"#;
    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    temp.write("tua.toml", "source = \"src\"\n");

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    server.send(on_type_formatting_request(
        2,
        &uri,
        lsp_position_after(source, "call(1,2)"),
        ")",
    ));

    let result = response_with_id(server.output(), 2)["result"]
        .as_array()
        .expect("on-type formatting result should be an array");

    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["range"]["start"]["line"], 2);
    assert_eq!(result[0]["range"]["end"]["line"], 2);
    assert_eq!(result[0]["newText"], "  local value = call(1, ");
}
#[test]
fn on_type_formatting_ignores_unstable_triggers_and_parse_errors() {
    let temp = TempProject::new("on-type-formatting-safety");
    let source = "local value=1\n";
    let invalid = "local value = \n";
    let uri = temp.uri_for("src/main.tua");
    let invalid_uri = temp.uri_for("src/invalid.tua");
    let mut server = TestServer::new();

    temp.write("tua.toml", "source = \"src\"\n");

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    server.send(on_type_formatting_request(
        2,
        &uri,
        lsp_position_after(source, "value"),
        "x",
    ));
    server.send(did_open(&invalid_uri, invalid));
    server.send(on_type_formatting_request(
        3,
        &invalid_uri,
        lsp_position_after(invalid, "local value = "),
        "\n",
    ));

    assert_eq!(response_with_id(server.output(), 2)["result"], json!([]));
    assert_eq!(response_with_id(server.output(), 3)["result"], Value::Null);
}
#[test]
fn did_change_configuration_can_disable_default_formatting_lints() {
    let temp = TempProject::new("configuration-formatting-lints");
    let source = "local value=1   \n";

    temp.write("tua.toml", "source = \"src\"\n");

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    let diagnostics = last_published_diagnostics(server.output(), &uri);

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic["code"] == "TL4021"
            && diagnostic["message"]
                .as_str()
                .is_some_and(|message| message.contains("trailing whitespace"))
    }));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic["code"] == "TL4024"));

    server.send(did_change_configuration(json!({
        "diagnostics": {
            "formattingLints": false
        }
    })));

    let diagnostics = last_published_diagnostics(server.output(), &uri);

    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

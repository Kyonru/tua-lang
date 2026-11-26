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
fn formatting_differences_do_not_publish_lint_diagnostics() {
    let temp = TempProject::new("formatting-is-not-linting");
    let source = "return 1   \n";

    temp.write("tua.toml", "source = \"src\"\n");

    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();

    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    let diagnostics = last_published_diagnostics(server.output(), &uri);

    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn formatting_uses_shared_readable_statement_and_table_defaults() {
    let temp = TempProject::new("formatting-readable-defaults");
    let source = "local a=1; local b=2\nreturn { a = a,\n b = b }\n";
    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();
    temp.write("tua.toml", "source = \"src\"\n");
    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    server.send(formatting_request(2, &uri));
    let expected = "local a = 1\nlocal b = 2\nreturn { a = a, b = b }\n";
    assert_eq!(
        response_with_id(server.output(), 2)["result"][0]["newText"],
        expected
    );
    server.send(did_change(&uri, expected, 2));
    server.send(formatting_request(3, &uri));
    assert_eq!(response_with_id(server.output(), 3)["result"], json!([]));
}

#[test]
fn formatting_preserves_scope_after_split_compact_guard() {
    let temp = TempProject::new("formatting-split-guard");
    let source = "local function allowed(a: boolean, b: boolean): boolean\n  if not a\n    or not b then return false end\n  return true\nend\nlocal after = 1\n";
    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();
    temp.write("tua.toml", "source = \"src\"\n");
    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    server.send(formatting_request(2, &uri));
    let expected = "local function allowed(a: boolean, b: boolean): boolean\n  if not a or not b then\n    return false\n  end\n  return true\nend\nlocal after = 1\n";
    assert_eq!(
        response_with_id(server.output(), 2)["result"][0]["newText"],
        expected
    );
    server.send(did_change(&uri, expected, 2));
    server.send(formatting_request(3, &uri));
    assert_eq!(response_with_id(server.output(), 3)["result"], json!([]));
}

#[test]
fn range_formatting_uses_utf16_columns_after_non_bmp_string() {
    let temp = TempProject::new("formatting-utf16");
    let source = "local label = \"😀\"; local amount=1\nlocal after=2\n";
    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();
    temp.write("tua.toml", "source = \"src\"\n");
    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    let start = lsp_position_of(source, "local amount");
    let end = lsp_position_of(source, "local after");
    server.send(range_formatting_request(2, &uri, start, end));
    let response = response_with_id(server.output(), 2);
    let edits = response["result"].as_array().expect("format edits");
    assert!(!edits.is_empty());
    for edit in edits {
        assert_eq!(edit["range"]["start"]["line"], 0);
        assert!(edit["range"]["start"]["character"].as_u64().unwrap() >= 20);
        assert!(!edit["newText"].as_str().unwrap().contains("😀"));
        assert!(!edit["newText"].as_str().unwrap().contains("after"));
    }
}

#[test]
fn on_type_formatting_preserves_statements_when_line_counts_match() {
    let temp = TempProject::new("on-type-formatting-statement-preservation");
    let source = "local first=1; local second=2\n\n";
    let uri = temp.uri_for("src/main.tua");
    let mut server = TestServer::new();
    temp.write("tua.toml", "source = \"src\"\n");
    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    server.send(on_type_formatting_request(
        2,
        &uri,
        lsp_position_after(source, "local second=2\n"),
        "\n",
    ));
    assert_eq!(response_with_id(server.output(), 2)["result"], json!([]));
}

#[test]
fn formatting_keeps_comments_between_table_keys_and_values() {
    let temp = TempProject::new("formatting-key-comments");
    temp.write("tua.toml", "source = \"src\"\n");
    for key in ["key", "[\"key\"]"] {
        let source = format!("local t={{ {key} -- key note\n= \"value\" }}\n");
        let uri = temp.uri_for("src/main.tua");
        let mut server = TestServer::new();
        server.send(initialize_with_root(1, temp.uri()));
        server.send(did_open(&uri, &source));
        server.send(formatting_request(2, &uri));
        let edits = response_with_id(server.output(), 2)["result"]
            .as_array()
            .expect("valid commented keys should format");
        let output = edits[0]["newText"].as_str().unwrap();
        assert!(output.find(key).unwrap() < output.find("-- key note").unwrap());
        assert!(output.find("-- key note").unwrap() < output.find("\"value\"").unwrap());
    }
}

#[test]
fn formatting_lua_documents_preserves_luajit_literals_and_labels() {
    let temp = TempProject::new("formatting-luajit");
    temp.write("tua.toml", "source = \"src\"\n");
    let source = "local value=1ULL\ngoto done\n::done::\nreturn value\n";
    let uri = temp.uri_for("src/legacy.lua");
    let mut server = TestServer::new();
    server.send(initialize_with_root(1, temp.uri()));
    let mut opened = did_open(&uri, source);
    opened["params"]["textDocument"]["languageId"] = json!("lua");
    server.send(opened);
    server.send(formatting_request(2, &uri));
    let edits = response_with_id(server.output(), 2)["result"]
        .as_array()
        .expect("LuaJIT document should format");
    let output = edits[0]["newText"].as_str().unwrap();
    assert!(output.contains("local value = 1ULL"));
    assert!(output.contains("::done::"));
    assert!(tua_core::parse_lua_source("legacy.lua", output).is_ok());
}

#[test]
fn formatting_honors_project_language_for_a_lua_extension() {
    let temp = TempProject::new("formatting-typed-lua");
    temp.write("tua.toml", "source = \"src\"\nextension = \"lua\"\n");
    let uri = temp.uri_for("src/main.lua");
    let mut server = TestServer::new();
    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, "local value: number=1\n"));
    server.send(formatting_request(2, &uri));
    let edits = response_with_id(server.output(), 2)["result"]
        .as_array()
        .expect("configured typed source should format");
    assert_eq!(edits[0]["newText"], "local value: number = 1\n");
}

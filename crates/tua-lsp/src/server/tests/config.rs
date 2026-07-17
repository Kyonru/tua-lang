use super::*;

#[test]
fn config_completion_suggests_root_keys_and_sections() {
    let uri = "file:///tmp/tua.toml";
    let source = "sou";
    let output = run_messages(vec![
        did_open(uri, source),
        completion_request(2, uri, source, "sou"),
    ]);
    let labels = completion_labels(&output, 2);

    assert!(labels.contains(&"source"));
    assert!(labels.contains(&"[diagnostics]"));
}

#[test]
fn config_completion_suggests_section_keys_and_enum_values() {
    let key_uri = "file:///tmp/keys/tua.toml";
    let value_uri = "file:///tmp/values/tua.toml";
    let section_source = "[diagnostics]\nmiss";
    let value_source = "[emit]\nannotation_mode = \"";

    let key_output = run_messages(vec![
        did_open(key_uri, section_source),
        completion_request(2, key_uri, section_source, "miss"),
    ]);
    let value_output = run_messages(vec![
        did_open(value_uri, value_source),
        completion_request(3, value_uri, value_source, "annotation_mode = \""),
    ]);
    let key_labels = completion_labels(&key_output, 2);
    let value_labels = completion_labels(&value_output, 3);

    assert!(key_labels.contains(&"missing_requires"));
    assert_eq!(value_labels, ["\"off\"", "\"luacats\""]);
}

#[test]
fn config_hover_documents_schema_settings() {
    let uri = "file:///tmp/tua.toml";
    let source = "[format]\nline_width = 100\n";
    let output = run_messages(vec![
        did_open(uri, source),
        hover_request(2, uri, source, "line_width"),
    ]);

    assert_hover_contains(&output, 2, "Preferred maximum formatted line width.");
    assert_hover_contains(&output, 2, "format.line_width = 100");
}

#[test]
fn config_code_action_fixes_unknown_key_suggestions() {
    let uri = "file:///tmp/tua.toml";
    let source = "[diagnostics]\nmissing_requirez = true\n";
    let diagnostic = Diagnostic::error(
        "tua.toml",
        2,
        1,
        "unknown diagnostics config key `missing_requirez`; did you mean `missing_requires`?",
    )
    .with_code(tua_core::DiagnosticCode::CONFIG_PARSE)
    .with_range(2, 1, 2, 17);
    let output = run_messages(vec![
        did_open(uri, source),
        code_action_request(
            2,
            uri,
            1,
            0,
            1,
            17,
            vec![protocol::lsp_diagnostic_from_core(diagnostic)],
        ),
    ]);
    let action = code_action_result(&output, 2, 0);

    assert_eq!(action["title"], "Rename config key to `missing_requires`");
    assert_eq!(action["kind"], "quickfix");
    assert_eq!(
        action["edit"]["changes"][uri][0]["newText"],
        "missing_requires"
    );
}

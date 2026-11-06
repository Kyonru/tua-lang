use super::*;

#[test]
fn standard_requests_and_rename_edits_use_utf16_columns() {
    for prefix in ["ascii", "日本語", "😀", "😀😀😀😀😀😀😀😀"] {
        let uri = "file:///tmp/utf16-boundaries.tua";
        let source =
            format!("local label = \"{prefix}\"; local score: number = 7; print(score, label)\r\n");
        let output = run_messages(vec![
            did_open(uri, &source),
            hover_request(2, uri, &source, "score, label"),
            definition_request(3, uri, &source, "score, label", 1),
            rename_request(4, uri, &source, "score, label", 1, "points"),
            semantic_tokens_request(5, uri),
        ]);
        assert_hover_contains(&output, 2, "score: number");
        let declaration = lsp_position_of(&source, "score:");
        let usage = lsp_position_of(&source, "score, label");
        assert_eq!(
            response_with_id(&output, 2)["result"]["range"]["start"],
            usage
        );
        assert_eq!(
            response_with_id(&output, 3)["result"]["range"]["start"],
            declaration
        );
        let edits = response_with_id(&output, 4)["result"]["changes"][uri]
            .as_array()
            .expect("rename edits");
        assert_eq!(edits.len(), 2);
        let mut changed = source.clone();
        let mut edits = edits.clone();
        edits.sort_by_key(|edit| {
            std::cmp::Reverse(edit["range"]["start"]["character"].as_u64().unwrap())
        });
        for edit in edits {
            let start = byte_offset_for_test(&changed, &edit["range"]["start"]);
            let end = byte_offset_for_test(&changed, &edit["range"]["end"]);
            let (start, end) = (start.unwrap(), end.unwrap());
            assert_eq!(&changed[start..end], "score");
            changed.replace_range(start..end, edit["newText"].as_str().unwrap());
        }
        assert_eq!(changed, source.replace("score", "points"));
        let data = response_with_id(&output, 5)["result"]["data"]
            .as_array()
            .unwrap();
        let mut character = 0;
        let mut found = false;
        for token in data.chunks_exact(5) {
            if token[0].as_u64().unwrap() == 0 {
                character += token[1].as_u64().unwrap();
            } else {
                character = token[1].as_u64().unwrap();
            }
            if character == declaration["character"].as_u64().unwrap() {
                assert_eq!(token[2], 5);
                found = true;
            }
        }
        assert!(found, "semantic token for declaration, {prefix}");
    }
}

#[test]
fn completion_edits_and_diagnostics_use_utf16_after_non_bmp_text() {
    let uri = "file:///tmp/utf16-completion.tua";
    let source = "local label = \"😀😀😀\"; local item = { width = 2 }; print(item.wi)\n";
    let output = run_messages(vec![
        did_open(uri, source),
        completion_request(2, uri, source, "item.wi"),
    ]);
    let item = completion_with_label(&output, 2, "width");
    assert_eq!(
        item["textEdit"]["range"]["start"],
        lsp_position_of(source, "wi)")
    );
    assert_eq!(
        item["textEdit"]["range"]["end"],
        lsp_position_after(source, "item.wi")
    );

    let source = "local label = \"😀😀😀\"; print(missingValue, label)\n";
    let mut server = TestServer::new();
    server.send(did_open(uri, source));
    server.send(text_document_diagnostic_request(3, uri));
    let diagnostics = response_with_id(server.output(), 3)["result"]["items"]
        .as_array()
        .unwrap();
    let missing = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic["message"]
                .as_str()
                .unwrap_or("")
                .contains("missingValue")
        })
        .expect("missing global diagnostic");
    assert_eq!(
        missing["range"]["start"],
        lsp_position_of(source, "missingValue")
    );
    assert_eq!(
        missing["range"]["end"],
        lsp_position_after(source, "missingValue")
    );
}

fn byte_offset_for_test(source: &str, position: &Value) -> Option<usize> {
    let line = position["line"].as_u64()? as usize;
    let column = position["character"].as_u64()? as usize;
    let start = source
        .split_inclusive('\n')
        .take(line)
        .map(str::len)
        .sum::<usize>();
    let text = source.get(start..)?.split('\n').next()?;
    let mut units = 0;
    for (offset, ch) in text.char_indices() {
        if units == column {
            return Some(start + offset);
        }
        units += ch.len_utf16();
    }
    (units == column).then_some(start + text.len())
}

#[test]
fn cross_file_navigation_and_rename_use_each_documents_encoding() {
    let fixture = Fixture::parse(
        r#"-- file: tua.toml
source = "src"
-- file: src/main.tua
local note = "😀"; local module = require("provider"); print(module.value, note)
-- file: src/provider.tua
local note = "😀😀😀"; local value = 7; return { value = value, note = note }
"#,
    );
    let temp = fixture.write_to_temp("utf16-cross-file").unwrap();
    let main = fixture.source("src/main.tua");
    let provider = fixture.source("src/provider.tua");
    let uri = path_uri(&temp.join("src/main.tua"));
    let provider_uri = path_uri(&temp.join("src/provider.tua"));
    let output = run_messages(vec![
        initialize_with_root(1, path_uri(temp.path())),
        did_open(&uri, main),
        definition_request(2, &uri, main, "module.value", 7),
        rename_request(3, &uri, main, "module.value", 7, "count"),
    ]);
    let definition = &response_with_id(&output, 2)["result"];
    assert_eq!(definition["uri"], provider_uri);
    let start = byte_offset_for_test(provider, &definition["range"]["start"]).unwrap();
    let end = byte_offset_for_test(provider, &definition["range"]["end"]).unwrap();
    assert_eq!(&provider[start..end], "value");
    let changes = response_with_id(&output, 3)["result"]["changes"]
        .as_object()
        .unwrap();
    assert!(changes.contains_key(&provider_uri));
    for (edit_uri, edits) in changes {
        let text = if edit_uri == &uri { main } else { provider };
        for edit in edits.as_array().unwrap() {
            let start = byte_offset_for_test(text, &edit["range"]["start"]).unwrap();
            let end = byte_offset_for_test(text, &edit["range"]["end"]).unwrap();
            assert_eq!(&text[start..end], "value");
        }
    }
}

#[test]
fn semantic_token_delta_and_range_preserve_utf16_units() {
    let uri = "file:///tmp/utf16-token-delta.tua";
    let before = "local note = \"a\"; local score = 1; print(score, note)\n";
    let after = "local note = \"😀😀\"; local score = 1; print(score, note)\n";
    let mut server = TestServer::new();
    server.send(did_open(uri, before));
    server.send(semantic_tokens_request(2, uri));
    let first = &response_with_id(server.output(), 2)["result"];
    let id = semantic_tokens_result_id(first);
    let data = semantic_token_data(first);
    server.send(did_change(uri, after, 2));
    server.send(semantic_tokens_delta_request(3, uri, &id));
    server.send(semantic_tokens_request(4, uri));
    server.send(semantic_tokens_range_request(
        5,
        uri,
        lsp_position_of(after, "score ="),
        lsp_position_after(after, "local note = \"😀😀\"; local score"),
    ));
    let delta = &response_with_id(server.output(), 3)["result"];
    assert_eq!(
        apply_semantic_token_edits(data, semantic_token_edits(delta)),
        semantic_token_data(&response_with_id(server.output(), 4)["result"])
    );
    let data = semantic_token_data(&response_with_id(server.output(), 5)["result"]);
    assert_eq!(data.len(), 5);
    assert_eq!(data[1], lsp_position_of(after, "score =")["character"]);
    assert_eq!(data[2], 5);
}

#[test]
fn full_document_formatting_range_counts_non_bmp_final_line() {
    let source = "local label=\"😀😀\"";
    let edit = protocol::lsp_full_document_text_edit(source, "local label = \"😀😀\"\n".to_owned());
    assert_eq!(
        edit["range"]["end"],
        lsp_position_at_byte(source, source.len())
    );
    assert_eq!(
        byte_offset_for_test(source, &edit["range"]["end"]),
        Some(source.len())
    );
}

#[test]
fn invalid_empty_document_positions_do_not_overflow() {
    for position in [
        json!({ "line": u64::MAX, "character": 0 }),
        json!({ "line": 0, "character": u64::MAX }),
    ] {
        assert!(protocol::source_position_from_lsp(&position).is_none());
        assert!(protocol::source_position_for_source("", &position).is_none());
    }
    assert!(
        protocol::source_position_for_source("😀", &json!({ "line": 0, "character": 1 })).is_none()
    );
}

#[test]
fn custom_graph_asset_and_type_explanation_ranges_use_utf16() {
    let temp = TempProject::new("custom-utf16");
    temp.write("tua.toml", "source = 'src'\n");
    temp.write("src/dep.tua", "return {}\n");
    temp.write("src/assets/p.png", "placeholder");
    let uri = temp.uri_for("src/main.tua");
    let mut starts = Vec::new();
    for prefix in ["x", "😀"] {
        let source = format!("local note = '{prefix}'; local dep = require('dep'); local tex = image 'assets/p.png'; local payload: any = {{}}; local value = payload.missing\n");
        let mut server = TestServer::new();
        server.send(initialize_with_root(1, temp.uri()));
        server.send(did_open(&uri, &source));
        server.send(module_graph_request(2));
        server.send(asset_inventory_request(3));
        server.send(type_explanation_request(
            4,
            &uri,
            lsp_position_after(&source, "payload.miss"),
        ));
        let graph = &response_with_id(server.output(), 2)["result"]["graph"];
        let inventory = &response_with_id(server.output(), 3)["result"]["inventory"];
        let explanation = &response_with_id(server.output(), 4)["result"]["explanation"];
        starts.push([
            graph["edges"][0]["range"]["start"]["character"]
                .as_u64()
                .expect("module range"),
            inventory["assets"][0]["references"][0]["range"]["start"]["character"]
                .as_u64()
                .expect("asset range"),
            explanation["range"]["start"]["character"]
                .as_u64()
                .expect("explanation range"),
        ]);
    }
    for index in 0..3 {
        assert_eq!(
            starts[1][index],
            starts[0][index] + 1,
            "non-BMP prefix adds one UTF-16 unit: {starts:?}"
        );
    }
}

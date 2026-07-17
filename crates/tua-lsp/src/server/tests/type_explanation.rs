use super::*;

#[test]
fn type_explanation_reports_dynamic_origin_and_document_revision() {
    let temp = TempProject::new("type-explanation");
    let source = r#"local payload: any = {}
local value = payload.missing
"#;

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/main.tua", source);

    let uri = temp.uri_for("src/main.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
        type_explanation_request(2, &uri, lsp_position_after(source, "payload.miss")),
    ]);
    let response = response_with_id(&output, 2);
    let result = &response["result"];
    let explanation = &result["explanation"];

    assert_eq!(result["documentVersion"], 1);
    assert!(result["analysisRevision"].as_u64().is_some());
    assert_eq!(explanation["expression"], "payload.missing");
    assert_eq!(explanation["type"], "any");
    assert_eq!(explanation["isAny"], true);
    assert!(explanation["steps"]
        .as_array()
        .expect("explanation steps should be an array")
        .iter()
        .any(|step| { step["kind"] == "dynamic" && step["title"] == "Explicit `any` annotation" }));
    assert!(explanation["steps"]
        .as_array()
        .expect("explanation steps should be an array")
        .iter()
        .any(|step| step["detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("receiver is `any`"))));
}

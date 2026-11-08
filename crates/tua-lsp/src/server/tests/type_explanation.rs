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

#[test]
fn protocol_members_flow_through_inlay_hints_and_type_explanation() {
    let temp = TempProject::new("protocol-editor-type-surfaces");
    let contracts = r#"export protocol Updatable {
  update(dt: number): nil
}
"#;
    let source = r#"import type { Updatable } from "./contracts"
local function tick(value: Updatable, dt: number)
  local current = value
  current:update(dt)
end
"#;
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write("src/contracts.tua", contracts);
    temp.write("src/main.tua", source);
    let uri = temp.uri_for("src/main.tua");
    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        did_open(&uri, source),
        inlay_hint_request(2, &uri, 0, 0, 5, 0),
        type_explanation_request(3, &uri, lsp_position_after(source, "current:update")),
    ]);
    let hints = response_with_id(&output, 2)["result"]
        .as_array()
        .expect("inlay hints should be an array");
    let labels = inlay_hint_labels(hints);
    assert!(
        labels.iter().any(|label| label.contains("Updatable")),
        "inferred local hints should preserve the imported protocol type: {labels:?}"
    );

    let explanation = &response_with_id(&output, 3)["result"]["explanation"];
    assert_eq!(explanation["expression"], "current.update");
    assert!(explanation["type"]
        .as_str()
        .is_some_and(|ty| ty.contains("number")));
}

use super::*;

#[test]
fn direct_require_exports_preserve_types_and_editor_fields() {
    for access in ["require(\"items\").items", "require(\"items\")[\"items\"]"] {
        let fixture = Fixture::parse(&format!(
            r#"-- file: src/items.tua
export type Item = {{ id: string, count: number }}
local items: Item[] = {{ {{ id = "rifle", count = 1 }} }}
return {{ items = items }}
-- file: src/main.tua
local items = {access}
local item = items[1]
local _wrong: number = item.id
print(item.<|complete|>id)
print(<|hover|>item.id)
"#
        ));
        let mut host = AnalysisHost::new();
        host.set_source_roots([PathBuf::from("src")]);
        for file in fixture.files() {
            host.set_file(&file.path, file.source.clone());
        }
        let snapshot = host.snapshot();
        let diagnostics = snapshot.pipeline_diagnostics("src/main.tua");
        assert!(
            diagnostics
                .iter()
                .any(|d| d.message.contains("expected number, found string")),
            "{access}: {diagnostics:?}"
        );
        let labels =
            completion_labels(&snapshot.completion("src/main.tua", fixture.position("complete")));
        assert!(labels.contains(&"count".to_owned()), "{access}: {labels:?}");
        let valid = fixture
            .source("src/main.tua")
            .replace("_wrong: number", "_wrong: string");
        let hover = snapshot
            .hover("src/main.tua", fixture.position("hover"))
            .unwrap();
        assert!(format!("{hover:?}").contains("id"), "{hover:?}");
        host.set_file("src/main.tua", valid);
        assert!(!host
            .snapshot()
            .pipeline_diagnostics("src/main.tua")
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Error));
        host.set_file(
            "src/items.tua",
            fixture
                .source("src/items.tua")
                .replace("id: string", "id: number")
                .replace("id = \"rifle\"", "id = 4"),
        );
        assert!(host
            .snapshot()
            .pipeline_diagnostics("src/main.tua")
            .iter()
            .any(|d| d.message.contains("expected string, found number")));
    }
}

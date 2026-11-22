use super::*;

#[test]
fn forwarded_save_callbacks_keep_imported_results_and_editor_types() {
    let fixture = Fixture::parse(
        r#"-- file: src/storage.tua
local function create()
  local contract = Save.contract({ slot = "profile", version = 1,
    schema = Save.record({ count = Save.number(0) }) })
  return { load = function() return contract:load() end }
end
return { create = create }
-- file: src/main.tua
local storage = require("storage")
local adapter = storage.create()
local saved, issue = adapter.load()
if saved then
  local _bad: string = saved.count
  print(saved.<|complete|>count)
end
if issue then
  print(<|hover|>issue.recoverable)
  local _bad: number = issue.recoverable
end
"#,
    );
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
            .any(|d| d.message.contains("expected string, found number")),
        "{diagnostics:?}"
    );
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("expected number, found boolean")),
        "{diagnostics:?}"
    );
    let labels =
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("complete")));
    assert!(labels.contains(&"count".to_owned()), "{labels:?}");
    let hover = snapshot
        .hover("src/main.tua", fixture.position("hover"))
        .unwrap();
    assert!(format!("{hover:?}").contains("SaveIssue"), "{hover:?}");
}

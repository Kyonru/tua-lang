use super::*;

#[test]
fn imported_module_names_do_not_override_inner_locals() {
    let fixture = Fixture::parse(
        r#"-- file: src/progression.tua
return { initialize = function(): boolean return true end }
-- file: src/element.tua
local function get(): { fireBonus: number } return { fireBonus = 1 } end
return { get = get }
-- file: src/main.tua
local profile = require("progression")
local elements = require("element")
local function use(value: { fireBonus: number }): number return value.fireBonus end
local function launch(): number
  local profile = elements.get()
  print(profile.<|complete|>fireBonus)
  print(<|hover|>profile.fireBonus)
  return use(profile)
end
local _value: number = launch()
local _started: boolean = profile.initialize()
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
        !diagnostics
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Error),
        "{diagnostics:?}"
    );
    let labels =
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("complete")));
    assert!(labels.contains(&"fireBonus".to_owned()), "{labels:?}");
    assert!(!labels.contains(&"initialize".to_owned()), "{labels:?}");
    assert!(snapshot
        .hover("src/main.tua", fixture.position("hover"))
        .is_some());
    host.set_file(
        "src/main.tua",
        fixture.source("src/main.tua").replace(
            "return use(profile)",
            "local _bad: string = profile.fireBonus\n  return use(profile)",
        ),
    );
    assert!(host
        .snapshot()
        .pipeline_diagnostics("src/main.tua")
        .iter()
        .any(|d| d.message.contains("expected string, found number")));
    host.set_file(
        "src/main.tua",
        fixture
            .source("src/main.tua")
            .replace("print(profile.fireBonus)", "print(profile.)"),
    );
    let labels = completion_labels(
        &host
            .snapshot()
            .completion("src/main.tua", fixture.position("complete")),
    );
    assert!(labels.contains(&"fireBonus".to_owned()), "{labels:?}");
    assert!(!labels.contains(&"initialize".to_owned()), "{labels:?}");
}

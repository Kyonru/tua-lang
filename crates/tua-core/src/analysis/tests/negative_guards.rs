use super::*;

#[test]
fn negative_else_guards_preserve_imported_returns_and_editor_fields() {
    let fixture = Fixture::parse(
        r#"-- file: src/points.tua
export type Point = { x: number }
local function select(a: Point | nil, b: Point | nil)
  if not a or not b then return nil else return { first = a, second = b } end
end
return { select = select }
-- file: src/main.tua
local points = require("points")
import type { Point } from "points"
local function sum(a: Point | nil, b: Point | nil): number
  if not a or not b then return 0 else
    local _bad: string = a.x
    local _x = a.<|complete|>x
    return <|hover|>b.x
  end
end
local result = points.select({ x = 1 }, { x = 2 })
if result then
  local _bad: string = result.second.x
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
    assert_eq!(
        diagnostics
            .iter()
            .filter(|d| d.message.contains("expected string, found number"))
            .count(),
        2,
        "{diagnostics:?}"
    );
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == Some(DiagnosticCode::TYPE_UNSAFE_UNION_OPERATION)),
        "{diagnostics:?}"
    );
    let labels =
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("complete")));
    assert!(labels.contains(&"x".to_owned()), "{labels:?}");
    let hover = snapshot
        .hover("src/main.tua", fixture.position("hover"))
        .unwrap();
    assert!(!format!("{hover:?}").contains("nil"), "{hover:?}");
}

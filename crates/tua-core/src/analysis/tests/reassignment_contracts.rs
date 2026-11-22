use super::*;

#[test]
fn nested_return_contracts_survive_reassignment_and_editor_queries() {
    for annotation in ["", ": Quote"] {
        let fixture = Fixture::parse(&format!(
            r#"-- file: src/types.tua
export type Modifier = {{ operation: "flat" | "percent" | "multiply" }}
export type Quote = {{ preview: {{ output: {{ breakdown: {{ sources: Modifier[] }}[] }} | nil }} }}
export type Owner = {{ quote: () -> Quote, confirm: (Quote) -> boolean }}
return {{}}
-- file: src/owner.tua
import type {{ Quote, Owner }} from "types"
local function quote(): Quote
  return {{ preview = {{ output = {{ breakdown = {{ {{ sources = {{ {{ operation = "flat" }} }} }} }} }} }} }}
end
local function confirm(value: Quote): boolean return value.preview.output ~= nil end
local function new(): Owner return {{ quote = quote, confirm = confirm }} end
return {{ new = new }}
-- file: src/main.tua
local owners = require("owner")
import type {{ Quote }} from "types"
local owner = owners.new()
local quote{annotation} = owner.quote()
assert(owner.confirm(quote))
quote = owner.quote()
assert(owner.confirm(<|hover|>quote))
local output = quote.preview.output
if output then
  local modifier = output.breakdown[1].sources[1]
  print(modifier.<|complete|>operation)
end
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
            !diagnostics
                .iter()
                .any(|d| d.severity == DiagnosticSeverity::Error),
            "{diagnostics:?}"
        );
        let hover = snapshot
            .hover("src/main.tua", fixture.position("hover"))
            .unwrap();
        assert!(format!("{hover:?}").contains("flat"), "{hover:?}");
        let labels =
            completion_labels(&snapshot.completion("src/main.tua", fixture.position("complete")));
        assert!(labels.contains(&"operation".to_owned()), "{labels:?}");
        host.set_file(
            "src/main.tua",
            fixture.source("src/main.tua").replace(
                "print(modifier.operation)",
                "modifier.operation = \"invalid\"",
            ),
        );
        let diagnostics = host.snapshot().pipeline_diagnostics("src/main.tua");
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == Some(DiagnosticCode::TYPE_MISMATCH)),
            "{diagnostics:?}"
        );
    }
}

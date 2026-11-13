# Prioritized improvements from the three-game trial

This is a proposed order, not an implementation record. Severity reflects observed impact in these small games; effort is a rough engineering estimate pending source-level design. Each item links to the minimal evidence in [FINDINGS.md](FINDINGS.md). No product fix was made during this audit.

| Order | Work | Type / effort | Acceptance criteria |
| --- | --- | --- | --- |
| 1 | Prevent constructor flattening from discarding statements after inline `super()` (GAME-08). | Correctness; small–medium | Inline and multiline constructors produce the same field values at Lua runtime, with parent args and duplicate writes covered. The optimization may fall back to the init path when uncertain. |
| 2 | Establish one shader asset path contract across editor, build, and LÖVE runtime (GAME-02). | Correctness/docs; medium | A documented path from a `source = "src"` project checks, builds, loads, includes, and reloads in LÖVE. Invalid source-only paths receive a diagnostic or are translated consistently. Bundled example is runnable. |
| 3 | Preserve nominal class/protocol identity through imports, inferred constructors, and generic bounds (GAME-01, GAME-03). | Shared type infrastructure; medium–large | Imported subclass locals assign to base types regardless of unrelated protocol use; imported constructor and one-hop factory method calls check without redundant annotations; wrong-class negative controls still fail. Hover/completion/check agree. |
| 4 | Carry SignalBus event schemas through exported fields and aliases (GAME-04, GAME-05). | Shared semantic infrastructure; medium | Wrong imported payloads fail both direct-property and local-alias calls; signature help shows the same inferred parameter type; edit/recovery updates the schema. |
| 5 | Preserve receiver types during incomplete imported-generic member edits (GAME-06). | LSP/parser recovery; medium | `jar:` offers `Jar<Potion>` members after typing the colon, while malformed syntax still reports diagnostics; completed call and unsaved provider edits continue to work. |
| 5a | Make nested imported generic-field hover agree with completion (GAME-12). | Shared editor semantics; small–medium | Hover on `Game.selected.value` shows the specialized instance type and a precise field range; completion and inferred-local hover remain correct after unsaved edits. |
| 6 | Correct generic-alias formatter indentation (GAME-07). | Formatter; small | The following top-level `return` remains unindented, `fmt` is idempotent, and generated Lua is unchanged. |
| 7 | Type stable nested audio snapshot fields and document their shape (GAME-09). | Type/docs; small–medium | `.music.state` completes and hovers correctly; `.musicState` at snapshot top level fails checking; runtime behavior remains unchanged. |
| 8 | Reduce pool warning noise for explicit ECS ownership handoff (GAME-10). | Diagnostic usability; medium/design | A documented handoff pattern avoids TL5007 while a forgotten release still warns; no unsafe alias inference is required. |
| 9 | Specify exported bounded instance-alias syntax (GAME-11). | Design/docs; small if docs only | Docs show a checked, runnable cross-file bounded-class pattern; any new syntax has parser/checker/LSP tests. |

## Qualification work before changing latency targets

The game trials measured CLI and LSP protocol requests on small projects, but did not capture trigger → client send → server response → widget paint. Measure that full path in an installed VS Code host on these fixtures and a larger game before defining an editor latency target. Recheck large graphs and long inspector sessions separately; three small games do not establish their performance.

The trial game reports also list feature-specific limits and design questions. Keep those as follow-up qualification, not as confirmed defects.
The maze's second-format-pass observation (GAME-13) needs a preserved input before entering the correctness queue.

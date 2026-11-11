# Recheck of November 7 language and toolchain findings

This is a recheck against the November 11 checkout, not a rerun of the original
0.6.0 audit. Historical cases live under
`internal/audits/features-2026-11-07/`; all execution below used temporary
copies of those inputs. The Lua oracle was PUC Lua 5.4, not LuaJIT or LÖVE.
The revision for the first CLI pass was `69e03d1a`. The ECS LSP case still
reproduced with a later rebuilt binary before `82b8674f`; the absolute-path
core regression, real LSP regression, and rebuilt stdio probe passed after
that fix. The final post-edit workspace gate is recorded in VERIFICATION.md.

## Executed rechecks

For each named fixture below, copy the historical case directory to a fresh
temporary directory, omitting `build`, `dist`, `.tua`, `*.log`, and `*.json`.
Run the current `target/debug/tua check` and `target/debug/tua build` with
that directory as the working directory. For runtime rows, run
`lua build/main.lua` there. The probes and their exact captured stdout/stderr
are in `/tmp/tua-prior-language-recheck.json` for this machine; the decisive
outcomes are reproduced here so that this report stands on its own.

| Finding | Current classification | Current evidence |
| --- | --- | --- |
| TYPING-01 | Fixed and rechecked | `typing/cases/dictionary_runtime`: check/build exit 1, TL1003 rejects `scores.missing` because `number | nil` was expected and `string` was found. |
| TYPING-02 | Fixed and rechecked | `typing/cases/multi_assignment`: check/build exit 1, TL1003 rejects `s`: expected `string`, found missing RHS value `nil`. |
| TYPING-03 | Fixed and rechecked | `typing/cases/callback_short`: check/build exit 0; short callback accepted. |
| TYPING-04 | Fixed and rechecked | `typing/cases/optional_dot`: check/build exit 0; nil assignment accepted. |
| FUNCTIONS-01 | Fixed and rechecked | `functions/bad_return`: check/build exit 1, TL1003 rejects `n`: expected number, found string. |
| FUNCTIONS-02 | Fixed and rechecked | `functions/bad_default`: check/build exit 1, TL1003 identifies default for parameter `x`, expected number, found string. |
| FUNCTIONS-03 | Fixed and rechecked | `functions/default_nil`: check/build exit 0; Lua exits 0 and prints `7\t7\t0`. |
| CLASS-01 | Fixed and rechecked | `classes/self-reference`: check/build exit 0; Lua exits 0 and prints `ok`. The emitted constructor's self reference works. |

Additional isolated probes used `TemporaryDirectory` and
`target/debug/tua` (captured in `/tmp/tua-prior-boundaries.json`):

| Finding | Current classification | Current evidence |
| --- | --- | --- |
| FMT-01 | Fixed and rechecked | `local s = [[a  \nb]]; print(#s)` prints `5` before formatting. `tua fmt` exits 0 and preserves the two trailing spaces in the long-string body; rebuilt output still prints `5`. |
| PROJECTS-01 | Fixed and rechecked | With `source="src"`, `out="src"`, `src/types.d.tua`, and handwritten `src/types.d.lua`, `tua build` exits 0 (`built 0 file(s)`), and the Lua file remains byte-for-byte unchanged. |
| DBG-01 | Fixed and rechecked | Runtime error `lua: build/nested dir/crash.lua:1: boom` maps through `tua traceback --maps build` to `src/nested dir/crash.tua:1`; exit 0. |

## Editor and cross-file recheck

The historical `modules/lsp.js` and `domains`/`declarations` fixture trees
were copied to a temporary directory. Only the script's repository constant
and server path were changed to point to this checkout and
`target/debug/tua`. From the repository root, the equivalent command was
`node /tmp/<temporary-directory>/lsp.js declarations`, then `domains`.
Both processes exited 0. This is an actual stdio LSP request, not a source
inspection or installed VS Code test.

| Finding | Current classification | Current LSP response |
| --- | --- | --- |
| MODULES-01 | Fixed in the probed binary | In `domains`, `direct = require("game").game` hovers `Store`, `ds` hovers `{ paused: boolean, score: number }`, `inline = require("normal").inc` hovers `(x: number) -> number`; `direct:get()` signature is typed. |
| MODULES-03 | Fixed in the probed binary | In `declarations`, `F.echo` hover and signature both say `(value: string): string`; `result` hover says `string` with real `src/lib/feel.lua` and mapped declaration both present. |
| MODULES-02 | **Fixed and rechecked during this audit** | Before the fix, the rebuilt server reported `position: any | nil` and `x: any` for `world:get(e, C.Position)`. The checker already rejected `p.x = "wrong"` with expected number. After the shared core fix, a new absolute-path core regression and a real LSP regression pass; the rebuilt stdio server reports `position: { x: number } | nil`. |

The MODULES-02 input minimizes to:

```tua
-- src/components.tua
local Position = ecs.component("Position", { x = "number" })
return { Position = Position }

-- src/main.tua
local C = require "components"
local world = ecs.world()
local e = world:create()
local p = world:get(e, C.Position)
print(p)
```

with `tua.toml` containing `source="src"` and `out="build"`. Hover on `p`
from `textDocument/hover` reports `local p: any | nil` after 1.2 or five
seconds, including when the provider file is also opened. In the same
temporary project, appending `p.x = "wrong"` then running
`target/debug/tua check` exits 1 with
`TL1003: type mismatch at p.x: expected number, found string`.
This established a compiler/editor discrepancy. The root was two-part:
`project_type_for_function_call` only considered imported/project local maps
for the receiver; a locally created `world` was absent from those maps under
an absolute project root. It now falls back to the file's base semantic
environment. The LSP fast hover also returned the old imprecise local type
before the project snapshot could refine the imported argument. It now
defers only an imprecise local whose initializer AST references a static
require alias. An unrelated `any` local still uses the fast path.

After the final edit, `cargo test -p tua-core -p tua-lsp imported_ecs_component
-- --nocapture` exited 0: two core tests and one LSP test passed. The new
tests are `absolute_project_imported_ecs_component_retains_payload_schema`
in `crates/tua-core/src/analysis/tests.rs` and
`imported_ecs_component_result_hover_keeps_schema_in_project` in
`crates/tua-lsp/src/server/tests/hover.rs`. `cargo build -p tua-cli` exited
0. A fresh disposable stdio probe using the copied `modules/lsp.js` and
the rebuilt `target/debug/tua` exited 0 and returned
`local position: { x: number } | nil`. `cargo fmt --all -- --check` and
`git diff --check` both exited 0. No installed VS Code host was exercised
in this lane.

## Inspected-only or policy findings

| Finding | Current classification | Evidence and limit |
| --- | --- | --- |
| LINTER-01 | Source/test rechecked; execution pending full gate | `crates/tua-core/src/code_action/tests.rs` has `unused_local_removal_preserves_adjacent_statements`, covering semicolons, comments, Unicode, quickfix and fix-all. The original risk was a safe edit deleting a neighboring call. This pass did not rerun the LSP edit application. |
| LINTER-02 | Design question remains | Organizing/removing imports can reorder or remove module initialization. It is an explicit source action, and the old report did not establish a correctness contract. No policy choice was made in this pass. |
| API-MCP-01 | Source/test rechecked; execution pending full gate | `crates/tua-cli/src/api.rs` has `refresh_preserves_monotonic_analysis_revisions`. |
| API-MCP-02 | Source/test rechecked; execution pending full gate | `crates/tua-cli/src/api.rs` has `diagnostics_include_new_unsaved_overlay_files`. |
| API-MCP-03 | Source/test rechecked; execution pending full gate | `crates/tua-cli/src/api.rs` has `protocol_envelopes_and_notifications_are_consistent`. |
| SETUP-DOCS-01 | Superseded by current README | `editors/vscode/README.md` now says the setup offers theme, layout, file icons, **shader-tool checks**, and inlay hints. The historical claim that the shader step is omitted no longer applies. The present concise README does not enumerate each optional registry action; the detailed editor reference remains the destination for setup details. |

The concurrent `make ci-check` result belongs in the main verification log.
No result from that gate is inferred here. A targeted Cargo test attempt was
canceled after waiting for the shared build lock, so it is **not** a pass.
This lane changed the shared core inference and fast hover path plus focused
regression tests. Historical fixtures, the Git index, and commits were not
changed by this lane; the lead owns integration and staging.

# Documentation, examples, and test-oracle follow-up

Baseline: `c08edbca`; reviewed current docs and CI on 2026-11-11. This is a bounded follow-up to the 60-row audit ledger, not a claim that all documented examples or feature interactions were executed. Prior November 7 reports were treated as historical evidence and checked against current files before carrying a concern forward.

## Commands and outcomes

| Command | Outcome | What it establishes |
| --- | --- | --- |
| `make feature-examples-check docs-build` | Pass: 13 mapped guides, 14 mapped tour files; Zensical build reported no issues | Navigation mapping and docs build on this checkout. It does not execute example behavior. Repeated after the wording correction. |
| `python3 internal/audits/feature-and-preview-2026-11-11/repro-feature-check-oracle.py` | Exit 0 while all 14 synthetic mapped files contain `local = ???` | The named target checks inventory/import strings and first-line descriptions, not Tua syntax or buildability. It edits only an automatically removed temporary tree. |
| Temporary copy of `examples/features`, `target/release/tua build`, then `lua -e 'package.path = "build/?.lua;" .. package.path; local example = require("behavior_tree_ecs"); assert(example.shots >= 2); assert(example.flashes:activeCount() == 0); print("shots=" .. example.shots)'` | Build 0; runtime 0; `shots=2` | The behavior-tree × ECS × pool example actually runs headlessly in this local Lua runtime. This is not a LuaJIT/LOVE result or evidence that CI runs it. |

## DOC-01 — Feature-example check description promised buildability

**Category:** documentation/tooling wording · **Severity:** Low · **Confidence:** High · **Status:** confirmed and wording corrected locally; product behavior unchanged.

**Expected versus actual:** `Makefile:135` described `feature-examples-check` as ensuring every Features guide has a “buildable” file. The script at `scripts/check-feature-examples.js:8-37` compares navigation, mapping, first-line comment, and `main.tua` import strings. It never calls Tua. The synthetic reproduction script writes invalid Tua into every mapped file and still prints `validated 13 feature guides against 14 feature-tour source files`. `docs/examples.md:42-44` could also leave readers thinking that command compiled the mapped tour.

**User impact and root cause:** A contributor running the named target alone could treat its green result as a compilation check. Trusted CI separately builds `examples/features` at `.github/workflows/ci.yml:132-134`, and the release workflow invokes CI at `.github/workflows/release.yml:45-48`; this finding is not an unguarded release-path syntax defect. The local `make verify` includes `cargo test`, whose `lua_syntax` integration may compile features when a Lua runtime is found, but that does not make this standalone target a build check.

**Correction and verification:** `Makefile` now calls it a guide-to-example mapping/import check. `docs/examples.md` distinguishes that check from `make build-features`. `make feature-examples-check docs-build` passed after the wording edit. No test is needed for a description-only correction; the synthetic script remains a useful oracle if the target's contract is later expanded.

## TEST-GAP-01 — Cross-feature gameplay example is built but not run in CI

**Category:** test-oracle gap · **Priority:** Medium · **Confidence:** High for the missing CI execution, not a confirmed runtime defect.

`docs/behavior-trees.md:81-89` links `examples/features/src/behavior_tree_ecs.tua` as a runnable example combining a shared tree, ECS enemies, and pooled effects. That file contains a headless 20-tick simulation and assertions for shots and pool release near its end. It is not imported by `examples/features/src/main.tua:1-37` and is not in `examples/features/coverage.json`, whose behavior-tree row maps only `src/behavior_tree.tua`. CI builds all feature files at `.github/workflows/ci.yml:132-134` and runs bundled runtime unit tests at lines 111-113, but no CI command invokes this combined example. Repository search found its references only in the behavior-tree guide and feature README; no dedicated Rust/JS integration test names it.

**Escaped defect class:** an ECS × behavior-tree × pool interaction could keep compiling while changing runtime semantics, callback order, entity reuse, or release behavior. The current local run passed (`shots=2`, active pool count zero), so the finding concerns future regression detection.

**Smallest useful oracle:** add a focused CLI integration test or tiny headless shell/Python runner that builds this exact example in a temporary project and requires the generated Lua with a pinned LuaJIT in Linux CI. Keep the two existing assertions and add one deterministic assertion for effect reuse or callback order if the interaction contract requires it. Do not make the whole feature-tour entry point the runtime oracle: many tour modules are meant to demonstrate editor/static behavior rather than execute without LOVE. Place the new check in the existing `cargo test --workspace` Linux LuaJIT path or as an explicit CI step after the feature build, and fail if LuaJIT is missing rather than skip there.

## TEST-GAP-02 — Graph protocol and rendered view are not joined in one oracle

**Category:** integration coverage gap · **Priority:** Medium · **Confidence:** High for the original gap; now covered by a bounded real-server oracle.

`crates/tua-lsp/src/server/tests/navigation.rs:104-164` checks Signal Graph protocol fields and source ranges. `editors/vscode/test/suite/ecs-explorer.js:14-38` checks that the command requests an ECS graph and opens/reuses a tab. `editors/vscode/test/webview/suite.js:215-250` renders populated ECS/pool/signal graphs from hand-authored raw response objects and checks relationships and source actions. Initially these layers were separate, leaving a response-to-view mismatch untested.

**New oracle:** `editors/vscode/test/real-server/run.js` now adds one source fixture with a component/system, pool acquire/release, and declared/listened/emitted signal. `editors/vscode/test/real-server/suite/graph-previews.js` opens the three graph commands against the real server, inspects their rendered DOM through the existing CDP helper, and clicks a source action in each view. It verifies the source URI and line against the fixture after first moving the source cursor elsewhere. `TUA_REAL_SERVER_PATH=$PWD/target/release/tua npm --prefix editors/vscode run test:vscode-real` passed in the installed VS Code host on macOS; output included `REAL_SERVER_GRAPH_PREVIEWS ECS, pool, and Signal render and navigate to source` and exit code 0. This test belongs to the existing Linux extension-host CI command at `.github/workflows/ci.yml:162-167` but has not yet been observed in that CI environment.

**Limit:** the test does not directly capture each raw `tua/*Graph` JSON response, so it establishes real-server-to-rendered-data and rendered-action-to-source behavior for this fixture, not a field-by-field protocol equivalence proof or cross-file graph behavior. The protocol tests remain the oracle for exact response ranges.

## Documentation and gate conclusions

The sampled feature guides for stores, signals, ECS, and behavior trees distinguish static typing from runtime behavior and explain opt-in helpers and limitations. The behavior-tree/ECS/pool example's “runnable” claim passed locally. `make docs-build` reported no issues. No additional current documentation discrepancy was confirmed in this bounded pass. Existing prior findings should remain open or closed only after their exact reproduction is rerun on the current revision.

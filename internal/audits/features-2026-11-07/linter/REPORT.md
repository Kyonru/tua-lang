# Linter, diagnostics and quick-fix audit

Candidate 0.6.0, baseline revision 9d490dc. One confirmed High defect; one separate import-policy design question. No product files changed. Used tua-audit-lead, tua-language-auditor and tua-lsp-parity guidance. Contracts inspected: docs/linter.md, docs/configuration.md suppression/severity sections, docs/lsp.md code actions; implementation and existing code-action tests. No standalone diagnostics guide exists under docs; configuration and linter pages supply that contract.

## Execution and coverage

Run `node internal/audits/features-2026-11-07/linter/probe.js` from repository root. This uses the absolute `target/release/tua` binary, actual stdio LSP, and `/opt/homebrew/bin/lua` (5.4.8). Exact inputs/configuration and applied edit copies are in case subdirectories; full protocol responses and CLI exits/output in evidence.json; concise run log in probe.log. original-runtime.json preserves original program output. policy-cli.json records four additional CLI checks. The harness applies LSP edits using UTF-16 JavaScript string offsets. One initial harness brace syntax error was corrected before execution and is not a product finding.

| Area | Executed result / limit |
| --- | --- |
| Standard rules | TL4005 unused local, TL4004 unknown global, TL4045 uninitialized read, TL4055 unused function, TL4054 unused import, TL4066 empty function exercised. Messages identify binding and unsafe read; ranges correct in these cases. Not exhaustive rule coverage. |
| CLI policy | Warning lint exit 1 versus check exit 0; rules=false suppresses standard check/editor diagnostics while lint still runs them; allow/info severity overrides agree across CLI/editor; info-only lint exits 0. Unknown severity code/level reject configuration. `lint --strict` smoke exercised, not every optional rule. |
| Suppressions | Header, same-line and valid next-line suppressions work. Misplaced next-line directive correctly reports unused suppression TL4026. Disabled suppression offers no suppression fixes; file-only scope offers file suppression only. Applied one line-suppression copy and reran lint: only the targeted warning disappears. Exhausted max-count behavior inspected, not separately exercised. |
| Unicode | Astral emoji before same-line declaration: identifier range is correct UTF-16 (19..25). Edit deletion defect below is independent of encoding. |
| Safe fixes | Separate-line pure unused local removed; effectful print initializer retained and runtime output preserved. Multiline unused function removed. Same-line function removal preserves neighboring print. Same-line local removal fails below. |
| Imports | Static require bindings organized/removed; actual Lua execution demonstrates ordering and side-effect consequences below. Dynamic/shadowed preservation inspected in existing tests, not freshly executed. |
| Freshness | All twelve cases changed in memory to `print("fresh 😀")`; immediate actions requested with stale prior diagnostics return []; fresh diagnostics clear. |
| Incomplete buffer | `local bad =` reports TL0001, no quickfix/fix-all; changing to valid unsaved text clears diagnostic. |
| Limits | No desktop VS Code automation, LuaJIT run, compiler-generated runtime comparison, all-rule enumeration, rapid cancellation stress or workspace-scale freshness test. Lua-compatible inputs run directly under Lua. Lead baseline reports 389 LSP tests passing; this lane did not rebuild/run suites. |

## LINTER-01 — Safe unused-local fix removes independent statements on the same line

Severity: **High**. Category: **confirmed defect**. Status: reproduced, new relative to searched prior audits/ROADMAP.

Reproduction: open `same-line/src/main.tua` (`local unused = 1; print("keep")`) in the actual LSP; request `textDocument/codeAction` with `context.only=["source.fixAll"]` (or quickfix) and the published TL4005. Full requests are reproducible through probe.js; full response in evidence.json case `same-line`.

Expected: delete only the unused declaration, preserving `print("keep")`, or offer no removal when a safe boundary cannot be established. Actual `Fix all safe diagnostics` and `Remove unused local` edit replace range `{start:{line:0,character:0},end:{line:1,character:0}}` with the empty string. Original runtime prints `keep`; applied `same-line/source-fixAll-0.lua` is empty and prints nothing. The `unicode` case reverses statement order (`print("😀"); local unused = 1`) and also loses the print. This is silent behavior loss through an explicitly safe action.

Source: crates/tua-core/src/code_action/mod.rs:771 (unused-local action), :2224 (remove_single_line_stmt_edit checks only statement start/end line and then removes the whole line), :582 (fix-all aggregates these fixes). docs/linter.md:364 and docs/lsp.md:444 describe safe removal. The existing span-aware `remove_statement_span_edit` at :2238 already preserves surrounding source for function removal.

Test blind spot: crates/tua-lsp/src/server/tests/code_actions.rs:406 and :744 exercise isolated-line unused locals; tests do not exercise another statement before/after the removable local. Prior devex audit's successful removal example does not cover this layout. No matching tracked finding found in ROADMAP or prior devex reports.

Smallest improvement: make unused-local removal use a validated statement span (including safe separator handling), expanding to full-line deletion only when it owns the line. Reuse the existing statement-removal primitive where appropriate. Regression: quickfix and source.fixAll with neighboring calls before/after, semicolon and comment variants, and Unicode; assert edits preserve the call and run applied output. Include self-assignment users of the same removal helper if affected (not freshly reproduced here).

## LINTER-02 — Explicit import cleanup has observable initialization consequences

Severity: **Medium**. Category: **design question**, not counted as a defect. Status: demonstrated behavior; policy decision required; no known prior tracking found.

Reproduction: `imports/src/main.tua` requires z, a, effect; each module prints its name. Original runtime: `z`, `a`, `effect`, then `A Z`. Applying source.removeUnusedImports drops `effect`; source.organizeImports additionally changes initialization order to `a`, `z`. Exact edits, source copies and runtime output are in evidence.json and original-runtime.json. source.fixAll correctly offers no import removal.

Source: crates/tua-core/src/code_action/mod.rs:2411 rewrites contiguous import blocks, :2446 removes unused bindings and :2450 sorts; :2470 checks unused/comment/underscore status, not module effects. docs/lsp.md:450 promises organizing/removing static imports but does not promise preserved module initialization. This is an explicit user-requested action, so observed behavior alone does not establish a violated contract.

Decision/improvement: document that import removal can discard initialization and sorting can reorder it, or define a conservative policy that retains bare require calls for runtime effects. Type-only imports could have a distinct safe-removal policy. Regression/coverage: a runtime side-effect fixture should lock in the chosen semantics; current source-action test at crates/tua-lsp/src/server/tests/code_actions.rs:594 asserts rewritten text only.

## Missing actions/options and coverage

No new missing feature claimed. Safe removal is intentionally unavailable for effectful local initializers; multi-binding and nested-local action breadth was inspected only. Exact-code suppression and source import actions exist. This bounded pass does not establish that every warning has its ideal specialized quick fix.

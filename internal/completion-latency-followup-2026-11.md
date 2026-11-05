# Cold and incomplete completion follow-up — November 2026

The incomplete-edit regression is recovered in the zombie replay: median
completion is 24.1 ms, compared with 63.0 ms before this pass and 41.7 ms in the
older pre-audit baseline. Cold completion improves from 432.5 to 276.6 ms but
remains an open bottleneck. This is protocol latency, not measured VS Code
key-to-menu latency.

## Changes

- `68ec329`: reuse the recovered type environment for incomplete-file checking,
  retaining environment errors, body errors and configured ECS behavior.
- `2c4a82d`: defer full checking/linting until a diagnostic query. Cache results
  with immutable file analysis; completion leaves the checker uninitialized,
  edits get fresh results and older snapshots retain their original results.
- `9e07411`: use the existing shared built-in alias catalog for identity checks.
  Previously each lookup rebuilt every domain's types. Repeated class and
  module-export inference made this allocation visible in typing latency.

The existing shared analysis and type registry own these changes. No diagnostic
is disabled and no stale imported type is substituted for current analysis.
Experiments reusing the export resolver or skipping redundant known-return
inference did not improve this workload and were removed.

## Method and results

Baseline checkout: `688a4ba`, captured before changes. Final measured compiler
SHA-256: `725823130d919faad937fcdd3a01237efea999dd6bed8452acd3823b8b3d4799`.
Three alternating baseline/candidate sessions used the unchanged local zombie
scenario and `lsp-editor-profile.js`, with attribution tracing disabled and no
build or test suite running. All six sessions passed expected completion-label
and response assertions. Only unsaved buffers were changed; project files were
not rewritten. Cold means a fresh LSP process, not a flushed filesystem cache.

Values are milliseconds, p50 / nearest-rank p95. Three cold samples are a small
local measurement. All rows are retained, including small hover regressions.

| Request | Samples per binary | Before p50 / p95 | After p50 / p95 |
| --- | ---: | ---: | ---: |
| `cold.module-completion` | 3 | 432.5 / 435.5 | 276.6 / 278.8 |
| `warm.self-completion` | 15 | 1.0 / 1.3 | 1.0 / 1.3 |
| `warm.module-completion` | 15 | 0.5 / 0.6 | 0.5 / 0.6 |
| `warm.hover` | 15 | 9.1 / 33.5 | 9.2 / 22.6 |
| `warm.format` | 15 | 3.3 / 3.4 | 3.2 / 3.5 |
| `edit.module-completion` | 15 | 93.6 / 203.8 | 52.3 / 67.1 |
| `edit.hover` | 15 | 18.0 / 21.2 | 17.7 / 22.1 |
| `partial.self-completion` | 15 | 63.0 / 71.5 | 24.1 / 28.3 |
| `cross-file.module-completion` | 15 | 50.1 / 123.5 | 27.5 / 77.5 |
| `cross-file.hover` | 15 | 14.7 / 17.9 | 15.8 / 18.2 |
| `immediate.format` | 15 | 44.2 / 45.5 | 24.9 / 26.4 |
| `post-save.format` | 15 | 17.8 / 19.1 | 16.2 / 17.5 |
| `save.hover` | 15 | 96.6 / 101.0 | 46.6 / 48.9 |
| `other.main-completion` | 15 | 0.4 / 237.7 | 0.4 / 216.7 |
| `other.main-hover` | 15 | 0.6 / 13.8 | 0.5 / 13.9 |
| `other.combat-completion` | 15 | 0.4 / 0.6 | 0.4 / 0.5 |
| `other.combat-hover` | 15 | 1.2 / 70.5 | 1.1 / 69.6 |
| `import.path-completion` | 15 | 0.4 / 0.7 | 0.4 / 0.7 |
| `contended.inline` | 15 | 0.2 / 0.3 | 0.2 / 0.3 |
| `contended.self-completion` | 15 | 1.0 / 1.2 | 0.9 / 1.0 |
| `contended.hover` | 15 | 9.8 / 16.4 | 9.0 / 15.9 |

Evidence: `target/completion-followup/accepted-{baseline,alias}-{1,2,3}.json`
and `aggregate.json`. Private scenario anchors remain local. Earlier exploratory
traces are retained separately and are not included in this table.

## Remaining work

Cold project analysis still costs about 277 ms, and first main-file completion
has a roughly 217 ms tail. Dependency-surface comparison and push fast
diagnostics still run on the input coordinator. Profile dependency-scoped or
incremental project construction while preserving cross-file event/type facts.
The immediate-format regression is also recovered in this replay (44.2 to
24.9 ms); warm formatting itself remains about 3 ms.

Physical typing, suggestion visibility, a pull-diagnostic client, larger
projects and LOVE hot reload still need separate qualification. The replay
cannot establish those results.

## Verification

`RUST_MIN_STACK=16777216 RUST_TEST_THREADS=4 make verify` passed after the final
implementation change: 1,596 core tests, 374 LSP tests, 122 CLI integration
tests, the remaining CLI/runtime checks, Clippy, metadata and language-corpus
checks, and 159 extension tests. The suite's three pre-existing ignored tests
remain ignored; no new test was skipped to obtain this result.

The new regressions cover recovered environment/body errors with ECS enabled
and disabled; lazy checker initialization; changed-file and old-snapshot
diagnostic identity; and exact built-in alias identity, user shadowing and ECS
configuration. The unchanged module, event, class and editor suites also pass.

`make representative-benchmark-smoke` passed with zero stale completion results
and complete workspace warming. The 1,000-line fixture measured 7.82 ms warm
completion p95 and 158.12 ms contended completion p95. The 100-file fixture
measured 0.45 ms and 30.41 ms respectively. These synthetic workloads supplement
the real-project replay rather than replacing it.

`bash scripts/docs.sh build --clean --strict` passed. Logs are retained under
`target/completion-followup/`. Installed-editor latency, graphical LOVE behavior
and the broader `make ci-check` gate were not exercised in this pass.

## Installable candidate

`make extension-package` passed. The macOS arm64 VSIX is
`target/completion-followup/tua-0.5.1-completion-followup-macos-arm64.vsix`.

SHA-256: `4ff68309a82df184696b665fbe07d00f332a2b7ae278d15bd41a201e8e1fac41`.

Archive integrity passed, and the bundled compiler matches the measured binary
byte for byte. Install with **Extensions: Install from VSIX…**. This candidate
was not published or installed into the user's editor.

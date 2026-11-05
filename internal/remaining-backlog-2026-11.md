# Remaining backlog implementation — November 2026

This pass follows `01a642f`. Each implemented behavior receives a separate commit;
remaining design/resource-dependent work is not silently marked complete.
Artifacts and verification logs are in `target/remaining-backlog/`.

## Completed

- `88bd6a6`: dedicated test roots shared by analysis/check/format/test discovery,
  isolated test output, and production/watch/package spec exclusion. Verified
  with 1,639 core tests (one ignored), 375 LSP tests, all 124 CLI integration
  tests, config/model regressions and the real three-test Busted example.
- `6fd9fe7`: static Busted discovery and structured JSON reporting, including
  mapped native/Lua locations, stdout isolation and stale-report prevention.
  Parser, argument, real Busted report and documentation checks pass.

- `887c609`: VS Code Test Explorer: lazy discovery, selected runs, cancellation, and
  opt-in continuous runs. Unit and extension activation checks pass. The packaged
  extension also passes the installed real-server suite described below.
- `8c0e362`: CLI LuaCov coverage: mapped native/Lua line counts, unloaded modules, generated
  helper exclusion and incomplete-run state. Verified against isolated LuaCov
  0.17.0 with real Busted, including a build failure after a successful run.

- `8ef5f45`: Test Explorer coverage profile displays the CLI source-mapped line counts;
  selected-run merging and failure/coverage coexistence have unit coverage.

## In progress

- Richer Busted contracts and explicit LOVE runtime testing.
- Annotation generation/update, documentation-aware parameter rename and
  annotation navigation/recovery.
- Further cold responsiveness, installed-editor/visual/long-session evidence,
  migration/upgrade qualification. A tested candidate package is available below.

The user replied “yes” to the request for a concrete reference/implementation
reproduction, without a symbol, file or expected destination. That report is
still unspecified; existing navigation regressions remain the baseline.

Test Explorer follows the [VS Code Testing API](https://code.visualstudio.com/api/extension-guides/testing),
using core/CLI discovery instead of a second Lua parser in the extension.

## Candidate verification

- `RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 make verify`: passed, including
  1,640 core tests (one ignored), 375 LSP tests, 127 CLI integration tests,
  83 CLI unit tests (two ignored), all 164 extension unit tests, Clippy, metadata,
  activation and language-reference checks. The LuaCov-dependent integration
  test also passed separately with LuaCov 0.17.0 installed in an isolated
  `/private/tmp` LuaRocks tree; it did not merely skip on missing dependencies.
- `make docs-build diff-check`: passed. Screenshot artifact checks passed.
- `8f6e3f6`: regenerated and visually reviewed all five editor gallery images
  against the release server and production extension bundle. Actual VS Code
  and LOVE rendered workspace, state machine, sprite, module graph and live
  runtime inspector views in a disposable profile. The inspector displayed
  live state, pool and store values. This is not full-game visual parity testing.
- Installed the candidate VSIX into a separate temporary extensions/profile
  directory and ran `editors/vscode/test/real-server/run.js`. Passed native/Lua/
  LOVE completion, hover, definitions, references, signatures, formatting,
  symbols, inlay hints and cache-stat checks using the packaged server.
  This suite verifies activation and language features; Test Explorer runs and
  coverage decorations currently have controller/unit and CLI backend evidence,
  not an end-to-end installed-editor interaction test.
- VSIX archive integrity checked with `unzip -t`. No publishing, tagging or
  normal-profile installation was performed.

## Installable artifact

`target/remaining-backlog/tua-0.5.1-testing-coverage-darwin-arm64.vsix`

macOS Apple silicon, extension/server 0.5.1; built from `8f6e3f6`.
Install with **Extensions: Install from VSIX…**. Busted remains an external
runtime dependency; coverage additionally requires LuaCov for its interpreter.

SHA-256:
`4bf3aa6f7e5aad539299d2e72108187145236e7642bd03f83a6e13c02c79ca38`

Bundled server SHA-256:
`cfa1e2c9f457cdefba6af69977aa9fcf973debed82396fc3ea1ed5449e63fa61`

## Still open

This candidate does not complete the entire roadmap. Richer Busted contracts,
explicit LOVE-runtime tests, annotation generation/update/rename/navigation,
literal-false LOVE configuration, further cold responsiveness, full-port visual
parity, prolonged editor sessions and adjacent-version upgrade/rollback
qualification remain open. Protocols, generic classes and initialization-order
changes remain design work; signing and additional native architectures require
external resources. The unspecified navigation report still needs a concrete
symbol and editor action. No unresolved item is marked fixed by this handoff.

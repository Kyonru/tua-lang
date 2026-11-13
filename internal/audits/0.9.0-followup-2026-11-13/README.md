# 0.9.0 candidate follow-up audit

**Scope:** the nine commits after published `v0.8.0` through `ff726b92`, their
0.9.0 VSIX, and the open priorities in the [candidate audit](../0.9.0-priorities-2026-11-13/README.md).
The fixes in this follow-up are `8cd993cc` and `1830bcf0`.
The tree was clean at the start. This is a local candidate; no tag or release
was created. The host was macOS arm64 with VS Code 1.142.0, Rust 1.97.1, and
the installed LÖVE app. The 0.8.0 and 0.9.0 VSIX files were available locally.

## Assessment and findings

| ID | Severity / status | Evidence and action |
| --- | --- | --- |
| F09-01: Font preview stays stale after an explicit reopen | Medium, confirmed and fixed | `showFontPreview` previously called only `existing.reveal()` for a repeated URI. A unit regression failed before the fix. The view now replaces its HTML and gives the font URL a new query revision. The installed VS Code host loaded a valid OTF, reported a load error after that file was replaced with invalid bytes and reopened, then loaded the restored font in the same tab. [Implementation](../../../editors/vscode/src/love/assets/font-preview.js) lines 13–43; [host regression](../../../editors/vscode/test/real-server/suite/index.js) around line 788. |
| F09-02: Unused imports cost time during export indexing | High performance risk, partly fixed | The 1,000-module fixture has 999 `require` locals in `main.tua` that no expression uses. Export inference called the semantic resolver for each one while building a return value unrelated to those imports. A conservative AST use scan now skips type resolution only for unused direct module aliases. Callable imported aliases and dependency edits retain their types in core tests. Matched release-binary profiles below show a cold improvement with steady sampled peak memory. [Implementation](../../../crates/tua-core/src/module_index/mod.rs) lines 542–563 and 760–779; [regression](../../../crates/tua-core/src/module_index/tests.rs) around line 863. Multi-second cold requests remain open. |
| F09-03: Generated workflow has no hosted-run evidence | Medium qualification gap, open | The template's archive URL, checksum command, version guard, and attestation arguments were inspected against the local release workflow and the [GitHub CLI verification reference](https://cli.github.com/manual/gh_attestation_verify). The prior audit downloaded and checked a published archive. A GitHub-hosted run from a game repository has not been observed; a local `gh` run lacks suitable authentication. Do not call the generated workflow end-to-end verified. [Template](../../../crates/tua-cli/src/github_actions/tua-game.yml). |
| F09-04: Visible completion stage attribution | Medium measurement gap, open | A same-host Starfall replay measured visible suggestion menus on installed 0.8.0 and the current 0.9.0 development extension. Median trigger-to-menu time was 122.6 and 121.8 ms across 12 samples each; the changed candidate had 10 candidates and no missing labels. The harness does not time wire send, server receive, or renderer paint individually, and the p95 samples varied. No regression is established; retain the measurement task. |

The feature examples intentionally include placeholder assets. In particular,
`examples/features/src/assets/ui.ttf` is ASCII placeholder text, as its README
states; its font preview should show a load error. The real-host font check
uses the bundled valid `sniglet.otf` instead. No independent game project was
opened or copied.

## Reproduction and measurements

- Font stale case: open a valid OTF through `tua.assets.preview`, replace its
  bytes, and run the same command again. Before the fix the existing tab was
  merely revealed. After the fix it reloads and reports the new font state.
  Run `node --test editors/vscode/test/unit/font-preview.test.js`, then
  `TUA_REAL_SERVER_PATH="$PWD/target/release/tua" node
  editors/vscode/test/real-server/run.js` in a graphical host.
- Export inference case: the generated `project-1000` fixture contains 1,000
  modules and 14,001 lines; the same fixture hash
  `a07851e38593fc8ecddda7ca653f1910b6e0621ff6144fc34bceaab730a97a53`
  was used before and after the skip. Set
  `TUA_REPRESENTATIVE_FIXTURES=project-1000`, cold repeat `2`, warm repeat `3`,
  contention repeat `3`, background burst `1`, and memory budget `4096`, then
  run `node editors/vscode/scripts/lsp-representative-profile.js BIN` for each
  release binary. Both runs exited 0.

| Protocol measurement | Before unused-import skip | After |
| --- | ---: | ---: |
| Cold completion p95 | 2,745.04 ms | 2,275.97 ms |
| Cold hover p95 | 2,808.12 ms | 2,239.76 ms |
| Cold definition p95 | 3,762.99 ms | 3,290.04 ms |
| Warm completion p95 | 9.37 ms | 9.70 ms |
| Contended completion p95 | 1,664.71 ms | 1,263.34 ms |
| Sampled peak RSS | 2,690.92 MiB | 2,688.30 MiB |

The workload is synthetic and these are server protocol round trips, not
visible menu timings. The separate Starfall replay used
`internal/audits/feature-followup-2026-11-11/starfall-visible-run.js` with
`TUA_STARFALL_BASELINE_VSIX=editors/vscode/tua-0.8.0.vsix` for the old package
and `TUA_TEST_SERVER=target/release/tua` for the candidate. Its old and new
median visible times were 122.6 and 121.8 ms; p95 was 178.7 and 184.5 ms.
That sample does not establish a p95 change.

## Verification and next work

- Core: focused callable-import and 64-import dependency-edit regressions
  passed. `RUST_MIN_STACK=16777216 cargo test -p tua-core` passed 1,950 tests.
  Without that repository gate setting, a pre-existing large built-in-module
  test overflows the default test thread stack; that first standalone run was
  not counted as a pass.
- LSP: `RUST_MIN_STACK=16777216 cargo test -p tua-lsp` passed 455 tests.
- VS Code: font and asset unit tests passed; the development-host suite passed
  the valid → invalid → restored font sequence and the existing editor checks.
- Aggregate gate: `CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 make ci-check`
  exited 0. It covered strict Clippy, the configured Rust advisory policy,
  148 CLI integration tests, 1,950 core tests, 455 LSP tests, emitted Lua,
  example builds, editor checks, docs, and benchmark smoke. The two known
  allowed Rust advisories remain warnings.
- Runtime: `TUA_LOVE_BIN=/Applications/love.app/Contents/MacOS/love make
  love-runtime-test` passed all three examples with the installed LÖVE app.
- Package: `make extension-package` produced
  `editors/vscode/tua-0.9.0.vsix` (41 files, 9.01 MB). `unzip -t` passed and
  extracted `extension/bin/tua --version` reported `tua 0.9.0`. The exact
  VSIX installed-host suite exited 0 and reported `fontReload: true`, Tua/Lua/
  LOVE completions, and zero fixture diagnostics. SHA-256:
  `27c140607481e47c7db3f0144013dc3d6a8e142b8b6508120e810b23d59e498c`.
- `make extension-screenshots-check` passed for five existing README captures,
  but they depict 0.8.0 and are not a 0.9.0 visual qualification.

Next, profile the remaining cold project snapshot and module-graph work in a
real game without copying private source into reports. Instrument wire and
renderer timestamps before changing visible completion scheduling. Exercise a
generated game workflow on a GitHub runner after 0.9.0 is published. Treat
Windows/Linux installed-host and long-session behavior as separate
qualification, not as passed checks from this macOS run.

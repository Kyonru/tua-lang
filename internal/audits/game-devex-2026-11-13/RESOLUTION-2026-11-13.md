# Game trial findings: implementation follow-up

This follows the three-game [audit](README.md) and its [backlog](BACKLOG.md).
The original findings remain a record of the 0.9.0 baseline; this file tracks
what changed afterward. Work began from `7fde1fbd` with a clean tracked tree.

| Finding | Current result | Commit and evidence |
| --- | --- | --- |
| GAME-08 inline `super` emission | Fixed | `36372459`; emitted-Lua regression and paired Lua runtime result both produce speed 120. |
| GAME-02 shader source-root path | Fixed for statically named `ShaderScan.new()` receivers | `c5c0c8a7`, `555614de`; bad `src/assets/pulse.glsl` now fails `check` and `build` with `TL2019` and a build-relative suggestion. Corrected path checks, builds, and loads in LÖVE. Dynamic paths and reassigned scanner receivers are intentionally not diagnosed. |
| GAME-01 imported subclass/protocol bound | Fixed | `b0092ff8`; positive and wrong-class negative core checks. |
| GAME-03 imported class receiver | Fixed | `b17be54c`; three-file imported constructor/factory regression. |
| GAME-04/05 imported SignalBus payload and signature | Fixed | `9aabe2f1`; project schema checking and signature help, including edit recovery. |
| GAME-06 cold `jar:` completion | Fixed | `a5493765`; fresh-snapshot core test plus stdio LSP replay of the alchemy probe. Cold `jar:` now returns `get` and `label`, and hover shows `Jar<Potion>` while parse diagnostics remain. |
| GAME-07 generic alias formatting | Fixed | `e63b7fe7`; formatter indentation/idempotence regression. |
| GAME-12 nested generic field hover | Fixed | `05618711`; hover, completion, range, and unsaved provider edit regression. |
| GAME-09 audio snapshot typing | Improved, with one known limitation | `645fd00f`; stable nested music, bus, and analysis fields are typed and documented. Snapshot completion omits `musicState`, but default record reads remain open, so the checker does not reject that misspelling. This is a broader record strictness decision. |
| GAME-10 pool handoff warning | Documented local mitigation | `3e428c86`, `4078d9e0`; exact `TL5007` suppression at a real ECS handoff is tested while another forgotten acquisition still warns. Tua does not infer ownership transfer through ECS. |
| GAME-11 bounded alias syntax | Canonical working form documented | `4a2311f0`; checked two-file example uses an unbounded instance alias and a protocol-bounded factory. Type-alias parameter bounds remain unsupported syntax. |
| GAME-13 second formatter pass | Still unverified | Fresh copies of all three trial games had no changes on a second `tua fmt` pass. The reported pre-format input was not preserved, so no defect is claimed. |

The CI gate found two Clippy issues in earlier follow-up changes. They were
corrected in separate behavior-preserving commits: `b50bdf55` and `4c070ad3`.
The more precise audio snapshot type exposed an unsafe assumption in the
bundled audio example; `c68d7c1d` guards the optional bus snapshot. The
installed-host font reload check exposed a test harness race: reopening the
webview replaces its browser execution context. `57a45d0b` reacquires that
context for each poll; the test then passed against the same packaged VSIX.

## Verification and limits

- `cargo test -p tua-core incomplete_ -- --quiet`: 18 passed.
- `cargo test -p tua-core shader_ -- --quiet`: 43 passed.
- `make docs-build`: passed, no issues found.
- A copied alchemy shader fixture: source-prefixed path fails both CLI `check`
  and `build` with `TL2019`; build-relative path passes both. LÖVE printed
  `build-path load true nil` for the built good fixture.
- Stdio LSP alchemy replay: completed call returned `label, get`; cold `jar:`
  returned `get, label` after the fix. First-request times were approximately
  0.8 seconds in separate debug server processes; these are not visible editor
  timings or stable performance baselines.
- Three copied trial games were formatted twice. Neither pass changed a
  `.tua` file in the copies, so GAME-13 remains unconfirmed.
- The current release binary checked and built all three trial games. Their
  Busted suites passed: arena 5, alchemy 3, maze 4. Each still emits the
  conservative TL5007 pool handoff warning recorded under GAME-10.
- `CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 make test`: passed after removing
  generated incremental caches to recover disk space. This includes 1,962
  core tests (one ignored), 455 LSP tests, 96 CLI unit tests (two ignored),
  148 CLI integration tests, and the other workspace/integration suites.
- Workspace formatting and Clippy passed. The other CI targets passed:
  `build-examples`, `editor-regression-smoke`, `version-drift-check`,
  `release-tool-test`, `config-schema-drift-check`, `llms-drift-check`,
  `llm-language-eval-test`, `feature-examples-check`, `extension-check`,
  `json-check`, `docs-build`, `benchmark-smoke`, and `diff-check`.
  The extension unit suite passed 231 tests. The representative benchmark
  smoke passed its configured latency and memory budgets; warm completion
  p95 was 6.58 ms for `single-1000` and 0.33 ms for `project-100`, with peak
  sampled memory of 297 and 263 MiB respectively. These are server measures.
- Literal `make ci-check` did not complete in the sandbox: `cargo audit`
  could not lock the read-only advisory database, an initial test build hit
  `ENOSPC`, two extension inspector tests needed loopback access, and the
  benchmark memory sampler needed process-inspection access. The affected
  targets passed after using permitted local access and disabling incremental
  builds. `cargo audit --no-fetch --db /Users/kyonru/.cargo/advisory-db`
  passed against the existing 1,490-advisory database with the repository's
  two allowed advisories. A fresh advisory fetch was not verified.
- An isolated VS Code 1.142.0 host installed the final VSIX and passed its
  real-server and sidebar suites. It exercised completion, hover, protocol
  navigation, diagnostics, graph previews, font reload, and eight filesystem
  and unsaved-edit sidebar steps. The first two runs failed at the font reload
  test because the test held a destroyed browser context; the fixed test
  passed with the same package.

## Editor timing qualification

The Starfall host harness observed the expected ten `world.` members in
12/12 visible-menu samples. Its warm median was 956.5 ms for the current
build and 910.5 ms for the preserved pre-fix 0.9.0 VSIX on the same VS Code
1.142.0 host. The current extension's provider occupied 21.1 ms median;
927.4 ms median elapsed after provider return before the expected labels
appeared. The comparison package is identified by SHA-256
`2dc1bfdf2e07057f5eee32310b93cd48bc7bef088d117a70032f3da2f34a22b0`;
the harness's historical `previous packaged 0.7.0` label is inaccurate for
this run. These samples do not establish a Tua regression: both packages
showed the same large host-side delay and only one short run of each was made.

On a separate synthetic 113-file project, typing-to-menu completion was
882.5 ms median / 955.6 ms p95, manual completion 951.0 / 992.3 ms, and
the JavaScript control 794.5 / 909.3 ms. Imported-class completion through
an unannotated two-hop factory showed `health`, `name`, and `takeDamage`;
its visible warm median was 894.9 ms and visible hover median was 68.8 ms.
Format-on-save applied the expected edits: 13.3 ms median / 34.1 ms p95 for
the small file (12 samples), and 20.6 / 35.7 ms for a 1,000-line file (five
samples). The host was under heavy build/disk pressure; previous reports
measured much faster visible menus. Repeating on a stable normal editor
session, including a larger real game, remains performance work rather than
a confirmed compiler or extension defect from this run.

## Test extension

`make extension-package` passed. The installable
[`tua-0.9.0.vsix`](../../../editors/vscode/tua-0.9.0.vsix) contains 41 files,
including `extension/bin/tua`. `unzip -t` passed, the bundled executable
reported `tua 0.9.0`, and its bytes matched `target/release/tua`. SHA-256:
`54aa4eb65324cd83d2048bf35d43f302a4365146652f88fb3ed4b331f99b30dc`.
This is a local test artifact; no publish, tag, push, or release was made.

The remaining decisions are record strictness for audio snapshots, whether
pool-to-ECS transfer should change TL5007's ownership model, and syntax for
bounds on exported instance aliases. GAME-13 still lacks a preserved failing
input. This follow-up did not rerun all three complete games in LÖVE after
the fixes; the focused shader runtime replay passed. It does not establish
long-session inspector behavior, large-preview performance, or cross-platform
packaging.

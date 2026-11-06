# Integrated verification

Scope: local `origin/main` b40dbb445f7d8ba36e9312909be7c5d2949ac4af to starting HEAD d7aed2e5f1297b80a948e055985375bceaff6f0c, plus the uncommitted audit fixes. No fetch, commits, pushes, publication or normal-profile extension installation. Original `devex-2026-11-06` evidence remains preserved.

## Verification method

The final gate follows the last product edit (the game-discovered formatter correction). An earlier complete Rust run passed 1,665 core and 387 LSP tests plus the CLI suites and strict Clippy. The final formatter-focused run passed 56 tests. The subsequent complete gate includes the new formatter regressions.

`make verify` initially stopped at two unnecessary-borrow Clippy warnings in new asset tests; those were corrected. A later restricted run passed Rust checks but hit localhost `listen EPERM` in extension inspector tests. Retrying exactly `make extension-check` with local execution permission passed all 166 extension tests, TextMate fixtures and contrast checks. The final gate therefore uses `make -o extension-check verify`, reusing that separately successful, unchanged extension check. This is an explicitly composed gate, not a claim the restricted command exited successfully.

## Final gate and server

**Passed:** composed final gate, 2,276 Rust tests (1,668 core, 387 LSP and 221 CLI/integration), strict Clippy, formatting, version/release tools, config/LLM drift, language evaluation, JSON and diff checks. Separately passed all 166 extension tests, TextMate/contrast checks, docs build and existing screenshot artifact validation. Three intentionally ignored Rust tests remain unrun: native glslang shader corpus, graphical LOVE shader validation and manual project-corpus coordinator latency probe. These are not passing results.

Evidence: [gate summary](evidence/gate-summary.json), [complete gate log](evidence/verify.log), [extension checks](evidence/extension-check.log), [docs/gallery checks](evidence/docs-gallery.log), [environment](evidence/environment.json).

Final server SHA-256: `1874c3b464b33e51c85f7be225b461b6071abffd82110df602f3475a25643f59`. [Release build log](evidence/release-build.log).

**Local test VSIX:** [tua-0.5.1-origin-audit.vsix](../../../editors/vscode/tua-0.5.1-origin-audit.vsix), 8,447,981 bytes, macOS arm64, manifest 0.5.1. Archive SHA-256: `462343ee5f025e8f2dad7d5f5f11c7adb66320b373327ecdca8d09ef70d45a27`. `unzip -t` passed; the bundled `extension/bin/tua` hash exactly matches the final replay server. [Package manifest](evidence/package.json) and [build log](evidence/package.log).

Packaging reused existing installed dependencies and the separately passed unchanged extension check: `make -o extension-deps -o extension-check extension-package PROJECT_VERSION=0.5.1-origin-audit` with explicit v0.5.1 README URL bases. The production extension bundle was built by VSCE's prepublish hook. No normal editor profile was modified. Install manually using VS Code's **Extensions: Install from VSIX…** command.

## Final rebuilt-server replay

All 14 runners exited successfully; [replay manifest](evidence/replay-manifest.json) records commands and exact binary provenance.

- 64 typing cases, no harness errors. Twelve acceptance outcomes changed in the intended direction: ten previously unchecked invalid contracts now reject, while valid final-call expansion and lexical shadowed-function calls now accept. Typed callback-local parameter hover is now `number`; class-following expression hover retains source ranges. Full [typing responses](evidence/typing/results.json) include supported controls and explicitly documented capability gaps, so this is not a claim every exploratory expectation is supported.
- 45 completion cases in each of invoked, omitted-context and no-snippet modes, no harness errors. [Invoked](evidence/completion/evidence.json), [omitted](evidence/completion/context-omitted.json), [plain-client](evidence/completion/no-snippet-support.json).
- Eight applied edits all pass CLI checking, including quoted asset filenames, existing call suffixes, trailing header comments and suppression edits. [Applied source and checks](evidence/completion/applied-evidence.json).
- Eight independently asserted hover/Unicode cases pass, including spaced/newline method chains, lexical identity, optional parameters, class source coordinates and applied rename. [Responses](independent-hover-replay.json).
- Lifecycle, Unicode and diagnostic freshness probes complete without harness errors; permanent protocol regressions additionally assert project overlays, malformed positions, custom source ranges and cache behavior.
- All three game probes pass explicit type, completion, signature, navigation and diagnostic assertions against the final binary.
- The arena formatter reproduction now has correct trailing indentation, passes a second `fmt --check`, and checks/builds successfully. [Final source](evidence/formatter-final.tua); 56 focused formatter tests passed before the full gate.

## Native runtime and actual editor

Final server builds passed the platformer, arena and puzzle native LOVE smoke scripts. [Platformer](evidence/platformer-runtime.log) includes host Lua plus LOVE LuaJIT gameplay assertions and 30 real graphics frames; [arena](evidence/arena-runtime.log) tests deterministic game logic and stubbed draw dispatch; [puzzle](evidence/puzzle-runtime.log) executes actual graphics/callbacks and gameplay assertions. Existing vendor runtime tests additionally passed under installed LOVE LuaJIT; see [runtime evidence](runtime-luajit-evidence.log).

An isolated **VS Code 1.142.0** development host passed the final completion and Unicode-rename scenario. The autoimport provider returned `createEntity` in 22.57 ms initially and 2.71 ms after idle. Actual rename edits changed both `score` occurrences to `points` while preserving the emoji and surrounding text. [Editor responses](evidence/editor-evidence.json), [log](evidence/editor.log), [suggestion screenshot](evidence/editor-suggest.png). The screenshot was visually inspected and the suggestion is visible; the script's DOM `visible:false` uses an unreliable offset-parent check for this fixed-position widget and is not treated as evidence it was hidden.

## Protocol performance observations

Three fresh processes per project, ten warm and ten post-edit samples per process. Values below are median milliseconds from stdio request to response, with no missing expected candidates. These small samples are observational, not a performance SLA or a regression attribution.

| Bundled project | Cold import completion | Warm completion | Warm hover | Warm signature | Post-edit completion | Incomplete edit |
|---|---:|---:|---:|---:|---:|---:|
| features | 129.36 | 0.21 | 0.17 | 0.20 | 16.18 | 16.09 |
| starfall | 42.63 | 0.14 | 0.13 | 0.15 | 4.36 | 3.75 |
| love-full | 83.07 | 0.26 | 0.16 | 0.19 | 5.46 | 5.20 |

[Full distributions and sample responses](evidence/workflow/performance-evidence.json). Cold project work remains the main measured latency opportunity; these results do not resolve historical ~431-ms cold completion on a different project or establish end-to-end format-on-save responsiveness.

## Reproduction instructions

From the repository root:

```sh
RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 make verify
make docs-build extension-screenshots-check
cargo build --release -p tua-cli
python3 internal/audits/origin-main-2026-11-06/replay.py
```

The replay script copies baseline JavaScript fixtures to ignored `target/audits/devex-fixed/`, preserving the baseline audit. It runs 64 typing cases, completion trigger/capability variants, applied edits, code actions, independent hover controls, workflow/Unicode/freshness/performance probes and all three game probes. Its manifest records the binary hash and command exit statuses. Negative language fixtures intentionally produce checking errors; a harness exit of zero alone is not proof all language expectations passed. Full recorded responses and explicitly asserted probes must be read together.

Game builds and native runtime smoke commands are in each game README. Installed-editor evidence uses an isolated VS Code profile and temporary workspace. Gallery checking verifies pre-existing screenshot hashes/dimensions only; it does not recapture or visually qualify the current UI.

## Qualification boundaries

- Protocol request timing is distinct from keyboard-to-popup latency. Small bundled-project runs are not comparable to a different historical zombie-project benchmark.
- Three two-module games exercise ordinary records, arrays, imports, LOVE calls, gameplay and incomplete member edits. They do not qualify large class/event architectures or all prior full application ports.
- Arena rendering/input is stubbed; platformer and puzzle additionally execute native graphics. No human playtest or screenshot-based game visual review is claimed.
- Existing runtime fixtures ran under installed LOVE LuaJIT and host Lua; this is not universal Lua-version or platform compatibility.
- The local test VSIX targets the host macOS architecture and retains manifest version 0.5.1. It is not a published release.
- Windows/Linux native execution, external services, arbitrary API overloads and multi-hour editor sessions remain unqualified. Ignored tests are listed in the gate log and are not counted as passing.

# Tua 0.9.0 improvement audit and test candidate

**Base:** published `v0.8.0` at `d19ec214`. **Host:** macOS arm64,
November 13, 2026; Cargo 1.97.1, Node 24.15.0, VS Code 1.142.0. The tracked
tree was clean at the base; ignored build and prior-audit artifacts existed.
This is a local candidate, not a tagged or published release. The execution
brief is [here](../../0.9.0-improvement-audit-execution.md).

## Assessment

Tua's ordinary typed editing path is functional in the installed-host fixture:
completion, hover, signature help, navigation, diagnostics, formatting,
protocol quick fixes, and unsaved generic edits passed. The most important
remaining engineering priority is large-workspace first-response cost. A
generated 1,000-module project took 2.92 seconds for cold completion and
peaked at 1,437 MiB RSS in one bounded replay. This candidate adds a font
preview and removes repeated Rust compilation from newly generated game CI.
Neither change alters compiler semantics or fixes the large-workspace cost.

## Ranked decisions

| ID | Classification | Priority | Evidence and disposition |
| --- | --- | --- | --- |
| 090-01 | Performance risk | High | The 1,000-module profile measured 2,920.51 ms cold completion, 2,691.80 ms cold hover, 3,718.16 ms cold definition, 9.15 ms warm completion, and 1,437.39 MiB sampled peak RSS. Tracing shows repeated entry-file export and snapshot work; the exact cause is unproven. **Open.** Profile and fix shared project analysis, then compare to 0.8 on the same fixture and a real game. |
| 090-02 | Editor capability gap | Medium | The November 12 installed-host audit found that a valid OTF opened a binary warning through `vscode.open`; the asset command delegated every format to that viewer. **Fixed:** source-scoped font webview with common game text sizes, glyph sample, load-error state, and tab reuse. Unit and real-host rendering regressions added. |
| 090-03 | Build-workflow usability | Medium | The generated game workflow used `cargo install --git` on every run, although versioned Linux CLI archives exist. **Improved:** new workflows download the matching archive, check SHA-256 and provenance, confirm CLI version, then package. Existing generated workflows are untouched. A generated GitHub-hosted run remains unverified. |
| 090-04 | Roadmap accuracy | Low | `ROADMAP.md` still listed FUP-01 cross-file factory completion as open, although its follow-up documented core, LSP, and installed-host resolution. **Fixed:** removed the stale item and completed 0.7 section while retaining open qualification work. |
| 090-05 | Measurement gap | Medium | The November 12 Starfall replay measured about 125 ms warm typed-dot to visible labels, but the roughly 103 ms after provider return includes scheduling and rendering. **Open:** instrument wire send, server receive, and widget paint before setting a visible-menu target. |

The further backlog is Test Explorer Debug, headless LÖVE test mode,
continuous-run and coverage UI qualification, manual long-game inspector use,
cross-platform installed hosts, and hand-authored large graph fidelity. These
have prior evidence in `ROADMAP.md` and the November follow-up; they are not
newly confirmed defects or promised for this candidate.

## Evidence and next implementation sequence

1. **Shared project analysis (090-01, high, medium confidence on cause).**
   `AnalysisHost::snapshot` builds the project index
   ([host.rs](../../../crates/tua-core/src/analysis/host.rs)), while per-file
   export inference is recorded at
   ([database.rs](../../../crates/tua-core/src/analysis/database.rs)). The trace
   counted 18 entry-file export spans and 11 snapshot spans across the
   multi-session probe. First isolate why the entry file is rebuilt or exports
   are rescanned; preserve project identity, unsaved dependency freshness, and
   cancellation. Estimated effort: several days. Accept only a matched
   before/after 100- and 1,000-module replay with correct editor results and no
   memory regression, plus a representative hand-authored game.
2. **Visible editor latency (090-05, medium, high confidence as a measurement
   gap).** The [Starfall follow-up](../feature-followup-2026-11-11/NOV12_QUALIFICATION.md)
   already timed trigger, provider entry/return, and visible labels. Add wire
   send, server receive, and renderer paint marks in the same isolated host
   run. Estimated effort: one to two days. Acceptance: a repeatable stage
   breakdown against 0.8 before changing scheduling or debounce behavior.
3. **Game testing and remaining preview qualification.** Design source-mapped
   persistent Busted builds for Test Explorer Debug before exposing a Debug
   button; qualify continuous runs and coverage with real LuaCov. Use a
   hand-authored large graph and an active game session for preview/inspector
   work. Effort depends on debugger and platform support. These are capabilities
   and qualification tasks, not confirmed regressions in this pass.

The font-preview gap (090-02) is visible in the old generic asset-open path
([assets/index.js](../../../editors/vscode/src/love/assets/index.js)); its new
webview is [font-preview.js](../../../editors/vscode/src/love/assets/font-preview.js).
The workflow cost (090-03) is in the
[generated template](../../../crates/tua-cli/src/github_actions/tua-game.yml).
The stale FUP-01 entry (090-04) was contradicted by the
[resolved follow-up](../feature-followup-2026-11-11/README.md).

## Coverage and limits

| Area | Current status | Evidence |
| --- | --- | --- |
| Language/checker/emitted Lua | Gate pending | Workspace and runtime checks after final edit; no semantic source changed here. |
| Cross-file generic/class/protocol editor paths | Tested | Installed real-server suite: completion, hover, signature, definition, diagnostics, and unsaved-edit recovery. |
| LOVE assets/previews | Partly tested | Valid OTF rendered in host; font HTML and escaping unit tests. Other asset types retain November 12 host evidence. |
| Generated game workflow | Partly tested | CLI integration, YAML parse and shell syntax, release archive URL/checksum/layout. GitHub-runner execution unavailable locally. |
| Large-workspace responsiveness | Measured | Released 0.8 server and generated 1,000-module LSP profile. One-round contention lacked a valid-document sample, so its qualification gate exited nonzero. |
| Game domains/runtime | Gate pending | Bundled examples and runtime checks planned; no independent private project opened or copied. |
| Platform/long session | Inspected only | Prior reports; no Windows/Linux installed host or multi-hour game session in this pass. |

## Reproduction

Run the bounded large-workspace probe with `TUA_REPRESENTATIVE_FIXTURES=project-1000`,
`TUA_REPRESENTATIVE_COLD_REPEAT=1`, `TUA_REPRESENTATIVE_WARM_REPEAT=1`,
`TUA_REPRESENTATIVE_CONTENTION_REPEAT=1`,
`TUA_REPRESENTATIVE_BACKGROUND_BURST=1`, and
`TUA_REPRESENTATIVE_MEMORY_BUDGET_MIB=4096` before
`node editors/vscode/scripts/lsp-representative-profile.js target/release/tua`.
The one-round run is diagnostic evidence, not a passing contention gate. The
first sandboxed attempt failed when its RSS sampler called `ps` (`spawn EPERM`).

For the editor fixture, run `make extension-server`,
`npm --prefix editors/vscode run build:dev`, and then
`TUA_REAL_SERVER_PATH="$PWD/editors/vscode/bin/tua" node
editors/vscode/test/real-server/run.js`. It uses a temporary workspace and
requires loopback and GUI access. The font test copies bundled `sniglet.otf`
and asserts successful load, an 18 px computed size, and glyph content.

Final gate outcomes, candidate revision, VSIX path, and SHA-256 are in
[VERIFICATION.md](VERIFICATION.md).

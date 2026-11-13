# Tua 0.9.0 improvement audit and test candidate

**Base:** published `v0.8.0` at `d19ec214`. **Product candidate:** `fb0514b0`
plus earlier 0.9.0 commits. **Host:** macOS arm64,
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
preview, removes repeated Rust compilation from newly generated game CI, and
reuses a semantic model while inferring each module's exports. The matched
1,000-module replay reduced cold completion from 3.02 to 2.66 seconds, but
the remaining delay still needs attention.

## Ranked decisions

| ID | Classification | Priority | Evidence and disposition |
| --- | --- | --- | --- |
| 090-01 | Performance risk | High | The initial 0.8 profile measured 2,920.51 ms cold completion and 1,437.39 MiB sampled peak RSS; its one-round contention gate failed. A matched 0.9 before/after profile with two cold and three warm rounds reduced cold completion 3,022.47 → 2,658.05 ms and cold hover 3,097.54 → 2,640.99 ms; warm completion stayed 9.76 → 9.41 ms. Sampled peak RSS rose 2,607.69 → 2,685.80 MiB (~3%), below the 4,096 MiB probe budget. **Partly improved, still open:** cold requests remain multi-second; check a real game and seek the remaining shared-analysis cost. |
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

1. **Shared project analysis (090-01, high, medium confidence on remaining cause).**
   `AnalysisHost::snapshot` builds the project index
   ([host.rs](../../../crates/tua-core/src/analysis/host.rs), line 504), while
   per-file export inference is recorded at
   [database.rs](../../../crates/tua-core/src/analysis/database.rs), line 489.
   The trace
   counted 18 entry-file export spans and 11 snapshot spans across the
   multi-session probe. Reusing the `SemanticModel` in `ExportInferer` removed
   repeated creation within one module and improved the matched 1,000-module
   profile. Next isolate why the entry file is rebuilt or exports are rescanned;
   preserve project identity, unsaved dependency freshness, and cancellation.
   Estimated effort: several days. Accept only a matched before/after 100- and
   1,000-module replay with correct editor results and bounded memory, plus a
   representative hand-authored game.
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
[generated template](../../../crates/tua-cli/src/github_actions/tua-game.yml)
(install step, line 19).
The stale FUP-01 entry (090-04) was contradicted by the
[resolved follow-up](../feature-followup-2026-11-11/README.md).

## Coverage and limits

| Area | Current status | Evidence |
| --- | --- | --- |
| Language/checker/emitted Lua | Tested | `make ci-check` passed, including 1,949 core tests, emitted-language Lua matrix, syntax validation, and bundled example builds. Export inference changed, with a 64-import dependency-edit regression. |
| Cross-file generic/class/protocol editor paths | Tested | Installed real-server suite: completion, hover, signature, definition, diagnostics, and unsaved-edit recovery. |
| LOVE assets/previews | Partly tested | Valid OTF rendered in the installed 0.9.0 VSIX host at 18 px; font HTML, escaping, and tab-reuse unit tests passed. Other asset types retain November 12 host evidence. |
| Generated game workflow | Partly tested | CLI integration, YAML parse and shell syntax, release archive URL/checksum/layout. GitHub-runner execution unavailable locally. |
| Large-workspace responsiveness | Measured | Released 0.8 baseline plus matched 0.9 before/after 1,000-module profiles. The three-round matched contention gate passed; a separate one-round 0.8 probe lacked a valid-document sample. |
| Game domains/runtime | Tested on bundled fixtures | Three examples launched under installed LÖVE; feature interactions for generic stores, ECS signals and pools passed. No independent private project opened or copied. |
| Platform/long session | Inspected only | Prior reports; no Windows/Linux installed host or multi-hour game session in this pass. |

## Reproduction

Run the bounded large-workspace probe with `TUA_REPRESENTATIVE_FIXTURES=project-1000`,
`TUA_REPRESENTATIVE_COLD_REPEAT=1`, `TUA_REPRESENTATIVE_WARM_REPEAT=1`,
`TUA_REPRESENTATIVE_CONTENTION_REPEAT=1`,
`TUA_REPRESENTATIVE_BACKGROUND_BURST=1`, and
`TUA_REPRESENTATIVE_MEMORY_BUDGET_MIB=4096` before
`node editors/vscode/scripts/lsp-representative-profile.js target/release/tua`.
The one-round run is diagnostic evidence, not a passing contention gate. The
matched comparison used cold repeat 2, warm repeat 3, and contention repeat 3
against the same fixture hash `a07851e3…`; both runs passed their probe gates.
Its sampled peak memory differs from the initial 0.8 single-round run because
the probe runs more sessions. The first sandboxed attempt failed when its RSS
sampler called `ps` (`spawn EPERM`).

For the editor fixture, run `make extension-server`,
`npm --prefix editors/vscode run build:dev`, and then
`TUA_REAL_SERVER_PATH="$PWD/editors/vscode/bin/tua" node
editors/vscode/test/real-server/run.js`. It uses a temporary workspace and
requires loopback and GUI access. The font test copies bundled `sniglet.otf`
and asserts successful load, an 18 px computed size, and glyph content.

Final gate outcomes, candidate revision, VSIX path, and SHA-256 are in
[VERIFICATION.md](VERIFICATION.md).

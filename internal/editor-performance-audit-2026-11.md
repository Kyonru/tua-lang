# Editor performance audit — November 2026

Follow-up: [cold and incomplete completion](completion-latency-followup-2026-11.md)
records newer measurements and fixes. The results below describe the original pass.

## Outcome

Repeated completion and hover are substantially faster in the zombie project.
Save diagnostics no longer build a project snapshot on the LSP input thread.
Opening or closing an unchanged indexed document preserves semantic facts.
Cold startup and post-edit analysis remain measurable bottlenecks; this pass
is not a claim that every editor action is instant.

## Changes

| Commit | Change | Correctness checks |
| --- | --- | --- |
| `d6720d9` | Reuse worker-published file facts for current-file queries and adopt unaffected facts before invalidation. | Source-version/content/kind identity, divergent worker edits, imported type invalidation, and overlay close restoration. |
| `588045a` | Schedule save diagnostics on the existing background lane without the typing debounce. | No coordinator parse on save, fresh diagnostic publication, discard after a newer edit. |
| `f3b7ae6` | Preserve analysis for identical disk/overlay transitions. | No parse misses on unchanged open/close; overlay ownership and newer disk contents remain correct. |
| `7176525` | Keep snapshot-backed background warm results adoptable. | Snapshot hit warms with no parse, coordinator accepts it, newer revisions reject it; representative warm-up gate. |

No diagnostic feature is disabled, and no old type result is intentionally
returned to make a request look faster. The existing worker and core analysis
infrastructure owns these changes; no parallel hover/type resolver was added.

## Method

Baseline: `2286beb`, with its release compiler copied before this pass.
Candidate: `7176525`, release compiler SHA-256
`677e63b4d92fa7bf1df8d63d9a37be35af4fccc8c01c191ab2f7f4802e096841`.
Host: macOS 26.6.1, arm64. Three alternating baseline/candidate sessions, no
build running, attribution tracing disabled. Cold means a new LSP process,
not a flushed operating-system filesystem cache.

The real zombie workspace was read without modifying its files. Its `src`
tree contains 16 Tua/Lua files totaling 54,890 bytes. Unsaved
buffers exercised dialogue (6,337 bytes), its imported writer (3,101 bytes),
main (2,830 bytes), and combat (20,550 bytes). The workspace's local inline
completion setting was enabled. This protocol client used push diagnostics;
pull-client behavior and VS Code rendering need separate qualification.

`editors/vscode/scripts/lsp-editor-profile.js` validates exact cursor anchors,
expected member labels and non-null hover responses. All six final sessions
passed their response assertions. Hover text/type correctness also has core/LSP
regression coverage; the replay itself does not exhaustively validate every type.
The inline probe permits an empty result and measures dispatch contention only,
not suggestion usefulness. Import-path responses are measured without an exact
expected-label assertion.

Each session opens dialogue cold; repeats warm requests; changes a class field;
types an incomplete `self.`; prepends comments to the imported writer; formats, applies the
edit and saves; opens main/combat; and overlaps completion, hover and local inline
requests. Changes use original-buffer replacements. Restoring incomplete edits
also sends a change, so burst timing includes queued work from those edits.
The writer edits preserve its export types; actual exported-type changes are
covered by the invalidation regression tests, not that timing row.

Evidence is under `target/editor-performance/`: `final-{baseline,candidate}-{1,2,3}`
JSON reports, `aggregate.json`, private `zombie-scenario.json`, and check logs.
See [the replay guide](completion-benchmarks.md#replay-a-project-editing-session)
for the scenario format. Source anchors and private project files are not copied
into this report or committed as benchmark fixtures.

## Results

Values are milliseconds, p50 / nearest-rank p95 across all three sessions.
Three cold samples and fifteen warm samples are small local measurements,
not service-level guarantees. Spikes and regressions are retained below.

| Request | Samples per binary | Baseline p50 / p95 | Candidate p50 / p95 |
| --- | ---: | ---: | ---: |
| `cold.module-completion` | 3 | 439.9 / 470.1 | 430.6 / 437.9 |
| `warm.self-completion` | 15 | 38.0 / 51.8 | 1.0 / 1.2 |
| `warm.module-completion` | 15 | 36.5 / 42.6 | 0.5 / 0.6 |
| `warm.hover` | 15 | 44.1 / 68.3 | 9.5 / 32.9 |
| `warm.format` | 15 | 3.2 / 3.6 | 3.3 / 3.6 |
| `edit.module-completion` | 15 | 88.9 / 195.4 | 91.5 / 199.4 |
| `edit.hover` | 15 | 62.5 / 68.1 | 17.6 / 20.6 |
| `partial.self-completion` | 15 | 41.7 / 66.2 | 62.4 / 72.8 |
| `cross-file.module-completion` | 15 | 84.7 / 121.3 | 50.4 / 124.6 |
| `cross-file.hover` | 15 | 48.5 / 64.6 | 14.5 / 17.9 |
| `immediate.format` | 15 | 38.4 / 40.7 | 42.9 / 45.0 |
| `post-save.format` | 15 | 113.9 / 219.9 | 16.7 / 17.7 |
| `save.hover` | 15 | 11.3 / 12.8 | 94.2 / 102.5 |
| `other.main-completion` | 15 | 0.4 / 291.3 | 0.4 / 243.8 |
| `other.main-hover` | 15 | 0.5 / 31.9 | 0.6 / 13.0 |
| `other.combat-completion` | 15 | 0.4 / 98.5 | 0.4 / 0.6 |
| `other.combat-hover` | 15 | 1.2 / 177.0 | 1.2 / 69.2 |
| `import.path-completion` | 15 | 0.4 / 98.6 | 0.4 / 0.6 |
| `contended.inline` | 15 | 0.1 / 0.2 | 0.2 / 0.3 |
| `contended.self-completion` | 15 | 0.9 / 1.2 | 1.0 / 1.2 |
| `contended.hover` | 15 | 8.1 / 14.3 | 9.5 / 16.5 |

The post-save formatting and hover rows are sequential: the baseline blocks
formatting while doing save analysis, then answers hover cheaply. The candidate
returns formatting sooner while the subsequent hover still waits for semantic
work. Their combined p50/p95 is **124.8/231.8 → 110.9/119.4 ms**. The hover row
alone would misleadingly suggest that all save analysis got slower.

The formatter itself remains about 3 ms. The immediate-after-edit formatting
probe includes coordinator work before formatting starts. File switching
improved tail latency, but first main completion can still take about 244 ms.

The first representative run caught a regression in this pass: warm workers
could reuse shared snapshot facts without filling the private cache consumed by
warm-result adoption. The coordinator then counted those jobs as failed. The
follow-up retains the reused file in the worker cache without reparsing; its
regression test also checks rejection after an intervening edit. Final timing
runs were repeated after that fix; earlier traces remain under `pre-warm-fix/`.

## Remaining performance work

1. Cold completion still builds project analysis: approximately 431 ms here.
   Profile larger projects and dependency closure before introducing laziness.
2. Post-edit completion remains about 92 ms median, with a 199 ms tail.
   Dependency-surface comparison and push fast diagnostics still run on the
   coordinator. Shared worker facts remove repeated queries but can also expose
   more precise surface-comparison work on subsequent edits. Move or reuse
   that work with generation-safe publication rather than weakening freshness.
3. Incomplete-code completion regressed from 41.7 to 62.4 ms median; formatting
   immediately after an edit rose from 38.4 to 42.9 ms. These are unresolved
   regressions in this workload, not hidden behind warm-cache improvements.
4. Reproduce physical typing, suggestion-menu visibility, hover presentation,
   format-on-save and diagnostic timing in an installed editor. Include a pull
   client and larger files; protocol latency excludes the UI and extension host.
5. LOVE hot-reload runtime latency is a separate workload and was not measured
   by this editor pass.

These items are carried in `ROADMAP.md`. None requires a new language-design
choice, but the coordinator work needs an architectural change with freshness
and contention tests before it can be called solved.

## Verification and artifact

`RUST_MIN_STACK=16777216 RUST_TEST_THREADS=4 make verify` passed, including
1,593 core tests (one existing ignored probe), 374 LSP tests, CLI/integration
checks, Clippy, metadata checks and 158 extension tests. Existing external
shader-host tests remain ignored. `make docs-build extension-screenshots-check`
also passed. The screenshot check validates the existing 0.4.1 captures; no
panel changed and no fresh visual qualification is claimed.

The full `make ci-check`, installed-editor/UI latency testing, physical typing,
and LOVE runtime hot-reload measurements were not run.

`make representative-benchmark-smoke` passed on the corrected candidate.
The 1,000-line fixture had 7.20 ms warm completion p95 and 203.06 ms contended
completion p95; the 100-file fixture had 0.65 ms and 47.61 ms respectively.
Both reported zero stale completion results and complete workspace warming.
The first sandbox attempt could not sample memory (`ps` was blocked); the
read-only sampling rerun ran outside that restriction. The earlier product
warm-up failure was fixed, not dismissed as an environment error.

`make lsp-features-smoke` and `make extension-package` passed after the final
implementation change. The installable macOS arm64 candidate is:

`target/editor-performance/tua-0.5.1-performance-macos-arm64.vsix`

VSIX SHA-256: `10c955b8a1836acbdddf1ea166402fc00a6d89600af8f5b54e7bc37a6f67e677`.

`unzip -t` passed, and the bundled `extension/bin/tua` SHA-256 exactly matches
the measured release compiler above. The artifact is version 0.5.1; this pass
does not tag, publish, or install it into the user's editor. Use VS Code's
**Extensions: Install from VSIX…** command to test it locally.

The performance fixes have individual commits; `2d0e2da` adds the reproducible
replay harness and its tests. This report and the roadmap retain remaining
performance work instead of marking cold typing or all format-on-save delay solved.

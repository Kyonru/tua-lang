# Implementation review — CLI and cross-surface follow-up

This bounded implementation review reconciles all 65 confirmed findings and six separately classified capabilities/design questions with implementation commits. It is not universal release qualification. The CLI implementation agent reviewed its own ownership changes and independently exercised formatter, linter, editor, declaration-overlay and signal-navigation fixes. The lead owns final integrated suite results; the final review agent owns the remaining active-overload follow-up.

## Executed evidence

- Fresh `cargo build -p tua-cli` completed. Probed debug binary SHA-256: `4a5be62baddf30cd55cdacbc53283367f7e33eeb4771e133b4db7b9e8335cc0e`. HEAD at review capture: `59efb09700e405a9fed31ddd56074401edc596da`; other language edits were in progress, so binary is a working-tree snapshot, not a release artifact.
- `cargo test -p tua-cli --lib -- --test-threads=1`: 90 pass, 2 shader-tool timeout failures, 2 native-tool tests ignored. All added CLI regressions passed. Native retry of `shaders::glslang::tests`: four pass, one ignored. The initial failures were environmental; all three affected integration shader tests also passed outside the sandbox.
- `cargo test -p tua-cli --test cli -- --test-threads=1`: 128 pass, three shader-tool failures under sandbox. Added ownership, dev root and Busted-report regressions all pass; all three failures passed when rerun individually outside the sandbox. This is combined evidence, not a single uninterrupted full-suite pass.
- `node --test editors/vscode/test/unit/source-maps.test.js`: pass. Rust source-map/traceback tests passed in the CLI suite.
- `python3 internal/audits/features-2026-11-07/post-fix-review/formatter.py`: LF, CRLF and interior controls preserve runtime length 5 before/after. [raw](post-fix-review/minimal-evidence.json).
- `node internal/audits/features-2026-11-07/post-fix-review/linter.js`: actual LSP safe fix retains print; before/after stdout both `keep`. [raw](post-fix-review/linter.json).
- `node internal/audits/features-2026-11-07/post-fix-review/editor.js`: actual LSP generic colon completion yields exactly yes/no; audio events correct; GLSL Markdown has one fence; other baseline navigation controls work. [raw](post-fix-review/responses.json).

## Cross-surface replay results

The latest actual LSP replay confirms **EDITOR-01**: audio `play` offers `ping`, `stopBus` offers `master`/`sfx`, and generic colon-call literals remain exactly `yes`/`no`. The initial incomplete closure was caught by this independent replay and fixed in ff8c5da.

**MODULES-03** required an additional fast-hover correction beyond snapshot tests: the server's source-only fast path returned `result: any` for locals initialized by imported calls, bypassing the correct snapshot result. Commit 8210170 makes declaration overlays authoritative in shared module/callable resolution and defers imported-call local hover to project analysis. `node internal/audits/features-2026-11-07/post-fix-review/declaration.js` now confirms signature, member hover and local-result hover all say `string`. The disk-backed Rust regression also exercises runtime files, stale output, subsequent deletion, incompatible arguments, and an inner local shadow with a `number` signature.

**SIGNALS-03** actual LSP definition resolves `events.tua` immediately, after idle and after opening the dependency. References include the declaration and consumer. See [responses](post-fix-review/signals/lsp-evidence.json).

**EDITOR-G01** remains assigned for one final follow-up: c39d757 selects the first compatible literal union rather than the narrowest literal overload. The fresh response still chooses generic return `number` for `get("move")`, although the precise `number, number` signature is present. Follow-up 5acbb1e ranks the smallest matching literal set and passes 39 signature tests; the final rebuilt-binary actual LSP replay passes with activeSignature 2 and the `number, number` pair result.

## Managed game-process restart

`python3 internal/audits/features-2026-11-07/post-fix-review/dev-restart.py` passed with actual CLI/watch processes and a disposable fake LOVE executable. Changing build→moved terminates/replaces the child (distinct PIDs), installs the hook and generation2 signal in moved, and propagates replacement child exit0. [raw](post-fix-review/dev-restart.json). This verifies process lifecycle, not graphical LOVE behavior.

## Ownership and cache adversarial checks

- Source==output declaration preserves handwritten Lua; separate-output unowned declaration preserves bytes; manually maintained maps survive disabled maps. All builds exit0. [three fresh CLI cases](post-fix-review/ownership.json).
- Changing pipeline output root refuses a same-name unowned destination with TL2011 and retains `USER AUTHORED CONTENT`. [fresh CLI case](post-fix-review/pipeline-root.json).
- Corrupting payload bytes in all cached entries forces the active entry to regenerate and materializes `GENERATED CONTENT`; the inactive corrupted entry remains unused. [raw](post-fix-review/cache-integrity.json).
- New unit coverage checks modified owned files and unowned files survive targeted cleanup while unchanged manifest-owned files are removed. Root guards prohibit this cleanup in overlapping source/output trees. Existing symlink ownership probes pass.
- Code review: pipeline root state is conservative for legacy state; same-content adoption remains existing intentional behavior. Cache fingerprints check bytes rather than only existence. Output cleanup validates ownership/fingerprint through a common helper. No newly confirmed bypass found in this bounded review. Atomic multi-output pipeline rollback, hostile concurrent filesystem races and a complete multiroot matrix were not qualified.

## Review of shared editor fixes

Formatter uses the existing literal state machine and restores opening-line trailing literal whitespace; active multiline closing-line logic remains unchanged. Runtime equivalence was checked, not just idempotence. Linter now uses statement spans and consumes at most its own semicolon, preserving adjacent statements; actual protocol edits were executed under Lua. Completion enumerates structured literals/unions and accounts for implicit self; domain literals now pass the fresh replay above. These are bounded reviews, not universal parser/Unicode proofs.

## Per-finding reconciliation

A commit reference means implementation exists; verification scope above and owning-agent results still govern closure. All 65 confirmed defects have implementation commits. This does not make all platform, external-tool, graphical, hardware, or long-session workflows qualified. The six separately classified feature/design rows remain distinct.

| Finding | Classification | Commit(s) | Review status |
|---|---|---|---|
| STORES-03 | confirmed defect | 25d7ff2 | committed; see verification scope |
| SIGNALS-01 | confirmed defect | 37f968f | committed; see verification scope |
| SIGNALS-02 | confirmed defect | 2ebe14c | committed; owning agent focused tests passed |
| SIGNALS-03 | confirmed defect | f66d9f4 | actual LSP definition and references replay passes |
| SM-01 | confirmed defect | 57c2d2f | committed; owning agent focused tests passed |
| SM-02 | confirmed defect | ad49394 | committed; owning agent focused tests passed |
| SM-03 | confirmed defect | 22ece87 | committed; see verification scope |
| SM-04 | confirmed defect | 107ced4 | committed; see verification scope |
| ECS-01 | confirmed defect | 068ae03 | committed; see verification scope |
| ECS-02 | confirmed defect | 87b0266 | committed; see verification scope |
| ECS-03 | confirmed defect | 773e2fd | committed; owning agent focused tests passed |
| ECS-GAP-01 | feature gap | 65f1af9,653eac9 | committed; owning agent focused tests passed |
| POOLS-01 | confirmed defect | 616ab74 | committed; owning agent focused tests passed |
| POOLS-02 | confirmed defect | af82045 | committed; see verification scope |
| INPUT-01 | confirmed defect | f48f41f | committed; see verification scope |
| INPUT-02 | confirmed defect | d064a06 | committed; owning agent focused tests passed |
| AUDIO-01 | confirmed defect | 68a72d7 | committed; see verification scope |
| AUDIO-02 | confirmed defect | f38674d | committed; see verification scope |
| AUDIO-03 | confirmed defect | 9954517 | committed; see verification scope |
| AUDIO-04 | confirmed defect | c6c43c0 | committed; see verification scope |
| AUDIO-05 | confirmed defect | 05a17d6 | committed; see verification scope |
| SPRITES-01 | confirmed defect | bc1934d,a171e82 | committed; see verification scope |
| SPRITES-02 | confirmed defect | 5c3987b | committed; see verification scope |
| SPRITES-03 | confirmed defect | 2b0f3b8 | committed; see verification scope |
| VECTORS-01 | confirmed defect | 850ab89 | committed; owning agent focused tests passed |
| VECTORS-02 | confirmed defect | 399d764 | committed; owning agent focused tests passed |
| VECTORS-03 | confirmed defect | bb78a91 | committed; see verification scope |
| VECTORS-04 | confirmed defect | ecbc226 | committed; see verification scope |
| CAMERA-01 | confirmed defect | 7b9ef3f | committed; owning agent focused tests passed |
| CAMERA-02 | confirmed defect | 54086a2 | committed; see verification scope |
| CAMERA-03 | confirmed defect | 4de9e0f | committed; see verification scope |
| CAMERA-04 | confirmed defect | b1efbe7 | committed; see verification scope |
| CAMERA-05 | confirmed defect | 5a872b9 | committed; see verification scope |
| TWEEN-01 | confirmed defect | f0c5b1b | committed; see verification scope |
| TWEEN-02 | confirmed defect | cc0298c | committed; owning agent focused tests passed |
| CLASS-01 | confirmed defect | eb79b03 | committed; see verification scope |
| TYPING-01 | confirmed defect | ac3875b | committed; see verification scope |
| TYPING-02 | confirmed defect | 77c9d5f | committed; see verification scope |
| TYPING-03 | confirmed defect | f487658 | committed; see verification scope |
| TYPING-04 | confirmed defect | df09e71 | committed; see verification scope |
| FUNCTIONS-01 | confirmed defect | dbbb2ca | committed; owning agent focused tests passed |
| FUNCTIONS-02 | confirmed defect | 7ec48dd | committed; owning agent focused tests passed |
| FUNCTIONS-03 | confirmed defect | d1b02a1 | committed; owning agent focused tests passed |
| MODULES-01 | confirmed defect | 6450b50 | committed; owning agent focused tests passed |
| MODULES-02 | confirmed defect | 5a82e0a | committed; owning agent focused tests passed |
| MODULES-03 | confirmed defect | 8210170,876101c2 | declaration/docs/shadow regression, 10 LSP signature tests, actual LSP replay pass |
| LUA-INTEROP-01 | confirmed defect | 59efb09 | committed; owning agent focused tests passed |
| LUA-INTEROP-02 | confirmed defect | 6f1100a,66cd38b,cad544d | committed; owning agent focused tests passed |
| LOVE-01 | confirmed defect | 1939ad7 | committed; owning agent focused tests passed |
| LOVE-02 | feature gap | 929810c | committed; owning agent focused tests passed |
| ASSETS-01 | confirmed defect | d97050c | committed; see verification scope |
| ASSETS-02 | confirmed defect | e72380e | committed; see verification scope |
| SHADERS-01 | confirmed defect | 8007938 | committed; see verification scope |
| FMT-01 | confirmed defect | 933c951 | committed; see verification scope |
| LINTER-01 | confirmed defect | 332f928 | committed; see verification scope |
| LINTER-02 | design question | bf2e845 | committed; owning agent focused tests passed |
| TESTING-01 | confirmed defect | 3e858f1 | committed; see verification scope |
| PROJECTS-01 | confirmed defect | db90df8 | committed; see verification scope |
| WATCH-01 | confirmed defect | c68c608 | committed; see verification scope |
| WATCH-02 | confirmed defect | 373bea1 | committed; see verification scope |
| DBG-01 | confirmed defect | a767cf4 | committed; see verification scope |
| EDITOR-01 | confirmed defect | d5e3b93,ff8c5da | actual LSP completion replay passes |
| EDITOR-02 | confirmed defect | 7f09d85 | committed; see verification scope |
| EDITOR-G01 | feature gap | c39d757,5acbb1e | 39 signature tests and actual LSP replay pass |
| EXPLORERS-01 | feature gap | 2516078 | committed; see verification scope |
| EXPLORERS-02 | feature gap | 0015e47 | committed; see verification scope |
| API-MCP-01 | confirmed defect | 322ad16 | committed; see verification scope |
| API-MCP-02 | confirmed defect | 671e394 | committed; see verification scope |
| API-MCP-03 | confirmed defect | eb5837b | committed; see verification scope |
| PACKAGING-01 | confirmed defect | 34fed98 | committed; see verification scope |
| SETUP-DOCS-01 | confirmed defect | fc7c5c8 | committed; see verification scope |

## Pending qualification

Full tests after final source edits; comprehensive GUI/hardware/platform/model/long-session checks from the audit remain unqualified. Narrative design questions are not silently counted as implemented capabilities.

## Resolved environment interruption

Cargo briefly failed with ENOSPC at 115 MiB free. The lead removed only regenerable `target/debug/incremental` cache, recovering about 43 GiB. Subsequent focused MODULES-03 tests and debug CLI build passed with `CARGO_INCREMENTAL=0`. No product files were removed. Native shader retries required execution outside the sandbox as recorded above.

## Reproduction instructions

Run scripts from the repository root after `CARGO_INCREMENTAL=0 cargo build -p tua-cli`. Node scripts use the repository LSP harness; Python formatter/linter execution requires Lua on PATH. Each script creates disposable fixture directories adjacent to itself. Raw JSON preserves the original absolute paths as capture metadata. The committed scripts and signal fixtures suffice to recreate the test projects; generated build outputs are intentionally excluded.

## Extension activation environment check

Two lead-run activation checks timed out waiting five seconds for the freshly generated Node shebang mock's `--version` process. An independent identical direct smoke run then passed, followed by three unchanged repeated runs in 1.240s, 1.294s and 1.220s (all exit0). A fresh standalone shebang probe took 379 ms on first launch, 26 ms through direct Node, 29 ms on repeat. No persistent source defect was reproduced and the production timeout was not increased. The final integrated gate remains the release authority; this intermittent host/process-startup limitation is retained in the evidence.

## Integrated regression follow-up

Full LSP testing caught loss of authored alias labels and parameter documentation after MODULES-03's structural callable reconstruction. Follow-up 876101c2 preserves presentation from cached, snapshot-owned module exports. It uses the authoritative declaration path, requires a visible import binding, and compares the resolved structural contract before applying presentation metadata, preserving specialized domain signatures. The declaration overlay regression (including docs and local shadow controls) and all 10 LSP signature tests pass. Recovery fixture 7bc070be now expects both `love.conf` strings to be nullable; observed actual hints were correctly `string | nil`, so no inference behavior was changed for that assertion.

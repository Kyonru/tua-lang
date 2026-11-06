# Verification log, scope and reproducibility

## Baseline

- Audit local date: 2026-11-06 (Asia/Tokyo); environment observation
  `2026-11-05T21:49:34Z`.
- Revision: `d7aed2e5f1297b80a948e055985375bceaff6f0c`.
- Initial working tree clean. Final changes are audit artifacts only under
  `internal/audits/devex-2026-11-06/`; generated scratch/logs under ignored
  `target/devex-audit-2026-11-06/`. No product fix, commit, push, publication,
  normal-profile installation or external message was made.
- No applicable `AGENTS.md` found in repository or ancestor paths. Applied skills:
  audit-lead, language-auditor, lsp-parity, semantic-resolver, lua-defaults,
  love-devex, architecture-steward and test-auditor. The matrix predates delegation.
- macOS 26.6.1, Darwin arm64; Rust 1.97.1; Node 24.15.0; Lua 5.4.8;
  actual VS Code 1.142.0 with Tua extension/server 0.5.1.
- Audited release executable SHA-256:
  `cfa1e2c9f457cdefba6af69977aa9fcf973debed82396fc3ea1ed5449e63fa61`.
  `cargo build --release -p tua-cli` completed successfully and confirmed the
  current build. Future binaries must not be assumed to reproduce this revision.

## Executed commands and outcomes

Commands are run from repository root unless a working directory is stated.
Large logs remain under `target/devex-audit-2026-11-06/`; durable raw responses,
source fixtures and summaries are beside the portable audit scripts.

| Command | Outcome / interpretation | Evidence |
|---|---|---|
| `cargo build --release -p tua-cli` | Exit 0, current release binary | `target/.../build.log`, binary hash above |
| `RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test --workspace` | Exit 0; 1,640 core tests / 1 ignored; 375 LSP; 127 CLI integration; 83 CLI unit / 2 ignored; auxiliary integration and doc tests passed | `target/.../baseline-tests.log`, [summary](evidence/baseline-summary.txt) |
| `node internal/audits/devex-2026-11-06/typing/run.js` | 64 observed CLI scenarios, 94 editor queries, 64 pull diagnostics; final run no harness errors. 36 accepted/28 rejected are outcomes, not “64 tests passed” | [typing verification](typing/VERIFICATION.md), `typing/results.json` |
| `lua internal/audits/devex-2026-11-06/typing/runtime-oracles.lua` | Three value-list runtime controls passed on Lua 5.4.8 | `typing/runtime-oracle-output.txt` |
| `node internal/audits/devex-2026-11-06/completion/probe.js` plus the omitted-context/no-snippet modes in its report | 45 scenarios in each of three modes; candidates/resolve data captured; independently replayed | `completion/evidence.json`, `context-omitted.json`, `no-snippet-support.json`; `typing/completion-command-replay-summary.json` |
| `node internal/audits/devex-2026-11-06/completion/actions.js` and `apply-edits.js` | Eight action source scenarios; returned same-document edits applied/rechecked; eight resulting source CLI checks. Expected failures/panic are findings | `completion/actions-evidence.json`, `applied-evidence.json` |
| Hover scripts listed in [hover report](hover/REPORT.md) | 15 broad and 12 focused cases plus imports/provenance/layout/cross-check variants; actual protocol and selected CLI results | `hover/*-evidence.json`, `hover/evidence.json` |
| `node internal/audits/devex-2026-11-06/workflow/unicode.js` | ASCII/BMP controls good, non-BMP standard ranges wrong; confirmed defect | `workflow/unicode-evidence.json` |
| `node internal/audits/devex-2026-11-06/workflow/lifecycle.js` | Single-root update/reload/delete/recovery controls work; two-root collision fails; matching standalone controls pass | `workflow/lifecycle-evidence.json` |
| `node internal/audits/devex-2026-11-06/workflow/freshness.js` | 24 rapid unsaved edits finish with correct number hover and empty final diagnostics; UTF-16/CRLF incremental replacement updates type correctly | `workflow/freshness-evidence.json` |
| `node internal/audits/devex-2026-11-06/workflow/crosscheck.js` | Independent callback/terminal-chain false acceptance; direct annotated-return negative control rejects; invoked autoimport omission reproduces | `workflow/crosscheck-evidence.json` |
| `node internal/audits/devex-2026-11-06/workflow/editor-run.js` | Actual isolated VS Code provider and UI: valid member positive control, missing autoimport, exact source corruption after applying rename; exit 0 means automation completed | `workflow/editor-evidence.json`, `editor-suggest.png`, `target/.../editor.log` |
| `node internal/audits/devex-2026-11-06/workflow/performance.js` | Three fresh processes/project, 30 warm and 30 post-edit samples/project; expected candidates present and sampled hover/signature non-null | `workflow/performance-evidence.json`, table in workflow report |
| `../../target/release/tua check` with cwd `examples/features`, `examples/starfall`, `examples/love-full` | Exit 0; checked 34, 7, 5 source files respectively | `target/.../check-features.log`, `check-starfall.log`, `check-love-full.log` |

The existing suite has optional-backend functions that can return early when a
runtime is unavailable. Runner counts are reported as such, not proof every
optional runtime executed. This audit's direct Lua oracles and editor automation
are separately evidenced. No new cross-platform CI, full release gate, Busted
coverage backend, or external service qualification was performed.

## Cross-review and rejected overclaims

- Lead → typing: distinct callback and factory fixtures, plus emitted-Lua runtime
  output, verify both High false-acceptance findings.
- Hover → lead: independent UTF-16 rename source application; separately passing
  single-root CLI/LSP controls. Actual VS Code independently applies the bad edit.
- Typing → lead: differently named importers and module keys verify multi-root
  contamination. Original lead B importer had a mismatched annotation;
  it was corrected to a matching string contract and standalone controls were
  added/rerun before final synthesis.
- Typing → hover: distinct same-name function fixtures confirm High lexical
  lookup failure. Arithmetic prefix controls confirm native-class hover loss.
- Hover → completion: ordinary typed header comment plus next-line comment
  control confirms the panic independently of the suppression action.
- Lead → completion: distinct autoimport fixture, omitted/invoked/retrigger
  requests and actual VS Code verify invoked identifier omission.
- Typing → completion: independent double-quote filename, `repeat` key, and
  `serviceXXX` suffix fixtures confirm insertion defects; reran all three modes.
- Completion → hover: independent Font method chain, Hero class expression
  prefix, and required-nilable versus optional parameter controls confirm medium
  findings. Reviews: [typing](typing/REVIEW-completion.md),
  [completion](completion/REVIEW-hover.md), [hover](hover/REVIEW.md).
- Typing → completion: distinct Tank/fuel/getFuel fixtures independently confirm
  CMP-07 whitespace dependence; checker rejection remains intact. See
  [class review](typing/REVIEW-class-whitespace.md). Direct constructor-chain
  behavior U-01 remains observed with unresolved attribution.
- 94 typing probe positions checked against exact source bounds in the review.
- Initial incorrect probe anchors and an initial shell command using one too many
  `../` segments were harness errors, corrected and rerun, not product findings.
- An initial screenshot DOM visibility heuristic (`offsetParent`) was unsuitable
  for a fixed-position suggestion widget. Visual inspection confirms the visible
  “No suggestions” popup; the heuristic was not used as evidence of visibility.

## Verification limits and explicit non-claims

- Tested means the named representative scenario ran, not every permutation of
  the topic. Each matrix row lists subcase limitations. No universal exhaustiveness.
- Third-party editor behavior is protocol-only; only VS Code was automated.
- Native Linux/Windows execution and standalone LuaJIT oracles were unavailable
  in this host audit. Lua 5.4 arithmetic/value-list observations are labeled.
  LOVE runtime API overload accuracy was not exhaustively compared against a
  separately fetched upstream catalog or every runtime operation.
- No hours-long human editing session, full-game visual parity or memory leak
  certification. Latency is stdio round-trip on three bundled projects, not a
  keystroke-to-menu benchmark, regression comparison, or performance guarantee.
- Inlay hints and Markdown were inspected via protocol and selected real editor
  provider calls; every rendered hover/signature variant was not screenshot-tested.
- Missing-module `CreateFile` quick-fix structure was inspected, not applied by
  the action harness. Inline-model/Ollama suggestions were intentionally not
  exercised; normal semantic completion was the focus.
- Generics/protocols and several documented narrowing/annotation limitations are
  capability/design notes, not counted as bugs. Unverified snippet filtering is
  excluded from confirmed findings.
- Source references support likely roots; no fixes were implemented, so no
  experimental “this patch fixes it” proof is claimed.

## Re-running safely

Build the release binary, run the lane scripts from repository root, and read
source/expected-vs-actual alongside JSON. `TUA_AUDIT_BINARY` overrides the shared,
typing and hover harnesses; completion's primary scripts use `TUA_BIN` instead.
The completion lane must run `probe.js` before `actions.js`/`apply-edits.js`.
Actual editor automation needs the repository's existing npm dependencies and
local graphical VS Code; set `VSCODE_EXECUTABLE_PATH` for another installation.
Scripts write only audit evidence and disposable/ignored fixtures. They are
observational reproducers, not a replacement for regression assertions.

## Final artifact validation

All saved JSON parsed successfully, all 26 JavaScript reproducers passed
`node --check`, and local Markdown links resolved. The inventory contains 58
unique coverage IDs (57 tested, one inspected only) and 18 unique confirmed
finding IDs (eight High, ten Medium), plus U-01 with unresolved attribution.
Audit artifacts occupy approximately 1.9 MB. Final `git rev-parse HEAD` still
reports the baseline revision; `git diff --stat` is empty and `git status --short`
shows only the untracked audit directory. No product files were modified.

An independent final synthesis review read all top-level reports and the finding
inventory, reconciled counts and High cross-checks, and found no blocking
overclaims. Its two wording corrections (optional/default display and the
original mismatched module control) were incorporated.

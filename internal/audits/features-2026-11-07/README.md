# Tua feature audit — 2026-11-07

31 dedicated feature agents tested Tua's language, game domains, editor and distribution tools. Two additional agents independently reviewed evidence and coverage. The audit found **65 distinct confirmed defects: 8 High, 49 Medium and 8 Low**, after consolidating four duplicate records. All eight High findings were independently reproduced. One counted defect, TYPING-02, is a residual of a previously known family.

Tua has a broad, working foundation: representative typed modules, inheritance, domain runtimes, actual LSP requests, Busted execution, native debugging and packaging work. Reliability is less consistent at feature boundaries: imported domain types, shortened callbacks, nested runtime callbacks, constructor lowering and editor assistance can disagree with runtime behavior. Existing passing suites do not cover these combinations sufficiently.

## Start here

- [Findings index](FINDINGS.md): every stable ID, severity and link to its detailed reproduction.
- [Prioritized backlog](BACKLOG.md): shared fix directions, dependencies and acceptance criteria.
- [Coverage matrix](COVERAGE.md): all 31 feature reports and qualification limits.
- [Verification log](VERIFICATION.md): baseline, checks, evidence and reproducibility instructions.
- [Independent High-finding reproductions](cross-checks/README.md) and [review conclusions](reviews/evidence-review.md).

## Most consequential problems

1. **File ownership:** changing pipeline output roots can overwrite unowned content (ASSETS-01); build cleanup can delete handwritten files (PROJECTS-01).
2. **Behavior-changing edits:** formatting changes long-string contents (FMT-01); a “safe” unused-local fix removes neighboring statements (LINTER-01).
3. **Language soundness:** constructor lowering loses self identity (CLASS-01); dictionary dot writes and missing reassignment values bypass type checks (TYPING-01/02).
4. **Runtime hangs:** zero-duration animation lists can make update nonterminating (SPRITES-01).

Repair these before treating the candidate as release-ready. Next address shared callback/nil/call validation and module schema propagation, then runtime reentrancy and editor consistency. The backlog separates these from optional capabilities and unresolved policy choices.

## What was dependable in the exercised cases

Positive controls include normal inheritance/virtual dispatch, common imports and declaration resolution, standard store/event/state transitions, sprite playback, configuration/build paths, Busted filtering/reporting, Unicode editor requests, asset metadata, graph protocols and deterministic inline arbitration. Fresh shared checks passed 389 LSP tests and four Lua runtime suites. Actual native LOVE, DAP and fused macOS app checks succeeded in selected lanes. Consult each report for precise controls; a passing example does not qualify the entire feature.

## Limits and preserved work

This is broad representative testing, not an exhaustive proof across every configuration, platform or GUI interaction. Audio/rendering/input tests sometimes use doubles. Explorer and extension integration tests include mocked VS Code APIs. Real Ollama generation was unavailable; long-session latency, full platform installation, audible output and complete visual behavior remain unqualified. All 105 editor commands were assigned an audit owner, not individually executed in the desktop UI.

The seven pre-existing 0.6.0 release metadata changes were preserved. No product fixes, commits, pushes or publication were performed. Two existing nominal-type test failures and the inline snapshot mismatch remain baseline release blockers, separately recorded in [BASELINE.md](BASELINE.md). Audit artifacts live only in this directory (ignored by the repository's existing rules).

# Verification log and reproduction guide

## Baseline and preservation

Audit revision: `9d490dcd299bd25218c9fa4adc83e080e617453c`, macOS arm64, Lua 5.4.8, candidate 0.6.0. The tested release CLI SHA-256 is `cabb9291f29eba8a1d007eb248ffc40d800622cd927afcc1086c2527eb9feef4`. See [BASELINE.md](BASELINE.md) for the initial dirty state and previous release checks.

Seven release metadata files were already modified: CHANGELOG.md, Cargo.lock, Cargo.toml, docs/releases.md, editors/vscode/package-lock.json, editors/vscode/package.json and fuzz/Cargo.lock. Final Git status has the same seven tracked modifications. The audit made no product edits or commits. Its generated projects and fake user files are disposable fixtures, not user projects.

Known baseline blockers remain the two nominal-assignment core tests (`check::tests::vec2_rejects_plain_structural_tables`, `types::tests::class_alias_targets_reject_structurally_matching_plain_tables`) and inline-eval-smoke snapshot mismatch. Prior release results are context, not fresh results from this audit.

## Fresh common checks

Run from repository root:

```sh
make runtime-test
RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -p tua-lsp
```

- Runtime: four suites passed (tween, camera, hot reload and Baton); [raw log](runtime-baseline.log). This target is not coverage of all game helpers.
- LSP: 389 passed, zero failed; [raw log](lsp-baseline.log).
- Each of the 31 feature agents exercised its own fixtures and recorded exact commands/outcomes in its [linked report](COVERAGE.md). These include real CLI builds/runs, LSP requests, helper/runtime probes, focused unit tests and labeled mock integrations.
- All eight High findings were independently reproduced by the lead: [commands and evidence](cross-checks/README.md). Additional Medium findings were cross-checked there too.
- A separate evidence reviewer checked High-finding support and reran audio probes; a separate coverage reviewer reconciled all reports, inventory and command ownership. [Evidence review](reviews/evidence-review.md); [coverage review](reviews/coverage-review.md).
- Manifest ownership reconciliation: 105 commands, each assigned once; 74 settings inventoried. Neither inventory proves every GUI action or setting combination was executed.

## Running reproductions

Start with a finding in [FINDINGS.md](FINDINGS.md), open its feature REPORT.md and use its exact fixture and command. Scripts generally assume this repository and its built `target/release/tua`; inspect paths and documented environment before rerunning on another machine. Preserve the original evidence when collecting a new run. Some fixtures deliberately trigger diagnostics, runtime failures or bounded hang detection; a nonzero result can be the expected reproduction, not a harness failure.

Ownership tests operate only on disposable fake content. Native LOVE and DAP probes may require a GUI-capable process and loopback access. Do not substitute a timeout or sandbox abort for a proven application defect. Each report identifies where Lua/LOVE doubles replace hardware or graphics, and where a subsequent native run succeeded.

The canonical count comes from [findings.json](findings.json), which normalizes severity and removes four duplicate records while preserving aliases. Original per-feature JSON remains available. Confirmed symptoms are distinguished from inferred root causes in the detailed reports; a proposed shared fix is not proof that two findings share identical implementation defects.

## Evidence limits and corrected attempts

- Audio uses deterministic Source doubles for its confirmed runtime failures; audible output, DSP/device behavior and all worker modes were not qualified.
- Camera uses arithmetic/rendering doubles. Input hardware and visual output are not comprehensively tested.
- Selected actual LOVE runs, sprite hang detection, native DAP and macOS fusion succeeded. Earlier sandbox aborts are recorded rather than counted as bugs. A success in one lane does not retroactively qualify another lane's visual workflow.
- Explorer sidebar and several extension checks use production providers with mocked VS Code APIs. This audit does not claim full desktop interaction coverage.
- Ollama's local endpoint refused connection; deterministic inline requests, arbitration and cancellation were exercised, real model quality was not.
- LuaCov was unavailable for full coverage execution; Busted execution/reporting and adapter behavior were exercised.
- Windows fused-output structure was checked, not Windows native execution. External publishing, registry installation, public services and remote CI runs were not performed.
- No comprehensive long-session, latency percentile, accessibility or platform matrix was completed.
- The initial explorer `output`/`out` fixture typo, store direct-file CLI invocation, packaging fixture-name correction and harness syntax/logging corrections are not findings. Corrected evidence controls the conclusions.
- Native watcher verification required the final reconciliation window; initial short waits were inconclusive rather than confirmed missed events.

## Final reconciliation

31 feature reports completed. 75 raw structured records = 69 defect records + five capability gaps + one design question. Deduplication yields **65 confirmed defects: 8 High, 49 Medium, 8 Low**, plus the six separately indexed nondefects. Additional narrative opportunities are intentionally not inflated into defect counts.

Store callback and pool callback symptoms map to TYPING-03; store optional nil maps to TYPING-04; shader hover formatting maps to EDITOR-02. MODULES-01/02 resolve initial store/ECS import observations. AUDIO-01 is Medium, and parent-bus inheritance is an open policy question. TYPING-02 is a known residual family. Full workspace release readiness was not asserted.

# Testing / Busted / Test Explorer audit

One confirmed medium-severity diagnostic defect; no demonstrated execution or source-map correctness defect in the sampled workflows. Audit only; product files unchanged. Used tua-audit-lead, tua-language-auditor, tua-test-auditor and tua-review instructions.

## Execution and evidence

From repository root:

```sh
python3 internal/audits/features-2026-11-07/testing/probe.py
python3 internal/audits/features-2026-11-07/testing/edges.py
node internal/audits/features-2026-11-07/testing/cancel.js
node --test editors/vscode/test/unit/testing.test.js
```

Scripts use the absolute existing `target/release/tua` (0.6.0), installed Busted 2.3.0 and Lua 5.4.8. No builds or dependency installs. Each CLI invocation records argv, cwd, exit code, stdout and stderr in its named JSON file. Scripts reconstruct their own fixtures; `probe.py` deliberately leaves type-invalid and failing specs for inspection, and `edges.py` later leaves a syntax-invalid spec. Rerun scripts in the order above to reproduce their staged probes.

| Area | Status / result | Evidence |
|---|---|---|
| Typed DSL, hook, assertions, production module import | Executed: typed number, describe/context, before_each, it and pending compile and run | all-no-coverage-results.json: 5 passed, 1 skipped |
| Mixed Lua/Tua and recursive discovery | Executed: both specs found, nested suites/positions, escaped literal decoded; dynamic Lua cases execute at file level | discovery.json, all-no-coverage-results.json |
| Suite, tags, zero-match filter | Executed: suite selects 2 passed + 1 pending, excludes neighboring mathematics; integration tag selects 2; no match succeeds with empty tests | suite-results.json, tags-results.json, empty-results.json |
| Structured reports and noisy print | Executed: arbitrary print text preserved without breaking JSON | tags-results.json |
| Source mapping with emit_maps=false | Executed: assertion location maps to tests/failing_spec.tua line 3; ordinary Lua locations remain original | failure-results.json, all-no-coverage-results.json |
| Compile errors / stale reports | Executed: type error in unselected spec blocks selected run, report completed:false | compile-block.json, compile-results.json |
| Parse-error discovery | Executed: list returns file-level parser error without invoking backend | parse-discovery.json |
| Backend configuration | Executed: missing executable fails informatively, CLI --busted overrides invalid TUA_BUSTED; missing interpreter fails, but report masks reason | missing-backend.json, precedence.json, missing-lua*.json |
| Conflicting report destinations | Executed: same report/coverage destination rejected | same-output.json |
| Production exclusion | Executed: build compiles only 2 production files | build-excludes-tests.json, edges-fixture/build/ |
| Cancellation | Executed real CLI + real Busted infinite test via extension process runner: canceled in 503 ms | cancel.js, cancel-results.json |
| Adapter TestItems / selection / continuous save events / cancellation / coverage merging / output bounds | Existing focused tests executed: 7 passed. VS Code API mocked; subprocess checks real | editor-unit.log; editors/vscode/test/unit/testing.test.js |
| Saved/unsaved editor behavior | Inspected: adapter invokes disk-based CLI and filesystem watchers; no unsaved-buffer feed. No new actual GUI replay | editors/vscode/src/testing/index.js:56, 225 |
| LuaCov actual line coverage | Blocked: require('luacov.runner') fails. Coverage-only error offers installation hint; combined --report obscures it | luacov-check.json, coverage-only.json, all.json |
| Actual installed editor continuous run, coverage UI, workspace churn | Not exercised this pass; earlier gutter verification does not establish these | ROADMAP.md:90-94, 137-139 |
| Rich helper declarations, LSP hover/completion/signatures | Implementation inspected, broad helper and LSP matrix not exercised in this bounded pass | crates/tua-core/src/busted.rs |

## TESTING-01 — Structured reports hide backend startup diagnostics

- **Severity:** medium. **Category:** confirmed defect. **Status:** reproduced. **Known:** not found in reviewed prior audit/ROADMAP text; existing LuaCov unavailability itself is known.
- **Affected behavior:** Every Test Explorer run requests `--report`; a misconfigured interpreter therefore presents an unrelated missing-report error instead of the actionable startup error. Coverage profile also requests both reports, hiding the missing LuaCov hint.
- **Exact reproduction:** Run `probe.py` above and compare `missing-lua.json` against `missing-lua-no-report.json` from `edges.py`. Both select `/no/such/lua`; only the first adds `--report`. The fixture has valid specs at the time of these invocations. Alternatively use any valid project: `tua test --lua /no/such/lua --report results.json`.
- **Expected:** Nonzero status and incomplete report, while retaining the captured backend diagnostic identifying `/no/such/lua`. Reporting failure can be appended. This follows docs/testing.md's preservation of terminal output and is necessary to repair backend configuration.
- **Actual:** With report: `error: Busted did not produce a test report: No such file or directory (os error 2)`. Without report: `unit tests failed (exit status: 1)` followed by `sh: /no/such/lua: No such file or directory`. Incomplete JSON remains safe, but omits captured output. Missing LuaCov with combined reports has the same generic error; coverage-only explicitly suggests installing LuaCov.
- **Source:** crates/tua-cli/src/testing.rs:242-253 propagates report-save error before returning captured output/status at 255-260; testing/report.rs:16-18 returns only raw-file read error. Test Explorer always supplies report (editors/vscode/src/testing/model.js:43).
- **Existing blind spot:** crates/tua-cli/tests/cli.rs `unit_test_json_report_keeps_stdout_and_maps_native_and_lua_locations` verifies successful backend report and compiler-error reset. Coverage test skips without installed LuaCov. Neither asserts startup stderr remains visible when report generation never starts.
- **Smallest improvement:** Preserve captured backend status/output when report/coverage finalization fails, while retaining completed:false and the reporting diagnostic. Keep this in shared CLI finalization so terminal and Test Explorer benefit together.
- **Regression:** Existing CLI integration harness, valid spec plus Busted selected with nonexistent Lua; assert nonzero, completed:false, and interpreter path in stderr under --report. A deterministic fake backend emitting a unique stderr marker then exiting before writing report can cover this cheaply without requiring Busted. Add combined-report missing-LuaCov case where dependency availability can be controlled.

## Explicit limitations and options

Documented limitations are not counted as defects: no CLI watch option; continuous mode belongs to the editor; no arbitrary Busted option pass-through; project `.busted` is not automatically loaded; LOVE context is not created; coverage is line-only; unsaved buffers are not run. LOVE test mode and installed-editor continuous/coverage qualification are known ROADMAP follow-ups. Custom extensions, duplicate names, excluded dynamic descendants, Windows process trees, large-scale discovery and prolonged continuous operation were not newly qualified. The 7 passing adapter tests and previous GUI gutter replay must not be interpreted as that broader qualification.

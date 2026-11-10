# 0.7.0 generics and polymorphism release reconciliation

Date: 2026-11-10. Audited implementation revision before this pass:
`adeff7ad`. This pass compared the plan and the November 9 and November 10
follow-up/final audit records named in the user request with the current code,
tests, emitted-Lua fixtures, and installed-editor suite. Earlier findings in
those documents are historical unless the final reconciliation says otherwise.

## Conclusion

No new generics or protocol correctness defect was reproduced in the documented
0.7.0 language surface. P0–P5 and G0–G3 have implementation and regression
coverage. R0, the release-qualification phase, remains partial because it
requires operating systems and runtime/editor sessions unavailable here. The
current package is a macOS arm64 manual beta artifact, not evidence of
cross-platform release readiness.

One current build gate defect was confirmed: `make verify` failed Clippy in
the shader-include code introduced after the generics audit. Two function
signatures in `crates/tua-core/src/analysis/shader_queries.rs` exceeded the
type-complexity lint, and `crates/tua-core/src/asset/mod.rs` retained a
single-binding match. Commit `022b1966` factors the shared result types and
removes that match. No shader analysis behavior was changed.

## Phase reconciliation

| Phase | Current result | Evidence and boundary |
| --- | --- | --- |
| P0 contract | Implemented | Nominal method-only protocols, explicit conformance and mutable-container soundness have positive and negative core/CLI tests. |
| P1 syntax | Implemented | Parser, erasure, comments, malformed-header recovery, Unicode spans and later declarations are covered. Arbitrary malformed edit streams are not exhaustively enumerated. |
| P2 conformance | Implemented | Local/imported requirements, inherited local implementations, variance, argument/return mismatches, and related ranges have core/LSP matrices. Cross-module class inheritance remains outside the documented contract. |
| P3 modules | Implemented | Type imports, re-exports, declaration files, cycles, missing/create/delete/rename, and unsaved dependency changes have regressions. All scheduler interleavings remain an unbounded qualification space. |
| P4 editor | Verified on this host | Core/LSP and the isolated installed VSIX suite exercise completion, hover, signatures, definitions, implementation navigation, references/rename, diagnostics and edits across files. Other clients/platforms remain untested. |
| P5 examples/runtime | Verified with system Lua | Emitted generic/protocol Lua, dispatch, tuple returns, inheritance order, source maps and traceback fixtures pass. LOVE runtime launch timed out in this session; visual game behavior remains unverified. |
| G0 design | Recorded | Binder identity, inference limits, substitution, variance, and erasure contracts are in the plan. |
| G1 aliases | Implemented | Exact arity, substitution, recursive limits, imports/re-exports and editor displays have core/project coverage. |
| G2 functions | Implemented for documented scope | Independent instantiation, callbacks, multi-return slots, imports and per-check budget pass. Generic methods and explicit call-site type arguments are deliberately unsupported. |
| G3 classes/bounds | Implemented for documented scope | Constructor/member/parent substitution, `super`, invariant mutable positions, one-protocol bounds, imports and editor navigation pass. Generic protocols, compound bounds and cross-module class inheritance remain design work. |
| R0 release qualification | Partial | Current-host build/test/editor gates pass after the Clippy fix. Linux, Windows, Intel Mac, a second LSP client, live LOVE/LuaJIT, visual review and long-session memory measurements remain open. |

## Verification and limits

- The previous full workspace run after the behavior-tree addition passed
  1,915 core tests, 446 LSP tests, CLI/gameplay/runtime suites, and all 180
  extension checks with `RUST_MIN_STACK=16777216`.
- The exact pre-fix VSIX installed in an isolated VS Code 1.142.0 profile and
  passed `npm --prefix editors/vscode run test:vscode-real` with its bundled
  server, covering generic and protocol editor workflows. Rebuild/replay of
  the final package is recorded below when complete.
- `TUA_TEST_LOVE_RUNTIME=1` with the installed LOVE 11.5 executable timed out
  before stdout/stderr from the minimal example. Earlier reports saying LOVE
  was absent refer to their own checkpoints; on this host LOVE is installed
  but a working graphical launch was not available for this run.
- The prior performance audit measured a 1,000-module LSP fixture with a
  2.53-second cold completion and 1,106 MiB peak sampled RSS in a narrowed
  run. Its contention requests were cancelled, so that result is a
  qualification/performance risk, not a verified functional regression. It
  remains a measured follow-up without a pre-change baseline.
- The two pre-existing edits in `crates/tua-cli/tests/cli.rs` and
  `internal/audits/generics-polymorphism-0.7.0-final/REPORT.md` were preserved
  and excluded from this pass's commits.

## Release decision

The documented language implementation is ready for manual beta use on this
host. General release claims still need the R0 environments above. The
deliberately unsupported language features require separate design decisions;
they are not missing pieces of the implemented 0.7.0 contract. A VSIX and its
SHA-256 are recorded in the final package addendum below.

## Final package addendum

After commit `022b1966`, `env RUST_MIN_STACK=33554432 make verify` passed in
full. The gate included the workspace Rust tests, Clippy with warnings denied,
language evaluation, version/schema/reference checks, JSON and documentation
checks, and all 180 VS Code extension checks. The earlier Clippy failure no
longer occurs.

`make extension-package` rebuilt the 40-file macOS arm64 VSIX and passed its
packaging checks. `unzip -tq editors/vscode/tua-0.7.0.vsix` found no archive
errors. The exact rebuilt VSIX was installed into an isolated VS Code profile;
`npm --prefix editors/vscode run test:vscode-real` exited 0 with the bundled
server, including imported generic class/member signatures and definitions,
protocol implementation/rename, recursive protocol hover/completion, and
unsaved generic type refresh. SHA-256:
`15bfcee79075ebcc8ddbce786ce20c13922a75bb9f4c533bc3ecee2085f6855d`.

This final host check changes the beta artifact and Clippy gate status. It does
not complete the operating-system, LOVE graphical-runtime, second-client, or
long-session performance qualification listed under R0.

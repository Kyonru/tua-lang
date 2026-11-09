# Generics and protocols follow-up audit — November 10, 2026

## Summary

The generic/protocol implementation plan is substantially implemented, and the
existing suite contains broad same-file, cross-file, editor, and runtime
coverage. This follow-up found one correctness issue in source preservation:
comments inside generic class headers were discarded by Lua emission. The
class-lowering parser preserved the comments, but attached them to a generated
type alias that the runtime emitter removes. The comments now precede the
generated runtime class declaration instead, and regression tests verify that
generic function and class bound comments survive in valid Lua 5.1 output.

The previously identified recursive protocol identity case is now covered by
same-file and cross-file conformance, hover, and completion tests. The full
`tua-core` suite passes these cases. This audit did not find a new failure in
generic substitution, protocol bounds, import/re-export propagation, or the
tested editor surfaces.

## Finding

### GP-FU-001 — Generic class-bound comments disappeared from emitted Lua

- **Category:** source preservation / lowering
- **Severity:** Low
- **Status:** Fixed and verified
- **Affected workflow:** formatting/commenting a generic class header and
  building or emitting that source
- **Reproduction:**

  ```tua
  protocol Runnable {
    run(): nil
  }

  final class Box<
    T -- binder note
    : -- bound note
    Runnable -- trailing note
  > {
    value: T
  }
  ```

  Before the fix, parser normalization retained these comments, but the emitted
  Lua contained none of them. The generic class still parsed and type-checked,
  so the defect was silent unless a user inspected built output or relied on
  comments for explanation or tooling directives.
- **Cause:** class lowering placed preserved header comments before the
  generated `type Box = {}` alias. Runtime emission erases that alias together
  with its leading trivia.
- **Fix:** preserve header comments after the generated type alias and before
  the emitted runtime class table. The comment scanner skips string literals
  and handles Lua line and long-bracket comments.
- **Regression coverage:** `generic_class_bounds_allow_comments_between_tokens`
  validates parsing; `generic_class_bound_comments_survive_type_erasure`
  validates emitted comments exactly once and parses the result as Lua 5.1.
  Generic function bounds have matching parser and emission coverage for
  comments before/after the colon and trailing bound comments.
- **Files:** `crates/tua-core/src/classes.rs`,
  `crates/tua-core/src/classes/tests.rs`,
  `crates/tua-core/src/emit/tests.rs`,
  `crates/tua-core/src/parser/mod.rs`,
  `crates/tua-core/src/parser/normalize.rs`, and
  `crates/tua-core/src/parser/tests.rs`.

## Coverage and evidence

| Area | Status | Evidence |
| --- | --- | --- |
| Generic aliases and substitution | Exercised | Full core tests, including recursive/growing aliases, import/re-export, arity, editor queries, and resource limits |
| Generic functions | Exercised | Full core tests; inference, callbacks, varargs, returns, imports, and bound-comment parsing/emission regressions |
| Generic classes and applied inheritance | Exercised | Full core tests; local/imported constructors, fields/methods, parent substitutions, `Self`, overrides, and emitted Lua |
| Protocol conformance and dispatch | Exercised | Full core tests; explicit conformance, inherited obligations, multiple requirements, negative signatures, generic bounds, and runtime examples |
| Same-file recursive protocols | Exercised | `same_file_recursive_protocol_signatures_keep_conformance_and_editor_types` and `same_file_recursive_protocol_method_returns_keep_nominal_identity` |
| Cross-file recursive protocols | Exercised | `cyclic_imported_protocol_signatures_preserve_conformance_identity` and imported recursive method-return identity coverage |
| Completion/hover agreement for recursive protocols | Exercised | Core analysis tests assert nominal hover and completion members; extension real-server suite covers generic/protocol editor queries |
| Formatter and comment round-trip | Exercised | Generic function-bound formatter regression plus class/function bound Lua-emission regressions |
| Cross-platform editor behavior | Not exercised | This run used the local macOS VS Code installation and macOS arm64 release server |
| LOVE/LuaJIT runtime behavior | Not exercised | The protocol feature-tour runtime test uses system Lua 5.4.8; LOVE/LuaJIT are unavailable or not qualified here |
| Long-session/performance behavior | Not exercised | No soak test or release-build latency benchmark was run in this follow-up |

## Commands and outcomes

- `RUST_MIN_STACK=33554432 RUST_TEST_THREADS=1 cargo test -p tua-core` —
  passed: 1,894 passed, 1 ignored, including all generic/protocol and new
  comment-preservation regressions.
- `cargo clippy -p tua-core --all-targets -- -D warnings` — passed.
- `cargo fmt --all -- --check` — passed.
- `make verify` — did not complete successfully. Its initial concurrent run
  encountered a temporary workspace disappearing during a CLI test. A serial
  rerun passed that test, then three CLI shader-validator tests failed in this
  host: two fake-validator/timeout cases and one malformed-response case. The
  malformed-response case passed when rerun alone; the two timeout-related
  cases continued to fail individually. These tests are outside the changed
  parser and lowering paths. VS Code loopback tests were denied by the default
  sandbox in the first extension check.
- `make extension-package` with loopback access — passed. The extension check
  reported 180 passed and 0 failed; all 454 tracked JSON files validated, and
  the release server and VSIX were built.
- `make extension-real-server-test` with loopback access — passed using the
  development extension and release-mode server; the VS Code extension-host
  process exited with code 0. The suite exercised protocol and generic
  completion, hover, signatures, definitions, rename, unsaved edits, and
  implementation navigation.
- `unzip -tq editors/vscode/tua-0.7.0.vsix` — passed, no archive errors.
- VSIX SHA-256: `ccdc8377b6a8f728ddbfa63255bac31a48a609b356c8835f73455c09a6991fd5`.

The generated VSIX is a local macOS arm64 beta artifact. This follow-up does
not claim cross-platform, LOVE/LuaJIT, visual, or long-session release
qualification.

## Remaining work and recommended order

1. Keep the current shared protocol identity and generic substitution paths;
   the audited cycles and cross-file cases now have regression coverage.
2. Investigate the two flaky/failing CLI shader timeout tests independently;
   do not attribute them to the generic/class lowering change without a
   reproduction linking them.
3. Run the documented LOVE/LuaJIT and cross-platform qualification gates before
   calling the 0.7.0 release fully qualified.
4. Protocol stub generation remains an optional editor capability, not a
   correctness defect or a blocker for the generic/protocol contract.

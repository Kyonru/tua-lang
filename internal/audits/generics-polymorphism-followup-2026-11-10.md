# Generics and protocols follow-up audit — November 10, 2026

## Summary

The generic/protocol implementation plan is substantially implemented, and the
existing suite contains broad same-file, cross-file, editor, and runtime
coverage. This follow-up found three defects: comments inside generic class
headers were discarded by Lua emission; deeply nested native array type suffixes
bypassed the configured parser depth limit; and protocol-bounded generic class
members disappeared from completion during an incomplete edit. All three are
fixed and have regressions.

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

### GP-FU-002 — Native array type nesting bypassed the parser depth limit

- **Category:** parser resilience / resource limits
- **Severity:** Medium
- **Status:** Fixed and verified
- **Affected workflow:** parsing deeply nested native array types, including
  editor requests and builds that analyze their annotations
- **Reproduction:**

  ```tua
  local value: number[][][][][][]... -- repeat `[]` 65 times
  ```

  The pre-normalization complexity scanner counted `[` and `]` as balanced
  delimiters. Later normalization erased each native `[]` suffix before the
  parser handled the annotation, so a 65-level array type was accepted even
  though the structural limit is 64.
- **Cause:** native array syntax is a postfix type suffix. Counting its two
  bracket characters as a balanced pair does not measure the resulting nested
  `Array` type depth.
- **Fix:** a lexical precheck counts contiguous native `[]` suffixes before
  normalization. It uses the existing comment/string protection scanner,
  rejects the 65th suffix at its original source position, and returns early
  without allocating a character map for files that contain no `[]` text.
- **Regression coverage:** `type_nesting_limit_accepts_its_boundary_and_rejects_the_next_level`
  verifies 64 suffixes are accepted, 65 are rejected at column 148, and the
  same bracket text inside comments and strings does not count.
- **Files:** `crates/tua-core/src/parser/limits.rs` and
  `crates/tua-core/src/parser/tests.rs`.

### GP-FU-003 — Incomplete generic class edits lost protocol-bound completion

- **Category:** editor recovery / completion consistency
- **Severity:** Medium
- **Status:** Fixed and verified
- **Affected workflow:** typing a member access on a protocol-bounded generic
  parameter while the containing class has an incomplete edit
- **Reproduction:** Open a valid file containing
  `final class Box<T: Readable> implements Readable`, with an
  `inspect(value: T)` method, then remove the class's final `}` while the cursor
  is immediately after `value:`. Request completion at that position.
- **Expected:** Completion offers `read`, using the known protocol bound on
  `T`.
- **Actual before fix:** Completion was empty. The current degraded parse could
  be built after recovering the unfinished class, but its partial method facts
  did not resolve the class generic parameter. Completion returned that empty
  result without retrying the last-good file semantics. Diagnostics correctly
  reported only the unclosed class.
- **Fix:** For an invalid current document, when current recovered facts and
  ordinary completion both return no results at a member-access cursor, retry
  that query against the file's last-good parsed facts and type environment.
  Successfully parsed documents continue to use current facts only. Last-good
  semantic facts are memoized per snapshot so successive keystrokes do not
  rebuild project inference.
- **Regression coverage:** Core snapshot and actual LSP edit-sequence tests
  open the valid generic class, remove the final class brace and method call,
  then assert that `read` is suggested after `value:`. The core test also
  verifies the unclosed-class diagnostic remains present and that last-good
  semantic facts reuse the same cached allocation.
- **Files:** `crates/tua-core/src/analysis/editor_queries.rs`,
  `crates/tua-core/src/analysis/tests.rs`, and
  `crates/tua-lsp/src/server/tests/completion.rs`.

The same follow-up adds exact generic alias expansion work-limit coverage at
4,096 and 4,097 applications, per-character recovery coverage for protocol and
generic class headers, a stale dependent-diagnostic cancellation case, and
delete/recreate churn behind each node in a three-module declaration-file
re-export chain. The expanded LSP regression passes after every removal and
restoration, and a renamed dependency also passes deletion, recreation, and
compatible-contract restoration. More complex multi-hop rename/churn
combinations and broader cancellation permutations remain open.

## Coverage and evidence

| Area | Status | Evidence |
| --- | --- | --- |
| Generic aliases and substitution | Exercised | Full core tests, including recursive/growing aliases, import/re-export, arity, editor queries, and exact 4,096/4,097 work-limit behavior |
| Generic functions | Exercised | Full core tests; inference, callbacks, varargs, returns, imports, and bound-comment parsing/emission regressions |
| Generic classes and applied inheritance | Exercised | Full core tests; local/imported constructors, fields/methods, parent substitutions, `Self`, overrides, and emitted Lua |
| Protocol conformance and dispatch | Exercised | Full core tests; explicit conformance, inherited obligations, multiple requirements, negative signatures, generic bounds, and runtime examples |
| Same-file recursive protocols | Exercised | `same_file_recursive_protocol_signatures_keep_conformance_and_editor_types` and `same_file_recursive_protocol_method_returns_keep_nominal_identity` |
| Cross-file recursive protocols | Exercised | `cyclic_imported_protocol_signatures_preserve_conformance_identity` and imported recursive method-return identity coverage |
| Completion/hover agreement for recursive protocols | Exercised | Core analysis tests assert nominal hover and completion members; extension real-server suite covers generic/protocol editor queries |
| Incomplete generic class completion | Exercised | Core snapshot and LSP edit-sequence regressions verify protocol-bound members remain suggested after an unclosed-class edit |
| Parser depth and malformed edits | Exercised | Exact 64/65 native array depth boundary, protected comment/string cases, and every character prefix of protocol/generic class headers |
| Formatter and comment round-trip | Exercised | Generic function-bound formatter regression plus class/function bound Lua-emission regressions |
| Cross-file invalidation | Exercised | Contract change/restore cancels stale diagnostics; delete/recreate of each node in a three-module type re-export chain and rename/delete/recreate of a direct dependency update dependent diagnostics |
| Cross-platform editor behavior | Not exercised | This run used the local macOS VS Code installation and macOS arm64 release server |
| LOVE/LuaJIT runtime behavior | Not exercised | The protocol feature-tour runtime test uses system Lua 5.4.8; LOVE/LuaJIT are unavailable or not qualified here |
| Long-session/performance behavior | Not exercised | No soak test or release-build latency benchmark was run in this follow-up |

## Commands and outcomes

- `RUST_MIN_STACK=16777216 cargo test -p tua-core` — passed: 1,898 passed,
  1 ignored, including all generic/protocol and new
  comment, type-depth, and exact alias-work-limit regressions.
- `RUST_MIN_STACK=16777216 cargo test -p tua-lsp` — passed: 433 tests,
  including the generic-class bound completion edit sequence.
- `cargo clippy -p tua-core -p tua-lsp --all-targets -- -D warnings` — passed.
- `cargo fmt --all -- --check` — passed.
- `cargo test -p tua-cli --lib shaders::tests -- --nocapture` — passed: four
  tests, one LOVE-dependent test ignored. This supersedes the earlier flaky
  shader-validator failures recorded below; they did not reproduce.
- `cargo test -p tua-core` with the Rust harness's default 2 MiB test-thread
  stack — overflows in a deep linter regression. The regression needs a larger
  stack than the Rust harness default; CLI and LSP workers already use 8 MiB.
  The Makefile's `test` target now sets an overridable 16 MiB default, leaving
  normal parallel test execution enabled.
- `env -u RUST_MIN_STACK -u RUST_TEST_THREADS make verify` — passed on the
  current tree using the Makefile's default 16 MiB stack and parallel tests:
  workspace tests (core 1,898 passed/1 ignored;
  LSP 433; CLI unit 92 passed/2 ignored; CLI integration 137; gameplay 3;
  helper-default 1; LOVE examples 3; LSP 1; Lua syntax 1), Clippy, release and
  config consistency, LLM language evaluation, extension checks (180/180),
  JSON validation, and diff checks. LOVE-dependent tests were skipped by their
  explicit environment guard. The first full-gate attempt was denied loopback
  access by the default sandbox; this full rerun used local loopback access.
- `make extension-package` with loopback access — passed. The extension check
  reported 180 passed and 0 failed; all 454 tracked JSON files validated, and
  the release server and VSIX were built.
- `make extension-package` with loopback access — passed on the final cached
  completion-fallback revision; extension checks reported 180/180 and npm
  reported zero vulnerabilities. The package contains 40 files and is 8.7 MB.
- `TUA_TEST_VSIX="$(pwd)/editors/vscode/tua-0.7.0.vsix" make extension-real-server-test`
  with loopback access — passed on the final package after installing it into
  a disposable VS Code profile and using its bundled release server; the VS Code extension-host
  process exited with code 0. The suite exercised protocol and generic
  completion, hover, signatures, definitions, rename, unsaved edits, and
  implementation navigation.
- `unzip -tq editors/vscode/tua-0.7.0.vsix` — passed, no archive errors.
- Final VSIX SHA-256: `87c33c1711f5ce22b14ae4809a80719223534dda2a1194740af59f5a746e34eb`.

The generated VSIX is a local macOS arm64 beta artifact. This follow-up does
not claim cross-platform, LOVE/LuaJIT, visual, or long-session release
qualification.

The packaged VSIX installation path was exercised directly after the artifact
was rebuilt: VS Code installed `tua-0.7.0.vsix` in a temporary extensions
directory, discovered the bundled `bin/tua`, and passed the real-server editor
suite without a development-server override.

## Remaining work and recommended order

1. Keep the current shared protocol identity and generic substitution paths;
   the audited cycles and cross-file cases now have regression coverage.
2. Run the documented LOVE/LuaJIT and cross-platform qualification gates before
   calling the 0.7.0 release fully qualified.
3. Protocol stub generation remains an optional editor capability, not a
   correctness defect or a blocker for the generic/protocol contract.

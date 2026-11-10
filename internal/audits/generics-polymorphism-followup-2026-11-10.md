# Generics and protocols follow-up audit — November 10, 2026

## Summary

The generic/protocol implementation plan is substantially implemented, and the
existing suite contains broad same-file, cross-file, editor, and runtime
coverage. This follow-up found four defects: comments inside generic class
headers were discarded by Lua emission; deeply nested native array type suffixes
bypassed the configured parser depth limit; protocol-bounded generic class
members disappeared from completion during an incomplete edit; and protocol
names were missing from incomplete class-header completion. All four are fixed
and have regressions.

The previously identified recursive protocol identity case is now covered by
same-file and cross-file conformance, hover, and completion tests. The full
`tua-core` suite passes these cases. The follow-up found one additional class-
header completion gap (GP-FU-004 below), now fixed. It did not find another
failure in generic substitution, protocol bounds, import/re-export propagation,
or the remaining tested editor surfaces.

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
  Successfully parsed documents continue to use current facts only. Degraded
  recovery replaces recoverable incomplete lines inside class bodies with a
  syntax-safe no-op block padded to preserve source columns. Outside class
  bodies it preserves the prior whitespace recovery, so table and callback
  context remains parseable. Fresh buffers therefore retain the generated
  class source mapping needed to locate method parameters. Last-good semantic
  facts are memoized per snapshot so successive keystrokes do not rebuild
  project inference.
- **Regression coverage:** Core snapshot and actual LSP edit-sequence tests
  open the valid generic class, remove the final class brace and method call,
  then assert that `read` is suggested after `value:`. They also open the same
  incomplete source without any prior valid snapshot. The core test verifies
  the unclosed-class diagnostic remains present and that last-good semantic
  facts reuse the same cached allocation. A direct recovery test checks that
  only class-body lines receive the no-op placeholder and that source length
  remains unchanged; a table-completion regression protects non-class context.
- **Files:** `crates/tua-core/src/analysis/editor_queries.rs`,
  `crates/tua-core/src/analysis/recovery.rs`,
  `crates/tua-core/src/analysis/tests.rs`, `crates/tua-core/src/classes.rs`, and
  `crates/tua-lsp/src/server/tests/completion.rs`.

### GP-FU-004 — Protocol types were missing from class-header completion

- **Category:** editor completion / incomplete-source recovery
- **Severity:** Medium
- **Status:** Fixed and verified
- **Affected workflow:** selecting a protocol after `implements`, or a protocol
  bound inside a multiline generic class header
- **Reproduction:**

  ```tua
  protocol Readable { read(): nil }
  final class Box<
    T: Re
  ```

  A second reproduction is `final class Box implements Re` after importing
  `Readable` from another file. Before the fix, these completion requests
  offered unrelated identifiers or generated LOVE types and omitted
  `Readable`.
- **Cause:** completion recognized ordinary annotations and member accesses,
  but not protocol-name positions in class headers. Its line-removal recovery
  also left earlier lines of an incomplete generic header in place, so parsing
  fell back to lossy identifier completion.
- **Fix:** shared editor-only class recovery now appends the missing `>` and
  empty class body for an incomplete generic class header, preserving the
  original source for diagnostics and builds. Completion recognizes protocol
  names after `implements` and inside unfinished generic bounds, including
  multiline headers. The context check excludes ordinary Lua identifiers and
  comment text.
- **Regression coverage:** core completion tests cover same-line and multiline
  conformance lists, local protocol names, unfinished generic bounds, and false
  positives in ordinary identifiers/comments. An LSP regression verifies the
  imported protocol case. A parser test verifies
  the recovered class retains its partial bound while the original source
  remains invalid. An in-process LSP test covers imported protocol completion
  after an unsaved conformance edit and in a fresh incomplete multiline generic
  bound.
- **Files:** `crates/tua-core/src/classes.rs`,
  `crates/tua-core/src/completion/context.rs`,
  `crates/tua-core/src/completion/mod.rs`, core/LSP completion and parser tests,
  and `docs/classes.md`.

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
| Generic functions | Exercised | Full core tests; inference, callbacks, varargs, returns, imports, and bound-comment parsing/emission regressions. A new LSP regression checks imported LuaDoc generic hover, instantiated signature, result hover, member completion, and definition to the `.lua` declaration. |
| Generic classes and applied inheritance | Exercised | Full core tests; local/imported constructors, fields/methods, parent substitutions, `Self`, overrides, and emitted Lua |
| Protocol conformance and dispatch | Exercised | Full core tests; explicit conformance, inherited obligations, multiple requirements, negative signatures, generic bounds, and runtime examples |
| Same-file recursive protocols | Exercised | `same_file_recursive_protocol_signatures_keep_conformance_and_editor_types` and `same_file_recursive_protocol_method_returns_keep_nominal_identity` |
| Cross-file recursive protocols | Exercised | `cyclic_imported_protocol_signatures_preserve_conformance_identity` and imported recursive method-return identity coverage |
| Completion/hover agreement for recursive protocols | Exercised | Core analysis tests assert nominal hover and completion members; extension real-server suite covers generic/protocol editor queries |
| Protocol type completion in class headers | Exercised | Core completion covers `implements` and incomplete multiline generic bounds; in-process LSP verifies imported protocol suggestions after edits |
| Incomplete generic class completion | Exercised | Core snapshot and LSP edit-sequence regressions verify protocol-bound members remain suggested after an unclosed-class edit |
| Parser depth and malformed edits | Exercised | Exact 64/65 native array depth boundary and protected comment/string cases. UTF-8 prefix tests include direct assertions that complete protocol and generic class headers recover before their first members; conditional prefix-loop assertions do not claim every malformed prefix recovers. |
| Formatter and comment round-trip | Exercised | Generic function-bound formatter regression plus class/function bound Lua-emission regressions |
| Cross-file invalidation | Exercised | Contract change/restore cancels stale diagnostics; a rapid incompatible-contract / re-export-rename / restore sequence does not publish stale consumer errors; delete/recreate of each node in a three-module type re-export chain and rename/delete/recreate of a direct dependency update dependent diagnostics |
| Cross-platform editor behavior | Not exercised | This run used the local macOS VS Code installation and macOS arm64 release server |
| LOVE/LuaJIT runtime behavior | Not exercised | The protocol feature-tour runtime test uses system Lua 5.4.8; LOVE/LuaJIT are unavailable or not qualified here |
| Long-session/performance behavior | Not exercised | No soak test or release-build latency benchmark was run in this follow-up |

## Independent audit addendum — November 10, 2026

Three read-only reviewers examined generic semantics, protocol/module behavior,
and editor/emission/runtime behavior. They found no new confirmed correctness
defect. They identified two evidence gaps, both addressed with regression tests:

- The malformed-header prefix test previously asserted only for prefixes where
  recovery returned source and reparsing succeeded. It now directly verifies
  recovery from a complete protocol header and a complete generic class header
  before their first body member. It still does not promise successful recovery
  from every arbitrary malformed prefix.
- LuaDoc generic checking was verified from a CLI fixture, but imported
  `.lua`-module editor behavior lacked coverage. A new actual LSP regression
  checks generic binder hover, instantiated signature help, inferred result
  hover, result member completion, and definition navigation to the Lua
  implementation.
- Cancellation coverage had a single contract change/restore. A new LSP
  regression sends a burst of incompatible contract and re-export edits, then
  restores both before draining diagnostics; the consumer receives neither
  stale conformance failures nor a missing-import error. Other cancellation
  schedules are still not exhaustive.

The reviewers also corrected the scope of prior fuzz evidence: `source_pipeline`
calls normalization/parsing and includes protocol-related seeds;
`protocol_and_paths` tests LSP framing and path confinement, not protocol
parsing. Neither target directly exercises editor declaration recovery.
Cross-module class inheritance remains an explicitly documented capability
limit, separate from this scoped protocol/generic implementation.

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
- Previously audited VSIX SHA-256: `87c33c1711f5ce22b14ae4809a80719223534dda2a1194740af59f5a746e34eb` (superseded by the revalidated artifact below).

### Revalidation after fresh-buffer recovery coverage

- `RUST_MIN_STACK=16777216 cargo test -p tua-core` — passed: 1,899 passed, 1 ignored (1,900 tests total), including the explicit class-only recovery placeholder regression and generic completion in both a new invalid buffer and after editing a valid document.
- `RUST_MIN_STACK=16777216 cargo test -p tua-lsp` — passed: 434 tests, including imported LuaDoc-generic hover, instantiated signature, result hover, member completion, and navigation to the `.lua` declaration.
- `RUST_MIN_STACK=16777216 cargo test -p tua-lsp rapid_protocol_and_reexport_edits_do_not_publish_stale_conformance_errors` — passed; exercises an incompatible contract, renamed re-export, restored contract, and restored re-export before draining diagnostics.
- `RUST_MIN_STACK=16777216 cargo test -p tua-core completes_protocol_names_in_class_implements_clauses` — passed; checks valid/incomplete same-line and multiline class headers plus ordinary-identifier and comment false positives.
- `RUST_MIN_STACK=16777216 cargo test -p tua-core completes_protocol_names_in_incomplete_multiline_generic_bounds` — passed; checks protocol suggestions before the generic list's `>` is typed.
- `RUST_MIN_STACK=16777216 cargo test -p tua-core editor_recovery_closes_incomplete_multiline_generic_class_headers` — passed; checks synthetic recovery preserves the partial class and bound while the original source remains invalid.
- `RUST_MIN_STACK=16777216 cargo test -p tua-lsp protocol_names_complete_in_an_incomplete_class_implements_clause` — passed; checks imported protocol suggestions after an unsaved edit and in a fresh incomplete multiline generic bound.
- `RUST_MIN_STACK=16777216 cargo test -p tua-core` — passed after the recovery and completion fixes: 1,902 passed, 1 ignored.
- `RUST_MIN_STACK=16777216 cargo test -p tua-lsp` — passed after the recovery and completion fixes: 436 passed.
- `cargo clippy -p tua-core -p tua-lsp --all-targets -- -D warnings` — passed.
- `make docs-build`, `cargo fmt --all -- --check`, and `git diff --check` — passed.
- `make extension-package` with local loopback access — passed: 180/180 extension tests, zero npm vulnerabilities, and a new 0.7.0 VSIX containing the updated release server.
- `TUA_TEST_VSIX=/Users/kyonru/Documents/projects/Experiments/tlua/editors/vscode/tua-0.7.0.vsix make extension-real-server-test` with local loopback access — passed against the packaged server; the installed VS Code suite now checks both imported protocol-name completion after `implements` and protocol-bound completion before the generic class header's closing `>`.
- `unzip -tq editors/vscode/tua-0.7.0.vsix` — passed. The rebuilt 8.72 MB artifact has SHA-256 `e7dfc55e3de1383a80336f4b2c07d81753d3ec257d506b926377dcce915464e9`.
- `cargo test -p tua-core declaration_library_drives_recursive_discriminated_table_completion` — passed; guards callback/table context outside class bodies.
- `cargo test -p tua-core incomplete_line_recovery_uses_class_placeholder_only_inside_class_bodies` — passed; directly checks the class-only no-op placeholder and equal source length.
- `cargo test -p tua-lsp generic_class_bound_completion_survives_incomplete_class_edits` — passed; actual in-process LSP requests cover fresh and edited documents.
- `env -u RUST_MIN_STACK -u RUST_TEST_THREADS make verify` — the initial sandboxed run blocked two loopback inspector tests. Rerunning with loopback access passed the full gate: core 1,900 passed/1 ignored; LSP 433; CLI library 92 passed/2 ignored; CLI integration 137; gameplay 3; helper defaults 1; LOVE examples 3; LSP smoke 1; Lua syntax 1; VS Code 180/180; Clippy, version/schema/docs generation, language evaluation, JSON validation, and diff checks all passed. LOVE-dependent tests were skipped by their explicit environment guards.
- `make docs-build` — passed.
- `make ci-check` — passed on macOS arm64, including strict Clippy, Rust dependency audit, workspace tests, all example builds, LSP feature smoke, VS Code checks, documentation, and benchmark smoke. Cargo audit reported only the already-allowed `paste` unmaintained advisory (RUSTSEC-2024-0436); it found no denied vulnerability.
- `make runtime-test LUA=lua` — passed the bundled vector, sprite, tween, camera, hot-reload, and input-helper Lua runtime tests under Lua 5.4.8.
- `env -u RUST_MIN_STACK -u RUST_TEST_THREADS make verify` — passed after the audit-only test additions, using the Makefile's 16 MiB default Rust test stack and local loopback access. This run included 1,899 core tests passed/1 ignored, 434 LSP tests, 180 VS Code tests, CLI/example/runtime/syntax tests, Clippy, version/schema/language checks, JSON validation, and diff checks.
- `make extension-package` — passed after granting the loopback access required by the inspector tests; 180/180 extension tests passed, npm reported zero vulnerabilities, the release server was bundled, and the VSIX was built.
- `TUA_TEST_VSIX="$(pwd)/editors/vscode/tua-0.7.0.vsix" make extension-real-server-test` — passed against the final packaged artifact. VS Code installed it into a disposable profile, launched its bundled real server, exercised the protocol/generic editor suite, and exited with code 0.
- `unzip -tq editors/vscode/tua-0.7.0.vsix` — passed, no archive errors. Artifact at that verification point: 8.7 MB, SHA-256 `bb06d1f4ea32c108ec0df543e89123d290eb2489b4cc13c24fe7d87c934cd8ba`; see the newer package record below for the current build.

The generated VSIX is a local macOS arm64 beta artifact. This follow-up does
not claim cross-platform, LOVE/LuaJIT, visual, or long-session release
qualification.

The packaged VSIX installation path was exercised directly after the artifact
was rebuilt: VS Code installed `tua-0.7.0.vsix` in a temporary extensions
directory, discovered the bundled `bin/tua`, and passed the real-server editor
suite without a development-server override.

### Installed-editor rerun — November 10, 2026

- `make extension-real-server-test` — the release server built successfully,
  but the VS Code test process terminated with `SIGABRT` before the extension
  test suite reported any assertions.
- Repeated with `TUA_TEST_KEEP_TEMP=1 ELECTRON_ENABLE_LOGGING=1` — same result.
  The macOS diagnostic report identifies VS Code 1.142.0 on macOS 26.6.1,
  arm64, aborting during `NSApplication` initialization while resolving menu-bar
  presentation state. This run does not establish an extension regression or
  invalidate the earlier passing installed-VSIX run; current installed-editor
  revalidation is blocked by host startup in this environment.
- The command rebuilt `target/release/tua` and staged
  `editors/vscode/bin/tua`; it did not modify tracked source files or rebuild
  the VSIX. LOVE and LuaJIT are unavailable on this host, so their runtime
  qualification remains open.
- `make extension-real-server-test` rerun with host-level launch access —
  passed. VS Code 1.142.0 launched, loaded the development extension with the
  freshly staged release server, ran the real-server protocol/generic suite,
  and exited with code 0. This confirms the earlier SIGABRT was specific to
  the restricted launch context, not a reproducible extension failure.
- `make extension-package` — passed: 180/180 extension checks, zero npm
  vulnerabilities, and a 40-file 0.7.0 VSIX was built with the current release
  server. `unzip -tq` passed; SHA-256:
  `98e457b2d5a613f35a1cecd20419c4ac16343fe930957488f73e0a8fd285e67b`.
- `TUA_TEST_VSIX="$(pwd)/editors/vscode/tua-0.7.0.vsix" make
  extension-real-server-test` with host-level launch access — passed. VS Code
  installed the packaged extension into a disposable profile, discovered its
  bundled server without a development override, and completed the protocol and
  generic real-server suite with exit code 0.

### Incomplete protocol-member recovery — November 10, 2026

- A stricter prefix assertion showed that the delimiter helper by itself can
  return text that still contains a partially typed protocol member. That is
  not the analysis entry point: `parse_degraded_analysis_source` first blanks
  recoverable incomplete lines, then closes the declaration.
- Added `incomplete_protocol_member_recovery_keeps_prior_contract_and_module_bindings`
  at that real analysis boundary. It verifies the unfinished `r` line is
  ignored while the earlier module local and completed `render` requirement
  remain in the recovered model.
- `RUST_MIN_STACK=16777216 cargo test -p tua-core
  incomplete_protocol_member_recovery_keeps_prior_contract_and_module_bindings`
  — passed. The two direct prefix-matrix tests also pass with their intentional
  conditional strict-parse checks.
- Extended the current-buffer core query regression with a partial trailing
  protocol member and added the actual LSP replay
  `incomplete_protocol_member_keeps_hover_and_completion_available`. Hover
  retains the earlier local, completion offers the completed `render` method,
  and the unclosed declaration diagnostic remains present. Both focused tests
  passed.

### Protocol diagnostic cancellation permutation — November 10, 2026

- Added `rapid_protocol_contract_edits_publish_the_latest_incompatible_signature`.
  It sends incompatible → compatible → incompatible contract revisions without
  draining the dependent diagnostics scheduler between edits, then verifies
  that an actual pull-diagnostics request reports the final parameter mismatch.
- `RUST_MIN_STACK=16777216 cargo test -p tua-lsp
  rapid_protocol_contract_edits_publish_the_latest_incompatible_signature`
  — passed. The reverse final-state direction is now covered alongside the
  existing incompatible → compatible stale-error case. Rename and
  filesystem delete/recreate races remain open.

### Watched dependency cancellation — November 10, 2026

- Added `watched_protocol_churn_cancels_queued_consumer_diagnostics`. It
  queues consumer conformance work, changes the imported protocol on disk,
  then sends watcher change, delete, and create events before draining the
  original due time. The consumer publishes the changed-contract result, then
  a missing-import diagnostic, then the recreated contract's mismatch.
- `RUST_MIN_STACK=16777216 cargo test -p tua-lsp
  watched_protocol_churn_cancels_queued_consumer_diagnostics` — passed. This
  verifies watcher invalidation cancels queued old-snapshot work; rename
  combined with repeated delete/recreate remains open.

### Renamed dependency repeated delete/recreate — November 10, 2026

- Extended `renaming_a_protocol_dependency_refreshes_importing_class_diagnostics`
  to repeat delete, recreate with an incompatible contract, and restore a
  compatible contract after the import has moved to the renamed path.
- `RUST_MIN_STACK=16777216 cargo test -p tua-lsp
  renaming_a_protocol_dependency_refreshes_importing_class_diagnostics` —
  passed. The second cycle reports missing import → latest conformance mismatch
  → no diagnostics. This closes the rename-plus-repeated-file-churn case;
  broader scheduler cancellation permutations remain open.

### Closing an unsaved protocol edit — November 10, 2026

- Added `closing_an_unsaved_protocol_edit_drops_stale_consumer_diagnostics`.
  It queues dependent diagnostics for an unsaved incompatible protocol edit,
  closes the protocol document to restore its compatible disk contents, and
  drains the queued pass. No stale conformance error is published, and the
  final consumer diagnostics are empty.
- `RUST_MIN_STACK=16777216 cargo test -p tua-lsp
  closing_an_unsaved_protocol_edit_drops_stale_consumer_diagnostics` — passed.
  This covers queue-before-close ordering; actual concurrent worker completion
  races and broader scheduler permutations remain open.

### Full-workspace verification rerun — November 10, 2026

- The first `env -u RUST_MIN_STACK -u RUST_TEST_THREADS make verify` run found
  one unrelated flaky CLI test setup: `diagnostics_include_new_unsaved_overlay_files`
  could not find its temporary `src` directory. It passed when run alone. The
  CLI API test helper used only PID plus a nanosecond timestamp and
  `create_dir_all`, allowing concurrent tests to silently share a temp root if
  the clock returned a duplicate value.
- Added a process-local atomic sequence to the temporary root and changed its
  creation to `create_dir`, which rejects collisions. The uniqueness regression
  and all CLI library tests passed (93 passed, 2 ignored).
- A second `env -u RUST_MIN_STACK -u RUST_TEST_THREADS make verify` passed:
  core 1,907 passed/1 ignored; LSP 438 passed; CLI library 93 passed/2 ignored;
  CLI integration 137 passed; gameplay 3; helper defaults 1; LOVE examples 3;
  LSP smoke 1; Lua syntax 1; strict workspace Clippy, version/schema checks,
  language evaluation, all 180 VS Code checks, JSON validation, and diff check.
  LOVE-dependent tests remained skipped by their explicit environment guards.

## Remaining work and recommended order

1. Keep the current shared protocol identity and generic substitution paths;
   the audited cycles and cross-file cases now have regression coverage.
2. Run the documented LOVE/LuaJIT and cross-platform qualification gates before
   calling the 0.7.0 release fully qualified.
3. Protocol stub generation remains an optional editor capability, not a
   correctness defect or a blocker for the generic/protocol contract.

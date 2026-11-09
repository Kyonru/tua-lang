# Tua 0.7.0 Generics and Protocols Final Audit

Date: 2026-11-09  
Audited base: `06c1065b940622eee2f179b4c7752ce496010b8c`
Final code and regression-test revision: `e385b99c`; the plan update is in
`fedfa2bc`. This report records verification after both.
Scope: `internal/generics-polymorphism-0.7.0.md`, protocol contracts, native generics, type inference, editor surfaces, Lua erasure, and the locally shareable VS Code extension.  
Working tree before audit: clean. No unrelated project changes were present.

## Executive summary

The documented 0.7.0 slice has strong compiler, project-analysis, LSP, runtime, and installed-editor coverage. This pass fixed three verified issues: growing recursive aliases now preserve the first repeated declaration symbolically; `protocol` and `implements` remain valid identifiers in ordinary and typed code instead of confusing grammar selection and protocol scanning; and protocol conformance messages explain required-argument, vararg, and return-arity failures. The audit added regressions for these behaviors, generic multi-return checker/runtime coverage, and installed VS Code protocol implementation navigation, generic binder references, and rename coverage.

The protocol/generic implementation is suitable for **manual beta testing on macOS arm64**. The exact packaged VSIX was installed into an isolated VS Code profile and passed the real-server suite using its bundled compiler. It is not qualified for a general 0.7.0 release: Linux, Windows, Intel Mac, LuaJIT/LOVE, source-map/hot-reload, visual screenshot review, and long-session checks remain open. The plan still records cross-module inherited conformance as unsupported because cross-module class inheritance itself is outside the current parser/lowering contract.

Repeated local profile measurements (five cold samples, twenty warm and contention samples) on the 1,000-line fixture show cold completion p95 251.10 ms, warm completion p95 6.24 ms, cold hover p95 266.65 ms, warm hover p95 20.03 ms, and completion p95 under contention 153.03 ms. The 100-module project reports cold completion p95 116.04 ms, warm completion 0.40 ms, and warm hover 1.66 ms. These have no before/after baseline; cold latency and large-project contention remain performance opportunities.

## Audit findings

### AUDIT-001 — Generic multi-return behavior lacked a persistent acceptance test

- **Category / severity:** Test coverage; Medium.
- **Status:** Fixed with regression tests in commit `10496682`.
- **Evidence:** The plan explicitly requires multiple generic return positions. A small CLI reproduction using `split<T, U>(left: T, right: U): (T, U)` checked successfully, but no test asserted both slot-specific inferred types and emitted-Lua runtime values together.
- **Change:** Added `generic_function_multiple_return_slots_infer_independently` in `crates/tua-core/src/check/tests.rs` and `generic_multiple_return_slots_preserve_lua_runtime_values` in `crates/tua-cli/tests/cli.rs`.
- **Verification:** Both focused tests pass. The CLI case builds the source, checks that generic syntax is erased and call arguments remain in place, and runs the emitted Lua with Lua 5.4.8.
- **Root fix direction:** None required; this was a regression-coverage gap.

### AUDIT-002 — Installed VS Code did not exercise protocol implementation navigation or rename

- **Category / severity:** Editor integration coverage; Medium.
- **Status:** Fixed with installed-editor regression coverage in commit `a71cf3e1`.
- **Evidence:** The core and in-process LSP tests already covered protocol navigation and rename, but the real-server VS Code suite stopped at protocol definition and generic selected definitions.
- **Change:** The real-server suite invokes VS Code's implementation provider from a protocol-typed call and renames a requirement from its declaration, asserting the implementation file is included.
- **Verification:** The development-extension real-server run and the exact packaged-VSIX run both pass on VS Code 1.142.0/macOS arm64. The test deliberately places the cursor inside the method identifier; a cursor at the end-exclusive identifier boundary correctly caused VS Code to reject rename during initial test development.
- **Root fix direction:** None required; the behavior works and is now exercised at the installed-editor boundary.

### AUDIT-003 — Growing recursive aliases expanded into verbose nested-type diagnostics

- **Category / severity:** Type display and diagnostics; Medium.
- **Status:** Fixed in `c25c5db0`.
- **Reproduction:**

  ```tua
  type Grow<T> = { next: Grow<T[]> }
  local function read(node: Grow<number>): number
    return node.next.next
  end
  ```

  Run `tua check` in a project containing this source. Before the fix, the mismatch rendered dozens of expanded records. After the fix, the actual error is `expected number, found { next: Grow<number[][][]> }` (less than 200 characters), retaining the incompatible value's shape without widening to `any`.
- **Root cause:** Both `GenericAliasExpansionContext.resolve` and `ProjectTypeDatabase::resolve_type_inner` stopped only when the full `(declaration, arguments)` pair recurred. Since `Grow<T[]>` changes arguments each time, expansion reached its depth bound before preserving a useful named edge.
- **Fix:** Stop expansion when the same alias declaration reappears on the current expansion path, even with changed arguments. Preserve that application and its substituted arguments. Other distinct alias chains remain protected by existing work/depth guards.
- **Verification:** `changing_argument_generic_alias_recursion_preserves_its_first_named_edge`, `changing_argument_generic_alias_recursion_preserves_its_first_project_edge`, `growing_recursive_generic_aliases_keep_a_symbolic_member_type`, and `growing_recursive_generic_alias_keeps_editor_types_finite_and_usable` pass. The last test checks imported cross-file completion and hover.
- **Regression value:** Checker reports remain accurate and compact; project and editor surfaces preserve the same changed generic arguments.

### AUDIT-004 — Cross-module inherited conformance remains unavailable

- **Category / severity:** Capability gap; Medium, known scope limit.
- **Status:** Open by design/architecture boundary.
- **Evidence:** G3 supports local generic inheritance and imported generic classes, but cross-module class inheritance is not currently in the parser/lowering contract. Therefore inherited protocol conformance cannot be qualified through an imported base class.
- **Recommended improvement:** Design cross-module class extension as a separate language/module feature. It needs constructor/init behavior, nominal identity, invalidation, emitted `super` behavior, and editor navigation defined before implementation. Do not claim G3 fully supports cross-module inherited conformance.

### AUDIT-005 — Cold editor requests scale noticeably with a larger source file

- **Category / severity:** Performance follow-up; Medium, measured but not compared to baseline.
- **Status:** Open; no regression claim.
- **Evidence:** A five-cold/twenty-warm repeated profile is recorded in the executive summary. `single-1000` warm hover p95 remains about 20 ms; contention completion p95 is about 153 ms. There is no baseline comparison or long-session memory data.
- **Recommended improvement:** Capture repeated cold and warm p50/p95 on the release host with project sizes, toolchain, and memory details. Investigate cold snapshot/index startup and the warm hover path if a repeatable budget is exceeded.

### AUDIT-006 — Runtime and platform release qualification is incomplete

- **Category / severity:** Release gate; Medium.
- **Status:** Pending external/runtime environments.
- **Evidence:** This host is macOS arm64 with VS Code 1.142.0 and Lua 5.4.8. `lua` is available; LOVE and LuaJIT are not. No Linux, Windows, or Intel Mac host was available.
- **Recommended improvement:** Run the release matrix on Linux, Windows, Intel Mac, and Apple Silicon; run supported LuaJIT and LOVE runtime tests; exercise source maps and hot reload with the game runtime; perform a long-session memory check. The macOS VSIX result must not be presented as universal release qualification.

### AUDIT-007 — Protocol keywords were misclassified in ordinary Lua code

- **Category / severity:** Parser compatibility; Medium.
- **Status:** Fixed in `2433056f` (`fix: keep protocol keywords contextual`).
- **Reproduction:**

  ```tua
  local protocol = "existing value"
  local implements = protocol
  return { protocol = protocol, implements = implements }
  ```

  `parse_tua_source` previously failed on the `implements` binding. After the
  grammar classifier was narrowed, the protocol declaration scanner then
  mistook the `protocol` value use for a declaration and consumed the following
  return table as a protocol body.
- **Expected:** These remain legal Lua identifiers. The same names also work
  alongside Tua type syntax, and real `protocol Name { ... }` declarations
  still parse.
- **Root cause:** The parser selected its Tua grammar merely because it
  encountered the words, and the protocol scanner searched for `protocol`
  without requiring declaration position. A value use could therefore alter
  grammar selection or absorb later source as a declaration.
- **Fix:** Grammar selection now requires protocol declaration shape at a
  statement position; the class keyword remains the trigger for class headers.
  Protocol declaration scanning also requires statement position. Identifier
  behavior is covered in ordinary and typed source, including emitted Lua.
- **Verification:**
  `protocol_and_implements_remain_usable_as_lua_identifiers` and the complete
  protocol-focused core test selection pass. The regression also confirms a
  real protocol declaration still reaches its normal parser path.
- **Related plan requirements:** P0 contextual keyword compatibility and P1
  declaration identity/recovery.

### AUDIT-008 — Protocol arity failures lacked actionable detail

- **Category / severity:** Diagnostic usability; Low.
- **Status:** Fixed in `22393904` (`fix: explain protocol arity mismatches`).
- **Reproduction:** A method implementing `find(index?: number)` with
  `find(index: number)` correctly failed, but the diagnostic showed only two
  full signatures. Missing protocol varargs and a mismatched number of return
  values were similarly reported without naming the arity rule.
- **Expected:** State which side requires or permits the argument count,
  whether the protocol requires variadic arguments, or how the declared return
  counts differ.
- **Root cause:** The shared assignability result records
  `FunctionParameterCount`, `FunctionVararg`, and `FunctionReturnCount`, but
  the protocol diagnostic formatter only rendered failures with a parameter
  or return path.
- **Fix:** Format those structured failure kinds in the protocol conformance
  diagnostic with singular/plural wording.
- **Verification:**
  `protocol_compatibility_respects_optional_varargs_and_each_return_slot`
  covers valid optional/variadic/multi-return conformance and rejects an
  implementation-required optional argument, missing varargs, a wrong second
  return type, and wrong return count.

## Coverage matrix

Status key: **Exercised** means a focused regression or real request/runtime test ran; **Inspected** means code/docs were reviewed but not independently executed for that specific edge; **Pending** means no qualifying environment or scenario was available.

| Plan area | Status | Evidence and limits |
| --- | --- | --- |
| P0 protocol syntax, nominality, colon receiver, mutable views | Exercised | Positive/negative assignments, colon calls, explicit dot/extracted receivers, class array mutation safety, nominal identity, inherited overrides, and contextual identifier compatibility are covered. |
| P1 parsing, identities, spans, formatting and erasure | Exercised, partial | Local/exported/multiline protocol headers, comments, spans, formatting, diagnostics, Lua erasure, and ordinary use of contextual words pass. Broader malformed-edit recovery remains incomplete. |
| P2 conformance, variance, multiple obligations, inheritance | Exercised, partial | Positive/negative local/imported conformance, inherited satisfaction/override, same-name obligations, optional parameters, varargs, multiple return slots/counts, variance, and exact override/parameter/return locations are tested; uncommon override/range combinations remain open. |
| P3 module identity, re-exports, cycles, edits | Exercised, partial | Named/namespace/re-export and cyclic protocol identity, saved/unsaved refresh tests pass; all dependency graph edit patterns are not exhausted. |
| P4 completion, hover, signatures, navigation, rename | Exercised on macOS, partial | Core/LSP suites and installed VSIX cover protocol completion, hover, signature, definition, implementation navigation, references/rename; generic alias/class/function completion/hover/signatures, alias definition, and generic binder definition/references/rename. Cross-module alias completion/hover are covered by in-process LSP tests, not a dedicated VS Code scenario. |
| P5 examples, generated Lua, runtime/source maps/hot reload | Exercised, partial | Feature/game examples build; gameplay polymorphism executes; generic multi-return emitted Lua executes on Lua 5.4. LOVE/LuaJIT, source-map/hot-reload, and visual behavior pending. |
| G0 design contract and limits | Inspected | Decisions are present in plan and user docs; future generic methods, generic protocols, packs, explicit call-site args, compound bounds, and advanced LuaCATS are explicit limitations. |
| G1 aliases, identity, substitution, recursion, imports | Exercised, partial | Exact arity, unknown names, capture-safe substitution, imported/re-exported aliases, same-argument recursion, and growing-argument recursion are tested. The first repeated declaration stays symbolic with substituted arguments; other alias cycles retain work/depth guards. Exhaustive adversarial alias graph limits remain untested. |
| G2 functions, inference, callbacks, LuaDoc | Exercised, partial | Independent calls, body checking, conflicts, nil/unions/empty table, callback contexts/ambiguity, bounds, varargs, LuaDoc generic forms, multi-return slot inference/runtime are covered. Callback-only/return-only inference and explicit callsite args are deliberately unsupported. |
| G3 classes, constructors, substitution, bounds | Exercised, partial | Local/imported generic instances, constructor/member inference, applied generic parents, `super`, invariant containers, protocol bounds, hover/completion/navigation covered. Cross-module inherited conformance and generic methods/protocols remain unsupported. |
| Real editing and snapshots | Exercised, partial | Unsaved dependency edits, cyclic imports, incomplete consumer edits, warm snapshots and watched changes have core/LSP coverage. Long-session and broad dependency churn remain open. |
| UTF-16/editor client integration | Exercised | LSP UTF-16 and cross-file navigation/rename suites pass; exact VS Code test passes on macOS arm64. Other clients and platforms pending. |
| Performance and memory | Exercised, partial | Repeated five-cold/twenty-warm profile completed with p50/p95 for single-file and project fixtures; no before/after baseline, dependency-churn timing or long-session memory qualification. |
| Release artifact/package | Exercised, partial | `make extension-package` passes (180/180 extension tests, npm audit 0 vulnerabilities), VSIX archive validates, exact current VSIX installs and runs in isolated VS Code. No signing, publishing, or external beta distribution performed. |

## Verification log

All commands ran from repository root unless noted.

| Command | Outcome |
| --- | --- |
| `env CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 make ci-check` | Passed. Includes formatting, clippy, RustSec audit, all workspace tests, bundled example builds, editor regression smoke, version/schema/generated-doc checks, extension check (180 tests), docs build, benchmarks, and `git diff --check`. Two runtime-dependent tests were ignored by their existing gates. `cargo audit` reported one allowed unmaintained-crate warning: `paste 1.0.15`, RUSTSEC-2024-0436; no vulnerability was reported. |
| `make extension-real-server-test` | Passed on installed local VS Code development extension after navigation/rename assertions were added. First runs exposed test cursor placement at the method token's end; changing it to an interior character made the provider test meaningful. |
| `make extension-package` | Passed. Package version synchronized at 0.7.0; extension check passed 180/180; npm audit reported 0 vulnerabilities; produced 40-file VSIX with bundled `tua` server. |
| `env TUA_TEST_VSIX="$PWD/editors/vscode/tua-0.7.0.vsix" npm --prefix editors/vscode run test:vscode-real` | Passed against the freshly packaged VSIX. Installs into an isolated profile and starts the bundled server; asserts protocol implementation navigation, generic binder definition/references/rename (three rename edits), imported generic hovers/completions/signatures. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -p tua-core` | Passed in the final `make ci-check`: 1,861 passed, 0 failed, 1 ignored. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -p tua-core protocol_` | Passed: 45 protocol-focused core tests, including contextual identifiers, explicit receivers, inherited overrides, and optional/variadic/multiple-return contracts. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -p tua-lsp` | Passed in the final `make ci-check`: 408 passed, 0 failed. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -p tua-cli` | Passed: 92 unit, 133 CLI integration, 3 gameplay, 1 helper defaults, 3 LOVE example, 1 LSP, and 1 Lua syntax integration tests; runtime-dependent LOVE execution tests use existing gates. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 cargo clippy --workspace --all-targets -- -D warnings` | Passed. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 cargo test -p tua-core generic_function_multiple_return_slots_infer_independently` | Passed, 1 test. |
| `cargo test -p tua-cli --test cli generic_multiple_return_slots_preserve_lua_runtime_values` | Passed, 1 test, Lua 5.4.8. |
| `make docs-build` | Passed; standalone build reported no issues. |
| `make extension-screenshots-check` | Passed: 5 linked PNGs and manifest hashes/dimensions valid. Manifest is from 0.6.0; no protocol/generic UI layout changed, so gallery recapture was not needed for this language-only work. Visual review remains a release-prep task. |
| `unzip -tq editors/vscode/tua-0.7.0.vsix` | Passed: no archive errors. |
| `shasum -a 256 editors/vscode/tua-0.7.0.vsix` | `18b4abf802d8bbc48687e5d244f003037d72c1d3a880ff58f2e06fde9ad566a9` (latest package, version 0.7.0). |
| `unzip -tq editors/vscode/tua-0.7.0.vsix` | Passed: no archive errors. |
| `cargo fmt --all -- --check`, `git diff --check` | Passed after implementation and test changes. |

The VS Code/Electron tests require local host access for the editor and language server. A sandboxed first attempt terminated Electron with `SIGABRT`; rerunning with the approved local host access completed and passed. This was a test-environment restriction, not a Tua assertion failure.

## Prioritized backlog

| Priority | Group | Effort | Acceptance criteria |
| --- | --- | --- | --- |
| 1 | Cross-module inheritance and inherited protocol conformance design | L | A written contract covers runtime initialization/`super`, nominal identity, imports, cycles/invalidation, navigation, and diagnostics before parser or emitter work begins. |
| 2 | Cold and long-session performance qualification | M | Repeated release-host cold/warm p50/p95, dependency-edit timing, peak/retained memory, and a documented comparison against an agreed baseline. |
| 3 | Runtime/platform qualification | M/L | Linux, Windows, Intel Mac, Apple Silicon CI; supported LuaJIT/LOVE smoke; generated behavior and source maps checked; hot reload and long-session results recorded. |
| 4 | Finish broad protocol malformed-edit, override, and diagnostic-range matrix | M | Table-driven tests cover partial headers, nested comments/types, all inherited overrides, cancellation and exact primary/related ranges across local and imported declarations. |
| 5 | Generic installed-editor parity expansion | S/M | Add installed-VSIX scenarios for unsaved generic dependency edits and cross-file generic class/function navigation beyond binder rename and current generic alias selection. |

Recommended sequence: design cross-module class extension separately; expand performance baselines and runtime/platform gates; complete protocol edge-case coverage and installed generic editor scenarios. Keep the current scoped 0.7.0 beta contract explicit until those workstreams land.

## Reproduction fixture

The generic multi-return regression is now permanent in the test suite. For a standalone copy, create `src/main.tua`:

```tua
local function split<T, U>(left: T, right: U): (T, U)
  return left, right
end

local text, count = split("enemy", 3)
local _text_check: string = text
local _count_check: number = count
```

Use the repository's standard `tua.toml` (`source = "src"`, `out = "build"`) and run:

```sh
tua check
tua build
lua build/main.lua
```

For installed-editor qualification, run:

```sh
make extension-package
env TUA_TEST_VSIX="$PWD/editors/vscode/tua-0.7.0.vsix" npm --prefix editors/vscode run test:vscode-real
```

This test uses an isolated VS Code profile and does not install into the user's normal profile.

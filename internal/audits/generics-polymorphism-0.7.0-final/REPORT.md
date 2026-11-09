# Tua 0.7.0 Generics and Protocols Final Audit

Date: 2026-11-09  
Audited base: `b1c31e81590a49e7441d2464e826596fecd13089`  
Final code and regression-test revision: `10496682` (the following commit only
updates the implementation plan and adds this report).  
Scope: `internal/generics-polymorphism-0.7.0.md`, protocol contracts, native generics, type inference, editor surfaces, Lua erasure, and the locally shareable VS Code extension.  
Working tree before audit: clean. No unrelated project changes were present.

## Executive summary

The documented 0.7.0 slice is implemented and has strong compiler, project-analysis, LSP, runtime, and installed-editor coverage. This audit found no new confirmed semantic or transpilation defect in the supported surface. It found two verification gaps and closed them with focused regression coverage: generic multi-return slots now have checker and Lua-runtime tests, and the exact VSIX test now exercises protocol implementation navigation and cross-file rename in VS Code.

The protocol/generic implementation is suitable for **manual beta testing on macOS arm64**. The exact packaged VSIX was installed into an isolated VS Code profile and passed the real-server suite using its bundled compiler. It is not qualified for a general 0.7.0 release: Linux, Windows, Intel Mac, LuaJIT/LOVE, source-map/hot-reload, visual screenshot review, and long-session checks remain open. The plan still records cross-module inherited conformance as unsupported because cross-module class inheritance itself is outside the current parser/lowering contract.

Key measurements from the CI smoke profile show warm completion at 6.76 ms and warm hover at 20.16 ms on the 1,000-line fixture. Cold completion and hover were 251.25 ms and 262.80 ms respectively. These are single-profile smoke measurements, not a before/after regression comparison; cold latency remains the most visible performance opportunity for larger projects.

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

### AUDIT-003 — Growing recursive aliases can produce verbose nested-type diagnostics

- **Category / severity:** Diagnostic usability / design question; Low.
- **Status:** The expansion limit is documented and tested; whether to reject or specially display non-regular recursive aliases remains open.
- **Reproduction:**

  ```tua
  type Grow<T> = { next: Grow<T[]> }
  local function consume(value: Grow<number>): nil
    return nil
  end
  ```

  Run `tua check` in a project containing this source. The project is accepted (apart from an unused-function warning). Expansion stops at 128 active alias instantiations (`crates/tua-core/src/types/environment.rs` and `crates/tua-core/src/type_database/mod.rs`) and preserves an unresolved named application at the recursion edge, as currently documented. If a consumer requires a different type, the resulting mismatch can show a long nested record prefix before the printer's depth guard.
- **Recommended improvement:** Decide whether changing-argument recursion should remain a valid but truncated type or receive a targeted “recursive generic alias exceeds supported expansion” diagnostic. If retaining it, render the recursive alias edge compactly so downstream diagnostics do not show dozens of repeated records. Add local and imported tests for the selected policy. Do not silently map the result to `any`.
- **Why not changed here:** The current behavior matches the implementation plan's explicit 128-instantiation bound and unresolved edge. Changing acceptance or alias display semantics deserves a language-contract decision rather than an arbitrary depth adjustment.

### AUDIT-004 — Cross-module inherited conformance remains unavailable

- **Category / severity:** Capability gap; Medium, known scope limit.
- **Status:** Open by design/architecture boundary.
- **Evidence:** G3 supports local generic inheritance and imported generic classes, but cross-module class inheritance is not currently in the parser/lowering contract. Therefore inherited protocol conformance cannot be qualified through an imported base class.
- **Recommended improvement:** Design cross-module class extension as a separate language/module feature. It needs constructor/init behavior, nominal identity, invalidation, emitted `super` behavior, and editor navigation defined before implementation. Do not claim G3 fully supports cross-module inherited conformance.

### AUDIT-005 — Cold editor requests scale noticeably with a larger source file

- **Category / severity:** Performance follow-up; Medium, measured but not compared to baseline.
- **Status:** Open; no regression claim.
- **Evidence:** `make ci-check`'s representative profile for `single-1000` reported cold completion p95 251.25 ms and cold hover p95 262.80 ms; warm completion was 6.76 ms and warm hover 20.16 ms. `project-100` cold completion was 115.25 ms and warm completion 0.42 ms. The smoke used one cold sample and is not sufficient for release budget qualification.
- **Recommended improvement:** Capture repeated cold and warm p50/p95 on the release host with project sizes, toolchain, and memory details. Investigate cold snapshot/index startup and the warm hover path if a repeatable budget is exceeded.

### AUDIT-006 — Runtime and platform release qualification is incomplete

- **Category / severity:** Release gate; Medium.
- **Status:** Pending external/runtime environments.
- **Evidence:** This host is macOS arm64 with VS Code 1.142.0 and Lua 5.4.8. `lua` is available; LOVE and LuaJIT are not. No Linux, Windows, or Intel Mac host was available.
- **Recommended improvement:** Run the release matrix on Linux, Windows, Intel Mac, and Apple Silicon; run supported LuaJIT and LOVE runtime tests; exercise source maps and hot reload with the game runtime; perform a long-session memory check. The macOS VSIX result must not be presented as universal release qualification.

## Coverage matrix

Status key: **Exercised** means a focused regression or real request/runtime test ran; **Inspected** means code/docs were reviewed but not independently executed for that specific edge; **Pending** means no qualifying environment or scenario was available.

| Plan area | Status | Evidence and limits |
| --- | --- | --- |
| P0 protocol syntax, nominality, colon receiver, mutable views | Exercised | Core conformance/receiver tests and mutable class-array widening tests; unusual extracted-method and callback receiver variants remain less exhaustive. |
| P1 parsing, identities, spans, formatting and erasure | Exercised | Parser/checker and emitter tests cover comments, malformed/unclosed declarations, type identity, formatting and ordinary Lua output; malformed-edit matrix is not exhaustive. |
| P2 conformance, variance, multiple obligations, inheritance | Exercised, partial | Positive/negative local/imported conformance, inherited satisfaction, same-name obligations, variance, and parameter/return mismatch locations are tested; uncommon override/range combinations remain open. |
| P3 module identity, re-exports, cycles, edits | Exercised, partial | Named/namespace/re-export and cyclic protocol identity, saved/unsaved refresh tests pass; all dependency graph edit patterns are not exhausted. |
| P4 completion, hover, signatures, navigation, rename | Exercised on macOS, partial | Core/LSP suites and installed VSIX cover protocol completion, hover, signature, definition, implementation navigation, references/rename; generic alias/class/function completion/hover/signatures and alias definition. Installed generic implementation navigation/rename are not separately asserted. |
| P5 examples, generated Lua, runtime/source maps/hot reload | Exercised, partial | Feature/game examples build; gameplay polymorphism executes; generic multi-return emitted Lua executes on Lua 5.4. LOVE/LuaJIT, source-map/hot-reload, and visual behavior pending. |
| G0 design contract and limits | Inspected | Decisions are present in plan and user docs; future generic methods, generic protocols, packs, explicit call-site args, compound bounds, and advanced LuaCATS are explicit limitations. |
| G1 aliases, identity, substitution, recursion, imports | Exercised, partial | Exact arity, unknown names, capture-safe substitution, imported/re-exported aliases and same-argument recursion tested; growing-argument aliases are bounded but lack a dedicated truncation diagnostic. |
| G2 functions, inference, callbacks, LuaDoc | Exercised, partial | Independent calls, body checking, conflicts, nil/unions/empty table, callback contexts/ambiguity, bounds, varargs, LuaDoc generic forms, multi-return slot inference/runtime are covered. Callback-only/return-only inference and explicit callsite args are deliberately unsupported. |
| G3 classes, constructors, substitution, bounds | Exercised, partial | Local/imported generic instances, constructor/member inference, applied generic parents, `super`, invariant containers, protocol bounds, hover/completion/navigation covered. Cross-module inherited conformance and generic methods/protocols remain unsupported. |
| Real editing and snapshots | Exercised, partial | Unsaved dependency edits, cyclic imports, incomplete consumer edits, warm snapshots and watched changes have core/LSP coverage. Long-session and broad dependency churn remain open. |
| UTF-16/editor client integration | Exercised | LSP UTF-16 and cross-file navigation/rename suites pass; exact VS Code test passes on macOS arm64. Other clients and platforms pending. |
| Performance and memory | Exercised, partial | CI profile and inline-completion smoke completed; cold large-file latency is measurable, repeated baseline comparison and long-session memory qualification pending. |
| Release artifact/package | Exercised, partial | `make extension-package` passes, VSIX archive validates, exact VSIX installs and runs in isolated VS Code. No signing, publishing, or external beta distribution performed. |

## Verification log

All commands ran from repository root unless noted.

| Command | Outcome |
| --- | --- |
| `env CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 make ci-check` | Passed. Includes formatting, clippy, RustSec audit, all workspace tests, bundled example builds, editor regression smoke, version/schema/generated-doc checks, extension check (180 tests), docs build, benchmarks, and `git diff --check`. Two runtime-dependent tests were ignored by their existing gates. `cargo audit` reported one allowed unmaintained-crate warning: `paste 1.0.15`, RUSTSEC-2024-0436; no vulnerability was reported. |
| `make extension-real-server-test` | Passed on installed local VS Code development extension after navigation/rename assertions were added. First runs exposed test cursor placement at the method token's end; changing it to an interior character made the provider test meaningful. |
| `make extension-package` | Passed. Package version synchronized at 0.7.0; extension check passed 180/180; npm audit reported 0 vulnerabilities; produced 40-file VSIX with bundled `tua` server. |
| `env TUA_TEST_VSIX=/Users/kyonru/Documents/projects/Experiments/tlua/editors/vscode/tua-0.7.0.vsix npm --prefix editors/vscode run test:vscode-real` | Passed. Installs into an isolated profile and starts the bundled server; includes protocol/generic editor checks and cross-file implementation navigation/rename. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 cargo test -p tua-core generic_function_multiple_return_slots_infer_independently` | Passed, 1 test. |
| `cargo test -p tua-cli --test cli generic_multiple_return_slots_preserve_lua_runtime_values` | Passed, 1 test, Lua 5.4.8. |
| `make docs-build` | Passed; standalone build reported no issues. |
| `make extension-screenshots-check` | Passed: 5 linked PNGs and manifest hashes/dimensions valid. Manifest is from 0.6.0; no protocol/generic UI layout changed, so gallery recapture was not needed for this language-only work. Visual review remains a release-prep task. |
| `unzip -tq editors/vscode/tua-0.7.0.vsix` | Passed: no archive errors. |
| `shasum -a 256 editors/vscode/tua-0.7.0.vsix` | `4b78202f07479ae394626fa0d27bed58f54c5a9fa425c2a5147bc6d4269ef825`. |
| `git diff --check` | Passed before audit commits. |

The VS Code/Electron tests require local host access for the editor and language server. A sandboxed first attempt terminated Electron with `SIGABRT`; rerunning with the approved local host access completed and passed. This was a test-environment restriction, not a Tua assertion failure.

## Prioritized backlog

| Priority | Group | Effort | Acceptance criteria |
| --- | --- | --- | --- |
| 1 | Type resolver truncation diagnostics for recursive generic aliases | M | Local/imported growing recursion emits one targeted error at the recursive application; no type truncation silently becomes `any`; hover/error text stays bounded; same-argument recursive aliases remain valid. |
| 2 | Cross-module inheritance and inherited protocol conformance design | L | A written contract covers runtime initialization/`super`, nominal identity, imports, cycles/invalidation, navigation, and diagnostics before parser or emitter work begins. |
| 3 | Cold and long-session performance qualification | M | Repeated release-host cold/warm p50/p95, dependency-edit timing, peak/retained memory, and a documented comparison against an agreed baseline. |
| 4 | Runtime/platform qualification | M/L | Linux, Windows, Intel Mac, Apple Silicon CI; supported LuaJIT/LOVE smoke; generated behavior and source maps checked; hot reload and long-session results recorded. |
| 5 | Finish broad protocol malformed-edit, override, and diagnostic-range matrix | M | Table-driven tests cover partial headers, nested comments/types, all inherited overrides, cancellation and exact primary/related ranges across local and imported declarations. |
| 6 | Generic installed-editor parity expansion | S/M | Installed VSIX asserts generic definition/implementation/reference/rename and unsaved dependency edits, beyond current generic completion/hover/signature/selected definition coverage. |

Recommended sequence: add resolver truncation state/diagnostic first; expand the cross-module inheritance design independently; then strengthen performance baselines and runtime/platform gates. Keep the current scoped 0.7.0 beta contract explicit until those workstreams land.

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

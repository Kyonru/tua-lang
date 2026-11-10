# Emitted Lua reliability audit — 2026-11-10

## Executive summary

This audit checked Tua's generated Lua by compiling representative programs and executing the output, then running the workspace verification gate. It added a cross-file runtime regression test in `crates/tua-cli/tests/emission_matrix.rs`. No emitter defect was found in the exercised paths. The result is strong regression evidence for the covered paths, not a proof that every possible program is correct.

Revision at audit start: `ec495055cc2326a13f53d34bbb504b0aa83d1678`. Two pre-existing working-tree edits, in `crates/tua-cli/tests/cli.rs` and `internal/audits/generics-polymorphism-0.7.0-final/REPORT.md`, were preserved. This audit's new test is separate from those edits.

Runtime matrix commit: `497430f9` (unsigned, no co-author trailer). The VSIX was packaged after this commit; the audit report itself does not change the bundled binary.

## Feature-to-output coverage

| Feature or output boundary | Verification | Evidence |
| --- | --- | --- |
| Primitive and table values; nested fields | Runtime | New emitted-language matrix and gameplay fixture |
| Type aliases, annotations, literal unions, enums | Runtime for erasure and values; syntax for broader examples | New matrix, `lua_syntax.rs` |
| Function defaults and optional arguments | Runtime | New matrix, `gameplay.rs` class/default test |
| Multiple return values | Runtime | New matrix, CLI generic multi-return test |
| Closures and captured mutable locals | Runtime | New matrix |
| Loops, conditions, primitive `is` narrowing | Runtime | New matrix |
| `+=`, `-=`, `*=`, `/=`, `%=` and nested field lowering | Runtime | New matrix, CLI bounded compound oracle, gameplay fixture |
| Cross-file `require` and mixed `.tua`/`.lua` | Runtime | New matrix, CLI imported generic/protocol cases |
| Class construction, inheritance, overrides, `super`, `Self`, virtual dispatch | Runtime | `gameplay.rs`, CLI generic override/initialization and protocol dispatch cases |
| Generic instantiation and protocol bounds across files | Runtime plus negative checker cases | CLI imported generic/protocol tests |
| Object pool, store, signals, ECS, state machine, sprite, audio | Runtime using fixture or stubbed LOVE surface | CLI runtime tests and gameplay fixture |
| Behavior tree, roguelike helpers, shader inclusion | Runtime using fixture/stubbed dependencies | `behavior_tree.rs`, `rot.rs`, `shader_scan.rs` |
| Vector lowering and fused arithmetic | Runtime | CLI `vec2` equivalence tests |
| Source maps and runtime traceback | Runtime | CLI traceback tests |
| LuaCATS annotation mode | Build/output assertions | CLI annotation emission tests |
| LOVE minimal/full applications | Build and Lua syntax; native run opt-in | `love_example.rs`, `lua_syntax.rs` |
| Bundled examples (features, LOVE minimal/full) | Build and Lua syntax | `lua_syntax.rs` |
| Unused opt-in runtime helpers | Build/output assertions | CLI and `rot.rs` tests |
| Asset copying, output cleanup and build manifest | Build/output assertions | CLI build/package tests |

“Runtime” means emitted Lua was executed and its assertions passed. Build/output assertions and syntax checks establish narrower claims; they do not establish LOVE rendering or user input behavior. Some helper tests use LOVE stubs, so they verify Lua-level contracts rather than a real graphics/audio device.

## New runtime reproduction

Run `RUST_MIN_STACK=33554432 cargo test -p tua-cli --test emission_matrix -- --nocapture`. The test creates a temporary mixed-source project, invokes `tua check` and `tua build`, then executes `build/main.lua` with the detected Lua interpreter. Its assertions check values and side effects, including a multi-return call, closure state, cross-file imports, default arguments, compound assignment precedence, and both branches of primitive `is` checks.

## Verification log

| Command | Result |
| --- | --- |
| `RUST_MIN_STACK=33554432 cargo test -p tua-cli --test emission_matrix -- --nocapture` | Passed: 1 test, executed under Lua 5.4.8 |
| `env RUST_MIN_STACK=33554432 make verify` | First run reached the extension suite; two inspector tests could not bind `127.0.0.1` in the sandbox (`EPERM`) |
| `env RUST_MIN_STACK=33554432 make verify` with loopback permission | Rust suite reached ShaderScan; its two runtime tests failed once during the full run, then both passed when rerun alone with and without loopback permission |
| `env RUST_MIN_STACK=33554432 RUST_TEST_THREADS=1 make verify` with loopback permission | Passed: 2,608 Rust test results across the workspace, Clippy, language eval, 180 extension unit tests, syntax and metadata checks |
| `make extension-package` with loopback permission | Passed; produced `editors/vscode/tua-0.7.0.vsix` (40 files, 8.82 MB) with the bundled `tua` binary |
| `unzip -tq editors/vscode/tua-0.7.0.vsix` | Passed, no archive errors |
| `make extension-real-server-test` with loopback permission | Passed: real VS Code extension host and staged Tua server, exit code 0 |

VSIX SHA-256: `72049f76bf537aac71594c2003a04b60b24d24335008ea8f892b73840b286f86`. The embedded extension manifest reports version `0.7.0`.

## Limits and remaining qualification

- The available standalone interpreter is Lua 5.4.8. This run did not execute emitted output under LuaJIT or Lua 5.1, the intended LOVE runtime family. The output syntax test validates generated files with the discovered validator, but cannot substitute for LuaJIT execution.
- Native LOVE smoke tests are opt-in and were not part of the standard gate. An earlier headless LOVE 11.5 attempt timed out before producing useful output, so graphics, audio and event-loop behavior remain unverified here.
- Finite tests cannot establish 100% correctness for all source programs or external libraries. Unknown combinations, metatable side effects, platform-specific filesystem behavior, and long-running editor/game sessions need separate qualification.
- The pre-existing edits to `cli.rs` include further emission tests; their verification is included in the full gate, but they are not attributed to this audit.
- The default parallel full run showed an intermittent ShaderScan test failure. A focused rerun and the serialized full gate passed. This leaves a test-interference or timing question for the default parallel gate; the root cause was not established by this audit.

## Recommended next qualification

Run the emitted-runtime tests in CI with pinned LuaJIT and Lua 5.1; enable the LOVE smoke tests in a graphical runner; retain the cross-file matrix as a small, fast regression gate. For future emitter changes, add a minimal runtime oracle for each changed lowering rule and compare emitted behavior with the documented source semantics.

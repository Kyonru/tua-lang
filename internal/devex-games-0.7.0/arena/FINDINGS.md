# Meteor Dodge developer-experience trial

## Scope and baseline

Trial date: 2026-11-10. Tua 0.7.0 checkout. The worktree already had unrelated edits in `crates/tua-cli/tests/cli.rs` and `internal/audits/generics-polymorphism-0.7.0-final/REPORT.md`; this trial did not edit them. The project is under `internal/devex-games-0.7.0/arena` and has no external assets.

The project is a small playable LOVE game, rather than a syntax-only fixture. Its update loop uses two unrelated classes through an imported `Actor` protocol, a bound generic function, and an imported generic class instance. The display, controls, collision, scoring, and reset paths are included in the generated Lua smoke.

## Confirmed defect and fix

**A-01 — False argument-count error for an imported class method used as the final call argument.** Severity: medium. Reproduction: `cd repros/nested-generic-call && ../../../../../target/debug/tua check`. Before the fix, `math.floor(box:get())` reported `TL1004: argument count mismatch: expected 1, found 0; missing required parameters: x` at `src/main.tua:4:23`. `box` had the explicit imported `BoxInstance<number>` type. A direct `local x: number = box:get()` checked correctly; parenthesizing `(box:get())` also avoided the error. The original game encountered this at `love.graphics.print("Survived: " .. math.floor(score:get()), 16, 34)`.

Instrumenting `expand_final_call_argument` showed two passes: the initial file-only pass saw zero return types for `box:get()`, while the project-aware pass saw `[number]`. The initial pass interpreted an unresolved return count as zero and retained the false diagnostic. `crates/tua-core/src/check/mod.rs` now carries unknown final-call arity until project resolution, including calls that may return multiple values. It still checks known prefix arguments, known excess arguments, and known no-return tails. Ambiguous overloads keep an unknown return instead of prematurely choosing one. `crates/tua-core/src/check/tests.rs` and `crates/tua-cli/tests/imported_nested_call.rs` cover these behaviors, including generated-Lua execution and a negative string argument.

Focused results after the fix:

```text
RUST_MIN_STACK=33554432 cargo test -p tua-core final_call -- --nocapture
5 passed; 0 failed
RUST_MIN_STACK=33554432 cargo test -p tua-core unresolved_final_call -- --nocapture
3 passed; 0 failed
RUST_MIN_STACK=33554432 cargo test -p tua-core known_no_return_chained_call_still_checks_nil_argument -- --nocapture
1 passed; 0 failed; a resolved `make():none()` chain still reports `expected number, found nil`
RUST_MIN_STACK=33554432 cargo test -p tua-core unresolved_final_call_does_not_choose_an_ambiguous_overload_return -- --nocapture
1 passed; 0 failed
RUST_MIN_STACK=33554432 cargo test -p tua-cli --test imported_nested_call -- --nocapture
1 passed; 0 failed
```

The game's original nested call is restored and checks cleanly.

## Game and editor verification

| Command | Observed result |
| --- | --- |
| `../../../target/debug/tua fmt --check` from `arena/`, after applying `tua fmt` | `checked formatting for 3 file(s)`; exit 0 |
| `../../../target/debug/tua check` from `arena/` | `checked 3 file(s)`; exit 0 |
| `../../../target/debug/tua build` from `arena/` | `built 3 file(s)`; exit 0 |
| `lua ../smoke.lua` from `arena/build/` | `arena smoke passed`; exit 0; Lua 5.4 |
| `python3 probe_lsp.py` from `arena/` | Real LSP hover on `score:get()` returned `(method) get: () -> number`; completion at `score:` included `set` and `get`; definition resolved to `entities.tua:62` |
| Native LOVE 11.5, four-second bounded launch | Process remained alive with no stdout/stderr; terminated by the trial harness. This is launch evidence, not proof of rendering or input behavior. |
| Native LOVE with a temporary draw/quit wrapper, six-second bound | Timed out without callback output; native frame rendering could not be verified in this environment. |

The headless smoke runs the emitted code with a LOVE API stub, calls load/update/draw/keypressed, observes five rendered circles, score advancement, and reset. It does not verify actual LOVE graphics or keyboard device input.

## Developer-experience assessment

- **Worked well:** protocol methods made the mixed actor loop short and statically checked. Generic `Box<number>` retained a concrete `get(): number` across files. Hover, completion, and definition agreed on that method in the protocol-level LSP probe. Generated Lua ran under the standalone interpreter.
- **Friction:** sharing the generic class required both a runtime `require` and an `import type` plus exported instance alias. This is documented and predictable, but repetitive for a small game. A future import assistance or quick fix could generate the paired type/runtime imports; it would need to preserve Lua module-load semantics. This is a product idea, not a defect.
- **Friction:** the false TL1004 on the natural nested call looked like an incorrect `math.floor` signature, which would mislead a game developer. The fix belongs in shared call-argument checking and now has a regression test.
- **Limit:** the LSP probe validates protocol responses, not VS Code menu rendering or typing latency. A real editor session is still needed for client-side UX qualification.

## Suggested follow-up

Keep the new focused regression in CI, and test the game under a graphical LOVE runner to observe rendering and keyboard controls. If the dual import pattern recurs in more trials, design a small editor action that pairs a runtime class import with its exported instance alias rather than changing the language's Lua-compatible module model.

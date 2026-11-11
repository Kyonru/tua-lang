# Language and emitted-Lua lane

Baseline: `c08edbca`, clean tracked worktree before investigation. Scope: a risk-selected language pass on imported generic protocol bounds and emitted class identity. This lane did not individually exercise every L01–L18 syntax form; the main coverage ledger must retain untested subrows. Lua 5.4.8 was available for pure Lua runtime checks; a supported LuaJIT executable was not on `PATH`. LÖVE 11.5 exists at `/Applications/love.app/Contents/MacOS/love` but was not used for these pure class cases.

## LANG-01: imported protocol bound falsely rejects a generic class method

- **Severity:** medium; **status:** confirmed, fixed, regression tested.
- **Contract:** `docs/typing.md` and `docs/modules.md` allow a generic class type parameter constrained by one imported protocol. A method may use that parameter and the class remains well typed.
- **Reproduction:** the permanent fixture in `crates/tua-core/src/analysis/tests.rs`, test `imported_protocol_bound_accepts_generic_class_methods`. Run `RUST_MIN_STACK=16777216 cargo test -p tua-core imported_protocol_bound_accepts_generic_class_methods`.
- **Observed before fix:** adding `get(): T { return self.item }` to `Box<T: Measurable>` in `box.tua` produced `TL1003: generic class Box argument 1 for T must explicitly conform to protocol Measurable, found T` at the class binder. The same class with only `init` passed. A local protocol instead of `import type` also passed. Both the file and project checker reproduced the error.
- **Cause:** project resolution canonicalizes the imported bound; `TypeEnvironment::to_type_context` adds it from the generic class definition, then adds a parser-level copy from the method/function signature with the same `TypeParameterId`. The latter has no canonical protocol identity and overwrote the resolved bound. `satisfies_protocol_bound` then rejected the class's own parameter (see `crates/tua-core/src/types/environment.rs` and `crates/tua-core/src/types/assignability.rs`).
- **Fix:** `insert_generic_protocol_bound` keeps an existing canonical bound when a later duplicate is unresolved. The regression also checks that a same-shaped class without `implements` remains rejected.
- **Coverage gap:** the earlier imported-bound CLI test covers a bounded free function; the generic-class runtime test uses an unbounded `Box<T>`. The previous imported-child analysis test queried `main.tua` diagnostics but did not check the declaring class file.

## LANG-02: cross-module class ID collision makes `is` return true

- **Severity:** high; **status:** confirmed, fixed, runtime regression tested.
- **Contract:** runtime `is` checks distinguish unrelated classes and accept an instance of a subclass as its base (`docs/language.md`, `docs/classes.md`).
- **Reproduction:** run `RUST_MIN_STACK=16777216 cargo test -p tua-cli --test cli runtime_class_checks_use_identity_across_modules`. The test builds `foreign.tua` and `main.tua`, then executes the emitted Lua. `foreign.tua` has `Foreign` as its first class; `main.tua` has `Base` and `Child`. Before the fix, `if foreign is Base then error(...) end` ran the error even though the classes are unrelated.
- **Observed Lua before fix:** each source file assigned its first class `__type_id = 1`. An `is Base` check read `getmetatable(foreign).__is[Base.__type_id]`; both modules used key `1`, so the check was true. The source passed `tua check` and `tua build`, then failed at runtime.
- **Cause:** `crates/tua-core/src/classes.rs` assigned IDs per file, while `crates/tua-core/src/emit/mod.rs` treated those IDs as process-wide identity.
- **Fix:** generated `__is` sets now also contain the actual class tables, including ancestors. Runtime `is` checks use the class table as key. Numeric metadata remains emitted for compatibility, but is no longer used for runtime identity. The regression checks unrelated cross-module classes, own-class identity, and subclass-to-base checks. The emission snapshot and generated-Lua explanations in `docs/language.md`, `docs/classes.md`, and `docs/learn.md` were updated.
- **Coverage gap:** prior emission snapshots and inheritance tests exercised classes in one file; the imported generic/protocol runtime test used multiple modules but did not exercise `is` across them.

## LANG-03: nested imported ECS world calls lose component result types

- **Severity:** medium; **status:** confirmed, fixed, regression tested. Reconciles the older MODULES-02 imported-component schema finding rather than counting another unrelated import feature. The earlier local-world imported-component hover case now has a passing test; this nested exported-world case remained.
- **Contract:** `docs/ecs.md` says imported component schemas flow through `world:add`, `world:get`, and `world:remove` without local annotations.
- **Reproduction:** `crates/tua-core/src/analysis/tests.rs`, `imported_ecs_world_and_component_keep_get_result_schema`, plus domain CLI interaction test. `components.tua` exports both `Position` and `world`; `main.tua` uses `Shared.world:get(entity, Shared.Position)` and assigns `"wrong"` to returned `x`. Before the fix, `tua check` exited 0. A local world with the same imported component rejected the assignment. Hover already displayed `{ x: number } | nil`, exposing checker/editor disagreement.
- **Cause:** `project_local_inference` resolved the imported method's generic catalog return, but did not apply the shared ECS contextual return helper to a nested module receiver. The inferred local lost the component payload schema. `crates/tua-core/src/analysis/project_queries.rs` now uses `ecs::inferred_method_return_types` with the imported receiver and component argument for `add/get/remove`.
- **Regression:** the core test checks hover plus invalid field writes through all three method results. Domain CLI test covers the build-facing diagnostic.

## LANG-04: checker loses nilability of ECS get/remove results

- **Severity:** high for runtime crash potential; **status:** confirmed, fixed, regression tested. Reopens old ECS-01 with a narrower current root cause. The old report identified a nonnullable domain return; current `ecs.rs` already returns `T | nil` for `get/remove`, but the checker still used a generic catalog result.
- **Contract:** `docs/ecs.md` states `get/remove` return nil when the component is absent. Reading a field without a guard can crash emitted Lua.
- **Reproduction:** `internal/audits/features-2026-11-07/ecs/missing_get` and `missing_remove`: before this fix, current-source `tua check`/`build` succeeded and emitted Lua indexed nil. After the fix, each `tua check` exits 1 with TL1011 at `position.x`, requiring a nil guard. The focused core regression `ecs_get_and_remove_results_require_nil_guards` checks both unguarded failures and a guarded success.
- **Cause:** `Checker::check_first_call_returns` used the typed method catalog's broad return instead of `ecs::inferred_method_return_types`; the editor and domain helper had more precise results. The checker now applies that shared helper after checking method arguments, preserving `T | nil` and keeping `add` nonnullable.

## Verification log for this lane

| Check | Outcome |
| --- | --- |
| New imported-bound regression before fix | Failed with the false TL1003 on `src/box.tua` |
| New imported-bound regression after fix | Passed |
| `RUST_MIN_STACK=16777216 cargo test -q -p tua-core` after each fix | 1,928 passed, one ignored (1,929 total) |
| `cargo test -q -p tua-core` without the repository's stack setting | Aborted on stack overflow in `builtin_module::tests::module_types_are_structural_and_carry_their_contracts`; rerunning with the Makefile's `RUST_MIN_STACK=16777216` passed. This is a test invocation requirement, not evidence of a product regression. |
| `RUST_MIN_STACK=16777216 cargo test -q -p tua-lsp` after bound fix | 448 passed |
| Existing imported-bound CLI test | Passed |
| New cross-module runtime `is` CLI test | Passed; would fail before fix on Lua 5.4 |
| `RUST_MIN_STACK=16777216 cargo test -q -p tua-cli --test cli` | 142 passed |
| `make docs-build` | Passed, no issues found |
| `make feature-examples-check` | Passed: 13 guides mapped to 14 feature-tour source files |
| `make build-examples` | Passed: love-minimal, love-full, love-tua, audio-system, features, project-model, and starfall built; audio-system emitted one low-tempo-confidence warning |
| Forward-declared parent scratch fixture | `Child extends Parent` declared before `Parent` built and ran under Lua 5.4; `child is Parent` stayed true with class-table keys |
| New cross-file ECS core regression before/after fix | Failed with no diagnostics before; passes after, including imported add/get/remove payload fields and hover |
| New nullable ECS core regression | Passed after checker fix; old `missing_get` and `missing_remove` CLI fixtures each now fail check with TL1011 rather than crash after a successful build |
| `RUST_MIN_STACK=16777216 cargo test -q -p tua-core` after both ECS fixes | 1,930 passed, one ignored (1,931 total) |
| `RUST_MIN_STACK=16777216 cargo test -q -p tua-lsp` after both ECS fixes | 448 passed |
| `cargo fmt --all -- --check` | Was clean after bound fix; later flagged an unrelated concurrent CLI ownership file and one line in `classes.rs`. `classes.rs` was formatted locally. Final workspace format gate belongs to the lead. |
| Three-file CLI fixture in `/private/tmp/tua-language-audit-2026-11-11/generic-protocol` | Current-source `tua check`, `tua build`, and Lua 5.4 execution passed after bound fix. This temporary fixture is not part of the committed test suite. |

No claim is made about LuaJIT-specific semantics, graphical LÖVE behavior, long-session editor behavior, or platform packages from these checks.

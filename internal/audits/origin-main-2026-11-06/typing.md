# Language typing and checker delta audit

Compared `origin/main` (`b40dbb445f7d8ba36e9312909be7c5d2949ac4af`) with initial HEAD `d7aed2e5f1297b80a948e055985375bceaff6f0c`. Investigation and candidate fixes are uncommitted. Prior audit artifacts remain unchanged under `../devex-2026-11-06/typing/`.

These are confirmed defects in paths reviewed for this delta. They are **not all demonstrated regressions introduced after origin/main**: no old-revision executable was built for attribution. The previous audit's 64-case CLI/LSP sweep supplies baseline reproductions and cross-surface evidence; this pass additionally inspects the actual changed checker, inference, narrowing, type-model, and scope code and adds permanent regression tests.

## Fixed findings

| Finding | Impact | Root and candidate fix | Regression tests in `crates/tua-core/src/check/tests.rs` |
| --- | --- | --- | --- |
| TYP-01 High: contextual callback return unchecked | Direct named typed callback calls and annotated callback locals accepted number bodies under string return contracts | Select named overload using signature metadata before checking anonymous bodies; use existing contextual expression checker for selected parameters and annotated local initializers. Avoid the body cache swallowing a later contextual check. | `contextual_callbacks_check_inferred_returns_for_named_calls_and_locals` (bad named/local, good context-derived parameter arithmetic) |
| TYP-02 High: terminal property after call loses checker type | `factory().nested.count` appears number in editor but accepted as string | `ast_utils::var_expression_call_prefix` lets both checker and environment use existing call resolution before terminal field/index resolution; still validate call arguments when environment already knows the initializer type. | `terminal_call_result_fields_preserve_contracts`, `terminal_call_result_fields_check_call_arguments` |
| TYP-03 Medium: native named vararg metadata erased | `log(prefix:string,...:string)` accepted numeric tail; signature omitted type | Preserve annotation from parser into canonical TypedFunction. Also retain analogous LuaDoc group's computed vararg type. | `native_named_vararg_contracts_check_tail_arguments` (local/global declarations, accepted/rejected tail) |
| TYP-04 Medium: final call arguments not expanded | Valid `consume(pair())` rejected for too few arguments | Shared checker argument normalization expands the final unparenthesized call, using its full existing checked return list. Applies to named, function-type, and external overload checking. Other expressions retain syntax for contextual checking. | `final_call_arguments_expand_multiple_returns` (direct/alias, parenthesized/nonfinal negative controls, wrong second return) |
| TYP-05 Medium: missing scalar assignment slot is any | `local a,b=1; local wrong:string=b` accepted although b is nil | Shared assignment-slot helper only expands final call/vararg expressions. Scalar/parenthesized excess slots take existing nil path for checker/environment/editor. | `missing_scalar_assignment_slots_are_nil` (scalar, parenthesized call, expanding call control) |
| HOV-02 High: name-only local-function lookup | Inner string function inherited outer number signature and false diagnostics | Declaration-keyed typed-function lookup exposed as `TypeEnvironment::function_at`; checker uses lexical body contracts and scoped callable values. Completion agent integrates shared API in semantic/hover/signature consumers. Function parameter/return inference also keyed by declaration body, not bare name. | `shadowed_named_functions_use_lexical_contracts`, `shadowed_named_function_results_keep_their_call_site_type`, `shadowed_named_functions_infer_returns_from_their_own_parameters` |

Baseline minimal sources, exact commands, diagnostics, source analysis, documentation basis, test gaps and raw LSP responses are in `../devex-2026-11-06/typing/REPORT.md`, `cases.json`, `results.json`. HOV-02 baseline corroboration is `results-shadowed.json`. Native vararg signature corroboration is in the hover lane's `focused-evidence.json`. The candidate does not overwrite those baseline results.

## Delta coverage

| Changed area | Verification | Assessment |
| --- | --- | --- |
| Declaration-specific native/LuaDoc local contracts; captured locals; lexical shadowing (`types/environment.rs`, `check/mod.rs`, `scope/mod.rs`) | Inspected diff and existing `declaration_contracts_do_not_leak_between_locals`, `declaration_contracts_keep_luadoc_locals_independent`, `unknown_local_initializers_still_shadow_unrelated_contracts`; checker/types suites; new shadow tests above | Local declaration contracts hold; named functions needed additional identity fix. |
| Anonymous optional/default parameter metadata (`types/mod.rs`, checker contextual path) | Inspected diff, `anonymous_optional_and_default_parameters_share_named_function_contracts`; existing optional/default rejection controls; TYP-01 new tests | Required arity is preserved through anonymous body metadata; missing contextual return checks fixed. |
| Builtin assert recognition and guard continuation (`narrowing.rs`, checker/environment) | Inspected assertion predicate, global override guards, captured/scoped lookup; `builtin_assert_narrows_locals_but_shadowing_and_reassignment_do_not` | Good and shadowed/reassigned/effectful-message controls pass. |
| Nullable member-path narrowing and invalidation | Inspected `guarded_member_type`, position conversion, effect traversal, alias writes, nested closure boundary; `stable_member_guards_expire_on_effects`, `love_conf_initial_window_read_state_preserves_nullable_writes` | Pure guarded reads work; direct/alias writes, intervening calls, short-circuit effects and closures invalidate. Captured scalar mutation remains documented conservative boundary below. |
| Chained expression and multi-return inference | Inspected shared `resolve_chained_call_returns` and checker counterpart; existing `a_chained_call_takes_its_last_call_result`, `chained_call_inference_preserves_multi_return_and_void_boundaries`; new terminal-chain/value-list regressions | Existing last-call path omitted VarExpression terminal fields; fixed. |
| Recursive nominal class back-edges (`types/model.rs`, alias resolver) | Inspected finite `nominal_reference` representation and alias resolution; existing type suite and class-array/checker tests | No new defect confirmed in inspected core paths. Editor source-coordinate issues owned by completion agent. |
| Table insert/sort/remove defaults and contextual comparators | Inspected binding override gates and builtin signatures; checker `table_sort_contextualizes_comparators_and_checks_their_contracts`; types tests for optional remove, nested budget, user replacement and insert writes | Typed arrays and dynamic override controls pass. |
| Imported module contracts and class Self | Baseline cross-file probes plus changed contract construction inspection; existing `accepts_required_module_function_returns_in_checked_function_bodies`, class Self/array polymorphism tests | Terminal-chain type loss fixed via shared call resolver; project isolation fixed by lead. Broader cross-file editor/query consumers owned by completion agent. |
| LOVE File/FileData and binding-aware assets | Read checker binding gates; existing generated overload, custom asset binding, image data and shader tests in checker suite | Covered by checker suite; deeper catalog/project inspection owned by completion agent. |
| Scope whitespace/default references | Inspected root/body span widening, default-reference recording, fallback binding lookup diff | Semantic consumer and editor provenance verification owned by completion agent. |
| Type shape mutation helper (`insert_field_path`) | Inspected iterative construction, existing structural/alias checker tests | No additional defect confirmed. |

## Verification and reproduction

Run from repository root, with Rust installed:

```sh
RUST_MIN_STACK=16777216 cargo test -p tua-core check::tests --lib
RUST_MIN_STACK=16777216 cargo test -p tua-core types::tests --lib
```

Each new regression can be isolated by replacing `check::tests` with its exact name in the table. Test helpers disable unrelated standard lint warnings, preserving checker diagnostics. Raw command logs are under ignored `target/devex-audit-checker-final.log`, `target/devex-audit-types-final.log`, and `target/devex-audit-shadow-inferred.log` (the last records the stronger HOV-02 test failing before the declaration-inference fix).

During development: 204 checker tests passed before lexical fixes; first lexical attempt regressed LuaDoc overload handling (204/205), caught by the existing suite. Fixed by retaining named overload metadata when registering local signatures. Then 207 checker tests and 73 type tests passed. The stronger inferred-return shadow test correctly failed against that intermediate candidate, prompting declaration-keyed inference rather than an annotated-only fix. Final candidate outcomes are recorded below after verification.

Formatting: targeted rustfmt on owned Rust files. Broad workspace gates, LSP cross-surface tests, release binary replay, docs build and final diff checks are coordinated by the lead, not claimed complete here.

## Remaining limits and design decisions

- Unknown dynamic call return counts and arbitrary `...` forwarding are not a new exact variadic-arity model. The new argument expansion uses statically available call return lists and preserves existing dynamic policy.
- Empty/no-return stubs intentionally retain existing conservative `any` behavior. This is distinct from a provably absent scalar assignment slot being nil.
- Negative `is` branch complements are not newly promised; true branches are covered. Generics/protocols remain unsupported design work, not silently implemented by this audit.
- Arbitrary calls mutating captured narrowed scalar locals are not effect-tracked; existing docs warn to copy a guarded value before indirect mutation. Member-path guards are more conservative and expire on effects.
- Indexed optional-function grouping and broad mixin/dynamic-service modeling remain separate language/design issues from the confirmed fixes.
- This lane does not claim installed VS Code rendering, visual runtime qualification, or universal language soundness.

Typing-lane checkpoint: **208 checker tests and 73 types tests passed**. Subsequent independent review extended HOV-02 to wrapper call lookup, which still used a global name map at that checkpoint. The completion lane fixed the remaining path with declaration-keyed callable lookup and passing `audit_wrapper_return_inference_uses_lexical_function_identity` outer/inner controls. It also corrected LuaDoc-only callable origins, with passing `.tua`/`.lua` guarded-projection coverage. See [editor.md](editor.md) for those completed follow-ups and [VERIFICATION.md](VERIFICATION.md) for final integrated counts; the checkpoint counts are not final totals.

Key candidate source locations (line numbers at lane handoff): `ast_utils.rs:40` shared call-prefix decomposition; `parser/mod.rs:1293` native vararg extraction; `types/environment.rs:305` scoped function API and `:2747` inference identity; `check/mod.rs:2808` typed-call context selection, `:2850` callback body checking, `:2972` final-call value expansion.

## Independent lead-change review and additional fix

Reviewed candidate project module graph ownership, auto-import routing and snippet negotiation, and bounded Busted output capture. Output capture keeps draining both streams after the cap and preserves child status; structured reports are separate files. Auto-import augmentation now shares normal completion filtering on the invoked fast path. Snippets are opt-in both initially and at resolve. Unicode protocol work was still active and is not independently qualified here.

**GRAPH-OVERLAY (High, independently reproduced): type-library declaration aliases still crossed workspace boundaries after ordinary require-root isolation.** `AnalysisHost::declaration_modules` flattened all configured libraries into one first-wins alias map. The new per-workspace search roots retained that global map, and require resolution consults declaration aliases before searching disk. A second project's `require('engine')` selected `first/types/api.lua` instead of its own string-valued declaration. Exact failing command/output: `RUST_MIN_STACK=16777216 cargo test -p tua-core workspace_declaration_overlays_keep_project_ownership --lib`, logged at `target/devex-audit-overlay-before.log` (assertion shows both actual and expected paths).

Fix direction implemented: carry each project's configured type libraries alongside its module workspace metadata; construct scoped declaration maps using one shared metadata-prefix helper; retain the legacy global overlay API only for callers without workspace ownership. Explicit libraries outside the project root remain supported. Declaration lookup fallback now accepts the source path, and an existing unresolved graph edge cannot fall through to unrelated global roots.

Regression `workspace_declaration_overlays_keep_project_ownership` checks two conflicting aliases, a library outside its owner root, and a third project with no overlay (must remain missing). LSP regression `multi_workspace_type_library_overlays_are_project_local` checks actual hover responses under two initialized workspace folders and actual config files. Candidate results and broad verification belong to the final lead gate.

Reproduction caveat: original pre-fix overlay log used the declaration inside `second/types`; the permanent regression additionally moved it to `shared/second-types` to test explicit external library ownership. Both are the same alias-collision mechanism. No old HEAD artifacts were rewritten.

Overlay verification note: the first LSP assertion accidentally hovered the `api` receiver (module path documentation), not `value`; corrected the request needle to the terminal field and reran. This was a harness cursor mistake, not a candidate product failure. Core before/after evidence uses exact resolved declaration paths and is unaffected.

Overlay candidate final focused outcomes:

- `RUST_MIN_STACK=16777216 cargo test -p tua-core workspace_declaration_overlays_keep_project_ownership --lib`: **1 passed**, including external library root and missing third-workspace controls.
- `RUST_MIN_STACK=16777216 cargo test -p tua-core module_graph::tests --lib`: **16 passed**, including legacy declaration overlays and `project_module_resolution_never_falls_back_to_another_workspace`.
- `RUST_MIN_STACK=16777216 cargo test -p tua-lsp multi_workspace_type_library_overlays_are_project_local --lib`: **1 passed**, actual two-project field hover responses.
- Focused logs: `target/devex-audit-overlay-after.log`, `target/devex-audit-modulegraph-final.log`, `target/devex-audit-overlay-lsp.log`.
- Owned source `git diff --check`: passed. No commits created by this agent.

## Release CLI/LSP replay

Ran `node target/audits/devex-fixed/typing/run.js` against the rebuilt release executable. The copy preserves the original audit runner/cases but writes candidate results into ignored `target/audits/devex-fixed/typing/results.json`; original baseline evidence remains untouched. **64 scenarios, 94 hover/completion/signature requests, 64 pull-diagnostic requests, zero harness errors.** Selected durable responses and acceptance deltas are in `typing-release-replay.json`.

All 12 CLI acceptance changes were expected corrections: bad contextual callbacks, call-result properties, native vararg tails, missing/parenthesized assignment slots now reject; valid final-call argument expansion and lexical shadow calls now pass. The `defaults` scenario also now rejects its deliberately wrong numeric typed-vararg tail. No newly rejected valid scenario or newly accepted invalid scenario was observed in this suite. Every CLI-success scenario also had no severity-error pull diagnostics.

Protocol changes match intended fixes: typed vararg hover/signature labels retain `...: string`; missing slots hover nil; inner lexical function and its result hover string while outer result remains number; class-prefix arithmetic now gets number hover. Completion replacement spans now cover token suffixes and no-snippet clients receive plain insertions. Union expression hover ranges changed to source coordinates without changing type. No unexpected protocol regression was observed in this suite.

That release replay exposed one additional pre-existing cross-surface gap: `callback-function-value` correctly rejected the numeric return under a contextual string contract, but hovering its unannotated parameter reported `any`, whereas a direct callback argument reported number. **This gap is now fixed**, as detailed below; the saved replay intentionally records the earlier executable rather than rewriting evidence. The suite does not replace the completed hover/projection and recursive-wrapper regressions documented by the completion lane.

The release replay above used SHA-256 `f8a80336d0859172bc56ec201ef322c679d600794597d984651f47884970a270`. Its residual callback-local hover gap subsequently received a shared semantic fix: match the lexical callback body to its annotated local initializer and obtain parameter context from the existing resolved function contract. This covers native signatures, aliases, and LuaDoc `---@type`, preserving an untyped same-named parameter in a nested function. Permanent regression: `hover::tests::annotated_callback_local_parameters_use_contextual_types`. Later candidate verification is separate from the earlier release replay.

To recreate the isolated release replay without overwriting baseline reports:

```sh
mkdir -p target/audits/devex-fixed/typing
cp internal/audits/devex-2026-11-06/typing/run.js internal/audits/devex-2026-11-06/typing/cases.json target/audits/devex-fixed/typing/
node target/audits/devex-fixed/typing/run.js
```

The runner defaults to `target/release/tua`; set `TUA_AUDIT_BINARY` to compare another executable. Generated fixture projects under target are intentionally reused. This copying scheme preserves the runner's four-level repository-root calculation.

Final callback-local verification: `RUST_MIN_STACK=16777216 cargo test -p tua-core hover::tests --lib` **78 passed**, including the new context and shadowing controls. One redundant focused invocation received SIGKILL before tests started; the subsequent complete hover suite passed. Log: `target/devex-audit-hover-final.log`. No further callback-local source edits followed that pass; later integrated verification supersedes these checkpoint totals.

## Integrated handoff

The checkpoints above preserve investigation history, including corrected fixture mistakes and intermediate failures. [Final integrated verification](VERIFICATION.md) is authoritative for the finished working tree, rebuilt-server replay and test VSIX; earlier counts and intermediate binaries are not final package provenance.

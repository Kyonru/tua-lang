# Editor semantics delta audit and fixes

Scope: `origin/main..HEAD`, plus the confirmed editor findings from `../devex-2026-11-06`. No commits made by this lane. Prior evidence and fixtures are preserved. Source inspection covers the changed shared semantic resolver, completion context/query/ranking paths, hover and signature resolver changes (992 additions /121 deletions across these paths in the audited committed delta). The prior audit supplies protocol reproductions; this follow-up adds permanent focused core regressions and integration remains the lead's responsibility.

## Changes

| Prior finding | Resolution | Permanent evidence |
|---|---|---|
| CMP-03 | Completion replacement includes identifier/path suffix after cursor; middle-identifier invocation is accepted; selecting a LOVE method preserves an already-written argument list while retaining dot-to-colon correction. | `audit_completion_edits_preserve_existing_suffix_and_call` |
| CMP-04 | Asset insertion escapes backslashes, line breaks and the active string delimiter through a shared completion string-encoding helper. Display paths remain literal filenames. | Same regression checks single-quote encoding; previous filesystem fixture remains in prior audit. |
| CMP-05 | Dot/colon candidates exclude names that require bracket notation, using the Lua lexer (so contextual Tua `type`/`class` remain valid members). | `audit_dot_completion_excludes_keys_requiring_brackets` |
| CMP-07 and U-01 | Shared call-return inference consults native class constructor contracts; removed the whitespace-sensitive source-line scan. Incomplete completion recovery resolves positions through normalized-class provenance even when the recovered line was erased. | `audit_native_constructor_completion_is_whitespace_independent`, `audit_native_constructor_signature_for_direct_and_local_receivers` |
| HOV-01 | Call-result member hover uses the shared receiver resolver before falling back to expression hover, preserving catalog identity, method documentation and precise member range. | `audit_call_result_hover_preserves_method_identity` covers value and void methods. |
| HOV-03 | Expression traversal receives parser coordinates; inferred expressions resolve names at source coordinates; final spans are mapped back and generated-only endpoints are excluded. | `audit_expression_hover_after_class_uses_source_coordinates` |
| HOV-04 | Native function hover and signature parameter labels show `?` using required-parameter count. Default/optional parameters are distinguishable from required nullable parameters. | `audit_hover_distinguishes_optional_from_required_nullable_parameters`, `audit_signature_distinguishes_optional_from_required_nullable_parameters` |
| HOV-02 (typing lane) | Editor lookups use typing lane's lexical `function_at` API through a shared source-position adapter. | Existing hover/signature suite; typing lane owns identity regressions. |

## Verification

Commands use `RUST_MIN_STACK=16777216`:

- `cargo test -p tua-core audit_ --lib -- --nocapture`: 10 passed before lexical-function integration; includes two preexisting class provenance tests.
- `cargo test -p tua-core hover::tests --lib`: 77 passed after lexical-function integration.
- `cargo test -p tua-core completion::tests --lib`: 140 passed (filter also matches inline/model-completion tests).
- `cargo test -p tua-core signature_help::tests --lib`: 37 passed.
- `rustfmt --edition 2021` on owned Rust files.

Exploration initially reproduced both constructor recovery failure and invalid-key insertion; their regressions failed before the corresponding fixes and passed afterward. A transient undefined-variable compile error during scoped-function integration was corrected before these successful checks.

## Limits and remaining capabilities

- Invalid dot-key candidates are omitted; automatic conversion to bracket syntax is a separate capability, not claimed implemented.
- Optionality is displayed; rendering the exact default expression is not added. LuaDoc overloads without required-arity metadata retain existing labels.
- Existing-call preservation currently covers LOVE method snippets, where the confirmed corruption occurred; other future function-snippet producers should reuse equivalent edit-context handling.
- The prior confirmed failures were protocol-level (except parent-owned actual-editor confirmations). These new lane checks are core query tests; current-binary protocol replay and full cross-crate gates are left to lead integration.
- No protocols/generics, class initialization order, dynamic runtime behavior, or other language design decisions were changed.

## Additional committed-delta coverage

Reviewed changes in `asset/mod.rs`, `asset/inline_shaders.rs`, `signal.rs`, `analysis/project_queries.rs`, `docs/signals.md`, `docs/testing.md`, `editors/vscode/src/testing/index.js`, `testing/coverage.js`, extension activation, LOVE game commands and the offline reference command.

- Asset metadata and inline shader recognition now consult lexical root resolution; project path completion uses the same binding guard. Checked that source/parser position mapping is applied in the completion guard. Ran `cargo test -p tua-core asset_metadata_and_completion_respect_loader_bindings --lib`: 1 passed. This exercises local helper/parameter/global assignment/shadowed LOVE roots against hover, definition, completion, asset inventory and rename edits.
- Signal schemas now prefer explicit contracts, infer only consistent emit payloads, preserve schema origin module for imported type resolution, and follow aliases with reassignment/cycle safeguards. Reviewed recursion guard drop restoration and conservative conflict handling. Ran `cargo test -p tua-core cross_file_event_payload_uses_imported_method_return_and_unsaved_edits --lib`: 1 passed. It checks boolean-to-string edits in the producer and old snapshot isolation.
- Test Explorer documentation matches lazy discovery, saved-buffer operation, explicit continuous opt-in, serialized refresh/runs, trust checks, dynamic case discovery and line-only coverage. Reviewed watcher exclusions, cancellation cleanup, result aggregation and coverage validation. Ran `node --test editors/vscode/test/unit/testing.test.js`: 5 passed, including lazy refresh/run lifecycle, coverage merge and the lead's new cancellation test. These tests mock VS Code; they do not qualify installed-editor rendering.
- Extension activation registers testing and the offline LOVE reference, while Markdown command trust is restricted to the annotation-guide command. Offline reference uses packaged local HTML with a restrictive CSP. This source inspection does not certify visual rendering or package-resource availability.

Integration caught an existing assertion expecting the old required-looking default parameter label; it now expects `value?: number`. Middle-token completion retains the original suppression at the **start** of an existing identifier (empty prefix), preventing unrelated global suggestions in unresolved contextual table-key slots, while nonempty middle-token prefixes remain supported.

## Final integration follow-up

A new adversarial test confirmed that a locally shadowed class constructor could retain native class members. The shared native-constructor lookup now checks the actual lexical initializer's source provenance before using generated constructor metadata. `audit_constructor_resolution_respects_local_shadowing` verifies a locally substituted, explicitly typed factory yields `other`, excluding native `read`. This is a confirmed correctness fix, not merely a hypothetical concern.

Final commands after this guard and contextual identifier correction:

- `RUST_MIN_STACK=16777216 cargo test -p tua-core audit_ --lib`: **11 passed**, including nine new editor tests and two existing provenance tests.
- `RUST_MIN_STACK=16777216 cargo test -p tua-core declaration_library_drives_recursive_discriminated_table_completion --lib`: **1 passed**.
- `RUST_MIN_STACK=16777216 cargo test -p tua-core anonymous_default_signature_and_body_hover_agree --lib`: **1 passed**.
- `git diff --check`: passed at lane handoff.

Product edits are ready for lead integration. Scope/typing implementation remains owned by the typing lane; LSP request/client capability paths and broad final gates remain owned by the lead.

## Independent review of the other fix lanes

Reviewed checker changes for call-prefix inference, contextual callback checking, native/LuaDoc variadic contracts, scalar assignment padding and expansion of the final call argument. The expansion preserves expressions in earlier positions and does not expand parenthesized calls. Reviewed emitter comment preservation against lexical closing delimiters, and hotreload helper-upvalue traversal against cycles and imported state.

One additional confirmed defect was found: wrapper return inference still used a global name-keyed function type map. A numeric outer `parse` and a later nested string `parse` made `outer() { return parse(1) }` infer string and produced two false `TL1003` diagnostics. The inference catalog now retains declaration-keyed callable types and selects them with the same scope binding identity used by editor queries. The permanent `audit_wrapper_return_inference_uses_lexical_function_identity` test failed before the change and passed afterward, covering both outer and inner wrappers. This extends HOV-02's root-cause fix across the remaining inference path.

`RUST_MIN_STACK=16777216 cargo test -p tua-core audit_ --lib -- --nocapture`: **12 passed** after the wrapper fix and contextual Lua member-name control.

`LUA_PATH='vendor/runtime/?.lua;;' lua vendor/tests/hotreload_test.lua`: passed independently, including private helper captures, preserved live instance state, failed preflight recovery and shader reload controls. This is the host Lua runtime; the lead separately owns native LOVE/LuaJIT qualification.

After formatting the shared inference follow-up, `cargo test -p tua-core check::tests --lib` passed **209 tests**, and `cargo test -p tua-core types::tests --lib` passed **73 tests** (both with `RUST_MIN_STACK=16777216`). The existing `emit::tests::function_header_comments_survive_annotation_erasure` regression also passed independently via the current compiled core test binary. `git diff --check` passed at final handoff.

## Final reviewer layout correction

Independent reviewer identified that call-result member hover still required an adjacent `:`/`.`. The extended regression confirmed `: getWidth()` returned the whole `expression: number` with no docs. Hover now trims whitespace before locating the separator and uses the existing lexical receiver helper, preserving identity for same-line spacing and newline/indented-newline layouts. The existing regression covers four separator layouts for both value-returning and void LOVE methods. Only `hover/mod.rs` and its test changed in this final correction; focused verification follows.

The final hover layout regression passed after the correction (eight method/layout combinations in one test). A second reviewer hypothesis concerned whitespace/comment-containing record keys being tokenized as identifiers with attached trivia. An independent temporary Rust probe linked to the existing Full Moon library disproved that hypothesis: trivia forms extra tokens and the two-token predicate already rejects it. An explicit spelling-equality guard and whitespace/comment/newline/tab controls were nevertheless added as defensive coverage; this is not counted as a new defect.

Final defensive member-name test: `RUST_MIN_STACK=16777216 cargo test -p tua-core audit_dot_completion_excludes_keys_requiring_brackets --lib` — **1 passed**. Product edits stopped after this check; lead owns the final integrated gate.

## Full-suite compatibility correction

The lead's integrated gate exposed a LuaDoc compatibility regression in declaration-keyed callable inference: `.lua` functions added during LuaDoc lowering have no corresponding native annotation-vector entry. Consequently their parameter and inferred-return keys had lost body origins, and the guarded projection example returned `{x:any,y:any}`. The shared callable-origin collector now preserves native annotation origins and recovers origins for appended LuaDoc-only entries from the existing Lua AST. Parameters, callable catalog entries, final return assignment and declaration lookup consume those origins consistently. `hovers_guarded_projection_from_tua_and_lua_modules` passed after the correction for both `.tua` and `.lua` providers. A fresh full core suite is running with output at `/tmp/tua-editor-final-core.log`; the lead's earlier failure predates this correction.

The fresh full-core runs both passed **1663 tests** with **1 ignored** and one failure in the concurrently added asset-decoder regression (first its unused-local warning expectation, then its rename-identity assertion). These are reported to the asset lane; neither run had remaining editor/callable-origin/projection failures. Do not read these runs as a complete gate pass. The lead's final gate will supersede them after the asset lane settles. Latest captured output: `/tmp/tua-editor-final-core.log`.

## Integrated handoff

The checkpoints above preserve investigation history, including corrected fixture mistakes and intermediate failures. [Final integrated verification](VERIFICATION.md) is authoritative for the finished working tree, rebuilt-server replay and test VSIX; earlier counts and intermediate binaries are not final package provenance.

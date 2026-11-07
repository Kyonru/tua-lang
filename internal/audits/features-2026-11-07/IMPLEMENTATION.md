# Feature-audit implementation reconciliation

This maps all 71 original normalized findings to implementation commits and regression artifacts: 65 confirmed defects, five scoped capability gaps, and one policy clarification. See [final verification](FIX-VERIFICATION.md) for the consolidated results and remaining environment/platform limits. Historical audit reports preserve the original baseline; integration repairs are recorded below.

For a named Rust regression below, run `CARGO_INCREMENTAL=0 cargo test -p <owning-crate> <test-name>` from the repository root; the listed file identifies
the crate. Full commands and results are in FIX-VERIFICATION.md. The CLI/runtime
replay scripts include their own fixture setup; see IMPLEMENTATION-REVIEW.md for
requirements. These tests exercise the reported cases, not every possible program.

## Per-finding implementation and regression coverage

### STORES-03 — Reentrant set returns a later state rather than its argument

Classification: confirmed defect. Original evidence: [stores/REPORT.md](stores/REPORT.md).

Implementation: 25d7ff2.

Test artifacts: `crates/tua-cli/tests/fixtures/store_runtime.lua`.

### SIGNALS-01 — `once` can invoke twice during nested emits

Classification: confirmed defect. Original evidence: [signals/REPORT.md](signals/REPORT.md).

Implementation: 37f968f.

Test artifacts: `crates/tua-cli/tests/fixtures/signal_runtime.lua`.

### SIGNALS-02 — Statically invalid handlers compile and fail at runtime

Classification: confirmed defect. Original evidence: [signals/REPORT.md](signals/REPORT.md).

Implementation: 2ebe14c,b5daf23.

Added focused regressions/helpers: `visit_function_call`, `rejects_known_nonfunction_signal_handlers`, `valid_handler`.

### SIGNALS-03 — Directly exported bus loses definition/reference identity

Classification: confirmed defect. Original evidence: [signals/REPORT.md](signals/REPORT.md).

Implementation: f66d9f4,ba9f4bb.

Added focused regressions/helpers: `directly_returned_signal_buses_preserve_navigation_through_reexports`.

Test artifacts: `crates/tua-core/src/analysis/tests.rs`, `crates/tua-core/src/inlay_hint/tests.rs`.

### SM-01 — Event names can overwrite typed API fields without a diagnostic

Classification: confirmed defect. Original evidence: [state_machines/REPORT.md](state_machines/REPORT.md).

Implementation: 57c2d2f.

Added focused regressions/helpers: `visit_function_call`, `reserved_event_names_are_diagnosed`.

### SM-02 — A present nonliteral initial state becomes a fictional `none`

Classification: confirmed defect. Original evidence: [state_machines/REPORT.md](state_machines/REPORT.md).

Implementation: ad49394.

Added focused regressions/helpers: `dynamic_initial_state_does_not_invent_none`.

### SM-03 — Cancelled before callback still executes leave side effects

Classification: confirmed defect. Original evidence: [state_machines/REPORT.md](state_machines/REPORT.md).

Implementation: 22ece87.

Test artifacts: `crates/tua-cli/tests/fixtures/state_machine_runtime.lua`.

### SM-04 — Chaining an asynchronous transition from an after callback loses it

Classification: confirmed defect. Original evidence: [state_machines/REPORT.md](state_machines/REPORT.md).

Implementation: 107ced4.

Test artifacts: `crates/tua-cli/tests/fixtures/state_machine_runtime.lua`.

### ECS-01 — get/remove erase component absence

Classification: confirmed defect. Original evidence: [ecs/REPORT.md](ecs/REPORT.md).

Implementation: 068ae03.

No newly named Rust regression was extracted from these commits. Use the changed runtime test artifacts where listed and the post-fix evidence in IMPLEMENTATION-REVIEW.md; the original feature report supplies reproduction history, not post-fix verification. Documentation-only changes require the docs gate.

### ECS-02 — World:get converts stored false to nil

Classification: confirmed defect. Original evidence: [ecs/REPORT.md](ecs/REPORT.md).

Implementation: 87b0266.

Test artifacts: `crates/tua-cli/tests/fixtures/ecs_runtime.lua`.

### ECS-03 — Documented optional component schema is required

Classification: confirmed defect. Original evidence: [ecs/REPORT.md](ecs/REPORT.md).

Implementation: 773e2fd.

Added focused regressions/helpers: `component_schema_is_optional`.

Test artifacts: `crates/tua-core/src/signature_help/tests.rs`.

### ECS-GAP-01 — System callback schema does not flow into checking/hover

Classification: feature gap. Original evidence: [ecs/REPORT.md](ecs/REPORT.md).

Implementation: 65f1af9,653eac9.

Added focused regressions/helpers: `system_callbacks_receive_component_schema_and_dt`.

### POOLS-01 — Typed factory does not constrain reset or dispose callbacks

Classification: confirmed defect. Original evidence: [pools/REPORT.md](pools/REPORT.md).

Implementation: 616ab74,9118565.

Added focused regressions/helpers: `check_table_callback_contexts`, `options_type_for_item`, `pool_hooks_inherit_and_validate_factory_payloads`, `pool_hooks_follow_imported_factories`, `contextual_callback_path`.

### POOLS-02 — Reentrant reset corrupts ownership counts and free stack

Classification: confirmed defect. Original evidence: [pools/REPORT.md](pools/REPORT.md).

Implementation: af82045.

Test artifacts: `crates/tua-cli/tests/fixtures/object_pool_runtime.lua`.

### INPUT-01 — Numeric spelling bypasses deadzone validation

Classification: confirmed defect. Original evidence: [input/REPORT.md](input/REPORT.md).

Implementation: f48f41f.

Added focused regressions/helpers: `validates_signed_and_hex_deadzone_literals`.

### INPUT-02 — Statically empty controls are treated as unknown controls

Classification: confirmed defect. Original evidence: [input/REPORT.md](input/REPORT.md).

Implementation: d064a06.

Added focused regressions/helpers: `literal_union_for_names`, `empty_static_controls_have_no_callable_names`.

### AUDIO-01 — Overlapping global and cue limits are not enforced together

Classification: confirmed defect. Original evidence: [audio/REPORT.md](audio/REPORT.md).

Implementation: 68a72d7,71a6c11.

Test artifacts: `crates/tua-cli/tests/fixtures/audio_runtime.lua`.

### AUDIO-02 — Bar transitions miss boundaries crossed by a long frame

Classification: confirmed defect. Original evidence: [audio/REPORT.md](audio/REPORT.md).

Implementation: f38674d.

Test artifacts: `crates/tua-cli/tests/fixtures/audio_runtime.lua`.

### AUDIO-03 — Looping music stops emitting beats after the first playback cycle

Classification: confirmed defect. Original evidence: [audio/REPORT.md](audio/REPORT.md).

Implementation: 9954517.

Test artifacts: `crates/tua-cli/tests/fixtures/audio_runtime.lua`.

### AUDIO-04 — Loop regions are ignored when a scene has no BPM

Classification: confirmed defect. Original evidence: [audio/REPORT.md](audio/REPORT.md).

Implementation: c6c43c0.

Test artifacts: `crates/tua-cli/tests/fixtures/audio_runtime.lua`.

### AUDIO-05 — snapshot analysis state is not detached

Classification: confirmed defect. Original evidence: [audio/REPORT.md](audio/REPORT.md).

Implementation: 05a17d6.

Test artifacts: `crates/tua-cli/tests/fixtures/audio_runtime.lua`.

### SPRITES-01 — Zero duration lists hang playback

Classification: confirmed defect. Original evidence: [sprites/REPORT.md](sprites/REPORT.md).

Implementation: bc1934d,a171e82.

Added focused regressions/helpers: `rejects_nonpositive_durations_in_lists_and_ranges`.

Test artifacts: `vendor/tests/run.lua`, `vendor/tests/sprite_test.lua`.

### SPRITES-02 — pauseAtStart stops on final frame

Classification: confirmed defect. Original evidence: [sprites/REPORT.md](sprites/REPORT.md).

Implementation: 5c3987b.

Test artifacts: `vendor/tests/sprite_test.lua`.

### SPRITES-03 — Loop callbacks lose multi-wrap counts

Classification: confirmed defect. Original evidence: [sprites/REPORT.md](sprites/REPORT.md).

Implementation: 2b0f3b8.

Test artifacts: `vendor/tests/sprite_test.lua`.

### VECTORS-01 — Chained constructor property reads escape builtin lowering

Classification: confirmed defect. Original evidence: [vectors/REPORT.md](vectors/REPORT.md).

Implementation: 850ab89.

Added focused regressions/helpers: `visit_var_expression_end`, `lowers_constructor_prefixes_of_property_reads`.

Test artifacts: `crates/tua-core/src/emit/tests.rs`.

### VECTORS-02 — Game-math helper injection misses comments and bracket members

Classification: confirmed defect. Original evidence: [vectors/REPORT.md](vectors/REPORT.md).

Implementation: 399d764.

Added focused regressions/helpers: `emits_math_helpers_used_through_comments_and_static_indices`, `visit_function_call`, `visit_var_expression`, `discovers_helpers_through_trivia_and_static_bracket_members`.

Test artifacts: `crates/tua-core/src/emit/tests.rs`.

### VECTORS-03 — Nonzero tiny vector divisors incorrectly raise zero-component error

Classification: confirmed defect. Original evidence: [vectors/REPORT.md](vectors/REPORT.md).

Implementation: bb78a91.

Test artifacts: `vendor/tests/run.lua`, `vendor/tests/vector_test.lua`.

### VECTORS-04 — Normalization collapses finite large and small vectors to zero

Classification: confirmed defect. Original evidence: [vectors/REPORT.md](vectors/REPORT.md).

Implementation: ecbc226.

Test artifacts: `vendor/tests/vector_test.lua`.

### CAMERA-01 — Aliasing Camera skips runtime injection

Classification: confirmed defect. Original evidence: [camera/REPORT.md](camera/REPORT.md).

Implementation: 7b9ef3f.

Added focused regressions/helpers: `builtin_modules_are_injected_when_used_as_values`.

Test artifacts: `crates/tua-core/src/emit/tests.rs`.

### CAMERA-02 — Rotated views escape world bounds

Classification: confirmed defect. Original evidence: [camera/REPORT.md](camera/REPORT.md).

Implementation: 54086a2.

Test artifacts: `vendor/tests/camera_test.lua`.

### CAMERA-03 — Pixel-perfect drawing and coordinate conversion disagree

Classification: confirmed defect. Original evidence: [camera/REPORT.md](camera/REPORT.md).

Implementation: 4de9e0f.

Test artifacts: `vendor/tests/camera_test.lua`.

### CAMERA-04 — visibleBounds excludes part of the shaken view

Classification: confirmed defect. Original evidence: [camera/REPORT.md](camera/REPORT.md).

Implementation: b1efbe7.

Test artifacts: `vendor/tests/camera_test.lua`.

### CAMERA-05 — Platformer preset contradicts documented deadzone proportions

Classification: confirmed defect. Original evidence: [camera/REPORT.md](camera/REPORT.md).

Implementation: 5a872b9.

Test artifacts: `vendor/tests/camera_test.lua`.

### TWEEN-01 — Callback cancellation can crash tween updates

Classification: confirmed defect. Original evidence: [tween/REPORT.md](tween/REPORT.md).

Implementation: f0c5b1b.

Test artifacts: `vendor/tests/tween_test.lua`.

### TWEEN-02 — Shorthand after accepts invalid duration

Classification: confirmed defect. Original evidence: [tween/REPORT.md](tween/REPORT.md).

Implementation: cc0298c.

Added focused regressions/helpers: `tween_after_checks_each_overload_as_a_complete_contract`.

### CLASS-01 — Flattening constructor self references loses object identity

Classification: confirmed defect. Original evidence: [classes/REPORT.md](classes/REPORT.md).

Implementation: eb79b03.

Added focused regressions/helpers: `class_constructor_self_references_require_allocated_receiver`.

Test artifacts: `crates/tua-core/src/emit/tests.rs`.

### TYPING-01 — Dictionary dot writes bypass the value contract

Classification: confirmed defect. Original evidence: [typing/REPORT.md](typing/REPORT.md).

Implementation: ac3875b.

Added focused regressions/helpers: `dictionary_dot_and_bracket_writes_share_contracts`.

Test artifacts: `crates/tua-core/src/check/tests.rs`.

### TYPING-02 — Existing-local reassignment skips missing RHS slots

Classification: confirmed defect. Original evidence: [typing/REPORT.md](typing/REPORT.md).

Implementation: 77c9d5f.

Added focused regressions/helpers: `reassignment_checks_nil_padding_and_expanded_returns`.

Test artifacts: `crates/tua-core/src/check/tests.rs`.

### TYPING-03 — Short callbacks reject ignored trailing arguments

Classification: confirmed defect. Original evidence: [typing/REPORT.md](typing/REPORT.md).

Implementation: f487658.

Added focused regressions/helpers: `shortened_callbacks_can_ignore_trailing_arguments`.

Test artifacts: `crates/tua-core/src/types/tests.rs`.

### TYPING-04 — Optional record fields reject nil deletion

Classification: confirmed defect. Original evidence: [typing/REPORT.md](typing/REPORT.md).

Implementation: df09e71.

Added focused regressions/helpers: `optional_record_fields_accept_nil_but_reject_wrong_values`.

Test artifacts: `crates/tua-core/src/check/tests.rs`.

### FUNCTIONS-01 — Return expressions skip call validation without matching result slots

Classification: confirmed defect. Original evidence: [functions/REPORT.md](functions/REPORT.md).

Implementation: dbbb2ca.

Added focused regressions/helpers: `return_expressions_validate_calls_without_result_contracts`.

Test artifacts: `crates/tua-core/src/check/tests.rs`.

### FUNCTIONS-02 — Default expressions bypass declared parameter types

Classification: confirmed defect. Original evidence: [functions/REPORT.md](functions/REPORT.md).

Implementation: 7ec48dd.

Added focused regressions/helpers: `check_parameter_defaults`, `defaults_are_checked_in_parameter_scope`.

Test artifacts: `crates/tua-core/src/check/tests.rs`.

### FUNCTIONS-03 — Explicit nil rejected for defaulted parameters

Classification: confirmed defect. Original evidence: [functions/REPORT.md](functions/REPORT.md).

Implementation: d1b02a1.

Added focused regressions/helpers: `call_argument_contract`, `nil_arguments_trigger_defaults_without_weakening_body_types`.

Test artifacts: `crates/tua-core/src/check/tests.rs`.

### MODULES-01 — Inline require field extraction loses types

Classification: confirmed defect. Original evidence: [modules/REPORT.md](modules/REPORT.md).

Implementation: 6450b50,8f7fdb4,0bffb26.

Added focused regressions/helpers: `inline_require_field_extraction_preserves_exported_types`, `inline_require_inference_respects_shadowed_loader`, `drop`, `final_review_transitive_module_cache_tracks_dependency_edits`.

Test artifacts: `crates/tua-core/src/analysis/tests.rs`, `crates/tua-core/src/module_index/tests.rs`.

### MODULES-02 — Imported ECS component loses payload schema

Classification: confirmed defect. Original evidence: [modules/REPORT.md](modules/REPORT.md).

Implementation: 5a82e0a.

Added focused regressions/helpers: `imported_ecs_component_retains_its_payload_schema`.

Test artifacts: `crates/tua-core/src/analysis/tests.rs`.

### MODULES-03 — Declaration overlay disagrees across hover, signature and result inference

Classification: confirmed defect. Original evidence: [modules/REPORT.md](modules/REPORT.md).

Implementation: 8210170.

Added focused regressions/helpers: `declaration_overlay_controls_callable_and_result_types_with_runtime_present`, `project_module_binding_visible`.

### LUA-INTEROP-01 — Documented metatable colon call rejected

Classification: confirmed defect. Original evidence: [lua_interop/REPORT.md](lua_interop/REPORT.md).

Implementation: 59efb09.

Test artifacts: `crates/tua-core/src/module_index/tests.rs`.

### LUA-INTEROP-02 — Lua exports discard overload call contracts

Classification: confirmed defect. Original evidence: [lua_interop/REPORT.md](lua_interop/REPORT.md).

Implementation: 6f1100a,66cd38b,cad544d.

Added focused regressions/helpers: `check_typed_argument_list`, `imported_lua_overloads_preserve_all_call_contracts`, `contextual_callback_path`.

Test artifacts: `crates/tua-core/src/module_index/tests.rs`.

### LOVE-01 — Known void LOVE call chains pass checking and fail at runtime

Classification: confirmed defect. Original evidence: [love/REPORT.md](love/REPORT.md).

Implementation: 1939ad7.

Added focused regressions/helpers: `void_love_method_results_cannot_receive_chained_calls`.

Test artifacts: `crates/tua-core/src/check/tests.rs`.

### LOVE-02 — Unannotated LOVE callback parameters remain any

Classification: feature gap. Original evidence: [love/REPORT.md](love/REPORT.md).

Implementation: 929810c.

Added focused regressions/helpers: `love_callback_parameters_use_catalog_without_overriding_annotations`.

### ASSETS-01 — Pipeline output-root change overwrites unowned source files

Classification: confirmed defect. Original evidence: [assets/REPORT.md](assets/REPORT.md).

Implementation: d97050c.

Added focused regressions/helpers: `changed_output_root_does_not_transfer_ownership`.

### ASSETS-02 — Corrupted cached output bytes are materialized without rebuild

Classification: confirmed defect. Original evidence: [assets/REPORT.md](assets/REPORT.md).

Implementation: e72380e.

Added focused regressions/helpers: `cache_fingerprints`, `write_cache_fingerprints`, `cached_output_fingerprints_detect_corruption`.

### SHADERS-01 — Macro-defined uniform names trigger false missing-uniform errors

Classification: confirmed defect. Original evidence: [shaders/REPORT.md](shaders/REPORT.md).

Implementation: 8007938.

Added focused regressions/helpers: `macro_uniform_names_and_types_defer_metadata`.

Test artifacts: `crates/tua-core/src/asset/tests.rs`.

### FMT-01 — Formatting removes semantic whitespace on multiline long-string opening lines

Classification: confirmed defect. Original evidence: [formatter/REPORT.md](formatter/REPORT.md).

Implementation: 933c951.

Added focused regressions/helpers: `preserves_literal_whitespace_on_multiline_opening_lines`.

Test artifacts: `crates/tua-core/src/formatter/tests.rs`.

### LINTER-01 — Safe unused-local fix deletes neighboring same-line statements

Classification: confirmed defect. Original evidence: [linter/REPORT.md](linter/REPORT.md).

Implementation: 332f928.

Added focused regressions/helpers: `remove_statement_span_edit`, `unused_local_removal_preserves_adjacent_statements`.

Test artifacts: `crates/tua-core/src/code_action/tests.rs`.

### LINTER-02 — Explicit import cleanup changes module initialization effects and order

Classification: design question. Original evidence: [linter/REPORT.md](linter/REPORT.md).

Implementation: bf2e845.

No newly named Rust regression was extracted from these commits. Use the changed runtime test artifacts where listed and the post-fix evidence in IMPLEMENTATION-REVIEW.md; the original feature report supplies reproduction history, not post-fix verification. Documentation-only changes require the docs gate.

### TESTING-01 — Structured reports hide backend startup diagnostics

Classification: confirmed defect. Original evidence: [testing/REPORT.md](testing/REPORT.md).

Implementation: 3e858f1.

Added focused regressions/helpers: `test_report_preserves_backend_startup_error`.

Test artifacts: `crates/tua-cli/src/testing/report.rs`, `crates/tua-cli/tests/cli.rs`.

### PROJECTS-01 — Special-case cleanup deletes files without build ownership

Classification: confirmed defect. Original evidence: [projects/REPORT.md](projects/REPORT.md).

Implementation: db90df8.

Added focused regressions/helpers: `special_cleanup_requires_unchanged_recorded_ownership`, `in_place_declarations_preserve_handwritten_lua`.

Test artifacts: `crates/tua-cli/tests/cli.rs`.

### WATCH-01 — dev retains stale output root after configuration reload

Classification: confirmed defect. Original evidence: [watch_reload/REPORT.md](watch_reload/REPORT.md).

Implementation: c68c608.

Added focused regressions/helpers: `exit_with_game`, `dev_output_root_change_prepares_new_location`.

Test artifacts: `crates/tua-cli/tests/cli.rs`.

### WATCH-02 — New conf.lua is never tracked for restart after startup

Classification: confirmed defect. Original evidence: [watch_reload/REPORT.md](watch_reload/REPORT.md).

Implementation: 373bea1.

Test artifacts: `vendor/tests/hotreload_test.lua`.

### DBG-01 — Spaces in generated paths disable traceback remapping

Classification: confirmed defect. Original evidence: [debugging/REPORT.md](debugging/REPORT.md).

Implementation: a767cf4.

Added focused regressions/helpers: `remaps_unquoted_paths_with_spaces_and_preserves_prefix`.

Test artifacts: `crates/tua-cli/src/traceback/tests.rs`, `editors/vscode/test/unit/source-maps.test.js`.

### EDITOR-01 — Colon string completion enumerates receiver internals

Classification: confirmed defect. Original evidence: [editor/REPORT.md](editor/REPORT.md).

Implementation: d5e3b93,ff8c5da,67e8e58.

Added focused regressions/helpers: `method_separator_before`, `complete_type_enum_values`, `collect`, `colon_string_arguments_skip_self_and_do_not_enumerate_record_fields`, `audio_string_arguments_use_specialized_domain_contracts`.

Test artifacts: `crates/tua-core/src/completion/tests.rs`.

### EDITOR-02 — Shader hover nests GLSL Markdown in a Tua fence

Classification: confirmed defect. Original evidence: [editor/REPORT.md](editor/REPORT.md).

Implementation: 7f09d85.

Test artifacts: `crates/tua-lsp/src/server/tests/shaders.rs`.

### EDITOR-G01 — Same-arity overload selection ignores literal argument

Classification: feature gap. Original evidence: [editor/REPORT.md](editor/REPORT.md).

Implementation: c39d757,5acbb1e.

Added focused regressions/helpers: `active_signature_for_literal_argument`, `literal_first_argument_selects_same_arity_overload`, `input_pair_literal_selects_specific_overload_over_union`.

Test artifacts: `crates/tua-core/src/signature_help/tests.rs`.

### EXPLORERS-01 — Sidebar graph cache does not follow unsaved or filesystem changes

Classification: feature gap. Original evidence: [explorers/REPORT.md](explorers/REPORT.md).

Implementation: 2516078,52b4572.

Test artifacts: `editors/vscode/test/unit/workspace-sidebar.test.js`.

### EXPLORERS-02 — Project files omits Lua sources that Modules includes

Classification: feature gap. Original evidence: [explorers/REPORT.md](explorers/REPORT.md).

Implementation: 0015e47.

Test artifacts: `editors/vscode/test/unit/workspace-sidebar.test.js`.

### API-MCP-01 — Refresh reuses analysis revisions across changed snapshots

Classification: confirmed defect. Original evidence: [api_mcp/REPORT.md](api_mcp/REPORT.md).

Implementation: 322ad16.

Added focused regressions/helpers: `refresh_analysis`, `refresh_preserves_monotonic_analysis_revisions`.

### API-MCP-02 — Project diagnostics omit new unsaved overlay files

Classification: confirmed defect. Original evidence: [api_mcp/REPORT.md](api_mcp/REPORT.md).

Implementation: 671e394.

Added focused regressions/helpers: `diagnostics_include_new_unsaved_overlay_files`.

### API-MCP-03 — JSON-RPC envelope and notification handling is inconsistent

Classification: confirmed defect. Original evidence: [api_mcp/REPORT.md](api_mcp/REPORT.md).

Implementation: eb5837b.

Added focused regressions/helpers: `dispatch_envelope`, `handle_api_request`, `handle_mcp_request`, `protocol_envelopes_and_notifications_are_consistent`.

### PACKAGING-01 — Fused macOS app retains LOVE document-handler claim

Classification: confirmed defect. Original evidence: [packaging/REPORT.md](packaging/REPORT.md).

Implementation: 34fed98.

No newly named Rust regression was extracted from these commits. Use the changed runtime test artifacts where listed and the post-fix evidence in IMPLEMENTATION-REVIEW.md; the original feature report supplies reproduction history, not post-fix verification. Documentation-only changes require the docs gate.

### SETUP-DOCS-01 — Setup README omits the shipped shader step

Classification: confirmed defect. Original evidence: [setup_docs/REPORT.md](setup_docs/REPORT.md).

Implementation: fc7c5c8.

No newly named Rust regression was extracted from these commits. Use the changed runtime test artifacts where listed and the post-fix evidence in IMPLEMENTATION-REVIEW.md; the original feature report supplies reproduction history, not post-fix verification. Documentation-only changes require the docs gate.

## Integration findings and qualification

- Independent review: [FINAL-REVIEW.md](FINAL-REVIEW.md). Transitive filesystem export-cache staleness was reproduced and repaired in `0bffb26`, together with recursive resolution bounds; final full-suite evidence is recorded in FIX-VERIFICATION.md.
- Historical staging repair `9118565` removes a duplicated contextual callback helper accidentally included in `616ab74`; this matters when bisecting intermediate commits. Missing Lua overload helper staging was repaired in `66cd38b`. Current-tree and final-gate evidence, not every intermediate commit, determines readiness.
- Loader shadowing repair: `8f7fdb4`. Schema provenance after domain specialization: `67e8e58`. These address interactions missed by isolated feature tests.
- EDITOR-G01 follow-up `5acbb1e`: exact literal overloads beat matching broad unions. All 39 signature-help tests passed, including Input pair/scalar calls. Initial `c39d757` alone did not satisfy the protocol reproduction; fresh actual LSP replay after rebuilding passed: activeSignature 2 selects `function c:get(name: "move"): number, number`; see post-fix-review/responses.json.
- The original inline smoke mismatch was independently reproduced on the pre-fix evaluator, then resolved with per-position menu controls in `4611f73e`; see INLINE-EVAL-RECONCILIATION.md.
- An interim core run had five checker regressions; subsequent integration fixes below resolved them. The final run passed 1722 core tests with one ignored. This is scoped automated evidence, not universal platform or release qualification.
- Actual protocol and CLI evidence: [IMPLEMENTATION-REVIEW.md](IMPLEMENTATION-REVIEW.md), [post-fix-review](post-fix-review/). Platform, GUI/hardware, long-session, and external-service limits from the original audit still apply.

Additional integrated language follow-ups: `8f58fb0` retains constructed-vector
nominal identity; `984a0b5` contextualizes nested optional-union callbacks;
`728da7c` deduplicates nested callback mismatch diagnostics. Their final gate
results belong in FIX-VERIFICATION.md.

### Subsequent integrated-suite repairs

- `8f58fb0`: constructed vectors retain nominal identity.
- `984a0b5`: callback context survives nested optional unions.
- `728da7c`: nested callback contract mismatches are reported once.
- `dcfccb6`: LOVE initial configuration read defaults remain separate from nullable writes.
- `892a9f6`: optional nested members narrow during short-circuit reads.
- `ba21b146`, `7bc070be`: LSP expectations acknowledge nullable LOVE configuration fields.
- `242a4f54`: recovered empty branch bodies retain their guard narrowing during completion; covers if/elseif/while and a negative else control.

The final consolidated outcomes supersede interim failing runs above; see
[FIX-VERIFICATION.md](FIX-VERIFICATION.md).

- `876101c2`: signature presentation preserves imported alias labels and parameter documentation from cached authoritative snapshots, guarded by semantic-contract equality. The declaration-overlay regression and all ten LSP signature-help tests passed.

- `e395c1bb`: migrate the LOVE full example to required initialized state and guarded delayed resources; all seven bundled example builds passed.
- `cf07d78a`: document optional record reads and module initialization.
- `0da28f88`: core recovery hints retain both nullable configuration types.

- `4611f73e`: reconcile the pre-existing inline evaluator snapshot using historical/current per-position controls; the freshly built current-tree smoke passes. See [INLINE-EVAL-RECONCILIATION.md](INLINE-EVAL-RECONCILIATION.md). This supersedes the earlier pending qualification note.

- `e3f32c3f`: Clippy match-guard cleanup; the final full gate and VSIX were rebuilt afterward.

- `57d455be`: refresh pinned editor replay hashes and cursor positions after the example migration, preserving the original contexts and tuning-only exclusions; all 172 Node tests pass.

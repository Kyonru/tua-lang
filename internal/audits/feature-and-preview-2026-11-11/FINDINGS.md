# Confirmed findings and dispositions

Baseline: `c08edbca` on 2026-11-11. “Fixed” below means the cited regression and focused checks passed locally; final workspace and packaged-artifact gates are recorded in VERIFICATION.md. Detailed reproductions and source evidence are in the linked lane reports. A test gap is not counted as a product defect.

| ID | Finding | Category / severity | Verification | Disposition |
| --- | --- | --- | --- | --- |
| F-01 | Cross-module class ID collision makes unrelated `is` true | Language/runtime · High | Emitted Lua run, failing-before/after CLI regression | Fixed in `ffea56ef`; see LANGUAGE.md LANG-02. |
| F-02 | Imported protocol bound rejected in a generic class method | Type checking · Medium | Failing-before/after analysis regression, CLI fixture | Fixed in `2015f0af`; see LANGUAGE.md LANG-01. |
| F-03 | Nested or imported SignalBus loses `emit` varargs | Type checking · Medium | Failing-before/after core regression | Fixed in `efb9ddbb`; see DOMAINS.md DOM-01. |
| F-04 | Unchanged owned bundled helper remains stale after build | Build/runtime · Medium | Runtime reproduction and CLI ownership regressions | Fixed in `4f649212`; see DOMAINS.md DOM-02. |
| F-05 | Imported ECS component loses field schema | Type checking/editor · Medium | Nested imported-world and local-world/imported-component cases reproduced and rechecked in core, CLI, and real LSP | Fixed in `14d9a1a2` and `82b8674f`; see DOMAINS.md DOM-03 and PRIOR_LANGUAGE.md MODULES-02. |
| F-06 | Absent ECS `get`/`remove` result permits unsafe field access | Type checking/runtime · Medium | Old CLI fixtures reproduced nil crash; new guarded/unguarded regressions | Fixed in `68c13600`; see DOMAINS.md DOM-04 / prior ECS-01. |
| F-07 | Graph filters and modes reset on refresh | Editor preview · Medium | Real VS Code host failure before fix; host regression after | Fixed in `97969b7a`; see EDITOR.md ED-01. |
| F-08 | Long Signal Graph metadata hides event name | Editor preview · Medium | Populated host capture and geometry assertion before/after | Fixed in `f97e7417`; see EDITOR.md ED-02. |
| F-09 | Feature-example check description promises compilation it does not do | Docs/tooling wording · Low | Synthetic invalid-source mapping check still exits 0 | Wording fixed in `94cf495b`; see DOCS_DEEP.md DOC-01. |
| F-10 | Signal Graph filter carries into a different workspace | Editor preview · Medium | Independent installed-host reproduction and after-fix regression | Fixed in `69e03d1a`; see FINAL_REVIEW.md REV-01. |

## Reproduction and fix notes

### F-01: runtime identity across modules

Build two source files whose first classes are unrelated, then evaluate `foreign is Base`. Before the fix, each file emitted `__type_id = 1` and the check looked up that numeric key in `__is`, so generated Lua took the wrong branch despite a successful `tua check` and `build`. Run `RUST_MIN_STACK=16777216 cargo test -p tua-cli --test cli runtime_class_checks_use_identity_across_modules`. The emitted check now uses the actual class table as the key; the test also verifies subclass-to-base positives. Source: `crates/tua-core/src/classes.rs` and `crates/tua-core/src/emit/mod.rs`. Previous same-file tests could not expose per-file ID collisions.

### F-02: imported generic bound in class methods

In `box.tua`, import protocol `Measurable` and declare `final class Box<T: Measurable>` with `init` and a second method returning `T`. Before the fix, `tua check` emitted false TL1003 at the class binder; the same-file protocol and init-only controls passed. Run `RUST_MIN_STACK=16777216 cargo test -p tua-core imported_protocol_bound_accepts_generic_class_methods`. The type context now retains the canonical imported bound when a parser-level duplicate method signature lacks identity. Source: `crates/tua-core/src/types/assignability.rs`. The new negative case still rejects a nonconformer. Earlier CLI tests covered bounded free functions, not this class/method combination.

### F-03: SignalBus vararg through a field

Store a bus in `Shared.events` and call `Shared.events:emit("move", entity, 5)` for a two-payload handler. Before the fix, TL1004 reported `expected 2, found 4`. Run `RUST_MIN_STACK=16777216 cargo test -p tua-core accepts_signal_emit_payloads_through_nested_bus_fields`. `method_function_type` now preserves the method's vararg type and names when constructing the structural function type. Source: `crates/tua-core/src/signal.rs`. The checker regression covers local/imported buses and bad payloads; same-file direct-bus tests missed the field path.

### F-04: owned helper refresh

Build a project requiring a bundled helper, then build it again after the compiler's helper content changes while leaving the previously generated output intact. Before the fix, `write_bundled_runtime` returned merely because the path existed, so an old `tuasignal.lua` could lack `declare` and fail at runtime. Run the new `build_refreshes_unchanged_owned_bundled_runtime` and `build_preserves_manually_changed_bundled_runtime_output` CLI tests. The build now refreshes only an unchanged output previously owned by its manifest; project and manually modified overrides remain. Source: `crates/tua-cli/src/build/mod.rs`, `build/ownership.rs`, and `build/watch.rs`. Old outputs with no ownership record are intentionally preserved; `docs/cli.md` explains their migration.

### F-05 and F-06: ECS type precision

F-05: require a module exporting `world` and `Position`, then assign `position.x = "wrong"` after `Shared.world:get(entity, Shared.Position)`. Same-file code produced TL1003, but the imported form passed before `14d9a1a2`. Project-local inference now consults the shared ECS contextual return helper for nested imported world calls; analysis hover and CLI regressions cover that form. The historical local-world/imported-`C.Position` case also hovered `any | nil` in an absolute-path LSP project. `82b8674f` resolves its local world receiver through the file's semantic environment and sends only imprecise locals with import-dependent initializers to the project hover path. An absolute-path core test, real LSP test, unrelated-`any` control, and rebuilt stdio probe now show `{ x: number } | nil`. F-06: create an entity without Position, call `world:get` or `remove`, and read `.x`; check/build passed while emitted Lua crashed indexing nil. The domain already modeled `T | nil`, but the checker used the generic catalog return `any`; `68c13600` applies the shared contextual result after argument checking. Guarded access passes and unguarded access yields TL1011. See DOMAINS.md, `crates/tua-cli/tests/feature_interactions.rs`, and historical ECS-01/MODULES-02 in PRIOR.md. More complex editor completion and signature surfaces were not independently rechecked for these exact fixtures.

### F-07, F-08, and F-10: preview information during editing

F-07: filter an ECS, pool, module, or Signal view and refresh analysis. Before `97969b7a`, the filter or mode reset; the installed-host regression now confirms context retention and source-focus reset where appropriate. F-08: render `enemy.hit` with long bus metadata; before `f97e7417`, its event name had zero visible width. The updated two-line node preserves the name and full title. F-10 was found during independent review: Signal Graph state persisted across a workspace switch, hiding a new project's `player.hit` behind the old `enemy` filter. `69e03d1a` scopes saved Signal state to the workspace-folder identity. Run `npm --prefix editors/vscode run test:webviews`; see EDITOR.md, PREVIEWS.md, and FINAL_REVIEW.md for captures, source paths, and remaining large-graph limits.

### F-09: check description

`make feature-examples-check` only validates guide mappings, first-line source descriptions, and imports. The script still exits 0 when mapped files contain invalid Tua; see `repro-feature-check-oracle.py`. Trusted CI separately builds the feature tour, so this was a misleading standalone target description, not an unguarded release syntax path. `94cf495b` corrects `Makefile` and `docs/examples.md`; `make feature-examples-check docs-build` passed.

## Coverage gaps and design questions

- **TEST-GAP-01:** The combined behavior-tree/ECS/pool example was built but not executed in trusted CI. `f302e006` adds a focused emitted-Lua CLI integration test; see DOCS_DEEP.md. Linux CI pins LuaJIT for workspace tests.
- **TEST-GAP-02:** Real graph protocol data and rendered views previously had separate tests. `c9232605` adds an installed real-server host test for Signal/ECS/pool rendered facts and source actions. It does not compare every raw response field or cover large graphs.
- Preview/live overlays, protocol-like language expansion, and broad performance redesign remain design or qualification questions, not confirmed defects from this audit. See BACKLOG.md for bounded acceptance criteria.

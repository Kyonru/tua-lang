# Tua Roadmap

This repository-level file tracks pending product and engineering priorities.
It is an internal planning aid, not documentation of current behavior or a
release commitment.

Current user-facing behavior belongs in the focused language, tooling, and
workflow pages under `docs/`. Completed work belongs in
`CHANGELOG.md` or Git history.

## Now

### Ship And Qualify The 0.4 Release Line

- Ship the filesystem-isolation, malformed-input, and analysis fixes in 0.4.0
  with migration notes and coordinated security guidance.
- Keep CLI, server, extension, changelog, and migration notes synchronized.
- Collect independent real-project evidence for deterministic inline
  completion before enabling it by default; keep exact project retrieval
  parked and the sparse statistical fallback explicitly opt-in until then.
- Fix correctness or responsiveness regressions before expanding language
  syntax.

### Resolve Correctness Defects Found In Full Ports

- Preserve cross-file global module identity before injecting built-in modules:
  BYTEPATH loads HUMP Camera globally, but another file's `Camera.smooth` can
  inject Tua Camera and change runtime behavior. An explicit local `_G.Camera`
  alias is the current workaround; use shared project binding provenance for the fix.

Owner: shared binding, project/module and checker semantics. Acceptance: retain
the SNKRX and BYTEPATH minimal reproductions, test editor/checker/emitter agreement,
and replay the full ports without their temporary interop workarounds.

### Deepen Everyday LOVE Workflows

- Harden asset path navigation and diagnostics across realistic project roots.
- Refine sprite-sheet preview and SignalBus graph workflows from user feedback.
- Benchmark the object-pool runtime on representative LOVE populations.
- Maintain one generated LOVE source for signatures, enums, callbacks,
  inheritance, and documentation.

### Qualify Real-Project Responsiveness And Editor Behavior

- Keep full SNKRX/BYTEPATH check and build timings as regression workloads;
  bound receiver inference and source rescanning, then profile remaining cost.
- Trace cold typing/completion in the zombie project, including the reported
  delay beside `Dialogue.init`; compare cold and warm request latency.
- Isolate slower require analysis outside `main`, hot-reload latency, and
  format-on-save latency. Record project size, editor action, timings, and
  invalidation work before choosing optimizations.
- Test the newly packaged VSIX in an installed editor and a graphical LOVE game.
  Headless tests and synthetic trials do not establish interactive usability,
  reload behavior, or large-project performance.
- Turn any still-unspecified reference/implementation request into an exact
  symbol, editor command, expected destination, and reproducible test before
  treating it as a confirmed defect.

Owner: LSP/project analysis and extension/runtime integration. Acceptance:
repeatable traces, before/after measurements for fixes, and recorded live-editor
results. Evidence: [development trials](internal/devex-trials-2026-11.md).

## Next

### Improve Semantic Precision Without Losing Lua Flexibility

- Broaden control-flow and metatable inference only where identities are
  compiler-proven.
- Expand LuaDoc/LuaCATS interoperability for common existing Lua libraries.
- Improve safe project-wide refactors and module changes.
- Preserve `any` and ordinary Lua as explicit dynamic escape paths.

### Finish Semantic Editor Consistency

- **Indexed expression receivers:** resolve record-array entries such as
  `State.allBullets[b].` and `value_to_emoji_data[a.value].`, including incomplete
  edits without unbounded analysis.
- **Side-effect module globals:** define a sound navigation model for functions
  introduced by `require` without an explicit returned module contract.
- **Fluent expression receivers:** support completion, hover, signatures, and
  navigation after calls such as `title:setValue("x"):draw(` and LOVE constructor
  chains through one shared receiver context. Check completed and incomplete
  expressions; void calls must not invent a receiver.
- **Imported `Self`:** retain nominal identity through imported instance aliases
  and recursive project-type expansion so fluent method results do not become
  `any`. Preserve recursive LuaCATS safeguards.
- **Cross-file event call results:** propagate the inferred boolean from
  `local dead = target:damage(amount)` into handlers in other modules. Bound
  dependency traversal and cycles; test unsaved producer edits.
- **Remaining class contexts:** restore captured narrowed-local completion in
  delayed callbacks and isolate full-method nested imported-field resolution.
  The porting audit retains failing cases alongside working minimal controls.
- **Native class parameter navigation:** preserve header identities and source
  provenance for definition, references, and rename, including shadowing and
  exact original-source edits.
- **Empty-argument overload selection:** isolate why `Text:getWidth(` selects
  the index overload, then distinguish an empty slot from a supplied argument
  without hiding other valid overloads.
- **Unclosed classes:** extend recovery and syntax guidance to wholly unclosed
  class bodies while preserving surrounding scopes and valid nested Lua blocks.

Owner: shared core semantics/parser, surfaced through LSP. Acceptance: extend
cross-surface conformance tests with exact cursor positions, cold snapshots,
and unsaved edits. The [trial report](internal/devex-trials-2026-11.md) owns the
reproductions and current workarounds.

### Complete Typed Lua Porting Workflows

- Complete optional/default parameter support for anonymous callbacks using
  parser-backed metadata; improve diagnostics for currently unsupported forms.
- Add assertion-based narrowing with correct control-flow semantics.
- Align LOVE File/FileData constructor overloads with the generated catalog;
  preserve invalid-argument diagnostics and actual LOVE runtime coverage.
- Extend helper binding awareness to asset indexing and editor metadata, with
  consistent local/parameter/global shadowing tests.
- Model initial LOVE configuration state and subsequent nullable window writes;
  evaluate literal `false` support without accepting arbitrary booleans.
- Derive `table.insert` append/indexed value contracts from typed arrays;
  preserve overridden library functions and dynamic `any` behavior.
- Infer `table.sort` comparator parameter types from its input array, with
  matching member completion and argument checking.
- Design stable nullable member-path narrowing with mutation/call invalidation;
  capturing and guarding a local remains the current workaround.
- Make init-before-field-default behavior easier to discover, and consider a
  targeted diagnostic when a default overwrites an init assignment. Changing
  ordering requires a compatibility decision, not a silent fix.
- Document migration from singleton tables and mixins to explicit services,
  native classes, and typed adapters. Evaluate reusable declarations for existing
  timer libraries before adding a built-in scheduler domain.

Owner: shared semantics/checker and documentation. Acceptance: focused core/LSP
regressions and replay of the [five upstream porting trials](internal/upstream-port-audit-2026-11.md),
including ordinary functions, class bodies, unsaved edits, custom library
mutations, and runtime equivalence. Preserve full-game regression harnesses;
installed-editor, long-session, external-service and visual parity validation
remain distinct follow-up work.

Migration tooling should also explain supported `emit_maps = false` for legacy
loaders that require every directory entry, documented `.tua` restrictions on
`goto`, and LOVE version compatibility without rewriting vendor dependencies.

### Improve Comment And Annotation Authoring

- **Discover supported tags:** derive completion and explanations from shared
  tag metadata, including missing `deprecated`, `nodiscard`, and declaration
  tags. Show syntax, examples, and whether each tag affects checking or only
  records metadata.
- **Generate/update documentation from parsed signatures:** support native
  class methods, multiline functions, nested table/function types, defaults,
  and varargs. Preserve prose and native types; do not invent descriptions.
- **Keep comments aligned:** offer targeted fixes for stale/missing parameter
  tags and update matching documentation references during parameter rename.
  Reuse existing type-disagreement diagnostics.
- **Navigate annotations:** share native type completion/navigation for type
  names in comments. Explain tags on hover with a local guide link, including
  the limited semantics of `@generic`, `@cast`, and `@as`.
- **Verify editing states:** cover Lua/native functions, class methods, imports,
  partial tags, and inserted generated snippets in one conformance matrix.
  Check the resulting program and explicit source-coordinate mappings.

Owner: core annotation metadata/semantics and LSP; the extension presents those
results rather than maintaining a second parser. Acceptance and rationale:
[documentation review](internal/documentation-review-2026-11.md).

### Make Releases Easier To Trust

- Add code signing when sustainable signing identities and key management are
  available.
- Test upgrade and rollback behavior between adjacent extension/server
  versions.
- Publish compatibility and migration notes for every minor release.
- Add more architectures only when CI can build and test them natively.

## Later

### Revisit Storage And Extensibility With Evidence

- Compact generated LOVE data only if measured compile time, binary size, or
  maintenance cost justifies replacing generated Rust source.
- Evaluate protocol-like behavior types against the concrete shared update
  scheduler for unrelated gameplay actors and UI objects. Specify receiver
  binding, method compatibility, diagnostics, and navigation before replacing
  the explicit adapters in `examples/features/src/polymorphism.tua`.
- Reconsider generic classes when identity-preserving APIs cannot be modeled
  clearly with concrete types. Abstract classes/methods and multiple inheritance
  remain deferred design questions, not promised features; require use cases
  beyond those already served by single inheritance and typed adapters.
- Explore typed physics/contact ownership and checked optional configuration
  schemas only against larger ports; investigate numeric units/nominal numeric
  aliases if repeated angle, force, or dimension mistakes justify them. These
  are evidence-gathering proposals from the porting audit, not commitments.
- Keep executable third-party plugins out until ownership, ordering, security,
  and performance contracts can be made stable.

## Planning Rules

- Keep this file limited to pending outcomes, not implemented feature
  inventories.
- Do not use roadmap placement as evidence that a feature is available.
- Move implemented behavior into the relevant docs and remove the completed
  roadmap item.
- Add priorities only when ownership, user value, and a verification path are
  clear.

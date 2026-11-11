# Tua feature and preview audit plan

Plan date: 2026-11-11 (Asia/Tokyo)
Planning baseline: c08edbca; worktree clean when inventoried.
Status: executed with bounded coverage. Evidence, ten locally fixed findings, remaining qualification, and the 391-row coverage ledger are recorded in `internal/audits/feature-and-preview-2026-11-11/`. Twenty-one of 74 historical finding IDs were not individually rechecked; this plan does not establish universal feature or release qualification.

## Goal

Review every shipped Tua capability as a developer would use it: discover the feature, write code, get editor feedback, build, run the emitted Lua, inspect failures, and learn from the docs. Measure friction in that path even when the feature is technically correct. Review each visual preview as a tool for a specific game-development question. Record confirmed defects, documentation corrections, missing capabilities, usability friction, performance risks, and design choices separately.

This is a new audit of the current code, not a claim that the November 7 feature reports or the 0.7.0 preview report still describe current behavior. Reproduce and close out each earlier finding before carrying it into a new backlog.

## Inventory and coverage rule

The coverage ledger must be generated from all of these sources before testing:

1. Documentation navigation in zensical.toml, feature guides in docs, and README claims.
2. The feature-to-example map in examples/features/coverage.json plus all examples/features source files and standalone example projects.
3. Compiler, domain, CLI, and runtime modules; CLI help and configuration schema.
4. LSP capabilities and custom requests, extension package.json commands/settings/views, and all editor webviews.
5. CI and release workflows, existing test and benchmark targets, and earlier audit findings.

Every discovered item gets its own ledger row. The IDs below are the starting inventory, not a closed list. Split grouped rows into subrows for each syntax form, helper method, CLI subcommand, editor command/view/setting, diagnostic family, and preview interaction before marking them complete. New commands, helpers, preview surfaces, or documented behaviors found during the audit must be added before declaring coverage complete. A row may be marked tested, inspected only, blocked, or not applicable for each applicable layer; blank cells are not completion.

### Language and interoperability inventory

| ID | Feature or contract | Primary documentation / example |
| --- | --- | --- |
| L01 | Lua 5.1-compatible syntax, control flow, operators, comments, and parser recovery | language; lua-basics; lua_cheatsheet |
| L02 | Primitive types, any, inference, assignment, reassignment, and deferred initialization | typing; types; inference |
| L03 | Aliases, recursive aliases, Self, and declaration-only types | typing; project_types |
| L04 | Table shapes, nested and optional fields, mutation, and indexes | typing; types; inference |
| L05 | Arrays, dictionaries, tuples, varargs, and multiple returns | typing; functions; inference |
| L06 | Unions, nilability, literal choices, and erased enums | typing; types |
| L07 | Control-flow narrowing and runtime is checks | typing; classes; lua_cheatsheet |
| L08 | Functions, defaults, optional parameters, callbacks, closures, and recursion | language; functions; scopes |
| L09 | Function types, overloads, colon/dot calls, and implicit self | typing; functions; classes |
| L10 | Generic aliases, functions, bounds, constructors, and cross-file use | typing; generics |
| L11 | Protocol declarations, conformance, method calls, and polymorphism | typing; protocols; polymorphism |
| L12 | Classes, inheritance, overrides, final methods, super, and runtime dispatch | classes; polymorphism |
| L13 | Metatables, metamethods, iterators, and conservative Lua object typing | language; lua_cheatsheet |
| L14 | Compound assignment and evaluation-order preservation | language; compound |
| L15 | Require, static import, re-export, module roots, cycles, and mixed Lua/Tua projects | modules; imports; require_graph |
| L16 | LuaDoc/LuaLS/LuaHelper annotations, documentation comments, and LuaCATS output | comments; annotations; luals_annotations; luacats_emit |
| L17 | LOVE API catalog, callbacks, constructors, methods, overloads, and color values | language; assets; conf |
| L18 | Source mapping, readable emitted Lua, helper inclusion, and LuaJIT/LOVE runtime behavior | language; emitted-Lua audit |
| L19 | Scope, shadowing, forward references, and unsaved-buffer consistency | scopes; navigation; lsp |

### Game-domain inventory

| ID | Feature or contract | Primary guide / example |
| --- | --- | --- |
| G01 | Asset helpers, path validation, metadata, and asset pipelines | language; asset-pipelines; assets |
| G02 | Sprite sheets, animation playback, markers, and callback semantics | sprites; sprites |
| G03 | Vec2 and opt-in math helpers | vectors; vectors; game_math |
| G04 | SignalBus declarations, handlers, event names, and payload inference | signals; signals |
| G05 | Stores, subscriptions, keyed watchers, and unsubscribe behavior | stores; store |
| G06 | State machines, routes, callbacks, and generated runtime | state-machines; state_machine |
| G07 | Typed input, source names, control pairs, and Baton runtime | input; input |
| G08 | Tweening, easing, delay, update, and cancellation | tweening; tweening |
| G09 | Camera transforms, following, drawing, and bounds | camera; camera |
| G10 | Procedural Rot helpers: generation, paths, visibility, dice, colors | roguelike; roguelike |
| G11 | Behavior trees, runner state, tick results, and opt-in inclusion | behavior-trees; behavior_tree; behavior_tree_ecs |
| G12 | ECS schemas, queries, systems, access inference, and runtime | ecs; ecs |
| G13 | Object pools, lifecycle, prewarming, safety, and emitted runtime | object-pooling; object_pool |
| G14 | Audio buses, cues, music scenes, metadata, and analysis cache | audio; audio-system |
| G15 | Shader values, uniforms, includes, validation, ShaderScan, and hot reload | shaders; shaders; assets |

### Toolchain and editor inventory

| ID | Feature or contract | Evidence source |
| --- | --- | --- |
| T01 | Project initialization, configuration defaults, roots, and workspace isolation | cli; configuration; project-model |
| T02 | Check, lint, diagnostics, suppression, and source locations | cli; linter; linter example |
| T03 | Formatter, range/whole-file edits, and format-on-save | formatter; extension |
| T04 | Build, output copying, helper inclusion, Lua syntax, and source maps | cli; examples; emitted-Lua audit |
| T05 | Busted testing, discovery, gutters, mapping, and coverage | testing; unit-testing |
| T06 | Watch, dev, hot reload, error recovery, and timing | hot-reload; love-minimal |
| T07 | Run, debug, traceback, game console, and multiplayer instances | debugging; love-tua; extension |
| T08 | Packaging for LOVE, Windows, and macOS | packaging; cli |
| T09 | LSP completion, hover, signature help, diagnostics, actions, navigation, rename, symbols, tokens, and inlays | lsp; editor tests |
| T10 | Unsaved edits, dependency invalidation, cancellation, long sessions, and project switching | lsp; architecture; benchmarks |
| T11 | API/MCP bridge, path/privacy boundaries, and versioned protocol | external-api-bridge; API tests |
| T12 | Editor setup, theme, toolbar, command discovery, offline references, and server health | extension manifest; editor reference |
| T13 | Asset, shader, audio, module, signal, ECS, pool, and runtime-inspector editor integrations | lsp; extension views |

## Per-feature review card

Each feature row receives one card with:

- Contract: exact documented promise, defaults, opt-in status, version and runtime prerequisites.
- Real task: a small example plus one use in a representative multi-file game; an independent project where practical.
- Correctness: positive case, invalid case, boundary case, and interactions with adjacent features. Check, build, and execute emitted Lua under LuaJIT or LOVE when runtime behavior matters; compare output and side effects, not just emitted text.
- Editor: completion, hover, signature help, navigation, diagnostics, quick fixes, unsaved edits, and reloads where applicable. Compare editor facts with compiler results.
- Generated output: ordering, evaluation count, helper inclusion only on reference, source maps, and readable/predictable Lua.
- Cost: cold and warm request latency, build/runtime overhead, and large-data behavior where the feature can scale.
- Developer experience: how the feature is found, what the first working use requires, whether completion and errors guide the next edit, how many context switches or manual type annotations are needed, and how a developer recovers from a mistake.
- Teaching: docs accuracy, discoverability, runnable example, prerequisite and failure guidance, and whether a user can predict generated code.
- Evidence: exact command, fixture, expected and actual result, file/line references, existing coverage, and verification status.

Use not applicable only with a reason. A docs page or passing unit test alone does not mark a runtime or installed-editor layer tested.

## Developer-experience journeys

Run these as agent-simulated development tasks in an installed editor and a terminal; label them as simulations, not human usability studies. Use the bundled examples for a stable baseline and at least one independent multi-file game for realistic pressure. Record the starting point, actions, searches, wrong turns, time to first useful feedback, time to a working result, and whether the editor or docs explained each correction. Reproduce a friction point in a small fixture before calling it a product finding.

| Journey | What a good experience should make possible |
| --- | --- |
| First project | Install, open or initialize a project, find the entry file, run LÖVE, and understand missing prerequisites without reading internal docs. |
| Existing Lua migration | Bring in a plain Lua module, add types gradually, navigate exports, and see where any enters without fighting the module model. |
| Typed gameplay API | Define a class, generic helper, and protocol in separate files; construct, call, override, and discover members with consistent hover, completion, and errors. |
| Game helper adoption | Add a store, signal, state machine, ECS system, pool, behavior tree, sprite, audio cue, or shader using suggestions and examples; understand opt-in inclusion and generated runtime cost. |
| Editing in motion | Type half an expression, fix malformed code, switch files, change an import, rename a symbol, and save; suggestions, diagnostics, and previews should remain fresh and responsive. |
| Diagnose a failure | Locate the argument, field, source path, or generated-Lua location responsible for an error; the message and related links should lead to a useful next edit. |
| Run and iterate | Hot reload a valid change, recover from a failed build, debug a mapped breakpoint, inspect live state, and rerun a Busted case from the gutter. |
| Learn a feature | Start from a command, hover, example, or docs search; find prerequisites, a working example, limitations, and the emitted-Lua model without conflicting claims. |

Score friction by user consequence rather than by how many clicks a script used: blocked task, misleading feedback, avoidable manual work, slow feedback, or minor discoverability. Distinguish a compiler defect from a rough but correct workflow. Compare related features so a similar action has a similar name, location, and keyboard path. Recommend the smallest shared improvement that removes a recurring obstacle; avoid adding one-off buttons to every surface.

## Cross-feature and release-context gates

Individual feature passes cannot establish that a game works when features interact. Select pairwise cases by shared lowering, type resolution, or runtime state rather than attempting every combination. At minimum, exercise generics × protocols × imported classes; nil narrowing × closures; annotations × mixed Lua/Tua imports; signals × stores/state machines; behavior trees × ECS × pools; sprites × assets/shaders; and hot reload × generated helpers/source maps. For each case, compare checker, editor, emitted Lua, and runtime results where applicable. Record the selection rationale and untested combinations in COVERAGE.

Build an interaction ledger from feature *boundaries*, not a Cartesian product of feature names. Mark each pair as sharing a type, module, lowering, generated helper, runtime state, editor cache, or no known boundary. Give every shared boundary at least one positive and one failure/recovery scenario. Use pairwise coverage across the remaining plausible combinations, then add targeted three-feature cases at high-risk junctions (for example, a generic store value passed through a signal handler in another module). Rotate one variable at a time within a scenario: same file versus imported, annotated versus inferred, valid versus invalid payload, fresh versus unsaved edit. This makes a failure attributable without multiplying every axis together.

Each interaction fixture must assert observable invariants, not only successful compilation. Example: `Store<T>` with a generic `T` keeps the same type through `get`, `set`, subscriptions, hover, and completion; a wrong value is rejected; emitted Lua runs and subscriptions receive the value once. For ECS × signals, an entity/component payload reaches the handler with its type and identity intact; handler removal and entity reuse do not deliver stale events. Compare a minimal unit fixture with one representative game workflow, and keep the smallest failing case as a regression test.

Use changes that should preserve meaning as additional oracles: moving a declaration to another module, replacing an inferred local with an equivalent annotation, or reordering unrelated declarations should not change accepted code, editor types, or runtime results. A negative case should still fail at the right source location with an actionable diagnostic. Prioritize coverage by impact, likelihood of real use, shared implementation, and past defects; record the uncovered boundary and reason rather than claiming all combinations are verified.

Verify migration and environment boundaries separately. Use a previous-release project to check upgrade and rollback of configuration, generated output, extension settings, and cached editor state. Record tested LuaJIT and LÖVE versions, OS/path behavior (separators, case sensitivity, Unicode), VS Code version, and any non-VS-Code LSP client. Follow the release-readiness checklist for actual release gates; this audit may mark unavailable platforms or published-artifact checks blocked, never passed by a local build.

Audit the evidence-producing tests as well as the product. For each high-risk behavior, name the oracle: runtime result, independent Lua implementation, compiler diagnostic, protocol response, or installed-editor action. Check whether its test would fail on a representative regression, whether it runs in CI, and whether skips or snapshot-only assertions conceal behavior. Favor a small mutation or negative fixture when a test's sensitivity is unclear. Use fuzz/property checks for parser recovery, positions/source maps, and recursive types only where they add an independent oracle; record seeds and minimized cases.

Exercise creation, repeated use, disposal, and failure recovery for stateful helpers and editor services. Include failed hot reloads, cancelled requests, renamed/deleted modules, stale previews, listener unsubscription, pool reuse, and long sessions. Check that an untrusted or malformed project cannot cause surprising external execution, private-path disclosure, or writes outside configured output roots; identify whether the boundary is compiler, CLI, extension, webview, or API/MCP bridge.

Before interpreting performance, capture a current baseline and workload. Measure cold and warm p50/p95 for completion and hover, editor key-to-menu delay, format-on-save, check/build, webview time to first useful content, and peak/steady memory on representative small and large projects. Compare against existing benchmarks and a prior release where feasible. Set a regression budget from observed variance before proposing a performance fix; keep server latency separate from extension and rendering cost.

## Preview and information-surface review

A preview should answer a question faster than reading source. The review begins with the question and the next useful action, then evaluates layout and appearance. Do not turn every view into the same graph or card grid. Keep static declarations visually distinct from live state; do not invent runtime information that the protocol does not provide.

| ID | Surface | User question to test |
| --- | --- | --- |
| P01 | Sprite animation preview | Which frames, timing, origins, markers, or image bounds make this animation look wrong? |
| P02 | State-machine preview | Which event reaches a state, from where, and which route or wildcard explains it? |
| P03 | Behavior-tree preview | In what order can children run, where are computed branches, and which node owns a behavior? |
| P04 | Signal graph | Who emits an event, who handles it, what payload is expected, and where do contracts conflict? |
| P05 | Module graph | Why is this module included, what imports it, and where is a cycle or unresolved edge? |
| P06 | ECS explorer | Which systems read or write a component, and where might access or query behavior surprise me? |
| P07 | Object-pool explorer | Where is a pool created and acquired/released, and is lifecycle use visible? |
| P08 | Audio Explorer | Which cue, bus, scene, source file, or analysis warning should I inspect next? |
| P09 | Asset Explorer and native asset preview | What asset is this, where is it used, and can I open the source or metadata quickly? |
| P10 | Live runtime inspector | What changed in this running player, and can I trace it back to the relevant declaration? |
| P11 | Server health | Is typing slow because of the server, project warming, a queue, or a failed request? |
| P12 | Generated Lua beside Tua | What Lua will run for this source, and can I navigate both directions accurately? |
| P13 | Offline LOVE/annotation references and Explain Type | Can I answer the immediate API or type question and return to the code? |

For each P row, exercise a populated project, empty/no-match state, loading, recoverable error, stale/out-of-order refresh, and changed unsaved source. Use the actual VS Code host for source actions, focus, and links. Capture wide and narrow panes, light/dark/high-contrast themes, 200% zoom, long names, large graphs, and reduced motion where relevant. Test keyboard navigation and screen-reader labels, not just click paths. Measure time to first useful content and interaction responsiveness; separate server cost from rendering cost.

The design review asks: What is the primary decision? Is the most important discrepancy or path visible first? Can the user move from a visual item to exact source? Is there enough context to understand why the item exists? Does the view preserve selection and scroll after edits? Is the layout specific to its data and task, or could its heading be swapped with another explorer without changing the UI? Propose a distinct layout only when it improves one of those tasks. Record a before/after task and screenshot criterion for each proposed change.

For each preview, map every displayed fact to its authoritative compiler, LSP, or runtime field. Compare names, links, warnings, and relationships with hover, diagnostics, and CLI output on the same fixture. Check whether filtering or graph clustering hides a relevant error, whether the exact source location survives refresh, and whether the view explains missing data instead of implying a clean state.

Previous preview evidence identified populated-graph, native asset appearance, long-session inspector, and large-graph limits. Recheck those first. A live behavior-tree tick overlay remains a design question requiring a runtime data contract; do not label its absence a static-preview defect.

## Execution sequence

1. Freeze revision and environment; record worktree, versions, available LuaJIT/LOVE/VS Code tools, baseline checks, and previous audit IDs. Generate the complete ledger from the five inventory sources above.
2. Audit core language and emission boundaries first. Reuse the existing emitted-Lua audit cases, then add missing feature interactions and independent runtime oracles. Treat an accepted bad program, rejected valid program, and miscompile as separate finding classes.
3. Audit each game domain end to end: type/check behavior, helper lowering and runtime, editor assistance, disabled/opt-in behavior, docs, and example. Recheck November 7 reports against the current revision.
4. Audit CLI, project model, and editor workflows with actual requests and installed-editor runs. Include cold project open, cross-file edits, malformed intermediate code, file rename/deletion, and project switching.
5. Run the developer-experience journeys in an installed editor and terminal. Record friction and recovery, then reproduce suspected product gaps in smaller fixtures.
6. Audit all P surfaces in realistic populated games and synthetic edge cases. Gather task observations, visual captures, accessibility and responsiveness checks, then write targeted UI directions. Do not redesign from source inspection alone. Preserve the existing VS Code visual language for local refinements; a replacement visual system needs separate product-context decisions.
7. Run the cross-feature, migration, lifecycle, trust-boundary, and performance gates above. Inspect whether existing CI tests would detect the failures found. Record platform and client limits explicitly.
8. Reconcile overlap: one root cause with multiple reproductions becomes one finding. Independently recheck high-severity results. Compare documentation, examples, runtime output, and editor claims in a final truthfulness pass.
9. Prioritize fixes and documentation refinements by user impact and shared infrastructure. Implement later in small, independently verified changes if that follow-on work is authorized; this plan itself changes no product behavior.

## Deliverables for the completed audit

Create internal/audits/feature-and-preview-2026-11-11/ with:

- README: executive assessment, dependable workflows, consequential gaps, and limits.
- COVERAGE: one row per discovered feature and preview, with contract/source/example/test links and layer statuses.
- INTERACTIONS: selected cross-feature cases, expected compiler/editor/runtime agreement, observed results, selection rationale, and remaining combinations.
- FINDINGS: stable IDs, severity, confidence, reproduction, expected/actual behavior, root-cause evidence, affected surfaces, and regression test.
- PREVIEWS: each user question, actual screenshots or interaction record, what helped or blocked the task, specific UI proposal, and acceptance check. Mark visual and source-only evidence distinctly.
- DEVEX: journey logs, time and action observations, recurring friction, what worked, and concrete changes to completion, errors, commands, examples, or workflow placement. Label agent-simulated observations and separate them from product defects.
- DOCS: claim-to-behavior discrepancies, missing examples/prerequisites, duplicated guidance, and precise page-level edits.
- BACKLOG: grouped correctness, capability, usability, documentation, and performance work with dependencies and acceptance criteria.
- VERIFICATION: exact commands, host/runtime versions, revision, previous-release migration outcome, performance baselines, test-oracle/CI checks, skipped or blocked checks, and links to manageable fixtures/captures.

## Completion standard

The audit is complete when every inventory row has a justified status in every applicable layer, the selected feature interactions and release-context gates have outcomes or explicit blockers, all developer-experience journeys have an observed outcome or explicit blocker, all confirmed findings have runnable evidence, high-impact claims are independently cross-checked, every preview has a task and rendered-host evidence or an explicit blocker, previous findings are reconciled, and the documentation backlog names the owning page. A check that cannot run is recorded as blocked and does not count as a pass. Avoid universal reliability claims; state the tested runtimes, projects, editor, and limits.

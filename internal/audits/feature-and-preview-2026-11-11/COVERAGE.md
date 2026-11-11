# Feature and preview coverage

Baseline: `c08edbca` (2026-11-11). Status values: tested, inspected only, blocked, not applicable; `pending` means work has not yet been done. Rows below are the initial grouped inventory; `tested` means at least one documented subcase was executed, not every clause in that row. The executed subcases and untested limits are recorded in the linked reports. Split grouped rows and add discovered capabilities during investigation.

| ID | Contract | Source lead | Core/runtime | Editor/preview | Docs/example | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| L01 | Lua 5.1-compatible syntax, control flow, operators, comments, and parser recovery | language; lua-basics; lua_cheatsheet | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| L02 | Primitive types, any, inference, assignment, reassignment, and deferred initialization | typing; types; inference | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| L03 | Aliases, recursive aliases, Self, and declaration-only types | typing; project_types | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| L04 | Table shapes, nested and optional fields, mutation, and indexes | typing; types; inference | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| L05 | Arrays, dictionaries, tuples, varargs, and multiple returns | typing; functions; inference | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| L06 | Unions, nilability, literal choices, and erased enums | typing; types | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| L07 | Control-flow narrowing and runtime is checks | typing; classes; lua_cheatsheet | tested | inspected only | inspected only | LANGUAGE.md LANG-02; class runtime `is` only |
| L08 | Functions, defaults, optional parameters, callbacks, closures, and recursion | language; functions; scopes | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| L09 | Function types, overloads, colon/dot calls, and implicit self | typing; functions; classes | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| L10 | Generic aliases, functions, bounds, constructors, and cross-file use | typing; generics | tested | tested | inspected only | LANGUAGE.md LANG-01; imported bound case only |
| L11 | Protocol declarations, conformance, method calls, and polymorphism | typing; protocols; polymorphism | tested | tested | inspected only | LANGUAGE.md LANG-01; imported protocol bound case only |
| L12 | Classes, inheritance, overrides, final methods, super, and runtime dispatch | classes; polymorphism | tested | tested | inspected only | LANGUAGE.md LANG-02; cross-file `is` and inheritance only |
| L13 | Metatables, metamethods, iterators, and conservative Lua object typing | language; lua_cheatsheet | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| L14 | Compound assignment and evaluation-order preservation | language; compound | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| L15 | Require, static import, re-export, module roots, cycles, and mixed Lua/Tua projects | modules; imports; require_graph | tested | inspected only | inspected only | LANGUAGE.md LANG-01/02; selected module paths only |
| L16 | LuaDoc/LuaLS/LuaHelper annotations, documentation comments, and LuaCATS output | comments; annotations; luals_annotations; luacats_emit | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| L17 | LOVE API catalog, callbacks, constructors, methods, overloads, and color values | language; assets; conf | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| L18 | Source mapping, readable emitted Lua, helper inclusion, and LuaJIT/LOVE runtime behavior | language; emitted-Lua audit | tested | inspected only | inspected only | LANGUAGE.md LANG-02; runtime class identity only |
| L19 | Scope, shadowing, forward references, and unsaved-buffer consistency | scopes; navigation; lsp | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| G01 | Asset helpers, path validation, metadata, and asset pipelines | language; asset-pipelines; assets | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| G02 | Sprite sheets, animation playback, markers, and callback semantics | sprites; sprites | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| G03 | Vec2 and opt-in math helpers | vectors; vectors; game_math | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| G04 | SignalBus declarations, handlers, event names, and payload inference | signals; signals | tested | inspected only | inspected only | DOMAINS.md; nested imported SignalBus emit |
| G05 | Stores, subscriptions, keyed watchers, and unsubscribe behavior | stores; store | tested | inspected only | inspected only | INTERACTIONS.md; generic Store case |
| G06 | State machines, routes, callbacks, and generated runtime | state-machines; state_machine | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| G07 | Typed input, source names, control pairs, and Baton runtime | input; input | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| G08 | Tweening, easing, delay, update, and cancellation | tweening; tweening | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| G09 | Camera transforms, following, drawing, and bounds | camera; camera | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| G10 | Procedural Rot helpers: generation, paths, visibility, dice, colors | roguelike; roguelike | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| G11 | Behavior trees, runner state, tick results, and opt-in inclusion | behavior-trees; behavior_tree; behavior_tree_ecs | tested | inspected only | inspected only | DOCS_DEEP.md; behavior-tree/ECS/pool runtime |
| G12 | ECS schemas, queries, systems, access inference, and runtime | ecs; ecs | tested | inspected only | inspected only | INTERACTIONS.md; ECS+Signal case |
| G13 | Object pools, lifecycle, prewarming, safety, and emitted runtime | object-pooling; object_pool | tested | inspected only | inspected only | DOCS_DEEP.md; behavior-tree/ECS/pool runtime |
| G14 | Audio buses, cues, music scenes, metadata, and analysis cache | audio; audio-system | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| G15 | Shader values, uniforms, includes, validation, ShaderScan, and hot reload | shaders; shaders; assets | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| T01 | Project initialization, configuration defaults, roots, and workspace isolation | cli; configuration; project-model | tested | inspected only | inspected only | VERIFICATION.md; fresh init/config smoke |
| T02 | Check, lint, diagnostics, suppression, and source locations | cli; linter; linter example | tested | inspected only | inspected only | VERIFICATION.md; check/lint/negative type error |
| T03 | Formatter, range/whole-file edits, and format-on-save | formatter; extension | tested | inspected only | inspected only | VERIFICATION.md; fmt and nonwriting --check |
| T04 | Build, output copying, helper inclusion, Lua syntax, and source maps | cli; examples; emitted-Lua audit | tested | inspected only | inspected only | VERIFICATION.md; build/maps/traceback/package |
| T05 | Busted testing, discovery, gutters, mapping, and coverage | testing; unit-testing | tested | inspected only | inspected only | VERIFICATION.md; bundled Busted example |
| T06 | Watch, dev, hot reload, error recovery, and timing | hot-reload; love-minimal | tested | inspected only | inspected only | VERIFICATION.md; watch edit/recovery and dev --no-launch |
| T07 | Run, debug, traceback, game console, and multiplayer instances | debugging; love-tua; extension | inspected only | inspected only | inspected only | Plan inventory, public docs, existing tests/reports; selected behavior not rerun |
| T08 | Packaging for LOVE, Windows, and macOS | packaging; cli | tested | inspected only | inspected only | VERIFICATION.md; .love and macOS package structure |
| T09 | LSP completion, hover, signature help, diagnostics, actions, navigation, rename, symbols, tokens, and inlays | lsp; editor tests | inspected only | tested | inspected only | EDITOR.md; real-server host subset |
| T10 | Unsaved edits, dependency invalidation, cancellation, long sessions, and project switching | lsp; architecture; benchmarks | inspected only | tested | inspected only | EDITOR.md; contention and unsaved subset |
| T11 | API/MCP bridge, path/privacy boundaries, and versioned protocol | external-api-bridge; API tests | tested | inspected only | inspected only | VERIFICATION.md; API and MCP stdio smoke |
| T12 | Editor setup, theme, toolbar, command discovery, offline references, and server health | extension manifest; editor reference | inspected only | tested | inspected only | EDITOR.md; activation/theme/server-health subset |
| T13 | Asset, shader, audio, module, signal, ECS, pool, and runtime-inspector editor integrations | lsp; extension views | inspected only | tested | inspected only | PREVIEWS.md; selected rendered views |
| P01 | Sprite animation preview | Which frames, timing, origins, markers, or image bounds make this animation look wrong? | inspected only | tested | inspected only | PREVIEWS.md; see per-view evidence and limits |
| P02 | State-machine preview | Which event reaches a state, from where, and which route or wildcard explains it? | inspected only | tested | inspected only | PREVIEWS.md; see per-view evidence and limits |
| P03 | Behavior-tree preview | In what order can children run, where are computed branches, and which node owns a behavior? | inspected only | tested | inspected only | PREVIEWS.md; see per-view evidence and limits |
| P04 | Signal graph | Who emits an event, who handles it, what payload is expected, and where do contracts conflict? | inspected only | tested | inspected only | PREVIEWS.md; see per-view evidence and limits |
| P05 | Module graph | Why is this module included, what imports it, and where is a cycle or unresolved edge? | inspected only | tested | inspected only | PREVIEWS.md; see per-view evidence and limits |
| P06 | ECS explorer | Which systems read or write a component, and where might access or query behavior surprise me? | inspected only | tested | inspected only | PREVIEWS.md; see per-view evidence and limits |
| P07 | Object-pool explorer | Where is a pool created and acquired/released, and is lifecycle use visible? | inspected only | tested | inspected only | PREVIEWS.md; see per-view evidence and limits |
| P08 | Audio Explorer | Which cue, bus, scene, source file, or analysis warning should I inspect next? | inspected only | tested | inspected only | PREVIEWS.md; see per-view evidence and limits |
| P09 | Asset Explorer and native asset preview | What asset is this, where is it used, and can I open the source or metadata quickly? | inspected only | inspected only | inspected only | PREVIEWS.md; see per-view evidence and limits |
| P10 | Live runtime inspector | What changed in this running player, and can I trace it back to the relevant declaration? | inspected only | inspected only | inspected only | PREVIEWS.md; see per-view evidence and limits |
| P11 | Server health | Is typing slow because of the server, project warming, a queue, or a failed request? | inspected only | tested | inspected only | PREVIEWS.md; see per-view evidence and limits |
| P12 | Generated Lua beside Tua | What Lua will run for this source, and can I navigate both directions accurately? | inspected only | inspected only | inspected only | PREVIEWS.md; see per-view evidence and limits |
| P13 | Offline LOVE/annotation references and Explain Type | Can I answer the immediate API or type question and return to the code? | inspected only | inspected only | inspected only | PREVIEWS.md; see per-view evidence and limits |

## Expanded command and setting inventory

These rows record discovery and source inspection only unless an exact execution is cited. A passing package manifest or activation smoke is not a user-visible command test.

| ID | Capability | Source | Status | Evidence |
| --- | --- | --- | --- | --- |
| TCLI-01 | `tua init` | CLI help and `docs/cli.md` | tested | Fresh CLI or bundled test fixture; see VERIFICATION.md |
| TCLI-02 | `tua check` | CLI help and `docs/cli.md` | tested | Fresh CLI or bundled test fixture; see VERIFICATION.md |
| TCLI-03 | `tua fmt` | CLI help and `docs/cli.md` | tested | Fresh CLI or bundled test fixture; see VERIFICATION.md |
| TCLI-04 | `tua lint` | CLI help and `docs/cli.md` | tested | Fresh CLI or bundled test fixture; see VERIFICATION.md |
| TCLI-05 | `tua test` | CLI help and `docs/cli.md` | tested | Fresh CLI or bundled test fixture; see VERIFICATION.md |
| TCLI-06 | `tua build` | CLI help and `docs/cli.md` | tested | Fresh CLI or bundled test fixture; see VERIFICATION.md |
| TCLI-07 | `tua package` | CLI help and `docs/cli.md` | tested | Fresh CLI or bundled test fixture; see VERIFICATION.md |
| TCLI-08 | `tua traceback` | CLI help and `docs/cli.md` | tested | One direct generated-to-source line mapping; VERIFICATION.md |
| TCLI-09 | `tua watch` | CLI help and `docs/cli.md` | tested | Source rebuild and invalid-edit recovery in a fresh project; VERIFICATION.md |
| TCLI-10 | `tua dev` | CLI help and `docs/cli.md` | tested | `--no-launch` marker/ready smoke only; LOVE reload delivery blocked; VERIFICATION.md |
| TCLI-11 | `tua shader-tools` | CLI help and `docs/cli.md` | tested | Command report parsed; LOVE validator unavailable; VERIFICATION.md |
| TCLI-12 | `tua lsp` | CLI help and `docs/cli.md` | tested | Stdio hover, completion, navigation, diagnostics and installed-editor selected requests; EDITOR.md and PRIOR_LANGUAGE.md |
| TCLI-13 | `tua api` | CLI help and `docs/cli.md` | tested | Initialize, project summary, source-context gate, path rejection, shutdown; VERIFICATION.md |
| TCLI-14 | `tua mcp` | CLI help and `docs/cli.md` | tested | Initialize, tools/list, shutdown only; VERIFICATION.md |
| TVSC-001 | `tua.tests.refresh` — Refresh Unit Tests | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-002 | `tua.openAnnotationGuide` — Open Annotation Guide (Offline) | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-003 | `tua.openLoveApiReference` — Open LÖVE API Reference (Offline) | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-004 | `tua.initProject` — Initialize Tua Project | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-005 | `tua.restartServer` — Restart Language Server | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-006 | `tua.output.focus` — Focus Output | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-007 | `tua.openSettings` — Open Settings | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-008 | `tua.selectServer` — Select Server | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-009 | `tua.copyDebugInfo` — Copy Debug Info | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-010 | `tua.copyCacheStats` — Copy Cache Stats | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-011 | `tua.saveCacheStats` — Save Cache Stats | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-012 | `tua.showServerHealth` — Show Language Server Health | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-013 | `tua.resetCompletionStats` — Reset Completion Learning Stats | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-014 | `tua.completion.toggleExperimental` — Toggle Experimental Completions | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-015 | `tua.completion.toggleLocalInline` — Toggle Deterministic Inline Completions | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-016 | `tua.completion.toggleOllama` — Toggle Local Ollama Completions | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-017 | `tua.completion.warmOllama` — Test and Warm Ollama Model | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-018 | `tua.saveCpuProfile` — Save CPU Profile | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-019 | `tua.saveHeapSummary` — Save Heap Summary | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-020 | `tua.openGeneratedLua` — Open Generated Lua Beside Tua | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-021 | `tua.jumpBetweenSourceAndGenerated` — Jump Between Tua and Generated Lua | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-022 | `tua.runLoveProject` — Run LOVE Project | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-023 | `tua.stopLoveProject` — Stop LOVE Project | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-024 | `tua.debugLoveProject` — Debug LOVE Project | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-025 | `tua.runLoveProjectWithHotReload` — Run LOVE Project With Hot Reload | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-026 | `tua.packageGame` — Package Game... | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-027 | `tua.restartLoveProject` — Restart LOVE Project | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-028 | `tua.copyQualifiedName` — Copy Fully Qualified Symbol Name | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-029 | `tua.tests.createForFunction` — Create Unit Test for Function | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-030 | `tua.showWorkspaceSymbolsByModule` — Workspace Symbols by Module | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-031 | `tua.hover.references` — Show References from Hover | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-032 | `tua.hover.implementations` — Show Implementations from Hover | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-033 | `tua.inlayHints.selectMode` — Select Inlay Hint Mode | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-034 | `tua.explainType` — Explain Type / Why Is This Any? | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-035 | `tua.previewSprites` — Preview Sprite Animations | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-036 | `tua.previewStateMachine` — Preview State Machine | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-037 | `tua.previewBehaviorTree` — Preview Behavior Tree | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-038 | `tua.assets.refresh` — Refresh Asset Explorer | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-039 | `tua.assets.preview` — Preview Asset | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-040 | `tua.assets.open` — Open Asset | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-041 | `tua.assets.openReference` — Open Asset Reference | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-042 | `tua.assets.findReferences` — Find Asset References | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-043 | `tua.assets.rename` — Rename Asset... | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-044 | `tua.assets.reveal` — Reveal Asset in File Explorer | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-045 | `tua.showSignalGraph` — Show Signal Graph | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-046 | `tua.showModuleGraph` — Show Module Dependency Graph | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-047 | `tua.showEcsExplorer` — Show ECS Explorer | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-048 | `tua.showObjectPoolExplorer` — Show Object Pool Explorer | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-049 | `tua.showAudioExplorer` — Show Audio Explorer | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-050 | `tua.whyModuleIncluded` — Why Is This Module Included? | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-051 | `tua.source.organizeImports` — Organize Imports | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-052 | `tua.source.sortImports` — Sort Imports | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-053 | `tua.source.removeUnusedImports` — Remove Unused Imports | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-054 | `tua.source.fixAll` — Fix All Safe Diagnostics | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-055 | `tua.reportIssue` — Report Issue | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-056 | `tua.openSetup` — Set Up Tua | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-057 | `tua.setup.applyTheme` — Use Indigo Carbon Theme | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-058 | `tua.setup.restoreTheme` — Restore Previous Theme | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-059 | `tua.setup.applyLayout` — Apply Recommended Layout | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-060 | `tua.setup.restoreLayout` — Restore Previous Layout | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-061 | `tua.setup.useMaterialIcons` — Install / Use Material File Icons | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-062 | `tua.setup.restoreIcons` — Restore Previous File Icons | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-063 | `tua.workspace.open` — Open Tua Workspace | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-064 | `tua.workspace.refresh` — Refresh Project | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-065 | `tua.workspace.graph` — Open Graph | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-066 | `tua.workspace.sprites` — Preview File Sprites | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-067 | `tua.workspace.states` — Preview File State Machine | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-068 | `tua.workspace.generated` — Open File Generated Lua | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-069 | `tua.activity.show` — Inspect Run Status | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-070 | `tua.activity.stop` — Stop Session | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-071 | `tua.run.select` — Select Run Configuration | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-072 | `tua.run.selected` — Run Selected Configuration | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-073 | `tua.run.configuration` — Run This Configuration | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-074 | `tua.run.configure` — Edit Run Configurations | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-075 | `tua.console.open` — Open Game Console | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-076 | `tua.console.clear` — Clear Game Console | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-077 | `tua.console.level` — Filter Console Severity | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-078 | `tua.console.module` — Filter Console Module | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-079 | `tua.console.session` — Filter Console Session | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-080 | `tua.console.copy` — Copy Console Message | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-081 | `tua.checks.run` — Check Project Setup | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-082 | `tua.checks.fix` — Resolve Setup Finding | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-083 | `tua.checks.chooseLove` — Choose LOVE Executable | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-084 | `tua.checks.enableMaps` — Enable Project Source Maps | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-085 | `tua.checks.build` — Build Source Maps | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-086 | `tua.testProject` — Run Unit Tests | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-087 | `tua.buildProject` — Build Project | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-088 | `tua.inlayHints.inspect` — Inspect Hints at Cursor | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-089 | `tua.run.stopSelected` — Stop Selected Run | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-090 | `tua.inspector.enable` — Enable Runtime Inspection | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-091 | `tua.inspector.disable` — Disable Runtime Inspection | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-092 | `tua.inspector.open` — Open Runtime Inspector | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-093 | `tua.inspector.select` — Select Runtime Player | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-094 | `tua.inspector.pause` — Pause Inspector Updates | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-095 | `tua.inspector.refresh` — Refresh Runtime Snapshot | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-096 | `tua.inspector.resume` — Resume Inspector Updates | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-097 | `tua.inspector.goToSource` — Go to Runtime Object Creation Site | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-098 | `tua.setup.applyShaders` — Use LÖVE Shader Associations | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-099 | `tua.setup.restoreShaders` — Restore Shader Associations | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-100 | `tua.setup.installShaderHighlighting` — Install Shader Highlighting | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-101 | `tua.setup.skipShaders` — Skip Shader Setup | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-102 | `tua.setup.checkShaderTools` — Check Shader Validators | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-103 | `tua.setup.selectGlslang` — Select glslangValidator | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-104 | `tua.declaration.tools` — Tools for Declaration… | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-105 | `tua.inspector.forDeclaration` — Inspect Runtime Declaration | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TVSC-106 | `tua.inspector.configure` — Configure Runtime Inspection… | `editors/vscode/package.json` | inspected only | Manifest registration; installed-editor action pending |
| TSET-001 | `tua.test.bustedPath` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-002 | `tua.test.luaPath` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-003 | `tua.server.path` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-004 | `tua.love.command` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-005 | `tua.love.windowsRuntime` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-006 | `tua.love.macosApp` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-007 | `tua.trace.server` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-008 | `tua.log.verbosity` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-009 | `tua.format.indentStyle` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-010 | `tua.format.indentWidth` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-011 | `tua.format.lineWidth` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-012 | `tua.format.quoteStyle` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-013 | `tua.format.lineEnding` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-014 | `tua.format.finalNewline` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-015 | `tua.diagnostics.rules` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-016 | `tua.diagnostics.duplicateKeys` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-017 | `tua.diagnostics.missingRequires` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-018 | `tua.diagnostics.selfAssignments` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-019 | `tua.diagnostics.docAnnotations` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-020 | `tua.diagnostics.undefinedVariables` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-021 | `tua.diagnostics.unusedLocals` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-022 | `tua.diagnostics.unusedParameters` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-023 | `tua.diagnostics.useBeforeDefinition` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-024 | `tua.diagnostics.duplicateParameters` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-025 | `tua.diagnostics.badRequires` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-026 | `tua.diagnostics.impossibleBooleanExpressions` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-027 | `tua.diagnostics.duplicateIfConditions` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-028 | `tua.diagnostics.duplicateBinaryOperands` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-029 | `tua.diagnostics.floatEquality` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-030 | `tua.diagnostics.constAssignments` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-031 | `tua.diagnostics.assignmentArity` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-032 | `tua.diagnostics.localDeclarationArity` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-033 | `tua.diagnostics.unusedAssignments` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-034 | `tua.diagnostics.binaryOperatorTypeMismatches` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-035 | `tua.diagnostics.uncalledLocalFunctions` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-036 | `tua.diagnostics.maxCyclomaticComplexity` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-037 | `tua.diagnostics.divideByZero` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-038 | `tua.diagnostics.almostSwapped` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-039 | `tua.diagnostics.mixedTables` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-040 | `tua.diagnostics.unknownRecordFields` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-041 | `tua.diagnostics.loveCallbackOrder` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-042 | `tua.diagnostics.inlineIgnores` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-043 | `tua.diagnostics.inlineIgnoreScopes` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-044 | `tua.diagnostics.inlineIgnoreMaxPerFile` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-045 | `tua.diagnostics.globals` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-046 | `tua.jit.enabled` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-047 | `tua.ecs.enabled` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-048 | `tua.completion.sequenceRanking.enabled` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-049 | `tua.completion.sequenceRanking.timeoutMs` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-050 | `tua.completion.sequenceRanking.maxCandidates` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-051 | `tua.completion.multiToken.enabled` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-052 | `tua.completion.multiToken.advanced` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-053 | `tua.completion.multiToken.allowModelIntegration` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-054 | `tua.completion.inline.enabled` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-055 | `tua.completion.inline.sources.semantic` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-056 | `tua.completion.inline.sources.currentFile` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-057 | `tua.completion.inline.sources.statistical` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-058 | `tua.completion.ollama.endpoint` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-059 | `tua.completion.ollama.model` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-060 | `tua.completion.ollama.timeoutMs` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-061 | `tua.completion.ollama.keepAlive` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-062 | `tua.completion.ollama.contextTokens` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-063 | `tua.completion.ollama.maxTokens` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-064 | `tua.completion.multiToken.maxSuggestions` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-065 | `tua.fileAssociations` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-066 | `tua.ignoredPaths` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-067 | `tua.requirePathSeparator` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-068 | `tua.preview.enabled` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-069 | `tua.editor.toolbarMode` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-070 | `tua.run.configurations` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-071 | `tua.runtimeInspector.enabled` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-072 | `tua.shaders.glslangPath` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-073 | `tua.runtimeInspector.categories` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TSET-074 | `tua.runtimeInspector.updateInterval` | `editors/vscode/package.json` | inspected only | Default/schema read; behavior pending |
| TVIEW-01 | `tua.run` (Run Configuration) | `editors/vscode/package.json`, tua | inspected only | View registration; rendered-host behavior pending |
| TVIEW-02 | `tua.workspace` (Project) | `editors/vscode/package.json`, tua | inspected only | View registration; rendered-host behavior pending |
| TVIEW-03 | `tua.assets` (Tua Assets) | `editors/vscode/package.json`, tua | inspected only | View registration; rendered-host behavior pending |
| TVIEW-04 | `tua.sessions` (Run Sessions) | `editors/vscode/package.json`, tua | inspected only | View registration; rendered-host behavior pending |
| TVIEW-05 | `tua.checks` (Project Checks) | `editors/vscode/package.json`, tua | inspected only | View registration; rendered-host behavior pending |
| TVIEW-06 | `tua.inspector` (Runtime Inspector) | `editors/vscode/package.json`, tua | inspected only | View registration; rendered-host behavior pending |
| TVIEW-07 | `tua.console` (Game Console) | `editors/vscode/package.json`, tua-console | inspected only | View registration; rendered-host behavior pending |

## Protocol, config, diagnostic, and helper subinventory

These are distinct contracts discovered from the versioned request list, TOML schema, diagnostic registry, and domain helper APIs. `inspected only` means the item was cataloged; it does not imply a request, invalid input, or runtime path passed. Current risk-selected executions are documented in the reports above.

| ID | Contract | Purpose | Source | Status |
| --- | --- | --- | --- | --- |
| TLSP-01 | `tua/explainType` | Source-linked type inference explanation | `docs/lsp.md` | inspected only |
| TLSP-02 | `tua/inspect` | Structured document inspector data, including sprite sheets, static state machines, and static behavior trees | `docs/lsp.md` | inspected only |
| TLSP-03 | `tua/assetInventory` | Project assets, metadata, references, and hints | `docs/lsp.md` | inspected only |
| TLSP-04 | `tua/signalGraph` | Typed SignalBus project graph | `docs/lsp.md` | inspected only |
| TLSP-05 | `tua/moduleGraph` | Module dependency and export graph | `docs/lsp.md` | inspected only |
| TLSP-06 | `tua/ecsGraph` | ECS components, systems, queries, and insights | `docs/lsp.md` | inspected only |
| TLSP-07 | `tua/objectPoolGraph` | Object-pool declarations and lifecycle facts | `docs/lsp.md` | inspected only |
| TLSP-08 | `tua/audioGraph` | Static audio-system declarations and their exact configured names | `docs/lsp.md` | inspected only |
| TLSP-09 | `tua/builtinDocument` | Generated Lua and LOVE builtin stub source | `docs/lsp.md` | inspected only |
| TLSP-10 | `tua/cacheStats` | Analysis cache, workspace-warming, and experimental pattern-corpus counters | `docs/lsp.md` | inspected only |
| TLSP-11 | `tua/health` | Server revision, queues, cache, pattern-corpus, stale-result, and latency health | `docs/lsp.md` | inspected only |
| TLSP-12 | `tua/resetCompletionStats` | Clear local adaptive completion statistics | `docs/lsp.md` | inspected only |
| TLSP-13 | `tua/localInlineCompletion` | Return a version-bound deterministic inline-completion candidate from enabled local sources | `docs/lsp.md` | inspected only |
| TLSP-14 | `tua/setTrace` | Trace configuration | `docs/lsp.md` | inspected only |
| TCFG-01 | `source` | Directory containing Tua source files. | `config/schema.json` | inspected only |
| TCFG-02 | `out` | Directory where emitted Lua files are written. | `config/schema.json` | inspected only |
| TCFG-03 | `main` | Source file under source that also emits to <out>/main.lua for LOVE entrypoints. | `config/schema.json` | inspected only |
| TCFG-04 | `target` | Runtime target label. | `config/schema.json` | inspected only |
| TCFG-05 | `extension` | Source extension to discover. | `config/schema.json` | inspected only |
| TCFG-06 | `emit_maps` | Write Lua source-map sidecars during builds. | `config/schema.json` | inspected only |
| TCFG-07 | `test.source` | Optional project-relative test directory. An empty value discovers specs under source. Dedicated test files are omitted from production builds. | `config/schema.json` | inspected only |
| TCFG-08 | `test.exclude_specs` | Omit *_spec source files from production builds, including specs under the main source root. | `config/schema.json` | inspected only |
| TCFG-09 | `type_libraries` | Declaration-only LuaCATS libraries mapped from declaration module identities to runtime require prefixes. | `config/schema.json` | inspected only |
| TCFG-10 | `asset_pipelines` | Opt-in external importers with content-addressed cached generated modules and structured diagnostics. | `config/schema.json` | inspected only |
| TCFG-11 | `emit.annotation_mode` | Controls annotation-preserving Lua emission. | `config/schema.json` | inspected only |
| TCFG-12 | `format.indent_style` | Indent nested code with spaces or tabs. | `config/schema.json` | inspected only |
| TCFG-13 | `format.indent_width` | Spaces per indentation level. | `config/schema.json` | inspected only |
| TCFG-14 | `format.line_width` | Preferred maximum formatted line width. | `config/schema.json` | inspected only |
| TCFG-15 | `format.quote_style` | Short-string quote policy: preserve source quotes or consistently use double or single quotes. | `config/schema.json` | inspected only |
| TCFG-16 | `format.line_ending` | Output line-ending policy. Preserve follows the source file's LF or CRLF convention. | `config/schema.json` | inspected only |
| TCFG-17 | `format.final_newline` | Write a final LF newline when formatting. | `config/schema.json` | inspected only |
| TCFG-18 | `jit.enabled` | Enable opt-in LuaJIT shape guidance diagnostics. | `config/schema.json` | inspected only |
| TCFG-19 | `jit.variable_type_drift` | Retain variable type drift guidance compatibility. | `config/schema.json` | inspected only |
| TCFG-20 | `jit.table_shape_drift` | Report clear table-shape drift. | `config/schema.json` | inspected only |
| TCFG-21 | `jit.hot_loop_hints` | Report dynamic table-key writes in loops. | `config/schema.json` | inspected only |
| TCFG-22 | `ecs.enabled` | Enable the built-in ECS runtime and first-class ECS analysis. | `config/schema.json` | inspected only |
| TCFG-23 | `ecs.backend` | Select the ECS runtime profile; tua uses the bundled zero-setup runtime. | `config/schema.json` | inspected only |
| TCFG-24 | `shaders.validate` | Compile referenced shaders during check and trusted editor validation. Auto prefers LOVE, falling back to glslangValidator when LOVE cannot validate. | `config/schema.json` | inspected only |
| TCFG-25 | `shaders.targets` | Shader targets to validate: desktop OpenGL, and gles for Android, iOS, and the web. | `config/schema.json` | inspected only |
| TCFG-26 | `shaders.warn_unsent` | Warn about read uniforms never sent by statically traceable project code. Escaped shaders and dynamic loaders suppress this optional lint. | `config/schema.json` | inspected only |
| TCFG-27 | `diagnostics.rules` | Enable Tua's standard code-quality lint rules. | `config/schema.json` | inspected only |
| TCFG-28 | `diagnostics.duplicate_keys` | Report duplicate table keys. | `config/schema.json` | inspected only |
| TCFG-29 | `diagnostics.missing_requires` | Report literal require and import targets that cannot be resolved. | `config/schema.json` | inspected only |
| TCFG-30 | `diagnostics.self_assignments` | Report assignments where the target and value are the same symbol. | `config/schema.json` | inspected only |
| TCFG-31 | `diagnostics.doc_annotations` | Report malformed LuaHelper/LuaLS-style documentation annotations. | `config/schema.json` | inspected only |
| TCFG-32 | `diagnostics.undefined_variables` | Report undefined variables when static analysis can prove them. | `config/schema.json` | inspected only |
| TCFG-33 | `diagnostics.unused_locals` | Report unused local variables, loop bindings, and direct function declarations. Prefix an intentionally unused binding with `_` to silence it. | `config/schema.json` | inspected only |
| TCFG-34 | `diagnostics.unused_parameters` | Report unused function parameters and unused `...` varargs. Prefix an intentionally unused named parameter with `_` to silence it. | `config/schema.json` | inspected only |
| TCFG-35 | `diagnostics.use_before_definition` | Report provable use of locals before their definition. | `config/schema.json` | inspected only |
| TCFG-36 | `diagnostics.duplicate_parameters` | Report duplicate function parameter names as TL4043. | `config/schema.json` | inspected only |
| TCFG-37 | `diagnostics.bad_requires` | Report invalid require targets. | `config/schema.json` | inspected only |
| TCFG-38 | `diagnostics.impossible_boolean_expressions` | Report boolean expressions that can never be true. | `config/schema.json` | inspected only |
| TCFG-39 | `diagnostics.duplicate_if_conditions` | Report repeated if or elseif conditions. | `config/schema.json` | inspected only |
| TCFG-40 | `diagnostics.duplicate_binary_operands` | Report suspicious repeated binary operands. | `config/schema.json` | inspected only |
| TCFG-41 | `diagnostics.float_equality` | Report direct equality comparisons on numbers. | `config/schema.json` | inspected only |
| TCFG-42 | `diagnostics.const_assignments` | Report assignments to values marked as constant. | `config/schema.json` | inspected only |
| TCFG-43 | `diagnostics.assignment_arity` | Report explicit assignment target/value-count mismatches while preserving Lua multi-return tails. | `config/schema.json` | inspected only |
| TCFG-44 | `diagnostics.local_declaration_arity` | Report explicit local/initializer-count mismatches while preserving Lua multi-return tails. | `config/schema.json` | inspected only |
| TCFG-45 | `diagnostics.unused_assignments` | Report assignments to locals that are never read afterwards. | `config/schema.json` | inspected only |
| TCFG-46 | `diagnostics.binary_operator_type_mismatches` | Report binary operators with statically incompatible operand types. | `config/schema.json` | inspected only |
| TCFG-47 | `diagnostics.uncalled_local_functions` | Report local functions that are referenced but never directly called; callback uses still count as uncalled. | `config/schema.json` | inspected only |
| TCFG-48 | `diagnostics.max_cyclomatic_complexity` | Report functions whose cyclomatic complexity exceeds this value; zero disables the rule. | `config/schema.json` | inspected only |
| TCFG-49 | `diagnostics.divide_by_zero` | Report division by a nonzero or dynamic value and a literal zero denominator. | `config/schema.json` | inspected only |
| TCFG-50 | `diagnostics.almost_swapped` | Report consecutive assignments that look like a failed value swap. | `config/schema.json` | inspected only |
| TCFG-51 | `diagnostics.mixed_tables` | Report table literals that mix sequential array entries with keyed fields. | `config/schema.json` | inspected only |
| TCFG-52 | `diagnostics.unknown_record_fields` | Report writes to fields that are absent from a statically known record shape. | `config/schema.json` | inspected only |
| TCFG-53 | `diagnostics.love_callback_order` | Report top-level love.load, love.update, and love.draw definitions that are not in the order LÖVE calls them. | `config/schema.json` | inspected only |
| TCFG-54 | `diagnostics.inline_ignores` | Enable exact-code diagnostic suppression comments. | `config/schema.json` | inspected only |
| TCFG-55 | `diagnostics.inline_ignore_scopes` | Allowed exact-code diagnostic suppression directives. | `config/schema.json` | inspected only |
| TCFG-56 | `diagnostics.inline_ignore_max_per_file` | Maximum inline diagnostic suppression comments per file; zero is unlimited. | `config/schema.json` | inspected only |
| TCFG-57 | `diagnostics.globals` | Additional global names accepted by diagnostics. | `config/schema.json` | inspected only |
| TCFG-58 | `diagnostics.severity` | Exact diagnostic-code overrides: allow, hint, info, warning, or error. | `config/schema.json` | inspected only |
| TDIAG-01 | `TL0001-TL0999 syntax` | Diagnostic family | `docs/luahelper-code-matrix.md` | inspected only |
| TDIAG-02 | `TL1000-TL1999 type` | Diagnostic family | `docs/luahelper-code-matrix.md` | inspected only |
| TDIAG-03 | `TL2000-TL2999 build and assets` | Diagnostic family | `docs/luahelper-code-matrix.md` | inspected only |
| TDIAG-04 | `TL3000-TL3999 project configuration` | Diagnostic family | `docs/luahelper-code-matrix.md` | inspected only |
| TDIAG-05 | `TL4000-TL4999 lint and guidance` | Diagnostic family | `docs/luahelper-code-matrix.md` | inspected only |
| TDIAG-06 | `TL5000-TL5999 game domains` | Diagnostic family | `docs/luahelper-code-matrix.md` | inspected only |
| GHELP-01 | `SignalBus.declare` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-02 | `SignalBus.on` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-03 | `SignalBus.once` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-04 | `SignalBus.emit` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-05 | `SignalBus.off/unsubscribe` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-06 | `Store.get` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-07 | `Store.set` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-08 | `Store.update` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-09 | `Store.watch` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-10 | `Store.unsubscribe` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-11 | `ECS.create` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-12 | `ECS.add` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-13 | `ECS.get` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-14 | `ECS.remove` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-15 | `ECS.query` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-16 | `ECS.system` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-17 | `ObjectPool.acquire` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-18 | `ObjectPool.release` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-19 | `ObjectPool.dispose` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-20 | `ObjectPool.prewarm` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-21 | `BehaviorTree.new` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-22 | `BehaviorTree.tick` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-23 | `BehaviorTree.reset` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-24 | `StateMachine.new` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-25 | `StateMachine.send/transition` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-26 | `StateMachine.update` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |
| GHELP-27 | `StateMachine.reset` | Helper lifecycle/API method | `docs domain guide and vendor/runtime` | inspected only |

## Standard LSP method inventory

These method families are separate from the Tua custom requests above. `tested` here means a selected real-server host case, not all symbols, positions, or edit states.

| ID | Method family | Status | Evidence / remaining limit |
| --- | --- | --- | --- |
| TSTD-01 | `diagnostics` | inspected only | docs/lsp.md and LSP server registration; no new method-specific host case |
| TSTD-02 | `completion` | tested | EDITOR.md installed real-server fixture; other contexts unqualified |
| TSTD-03 | `hover` | tested | EDITOR.md installed real-server fixture; other contexts unqualified |
| TSTD-04 | `signatureHelp` | tested | EDITOR.md installed real-server fixture; other contexts unqualified |
| TSTD-05 | `definition` | tested | EDITOR.md installed real-server fixture; other contexts unqualified |
| TSTD-06 | `typeDefinition` | inspected only | docs/lsp.md and LSP server registration; no new method-specific host case |
| TSTD-07 | `implementation` | inspected only | docs/lsp.md and LSP server registration; no new method-specific host case |
| TSTD-08 | `references` | tested | EDITOR.md installed real-server fixture; other contexts unqualified |
| TSTD-09 | `rename` | tested | EDITOR.md installed real-server fixture; other contexts unqualified |
| TSTD-10 | `documentFormatting` | inspected only | docs/lsp.md and LSP server registration; no new method-specific host case |
| TSTD-11 | `rangeFormatting` | inspected only | docs/lsp.md and LSP server registration; no new method-specific host case |
| TSTD-12 | `codeAction` | inspected only | docs/lsp.md and LSP server registration; no new method-specific host case |
| TSTD-13 | `documentSymbol` | inspected only | docs/lsp.md and LSP server registration; no new method-specific host case |
| TSTD-14 | `workspaceSymbol` | inspected only | docs/lsp.md and LSP server registration; no new method-specific host case |
| TSTD-15 | `semanticTokens` | inspected only | docs/lsp.md and LSP server registration; no new method-specific host case |
| TSTD-16 | `inlayHint` | tested | EDITOR.md installed real-server fixture; other contexts unqualified |
| TSTD-17 | `callHierarchy` | inspected only | docs/lsp.md and LSP server registration; no new method-specific host case |
| TSTD-18 | `foldingRange` | inspected only | docs/lsp.md and LSP server registration; no new method-specific host case |
| TSTD-19 | `documentHighlight` | inspected only | docs/lsp.md and LSP server registration; no new method-specific host case |
| TSTD-20 | `codeLens` | inspected only | docs/lsp.md and LSP server registration; no new method-specific host case |

## Split information surfaces

| ID | Surface | Status | Evidence / limit |
| --- | --- | --- | --- |
| P09.A | Asset Explorer graph/list | inspected only | PREVIEWS.md; actions and metadata test sources reviewed, no fresh populated-host asset run. |
| P09.B | Native VS Code image/audio preview | blocked | PREVIEWS.md; no current installed-project capture with representative asset variants. |
| P13.A | Offline LOVE API reference | inspected only | PREVIEWS.md; command/docs reviewed, no host navigation and focus-return scenario. |
| P13.B | Offline annotation reference | inspected only | PREVIEWS.md; command/docs reviewed, no host navigation and focus-return scenario. |
| P13.C | Explain Type surface | inspected only | PREVIEWS.md; source/docs reviewed, no task-based rendered-host check. |

## Executed boundary cases

The grouped and discovered rows above are an inventory. This smaller ledger identifies the scenarios with direct evidence; a pass here does not extend to all forms of the parent feature.

| Case | Boundary | Outcome | Oracle and limit |
| --- | --- | --- | --- |
| X01 | Imported generic class bound × protocol × second method | tested; fixed false TL1003 | LANGUAGE.md LANG-01; failing-before core test, after-fix core/LSP/CLI fixture. |
| X02 | Cross-module unrelated class × runtime `is` | tested; fixed false positive | LANGUAGE.md LANG-02; emitted Lua runtime under Lua 5.4 plus subclass/base positive. |
| X03 | Nested imported SignalBus × `emit` varargs | tested; fixed false TL1004 | DOMAINS.md DOM-01; local/imported checker cases and wrong payload. |
| X04 | Generic alias × Store × ECS × SignalBus | tested for one numeric payload and wrong-string mutations | INTERACTIONS.md; check/build and Lua 5.4 runtime. Unsaved editor edits not tested. |
| X05 | Behavior tree × ECS × pool | tested for one 20-tick game scenario | DOCS_DEEP.md and INTERACTIONS.md; emitted Lua under Lua 5.4; LuaJIT CI test being added. |
| X06 | SignalBus × Store × state machine | tested for event/update/unsubscribe path | INTERACTIONS.md; Lua 5.4 output. Reentrant invalid path untested. |
| X07 | Stale owned helper × later build × manual override | tested; fixed stale helper | DOMAINS.md DOM-02; two CLI regressions and override control. |
| X08 | Watch × invalid edit × recovery | tested in one fresh CLI project | VERIFICATION.md; preserved last good output, then rebuilt valid change. LOVE hot reload not tested. |
| X09 | Real server graph × rendered preview × source action | tested in VS Code host | DOCS_DEEP.md; rendered Signal/ECS/pool content and URI/line action. Raw response-to-render field equality is not exhaustive. |
| X10 | Assets × sprites × shaders | inspected only as one combined interaction | INTERACTIONS.md; each feature built, no shared runtime/preview fixture. |
| X11 | Hot reload × helper inclusion × source-map freshness | inspected only as one combined interaction | INTERACTIONS.md; separate helper and reload checks, no joined replay. |
| X12 | Nil narrowing × closures; annotations × mixed Lua/Tua | inspected only | Existing feature tests/docs were reviewed, but no new combined scenario was run. |

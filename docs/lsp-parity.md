---
icon: lucide/table-2
---

# LSP Parity Matrix

This matrix tracks Tua's current LSP behavior against LuaHelper-style Lua
tooling and useful rust-analyzer patterns. It is intentionally conservative:
the Tua column describes checked-in behavior, not planned behavior.

Numbers in the final column are historical implementation-ledger references,
not current product priorities. Current direction lives in the
[Now, Next, Later roadmap](roadmap.md), and current support is summarized in the
[Capability Matrix](capabilities.md).

For diagnostic-code-level LuaHelper parity, see the
[LuaHelper code matrix](luahelper-code-matrix.md). That page maps LuaHelper
`CheckError*` constants, warning settings, annotation tags, and quick-fix
families to Tua `TLxxxx` diagnostics and LSP surfaces.

## Status Legend

- **Supported**: implemented and covered by current core or LSP tests.
- **Shallow**: implemented, but narrower than mature LuaHelper or
  rust-analyzer behavior.
- **Planned**: not implemented yet and assigned to a roadmap item.
- **Out of scope**: deliberately not planned for Tua's current direction.

## Already Supported

| Feature | Tua today | LuaHelper / rust-analyzer comparison | Roadmap |
| --- | --- | --- | --- |
| Stdio language server | `tua lsp` and `tua lsp --stdio` serve JSON-RPC over stdio. | Standard editor integration shape. | Landed before 30 |
| Incremental document sync | Open, change, save, and close notifications update diagnostics and open-document state. | Basic live diagnostics behavior. | 22, 31 |
| Preemptible request scheduling | Transport and document updates stay on the coordinator while revision-captured queries run in interactive and background worker lanes. Cancellation and stale-result rejection are implemented, and duplicate same-revision background requests are coalesced. | Core queries already executing are not interrupted internally; their cancelled results are discarded when they finish. | 53-70 |
| Diagnostics for `.tua` | Parser, checker, asset, and lint diagnostics publish for open `.tua` documents when the client does not use pull diagnostics. `didOpen` and `didChange` use a fast current-file lane before 200ms-debounced full project-aware diagnostics on the background worker lane, and newer document versions cancel stale slow diagnostics. | Comparable base diagnostic loop, with fewer Lua-specific checks than LuaHelper. | 36, 37, 48.6 |
| Pull diagnostics | Pull-capable clients receive `diagnosticProvider`, can request `textDocument/diagnostic`, and receive `workspace/diagnostic/refresh` after configuration, watched-file, and workspace graph changes. Pull and push use the same snapshot diagnostic conversion. | Tua advertises document pull diagnostics and refresh, not full `workspace/diagnostic` enumeration yet. | 83 |
| LuaJIT guidance diagnostics | Opt-in `[jit]` / `tua.jit.enabled` diagnostics report clear local type drift, table-shape drift, and dynamic table-key writes in loops. | Tua-specific LOVE/LuaJIT guidance rather than a general optimizer or profiler. | 73 |
| LuaDoc annotation parsing | Parses/lower core LuaDoc tags plus practical LuaLS/LuaHelper forms such as literal unions, `---@overload`, `---@vararg`, and shallow metadata tags. | LuaHelper has a broader annotation ecosystem, but the common interop tags exist. | 33, 70.10, 70.25 |
| Annotation-backed type environment | Supported annotations can provide aliases, function types, locals, docs, and simple module exports. LuaDoc `fun(...)` and native `(...) -> ...` callbacks lower to one semantic representation. | LuaHelper supports more patterns and library stubs. | 34 |
| Hover | Shows aliases, locals, parameters, functions, table fields, Lua builtins, LOVE items, require/import literals, module bindings, annotation docs, and inferred returns. | Mature tools attach richer docs and more precise project context. | 40 |
| Completion | Completes aliases, locals, fields, Lua builtins, LOVE items, LuaDoc tags/types/snippets, require paths, import paths, module fields, auto-import candidates, and concise label details, with case-insensitive prefix/contains matching, prefix-stable cache reuse, local/project/adaptive ranking for known receivers, an opt-in bounded local contextual sequence ranker, and opt-in validated local multi-token suggestions. | LuaHelper has deeper dynamic Lua/project completions. | 38, 45, 77, 90 |
| Completion resolve | Completion items can resolve documentation/details from stable item data. | Mature tools use deeper incremental databases, but resolve is no longer just eager payload reuse. | 39 |
| Signature help | Shows known function signatures, native callback aliases, Lua/LOVE builtins, module-exported functions, LuaDoc overloads, generated LOVE overload variants, typed varargs, active parameters, return labels, supported parameter docs, and `)` retrigger support. | Comparable shape, shallower type inference. | 41, 70.25, 91 |
| Go to definition | Resolves aliases, locals, functions, parameters, typed table fields, require/import literals, LOVE virtual stubs, and module exports. | Mature tools support more dynamic module patterns. | 22, 31 |
| Type definition and implementation | Type definition resolves aliases, annotated locals, required-module aliases, table-shape fields, and known builtin or module origins. Implementation resolves known local/module functions and typed receiver methods. | Conservative compared with mature servers: dynamic `any`, metatable-only dispatch, and ambiguous globals return empty results instead of guessed locations. | 82 |
| Document links | Resolved `require("...")` and static `import(...)` literals become module links. | Comparable Lua module navigation. | 31 |
| Document symbols | Type aliases, functions, annotated locals, parameters, table fields, and simple returned exports appear in file outline. | Comparable file outline basics. | 23 |
| Document highlights | Known symbols can be highlighted within the active document. | Comparable local highlight behavior. | 31 |
| References | Finds same-file references and project-wide references for supported globals, module exports, aliases, and module callers. | Still less complete than LuaHelper for dynamic fields/metatables. | 42 |
| Rename | Supports same-file rename and safe cross-file rename for project-identity symbols such as exported functions and type aliases. | Dynamic globals, `any`, and ambiguous fields are rejected. | 43 |
| Workspace symbols | Uses configured workspace indexes for modules, aliases, and functions with fuzzy matching. | Smaller than rust-analyzer's global index, but it no longer has to behave like a raw scan. | 24, 64 |
| Code actions | Provides conservative fixes for missing require targets, inferred local annotations, unused locals, duplicate table keys, self-assignments, trailing whitespace, LOVE dot-method calls, and obvious LOVE constructor-to-helper rewrites. | Smaller action set than rust-analyzer assists. | 44, 70.9 |
| Formatting protocol | Full-document, range, and on-type formatting are advertised and implemented through the same standard Tua formatter. Range edits stay inside the requested range, and on-type edits are limited to stable local line changes. | Less aggressive than mature servers for unstable partial syntax; parse errors produce no destructive edits. | 84 |
| Auto-import completion | Unresolved exported functions can complete with a static import insertion or reuse of an existing require/import alias. | Narrower than rust-analyzer auto-imports, but the workflow exists. | 45 |
| Call hierarchy | Incoming and outgoing calls work for known local functions, module-exported functions, and typed receiver methods. | Dynamic calls through `any` are ignored instead of guessed. | 48 |
| Inlay hints | Provides local type hints and parameter-name hints for supported known expressions. | Shallower than rust-analyzer expression-chain hints. | 60, 64 |
| Selection and folding ranges | Provides selection growth and folding for common syntax structures. | Smaller than rust-analyzer's syntax-tree backed editing surface, but the protocol surface exists. | 67 |
| Semantic tokens | Covers aliases, functions, locals, parameters, fields, builtins, unresolved references, globals, module names, methods, annotation tags, and modifiers. | Core semantic classes are present; deep dynamic Lua remains limited. | 47 |
| Lua and LOVE builtin surface | Curated Lua catalog plus generated LOVE 11.x catalogs provide hover, completion, official LOVE signature overloads, object method signatures, selected return types, LOVE wiki links, and signature-driven LOVE call snippets. | LuaHelper commonly uses richer annotation stubs. | 35, 48.5 |
| LOVE helpers | `image`, `sound`, `music`, `font`, `shader`, `imageData`, `canvas`, `quad`, `spriteBatch`, and `particles` infer LOVE object types, emit ordinary LOVE calls, validate obvious missing paths, unsupported formats, and wrong source types for path-based helpers and native LOVE constructors, and drive loader-specific asset path completion from the warmed asset index. | Tua-specific LOVE-focused workflow. | 48.6 |
| LOVE vector helper | Built-in `Vec2` and `vec2(x, y)` provide completion, hover, signature help, checking, BrineVector-backed emission, and bundled runtime output. | Deeper vector-library-specific docs and richer overload presentation can still improve. | 73 |
| Signal helper | Built-in `SignalBus` and `signal()` provide bus-aware event-name completion, payload signature help, hover, definition, references, rename, `TL5005` schema-conflict diagnostics, Hump-style aliases, `tuasignal`-backed emission, and bundled runtime output by default. | Tua keeps this as ordinary Lua helper/runtime support, not new syntax or a reactive framework; unresolved dynamic buses remain permissive. | 73 |
| ECS domain helper | Built-in `ecs` APIs, named signatures, component/query diagnostics, and on-demand `tuaecs` runtime emission are enabled by default. | Tua models clear static ECS patterns, permits explicit runtime overrides, and leaves dynamic composition permissive. | 74 |
| Object pooling helper | Built-in `objectPool({...})` provides item-aware checking, completion, hover, signature help, inlay hints, prewarming, guarded release, cleanup hooks, and on-demand `tuaobjectpool` output. | Factory return annotations preserve item types; unannotated factories remain permissive, and active-object iteration stays outside the pool. | - |
| Workspace folders and watchers | Workspace folders, folder changes, watched source files, watched LOVE assets, and `tua.toml` changes refresh cached project state, diagnostics, and editor results. | Similar shape to mature LSP servers, with simpler caching. | 32, 46 |
| VS Code lifecycle commands | The extension contributes restart, output-focus, settings, server-selection, trace/log configuration, debug-info copy, cache-stats copy/save, completion-learning reset, profile artifact probes, compact inlay-hint presentation, report-issue commands, and a preview-gated sprite animation preview command, plus a language status item with server state, resolved path, version, and workspace detail. | Tua intentionally avoids telemetry and runtime operations it does not support. | 78, 79, 80, 81 |

## Protocol Surface Supported But Shallow

| Feature | Tua today | Gap | Roadmap |
| --- | --- | --- | --- |
| Analysis architecture | Shared analysis host/snapshot, VFS overlays, cached project model, current-file fast IDE queries, warmed asset index, HIR-like modules, module graph, scope maps, project type database, last-good semantic recovery, cache stats, and profiling exist. | It remains smaller and more file-granular than rust-analyzer's salsa-backed database. | 53-70 |
| LuaDoc coverage | Common interop tags are parsed/lowered, including overload and vararg signatures. | No complete LuaLS compatibility, no full standard-library stub ingestion, and limited generic behavior. | 35, 49, 70.25 |
| Type system | Supports primitives, `any`, aliases, scoped `Self`, unions, native named function types with typed varargs and multiple returns, LuaDoc function interoperability, table shapes, optional fields, stable widened contracts for inferred `.tua` locals, selected flow narrowing, practical metatable facts, runtime `is` checks, optimized class syntax, and multi-return paths. | Generics remain deliberately deferred; there is no structural runtime validation or deep path-sensitive flow typing, and dynamic table shape changes remain permissive. | 61, 62 |
| Required Lua module typing | Simple returned module tables and supported LuaDoc annotations contribute exports. | Complex runtime module construction, side effects, and metatable exports are not modeled. | 59, 61 |
| Workspace symbols | Uses indexed aliases, functions, modules, and exports across configured workspaces. | Symbol classes are still smaller than rust-analyzer's project-wide index. | 64 |
| Inlay hints | Useful local and parameter hints exist. | No full expression-chain or flow-sensitive hint model. | 61, 62, 65 |
| Code actions | Mechanical fixes exist for a focused set of diagnostics and LOVE helper assists. Source actions organize/sort imports, remove unused static import bindings, and fix all safe diagnostics in a file. | Still intentionally conservative: dynamic imports, unsafe creates, overlapping edits, and broader refactor menus are skipped. | 66, 70.9, 89 |
| Code lens | Lazily resolved reference lenses are provided for exported functions and unambiguous type aliases. | No run/check/build or LOVE asset lenses until project intent and command behavior are explicit. | 86 |
| Syntax and semantic highlighting | Immediate TextMate scopes cover Lua/Tua syntax and game-domain helpers; full, range, and full/delta semantic token requests refine standard VS Code classes, enums, types, functions, methods, parameters, variables, properties, namespaces, events, labels, decorators, and modifiers. | Delta edits are intentionally array-based and conservative rather than a deeper incremental semantic-token engine; temporarily invalid source uses a lossy current-text classifier. | 87 |

## Missing And Planned

| Feature | Current Tua behavior | Expected direction | Roadmap |
| --- | --- | --- | --- |
| Deeper analysis precision | A smaller snapshot/database exists, but some precision work still falls back to conservative dynamic results. | Keep moving semantic facts into shared snapshot queries instead of feature-specific paths. | 62, 70.10 |
| LuaHelper parity fixtures and code inventory | Coverage exists through focused tests, and the checked-in LuaHelper code matrix inventories current diagnostic/tag parity. | Add representative fixtures for annotations, builtin stubs, watched-file reindexing, references, completion resolve, signature docs, and lints. | 49, 52, 70.16 |
| Flow-sensitive narrowing | Equality, `type(value) == "..."`, and supported `value is Type` checks narrow true branches, but control-flow typing is still shallow. | Expand negative-branch and multi-condition narrowing only where static proof is reliable. | 62 |
| Rich project edge cases | Project roots, require paths, ignored paths, and targets are modeled. | Add more duplicate-candidate, cycle, generated-source, and multi-root regression coverage as real projects expose gaps. | 55, 59 |
| Profiling expansion | Opt-in tracing covers parse/check/index/request work, with small/medium/large LSP benchmark fixtures, completion-specific p50/p95/p99 profiling, ranking-overhead spans, completion-context cache stats, and local baseline comparison. | Expand profiling only when new hot paths appear. | 69, 77.5 |
| Documentation refresh after architecture work | Docs match the current shared-analysis architecture surface. | Refresh again when LOVE domain extensions or LuaLS compatibility work changes user-visible behavior. | 50, 51, 70 |
| VS Code lifecycle depth | The extension has restart/output/settings commands, server selection, trace/log settings with live `tua/setTrace` updates, cache-stats copy/save, completion-learning reset, profile artifact probes, compact inlay-hint presentation, support commands, a preview-gated sprite animation preview command, and a server status item. | Expand CPU/heap artifact generation only when the Tua server/runtime supports those outputs. | 80 |
| File rename refactors | `workspace.fileOperations.willRename` updates static `require(...)` and `import(...)` paths for safe same-project module moves. | Dynamic specifiers, ignored/generated files, ambiguous module names, and cross-project moves return no edits. | 88 |
| Source actions and fix-all | `source.organizeImports`, `source.sortImports`, `source.removeUnusedImports`, and `source.fixAll` are advertised and implemented conservatively. | Broader source actions such as fix-all across workspaces, organize exports, and refactor menus remain future work. | 89 |
| External tooling bridge | A design exists for an optional local JSON-RPC bridge, but Tua does not expose or start a separate API session today. | Implement only after a concrete consumer exists; the bridge must stay disabled by default, authenticated for pipe/socket use, local-only, and independent of LSP startup. | 93 |

## Intentionally Out Of Scope

| Feature | Reason |
| --- | --- |
| Full LuaHelper clone compatibility | Tua should match useful Lua project workflows, not replicate every LuaHelper diagnostic, setting, or internal behavior. |
| Runtime execution or require side effects | Tua remains static source tooling. It should not run project code to discover symbols. |
| Lua debugger/profiler integration | Debugging is outside the compiler/LSP v1 scope. External Lua/LOVE debuggers can be used separately. |
| Linked editing for current Lua/Tua syntax | Plain Lua/Tua has no tag-like paired identifiers that benefit from synchronized editing. Revisit only if future domain syntax introduces paired names. |
| Custom/proposed VS Code feature forks | Verbose hover, source definition, multi-document highlight, and auto insert were evaluated in roadmap item 92. Tua keeps hover, definition/type-definition, references, document highlight, completion snippets, and `tua:/builtins` on stable standard LSP/VS Code APIs until a concrete Lua/Tua workflow needs a preview-gated custom feature. |
| Package manager ownership | Tua should understand project files and require paths, but package management is not part of the language server. |
| Roblox/Luau runtime compatibility | Luau is a reference point for typed Lua ideas; Tua targets Lua 5.1/LuaJIT/LOVE output. |
| Arbitrary dynamic table/metatable proof | Dynamic tables, metatables, `_G`, and `any` remain gradual escape hatches unless future specs define narrower support. |
| Whole-program optimization or minification | Emitted Lua should stay readable and LuaJIT-compatible rather than optimized or minified by the LSP/toolchain. |

## Maintenance Rules

Run `make editor-regression-smoke` for a focused editor reliability gate. It
combines deterministic scheduler and module-lifecycle tests with requests
against the real `tua lsp` process and the bundled feature project. The smoke
keeps required providers unopened, verifies navigation after a provider closes,
requires inlay hints from the latest document version after an edit, and fails
when post-edit Lua/Tua field completion exceeds one second. Known current-file
receivers use cached active-file semantics; required modules and unresolved
receivers retain the project-snapshot fallback so unsaved exports stay visible.

Use `make extension-test` for VS Code client synchronization and visible editor
behavior, and `make benchmark-smoke` for completion freshness and latency
budgets under representative contention.

- Move a row only when implementation and tests have landed.
- Keep the Tua column factual and tied to checked-in behavior.
- Every planned gap should name a roadmap item.
- Keep diagnostic-level LuaHelper parity claims synchronized with
  [LuaHelper Code Matrix](luahelper-code-matrix.md).
- If a LuaHelper behavior is rejected, move it to **Intentionally Out Of
  Scope** with a short reason.

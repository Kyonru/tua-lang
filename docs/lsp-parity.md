# LSP Parity Matrix

This matrix tracks Tua's current LSP behavior against LuaHelper-style Lua
tooling and useful rust-analyzer patterns. It is intentionally conservative:
the Tua column describes checked-in behavior, not planned behavior.

Roadmap numbers refer to the [roadmap](roadmap.md), which points at the
canonical root `ROADMAP.md`.

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
| Diagnostics for `.tua` | Parser, checker, asset, and lint diagnostics publish for open `.tua` documents. `didChange` uses a fast current-file lane before 200ms-debounced full project-aware diagnostics, and newer document versions cancel stale slow diagnostics. | Comparable base diagnostic loop, with fewer Lua-specific checks than LuaHelper. | 36, 37, 48.6 |
| LuaJIT guidance diagnostics | Opt-in `[jit]` / `tua.jit.enabled` diagnostics report clear local type drift, table-shape drift, and dynamic table-key writes in loops. | Tua-specific LOVE/LuaJIT guidance rather than a general optimizer or profiler. | 73 |
| LuaDoc annotation parsing | Parses/lower core LuaDoc tags plus practical LuaLS/LuaHelper forms such as literal unions, `---@overload`, `---@vararg`, and shallow metadata tags. | LuaHelper has a broader annotation ecosystem, but the common interop tags exist. | 33, 70.10, 70.25 |
| Annotation-backed type environment | Supported annotations can provide aliases, function types, locals, docs, and simple module exports. | LuaHelper supports more patterns and library stubs. | 34 |
| Hover | Shows aliases, locals, parameters, functions, table fields, Lua builtins, LOVE items, require literals, module bindings, annotation docs, and inferred returns. | Mature tools attach richer docs and more precise project context. | 40 |
| Completion | Completes aliases, locals, fields, Lua builtins, LOVE items, LuaDoc tags/types/snippets, require paths, required-module fields, and auto-import candidates. | LuaHelper has deeper dynamic Lua/project completions. | 38, 45 |
| Completion resolve | Completion items can resolve documentation/details from stable item data. | Mature tools use deeper incremental databases, but resolve is no longer just eager payload reuse. | 39 |
| Signature help | Shows known function signatures, Lua/LOVE builtins, required-module functions, LuaDoc overloads, typed varargs, active parameters, and supported parameter docs. | Comparable shape, shallower type inference. | 41, 70.25 |
| Go to definition | Resolves aliases, locals, functions, parameters, typed table fields, require literals, LOVE virtual stubs, and required-module exports. | Mature tools support more dynamic module patterns. | 22, 31 |
| Document links | Resolved `require("...")` literals become module links. | Comparable Lua module navigation. | 31 |
| Document symbols | Type aliases, functions, annotated locals, parameters, table fields, and simple returned exports appear in file outline. | Comparable file outline basics. | 23 |
| Document highlights | Known symbols can be highlighted within the active document. | Comparable local highlight behavior. | 31 |
| References | Finds same-file references and project-wide references for supported globals, module exports, aliases, and required-module callers. | Still less complete than LuaHelper for dynamic fields/metatables. | 42 |
| Rename | Supports same-file rename and safe cross-file rename for project-identity symbols such as exported functions and type aliases. | Dynamic globals, `any`, and ambiguous fields are rejected. | 43 |
| Workspace symbols | Uses configured workspace indexes for modules, aliases, and functions with fuzzy matching. | Smaller than rust-analyzer's global index, but it no longer has to behave like a raw scan. | 24, 64 |
| Code actions | Provides conservative fixes for missing require targets, inferred local annotations, unused locals, duplicate table keys, self-assignments, trailing whitespace, LOVE dot-method calls, and obvious LOVE constructor-to-helper rewrites. | Smaller action set than rust-analyzer assists. | 44, 70.9 |
| Auto-import completion | Unresolved exported functions can complete with a require insertion or reuse of an existing require alias. | Narrower than rust-analyzer auto-imports, but the workflow exists. | 45 |
| Call hierarchy | Incoming and outgoing calls work for known local functions, module-exported functions, and typed receiver methods. | Dynamic calls through `any` are ignored instead of guessed. | 48 |
| Inlay hints | Provides local type hints and parameter-name hints for supported known expressions. | Shallower than rust-analyzer expression-chain hints. | 60, 64 |
| Selection and folding ranges | Provides selection growth and folding for common syntax structures. | Smaller than rust-analyzer's syntax-tree backed editing surface, but the protocol surface exists. | 67 |
| Semantic tokens | Covers aliases, functions, locals, parameters, fields, builtins, unresolved references, globals, module names, methods, annotation tags, and modifiers. | Core semantic classes are present; deep dynamic Lua remains limited. | 47 |
| Lua and LOVE builtin surface | Curated Lua catalog plus generated LOVE 11.x catalog provide hover, completion, signatures, object method signatures, selected return types, LOVE wiki links, and signature-driven LOVE call snippets. | LuaHelper commonly uses richer annotation stubs. | 35, 48.5 |
| LOVE helpers | `image`, `sound`, `music`, `font`, `shader`, `imageData`, `canvas`, `quad`, `spriteBatch`, and `particles` infer LOVE object types, emit ordinary LOVE calls, validate obvious paths/extensions for path-based helpers, and drive helper-specific asset path completion from the warmed asset index. | Tua-specific LOVE-focused workflow. | 48.6 |
| LOVE vector helper | Built-in `Vec2` and `vec2(x, y)` provide completion, hover, signature help, checking, BrineVector-backed emission, and bundled runtime output. | Deeper vector-library-specific docs and richer overload presentation can still improve. | 73 |
| ECS domain helper | Opt-in `[ecs]` / `tua.ecs.enabled` support adds typed `ecs` APIs, component/query diagnostics, and a tiny-ecs-compatible reference adapter. | Tua models clear static ECS patterns and leaves dynamic runtime composition permissive. | 74 |
| Workspace folders and watchers | Workspace folders, folder changes, watched source files, watched LOVE assets, and `tua.toml` changes refresh cached project state, diagnostics, and editor results. | Similar shape to mature LSP servers, with simpler caching. | 32, 46 |

## Protocol Surface Supported But Shallow

| Feature | Tua today | Gap | Roadmap |
| --- | --- | --- | --- |
| Analysis architecture | Shared analysis host/snapshot, VFS overlays, cached project model, warmed asset index, HIR-like modules, module graph, scope maps, project type database, last-good semantic recovery, cache stats, and profiling exist. | It remains smaller and more file-granular than rust-analyzer's salsa-backed database. | 53-70 |
| LuaDoc coverage | Common interop tags are parsed/lowered, including overload and vararg signatures. | No complete LuaLS compatibility, no full standard-library stub ingestion, and limited generic behavior. | 35, 49, 70.25 |
| Type system | Supports primitives, `any`, aliases, unions, function types, table shapes, optional fields, selected inference, and multi-return paths. | No generics in Tua syntax, no metatable typing, no deep flow typing, and dynamic shape changes remain permissive. | 61, 62 |
| Required Lua module typing | Simple returned module tables and supported LuaDoc annotations contribute exports. | Complex runtime module construction, side effects, and metatable exports are not modeled. | 59, 61 |
| Workspace symbols | Uses indexed aliases, functions, modules, and exports across configured workspaces. | Symbol classes are still smaller than rust-analyzer's project-wide index. | 64 |
| Inlay hints | Useful local and parameter hints exist. | No full expression-chain or flow-sensitive hint model. | 61, 62, 65 |
| Code actions | Mechanical fixes exist for a focused set of diagnostics and LOVE helper assists. | No larger assist framework, refactor menu, imports organizer, or bulk fixes. | 66, 70.9 |

## Missing And Planned

| Feature | Current Tua behavior | Expected direction | Roadmap |
| --- | --- | --- | --- |
| Deeper analysis precision | A smaller snapshot/database exists, but some precision work still falls back to conservative dynamic results. | Keep moving semantic facts into shared snapshot queries instead of feature-specific paths. | 62, 70.10 |
| LuaHelper parity fixtures and code inventory | Coverage exists through focused tests, and the checked-in LuaHelper code matrix inventories current diagnostic/tag parity. | Add representative fixtures for annotations, builtin stubs, watched-file reindexing, references, completion resolve, signature docs, and lints. | 49, 52, 70.16 |
| Flow-sensitive narrowing | Equality and obvious mismatches are checked, but control-flow typing is shallow. | Narrow unions and known values through branches where static proof is reliable. | 62 |
| Rich project edge cases | Project roots, require paths, ignored paths, and targets are modeled. | Add more duplicate-candidate, cycle, generated-source, and multi-root regression coverage as real projects expose gaps. | 55, 59 |
| Profiling expansion | Opt-in tracing covers parse/check/index/request work, with small/medium/large LSP benchmark fixtures and cache stats reporting. | Expand profiling only when new hot paths appear. | 69 |
| Documentation refresh after architecture work | Docs now match the current Phase 12 architecture surface. | Refresh again when later LOVE domain extensions or LuaLS compatibility work changes user-visible behavior. | 50, 51, 70 |

## Intentionally Out Of Scope

| Feature | Reason |
| --- | --- |
| Full LuaHelper clone compatibility | Tua should match useful Lua project workflows, not replicate every LuaHelper diagnostic, setting, or internal behavior. |
| Runtime execution or require side effects | Tua remains static source tooling. It should not run project code to discover symbols. |
| Lua debugger/profiler integration | Debugging is outside the compiler/LSP v1 scope. External Lua/LOVE debuggers can be used separately. |
| Package manager ownership | Tua should understand project files and require paths, but package management is not part of the language server. |
| Roblox/Luau runtime compatibility | Luau is a reference point for typed Lua ideas; Tua targets Lua 5.1/LuaJIT/LOVE output. |
| Arbitrary dynamic table/metatable proof | Dynamic tables, metatables, `_G`, and `any` remain gradual escape hatches unless future specs define narrower support. |
| Whole-program optimization or minification | Emitted Lua should stay readable and LuaJIT-compatible rather than optimized or minified by the LSP/toolchain. |

## Maintenance Rules

- Move a row only when implementation and tests have landed.
- Keep the Tua column factual and tied to checked-in behavior.
- Every planned gap should name a roadmap item.
- Keep diagnostic-level LuaHelper parity claims synchronized with
  [LuaHelper Code Matrix](luahelper-code-matrix.md).
- If a LuaHelper behavior is rejected, move it to **Intentionally Out Of
  Scope** with a short reason.

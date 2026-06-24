# LSP Parity Matrix

This matrix tracks Tua's current LSP behavior against LuaHelper-style Lua
tooling and useful rust-analyzer patterns. It is intentionally conservative:
the Tua column describes checked-in behavior, not planned behavior.

Roadmap numbers refer to the [roadmap](roadmap.md), which points at the
canonical root `ROADMAP.md`.

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
| Diagnostics for `.tua` | Parser, checker, asset, and lint diagnostics publish for open `.tua` documents. | Comparable base diagnostic loop, with fewer Lua-specific checks than LuaHelper. | 36, 37, 48.6 |
| LuaDoc annotation parsing | Parses `---@class`, `---@field`, `---@alias`, `---@type`, `---@param`, `---@return`, `---@generic`, and `---@version`. | LuaHelper has a broader annotation ecosystem, but the core tags exist. | 33 |
| Annotation-backed type environment | Supported annotations can provide aliases, function types, locals, docs, and simple module exports. | LuaHelper supports more patterns and library stubs. | 34 |
| Hover | Shows aliases, locals, parameters, functions, table fields, Lua builtins, LOVE items, require literals, module bindings, annotation docs, and inferred returns. | Mature tools attach richer docs and more precise project context. | 40 |
| Completion | Completes aliases, locals, fields, Lua builtins, LOVE items, LuaDoc tags/types/snippets, require paths, required-module fields, and auto-import candidates. | LuaHelper has deeper dynamic Lua/project completions. | 38, 45 |
| Completion resolve | Completion items can resolve documentation/details from stable item data. | Mature tools use deeper incremental databases, but resolve is no longer just eager payload reuse. | 39 |
| Signature help | Shows known function signatures, Lua/LOVE builtins, required-module functions, active parameters, and supported parameter docs. | Comparable shape, shallower type inference. | 41 |
| Go to definition | Resolves aliases, locals, functions, parameters, typed table fields, require literals, LOVE virtual stubs, and required-module exports. | Mature tools support more dynamic module patterns. | 22, 31 |
| Document links | Resolved `require("...")` literals become module links. | Comparable Lua module navigation. | 31 |
| Document symbols | Type aliases, functions, annotated locals, parameters, table fields, and simple returned exports appear in file outline. | Comparable file outline basics. | 23 |
| Document highlights | Known symbols can be highlighted within the active document. | Comparable local highlight behavior. | 31 |
| References | Finds same-file references and project-wide references for supported globals, module exports, aliases, and required-module callers. | Still less complete than LuaHelper for dynamic fields/metatables. | 42 |
| Rename | Supports same-file rename and safe cross-file rename for project-identity symbols such as exported functions and type aliases. | Dynamic globals, `any`, and ambiguous fields are rejected. | 43 |
| Workspace symbols | Scans configured workspaces for modules, aliases, and functions with fuzzy matching. | Not a persistent rust-analyzer-style symbol index yet. | 24, 63 |
| Code actions | Provides conservative fixes for missing require targets, inferred local annotations, unused locals, duplicate table keys, and self-assignments. | Smaller action set than rust-analyzer assists. | 44 |
| Auto-import completion | Unresolved exported functions can complete with a require insertion or reuse of an existing require alias. | Narrower than rust-analyzer auto-imports, but the workflow exists. | 45 |
| Call hierarchy | Incoming and outgoing calls work for known local functions, module-exported functions, and typed receiver methods. | Dynamic calls through `any` are ignored instead of guessed. | 48 |
| Inlay hints | Provides local type hints and parameter-name hints for supported known expressions. | Shallower than rust-analyzer expression-chain hints. | 60, 64 |
| Semantic tokens | Covers aliases, functions, locals, parameters, fields, builtins, unresolved references, globals, module names, methods, annotation tags, and modifiers. | Core semantic classes are present; deep dynamic Lua remains limited. | 47 |
| Lua and LOVE builtin surface | Curated Lua catalog plus generated LOVE 11.x catalog provide hover, completion, signatures, object method signatures, and selected return types. | LuaHelper commonly uses richer annotation stubs. | 35, 48.5 |
| LOVE asset helpers | `image "..."` and `sound "..."` infer `Image`/`Source`, emit ordinary LOVE calls, and validate obvious paths/extensions. | Tua-specific LOVE-focused workflow. | 48.6 |
| Workspace folders and watchers | Workspace folders, folder changes, watched files, and `tua.toml` changes affect diagnostics and editor results. | Similar shape to mature LSP servers, with simpler caching. | 32, 46 |

## Protocol Surface Supported But Shallow

| Feature | Tua today | Gap | Roadmap |
| --- | --- | --- | --- |
| Analysis architecture | A shared analysis snapshot exists, and many requests reuse it. | It is not yet a rust-analyzer-style dependency-tracked query database with a VFS/HIR split. | 53-65 |
| LuaDoc coverage | Core tags are parsed and lowered. | No complete LuaLS compatibility, no full standard-library stub ingestion, and limited generic behavior. | 35, 49 |
| Type system | Supports primitives, `any`, aliases, unions, function types, table shapes, optional fields, selected inference, and multi-return paths. | No generics in Tua syntax, no metatable typing, no deep flow typing, and dynamic shape changes remain permissive. | 61, 62 |
| Required Lua module typing | Simple returned module tables and supported LuaDoc annotations contribute exports. | Complex runtime module construction, side effects, and metatable exports are not modeled. | 59, 61 |
| Workspace symbols | Scans files on request and supports multiple folders. | No durable symbol index or rich export/field/global symbol classes yet. | 64 |
| Inlay hints | Useful local and parameter hints exist. | No full expression-chain or flow-sensitive hint model. | 61, 62, 65 |
| Code actions | Mechanical fixes exist for a focused set of diagnostics. | No larger assist framework, refactor menu, imports organizer, or bulk fixes. | 66 |
| Selection/folding ranges | Not part of the current editor surface. | Would round out rust-analyzer-style navigation/editing ergonomics. | 67 |

## Missing And Planned

| Feature | Current Tua behavior | Expected direction | Roadmap |
| --- | --- | --- | --- |
| Full analysis database | Requests still rely on a simpler snapshot and helper APIs. | Add VFS, open-file overlays, HIR-like lowering, require graph, dependency-tracked queries, and shared IDE routing. | 53-65 |
| LuaHelper parity fixtures | Coverage exists through focused tests, but not a dedicated LuaHelper parity suite. | Add representative fixtures for annotations, builtin stubs, watched-file reindexing, references, completion resolve, signature docs, and lints. | 49, 52 |
| Flow-sensitive narrowing | Equality and obvious mismatches are checked, but control-flow typing is shallow. | Narrow unions and known values through branches where static proof is reliable. | 62 |
| Rich project model | Source roots and require behavior are pragmatic. | Model roots, require paths, duplicate candidates, cycles, ignored paths, and target versions more explicitly. | 55, 59 |
| Analysis tracing and benchmarks | No dedicated performance tracing. | Add profiling for parse/check/index/request work. | 69 |
| Documentation refresh after architecture work | Docs now match the current feature surface. | Refresh again after the analysis database and HIR phases land. | 50, 51, 70 |

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
- If a LuaHelper behavior is rejected, move it to **Intentionally Out Of
  Scope** with a short reason.

# LSP Parity Matrix

This matrix tracks Tua's current LSP behavior against LuaHelper-style Lua
tooling and useful rust-analyzer patterns. It is intentionally conservative:
the Tua column describes what the checked-in implementation does today.

Roadmap numbers refer to [ROADMAP.md](../ROADMAP.md).

## Status Legend

- **Supported**: implemented and covered by current core or LSP behavior.
- **Shallow**: the LSP protocol surface exists, but the implementation is
  narrower than LuaHelper-style tooling or rust-analyzer-style IDE behavior.
- **Planned**: not implemented yet and assigned to a roadmap commit.
- **Out of scope**: deliberately not planned for Tua's current direction.

## Already Supported

| Feature | Tua today | LuaHelper comparison | rust-analyzer pattern | Roadmap |
| --- | --- | --- | --- | --- |
| Stdio language server | `tua lsp` serves JSON-RPC over stdio. | Standard editor integration shape. | `rust-analyzer` binary speaks LSP over stdio. | Landed before 30 |
| Incremental document sync | Open, change, save, and close notifications update open-document diagnostics. | Basic live diagnostics behavior. | Open-file overlays are expected. | Deepen with 31 |
| Diagnostics for `.tua` | Parser, checker, and lint diagnostics publish for open `.tua` documents. | Comparable base diagnostic loop, but fewer Lua-specific checks. | IDE diagnostics from the analysis database. | Expand with 36, 37 |
| Hover | Shows known aliases, locals, parameters, functions, table fields, Lua builtins, LOVE items, require literals, module bindings, and some inferred returns. | LuaHelper also attaches richer comment docs and annotation docs. | Hover combines type, docs, and source context. | Enrich with 40 |
| Completion | Completes aliases, locals, table fields, Lua builtins, LOVE items, require paths, and required-module fields. | LuaHelper also completes annotation tags, snippets, and richer module symbols. | Completion is driven by an indexed semantic model. | Expand with 38, 39, 45 |
| Signature help | Shows signatures for known functions, Lua/LOVE builtins, and known required-module functions. | LuaHelper can include parameter docs from comments. | Signature help carries docs and active-parameter context. | Expand with 41 |
| Go to definition | Resolves same-file aliases, locals, functions, parameters, typed table fields, require literals, and known module-export fields. | LuaHelper supports broader Lua project navigation. | Go to definition/declaration/type definition are core navigation paths. | Deepen with 31, 42 |
| Document links | `require("...")` literals become module links when resolvable. | Comparable to Lua module navigation. | File/module links are part of navigation support. | Deepen with 31, 32 |
| Document symbols | Type aliases, functions, annotated locals, parameters, table fields, and simple returned exports appear in file outline. | Comparable file outline basics. | File structure powers outline and breadcrumbs. | Landed before 30 |
| Document highlights | Known symbols can be highlighted within the active document. | Comparable local highlight behavior. | Highlight references for symbol under cursor. | Deepen with 31, 42 |
| Lua and LOVE builtin surface | Curated builtin catalog provides hover, completion, and signature help. | LuaHelper commonly uses annotation stubs for this. | Standard-library knowledge comes from indexed metadata. | Replace/adapt with 35 |
| Require path completion | Offers module path completions from resolvable project files. | Comparable Lua module workflow. | Project model drives path/module completion. | Deepen with 31, 32 |
| Workspace folders | Initialize and workspace-folder change notifications are accepted. | Basic multi-root editor support. | Workspace roots feed project discovery. | Deepen with 31, 46 |
| Watched files registration | Dynamically registers watchers for `**/*.tua`, `**/*.lua`, and `**/tua.toml` when the client supports it. | Comparable editor watcher setup. | File watching invalidates indexed state. | Deepen with 32 |
| Workspace configuration changes | Handles `workspace/didChangeConfiguration` for diagnostics, file associations, ignored paths, require path separator, and preview settings. | Comparable editor-settings overlay behavior. | Editor/project config feeds the analysis host without restarting the server. | Landed with 46 |

## Protocol Surface Supported But Shallow

| Feature | Tua today | Gap | LuaHelper / rust-analyzer target | Roadmap |
| --- | --- | --- | --- | --- |
| Project analysis model | Requests call parser/checker helpers directly and may re-read required modules independently. | No shared project snapshot or dependency-aware cache. | rust-analyzer-style analysis snapshot/query database. | 31 |
| Watched-file reactions | File changes republish diagnostics for affected open files and disk `.tua` changes. | No durable module/export index to update after create/delete/change. | Watch changes invalidate only affected project data. | 32 |
| References | `textDocument/references` is advertised, but reference search is same-file and depends on current parsed source. | No project-wide references for module exports or required fields. | LuaHelper/rust-analyzer-style project reference search. | 42 |
| Rename | `prepareRename` and `rename` work for same-file aliases, locals, functions, and parameters. | No cross-file rename, no module export rename, no field rename. | Safe workspace edits for symbols with project identity. | 43 |
| Workspace symbols | Scans configured `.tua` files on request and returns modules, aliases, and functions. | No persistent index, no Lua files, no fields/exports/globals, limited fuzzy metadata. | Indexed fuzzy symbol search. | 31, 63 |
| Completion resolve | `completionItem/resolve` returns information already attached to completion data. | It is not truly lazy and cannot fetch fresh docs/details from a snapshot. | Lazy resolve from stable item data. | 39 |
| Hover documentation | Hover shows compact type/signature labels. | No doc comments, annotation docs, declaration path, or rich module context. | Hover combines type, docs, and declaration context. | 40 |
| Signature documentation | Signature help has labels and active parameters. | No parameter documentation from comments or annotations. | Parameter docs attached to signatures. | 41 |
| Inlay hints | Type and parameter hints exist for supported inferred/known cases. | No deeper expression-chain type hints or project-aware hints. | rust-analyzer-style local type, argument, and expression hints. | 60, 64 |
| Semantic tokens | Compact legend covers type aliases, functions, variables, parameters, properties, builtin types, unresolved references, and declarations. | No globals, modules, methods, annotation tags, deprecated/readonly modifiers, or richer token classes. | LuaHelper/rust-analyzer-style semantic token detail. | 47 |
| Required Lua module typing | Tua can infer simple exports from `.lua` modules in some hover/definition paths. | No LuaHelper annotation parsing, comment docs, or typed Lua stub ingestion. | Annotated Lua modules contribute full type info. | 33, 34 |

## Missing And Planned

| Feature | Current Tua behavior | Expected parity direction | Roadmap |
| --- | --- | --- | --- |
| LuaHelper doc annotation parser | `---@...` comments are ordinary comments. | Parse `---@class`, `---@field`, `---@alias`, `---@type`, `---@param`, `---@return`, `---@generic`, and related docs into a source model. | 33 |
| Annotation-backed type environment | `.lua` comments do not become typed declarations. | Lower parsed annotations into aliases, function types, globals, fields, and module exports. | 34 |
| Standard-library annotation stubs | Builtins are represented by a curated Rust catalog. | Ingest Lua/LuaJIT-compatible annotation stubs as the source of truth where possible. | 35 |
| Undefined and unused diagnostics | Some correctness diagnostics exist, but undefined/unused symbol analysis is not implemented. | Configurable undefined variables, unused locals, unused parameters, and use-before-definition checks. | 36 |
| LuaHelper-style lint depth | Current lints are smaller and targeted. | Add duplicate params, bad require files, duplicate conditions, duplicate operands, float equality, const assignment, and richer type mismatch checks. | 37 |
| Annotation and doc snippet completions | No `---@` tag completion or generated doc snippets. | Complete annotation tags/types and generate function-doc snippets. | 38 |
| Project-wide references | Same-file reference search only. | Use the shared project snapshot to find references across indexed workspace files. | 42 |
| Safe cross-file rename | Same-file rename only. | Workspace edits for project-identity symbols such as exported functions and aliases. | 43 |
| Code actions / quick fixes | `codeActionProvider` is not advertised. | Conservative mechanical fixes for selected diagnostics. | 44 |
| Auto-import completion | Unresolved identifiers do not propose imports. | Offer exported module symbols and insert or reuse `require` bindings. | 45 |
| Richer semantic tokens | Compact token legend only. | Add globals, module names, methods, fields, annotation tags, readonly/deprecated modifiers, and unresolved categories. | 47 |
| Call hierarchy | Call hierarchy is not advertised. | Show incoming/outgoing calls for known functions and module exports. | 48 |
| LuaHelper parity fixtures | No dedicated parity fixture suite. | Add representative fixtures for annotations, builtin stubs, watched-file reindexing, references, completion resolve, signature docs, and lints. | 49 |
| Post-milestone doc refresh | Current docs track the pre-parity implementation. | Update docs and non-goals after parity commits land. | 50 |

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

- Move a row from **Planned** or **Shallow** only when the implementation and
  tests have landed.
- Keep the Tua column factual and tied to current behavior.
- Every planned gap must name a roadmap commit number.
- If a LuaHelper behavior is rejected, move it to **Intentionally Out Of
  Scope** with a short reason.

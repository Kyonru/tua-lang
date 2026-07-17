---
icon: lucide/list-checks
---

# Capability Matrix

This matrix is the concise source of truth for current language and editor
surface. Detailed LuaHelper comparisons remain in the
[LSP Parity Matrix](lsp-parity.md) and
[LuaHelper Code Matrix](luahelper-code-matrix.md).

Status meanings:

- **Supported**: implemented and covered by current tests.
- **Partial**: useful common cases work, but dynamic or advanced cases remain.
- **Preview**: implemented behind explicit preview configuration.
- **Opt-in**: disabled by default and enabled through configuration.
- **Deferred**: intentionally not in the current release surface.
- **Out of scope**: not planned for Tua's current direction.

## Language And Compiler

| Capability | Status | Current contract |
| --- | --- | --- |
| Lua 5.1/LuaJIT syntax and emission | Supported | Tua emits readable Lua compatible with LuaJIT and LOVE 11.x. |
| Local, parameter, return, and field annotations | Supported | Annotations are checked and erased. |
| Aliases, table shapes, optional fields, unions, literals | Supported | Structural checking is available where stable types are known. |
| Arrays, dictionaries, tuples, and typed loops | Supported | Common indexing, `pairs`, and `ipairs` inference is implemented. |
| Native function types and typed varargs | Supported | Native and LuaDoc function forms share one semantic type. |
| Default parameters and compound assignments | Supported | Lowered to readable Lua with source provenance. |
| Classes, inheritance, final members, and enums | Supported | Class/runtime identity and enum values are checked and lowered. |
| Runtime `is` narrowing | Partial | Primitive, supported alias, class, and known metatable checks narrow; arbitrary structural runtime proof is rejected. |
| Inferred local reassignment contracts | Supported | Tua locals retain stable widened types; explicit `any` remains dynamic. |
| LuaDoc/LuaCATS interoperability | Partial | Practical aliases, classes, overloads, function types, varargs, docs, and module exports work; the full LuaLS annotation ecosystem is larger. |
| Static project modules | Supported | `require` and static imports resolve through the project model; imports lower to `require`. |
| LOVE 11.x catalog | Supported | Generated overloads, enums, callback snippets/validation, methods, docs, and inheritance drive semantics. |
| Asset helpers and metadata | Supported | Known images, sounds, fonts, shaders, and related assets support validation, casing checks, usage inventory, navigation, references, and path-preserving file rename edits. |
| Vec2, SignalBus, and object-pool helpers | Supported | Ordinary APIs emit vendored helper modules only when used; typed pool factories preserve acquired item types, and visible same-file unreleased acquires produce conservative lifecycle warnings. |
| Sprite-sheet semantics | Preview | `spriteSheet({...})` analysis and anim8 emission work; editor preview remains gated. |
| ECS semantics | Supported | Zero-setup built-in runtime, typed world/component/query/system APIs, and static component diagnostics are enabled by default. |
| Generics | Deferred | Concrete function types and container behavior take priority. |
| Executable third-party plugins | Out of scope | Domain behavior stays first-class until a stable trust and ownership model exists. |

## Editor And Tooling

| Capability | Status | Current contract |
| --- | --- | --- |
| Push and pull diagnostics | Supported | Fast current-file diagnostics precede debounced project diagnostics; stale work is discarded. |
| Completion and completion resolve | Supported | Compiler-valid locals, fields, modules, Lua, and LOVE candidates are ranked deterministically. |
| Project-adaptive completion ranking | Supported | Privacy-safe local frequency and structural statistics stay on-device and can be reset. |
| Contextual sequence and multi-token completion | Opt-in | Bounded local lanes preserve compiler completion and fail back without user-visible errors. |
| Hover, signature help, and type explanations | Supported | Includes Tua, LuaDoc, module, LOVE overload, asset, SignalBus, object-pool, and documentation facts. A source-linked Explain Type command reports annotation, inference, widening, narrowing, module, field, and dynamic fallback steps for identifiers and member chains. |
| Definition, type definition, implementation | Supported | Known local, module, field, type, asset, and builtin identities navigate conservatively. |
| References and rename | Supported | Same-file and compiler-identified project symbols are supported; static asset references update when the asset file is renamed, while ambiguous dynamic identities are rejected. |
| Symbols, call hierarchy, links, code lens | Supported | Document/workspace structure and known callers use shared semantic indexes; the VS Code extension can copy module-qualified symbol names and show workspace symbols grouped by module. |
| Formatting and source actions | Supported | Full/range/on-type formatting plus conservative import, inferred annotation, extract local/function, inline local, table-type generation, LOVE callback generation, object-pool, and fix-all actions are available. |
| Syntax highlighting | Supported | An immediate TextMate grammar covers Lua, Tua types/classes/enums, LuaDoc, modules, domain helpers, event names, calls, members, labels, literals, and operators. |
| Semantic tokens and inlay hints | Supported | Full/range/delta tokens classify standard VS Code symbol roles and declaration/readonly/deprecated/modification/default-library modifiers; inferred type/parameter hints are revision-aware, and the extension offers Full, Compact, Types Only, Parameters Only, and Off presentation modes without changing server results. |
| Watched files and workspace folders | Supported | Source, Lua, assets, config, module creation/deletion, and folder changes refresh project state. |
| Language-server health view | Supported | A VS Code command shows the current server revision, warm coverage, queue depth, cache hit rates, stale-result counts, and recent request p95 latency from `tua/health`. |
| Run LOVE project | Supported | VS Code tasks build the active Tua project, run LOVE from the configured output directory, support restart/cancel, and annotate runtime tracebacks with source-map links back to Tua. |
| Sprite animation preview | Preview | A sticky webview renders compiler-inspected local sprite metadata without executing project code. |
| Signal call graph | Supported | The extension visualizes compiler-indexed event calls, filters by event/bus/file/listener/emitter, shows schema conflicts, opens exact locations, and creates typed listener or declaration snippets from semantic metadata. |
| Module dependency explorer | Supported | A compiler-backed graph shows `require`/`import` edges, unresolved modules, cycles, reverse dependencies, exported surfaces, and deterministic root-to-module inclusion paths. |
| ECS Explorer | Supported | A compiler-backed project view lists static components, systems, and queries; infers conservative read/write access; navigates component relationships; and presents allocation patterns as non-diagnostic insights. |
| Object Pool Explorer | Supported | A compiler-backed project view lists pool declarations, item types, lifecycle operations, and conservative unreleased-acquire warnings with source navigation. |
| Asset Explorer | Supported | The Explorer groups compiler-indexed assets by type and static usage, publishes conservative unused-asset hints, opens references, and performs LSP-aware file renames. |
| Debugger and runtime profiler | Out of scope | Tua provides static tooling and source-map traceback remapping, not a Lua debugger. |
| Package management | Out of scope | Tua understands module paths but does not install Lua packages. |
| Network-required language features | Out of scope | Core checking and token completion remain fully local. |

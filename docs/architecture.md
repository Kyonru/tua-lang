---
icon: lucide/workflow
---

# Architecture

Tua is organized around one reusable language engine. The CLI and language
server provide different transports and lifecycles, but both read the same
project model, semantic data, diagnostics, and generated Lua from `tua-core`.

This page documents the current implementation boundaries for contributors.
For language behavior, see the
[Language and Toolchain Reference](specification.md).

!!! warning "Keep semantics in shared analysis"
    A CLI command, LSP handler, or VS Code feature must not recreate parser,
    type, module, or domain knowledge. Add the fact or query to `tua-core`, then
    adapt its result at the edge.

## At A Glance

```mermaid
flowchart LR
  Editor[VS Code] --> Extension[editors/vscode]
  Extension -->|starts tua lsp| CLI[tua executable]
  CLI --> LSP[tua-lsp]
  CLI --> Core[tua-core]
  LSP --> Core
  Core --> Lua[Lua emission and provenance]
  CLI --> Output[Lua files and assets]
```

The implementation follows four rules:

1. **One semantic authority.** Parsing, types, module resolution, diagnostics,
   and editor facts live in `tua-core`.
2. **Revisioned queries.** Mutable project inputs belong to `AnalysisHost`;
   requests read immutable `AnalysisSnapshot` values.
3. **Side effects stay at the edges.** The CLI owns disk output and watch
   events. The LSP owns JSON-RPC and protocol conversion. The extension owns
   VS Code integration.
4. **Dynamic Lua stays conservative.** Static analysis reports what source and
   configured catalogs establish. It does not execute project code or invent
   identities for dynamic values.

## Repository Boundaries

| Path | Owns | Does not own |
| --- | --- | --- |
| `crates/tua-core` | Source models, parser, HIR, scopes, types, checking, linting, diagnostics, project model, VFS, module graph, IDE queries, domain analysis, lowering, and Lua emission | Filesystem watching, JSON-RPC, terminal presentation, or editor UI |
| `crates/tua-cli` | Arguments, command orchestration, project discovery, builds, output files, asset copying, watch mode, source maps, traceback mapping, and terminal diagnostics | Independent language or editor semantics |
| `crates/tua-lsp` | Stdio transport, scheduling, cancellation, document synchronization, settings, capability advertising, URI and position conversion, and LSP request routing | Parsing or inferring source in handlers |
| `editors/vscode` | Language registration, grammar, settings, tasks, extension activation, server discovery, custom views, packaging, and extension tests | Type inference or project analysis in TypeScript |

The Rust dependency direction is intentionally small:

```mermaid
flowchart TD
  CLI[tua-cli] --> Core[tua-core]
  CLI --> LSP[tua-lsp]
  LSP --> Core
```

`tua-core` must remain usable without an editor or command-line process. This
keeps semantic tests fast and lets future integrations reuse the same engine.

## Core Analysis Model

```mermaid
flowchart TD
  Inputs[Config, roots, catalogs, and files] --> Host[AnalysisHost]
  Overlays[Open document overlays] --> Host
  Host --> Snapshot[AnalysisSnapshot at revision N]
  Snapshot --> FileFacts[Per-file semantic facts]
  Snapshot --> ProjectFacts[Project indexes and graphs]
  FileFacts --> Queries[Diagnostics, IDE, and compiler queries]
  ProjectFacts --> Queries
```

### Inputs And Identity

`ProjectModel` interprets workspace configuration: source and output roots,
declaration-only type-library roots and module-prefix mappings, asset roots,
ignored paths, file associations, and module paths. `Vfs` stores the
corresponding analysis files. An open document overlay takes precedence over
the disk copy until the document closes, so editor requests always analyze the
text the user can see.

Paths are normalized before they become analysis identities. Module, symbol,
asset, and reverse-reference indexes should use those normalized identities
instead of retaining ad hoc path strings.

`AnalysisHost` owns mutable inputs and the current revision. Updating a file,
document overlay, project option, or builtin catalog invalidates the affected
cached products and advances that revision.

`AnalysisSnapshot` is an immutable view of one revision. It combines cached
per-file facts with project-wide indexes and exposes stable query APIs to the
CLI and LSP.

Declaration libraries enter the VFS as Lua analysis inputs, but remain outside
source roots, require search roots, asset copying, and emission. During snapshot
construction, `---@meta` identities are mapped to configured runtime
`require(...)` names and added as declaration overlays in the shared module
graph. The type database and editor queries consume that same resolved module
instead of maintaining a separate external-library path.

| Use | API |
| --- | --- |
| Update configuration, roots, catalogs, disk files, or open documents | `AnalysisHost` |
| Serve a fast query against the current file during typing | Current-file `AnalysisHost` query |
| Resolve modules, symbols, references, assets, or project types | `AnalysisSnapshot` query |
| Keep several related reads internally consistent | One shared `AnalysisSnapshot` |

!!! tip "Snapshot once per logical operation"
    A handler or command that needs several project facts should take one
    snapshot and reuse it. Taking snapshots between related reads can mix
    revisions and duplicate project work.

### Per-File Products

The per-file pipeline produces reusable semantic data rather than a separate
parse for every feature:

```mermaid
flowchart LR
  Source --> Normalize[Normalize syntax]
  Normalize --> Parse[Parsed source]
  Parse --> HIR[HIR and scopes]
  HIR --> Types[Type environment]
  Types --> Summary[Exports, usages, assets, and domain summaries]
  Types --> Diagnostics[Check and lint diagnostics]
```

A successfully parsed file has one semantic bundle containing its HIR, scope
map, and base type environment. Other per-file products include module exports
and require edges, symbols, member usages, asset references, and compact
summaries for Signals, ECS, and object pools.

Temporarily invalid text follows the shared recovery path. The lossy parse and
degraded environment are cached for that snapshot so hover, completion, and
other typing features can keep working without polluting the valid semantic
cache.

!!! warning "Do not add feature-specific fallback parsing"
    If an editor feature fails on incomplete source, improve the shared parser
    recovery or degraded semantic facts. A private fallback environment inside
    completion, hover, or an LSP handler will drift from the checker.

### Project Products

Snapshots aggregate per-file summaries into project-aware structures:

- `ModuleGraph` connects imports and exports.
- `ProjectTypeDatabase` resolves exported and cross-file types.
- Symbol and usage indexes support workspace symbols, references, rename,
  call hierarchy, and ranking.
- Asset indexes support validation, completion, hover metadata, and rename.
- Signal, ECS, and object-pool graphs support diagnostics and structured
  inspector views.

Project indexes retain compact facts rather than duplicating all project source
text. Reverse relationships are derived from indexed files and reused by
navigation queries.

### Query Surface

The public analysis API is the integration boundary. A minimal core consumer
looks like this:

```rust
use std::path::PathBuf;
use tua_core::{AnalysisHost, AnalysisSourceKind, SourcePosition};

let path = PathBuf::from("src/main.tua");
let mut host = AnalysisHost::new();
host.set_source_roots([PathBuf::from("src")]);
host.set_file_with_kind(
    path.clone(),
    "local score: number = 0\n",
    AnalysisSourceKind::Tua,
);

let snapshot = host.snapshot();
let hover = snapshot.hover(&path, SourcePosition { line: 1, column: 8 });
```

Snapshot queries cover diagnostics, completion, hover, signature help,
inlay hints, semantic tokens, symbols, links, code actions, navigation,
references, rename, call hierarchy, module graphs, assets, type explanations,
and domain inspectors. Prefer extending that surface over exposing internal
parser or cache types to another crate.

## Editor Request Flow

```mermaid
sequenceDiagram
  participant E as Editor
  participant L as tua-lsp
  participant H as AnalysisHost
  participant S as AnalysisSnapshot
  participant C as tua-core query

  E->>L: didOpen / didChange
  L->>H: update document overlay
  H-->>L: new revision
  E->>L: completion / hover / definition
  alt file-local request
    L->>H: run current-file query
    H->>C: read cached file facts
    C-->>L: core result
  else project-aware request
    L->>H: request snapshot
    H-->>S: freeze revision
    S->>C: read semantic and project facts
    C-->>L: core result
  end
  L-->>E: LSP result
```

The server separates transport and scheduling from language work. Interactive
requests use the responsive worker path, while indexing and diagnostics can be
debounced or warmed in the background. Current-file diagnostics can appear
quickly during typing; project-aware diagnostics follow when the relevant
snapshot is ready.

Results are associated with document versions and analysis revisions. The
server discards stale work instead of publishing it over newer text. Cancellation
is cooperative at the scheduling boundary: a core query that already started
may finish, but its stale result must not be applied.

LSP handlers should have a narrow shape:

1. Convert URI, position, and settings into core inputs.
2. Select a current-file query or take one snapshot.
3. Call a typed `tua-core` query.
4. Convert the core result into an LSP response.

If a handler starts parsing text, resolving a module, or constructing a type,
the behavior belongs lower in the stack.

## Build And Watch Flow

```mermaid
flowchart LR
  Config[Project config] --> Discover[CLI source discovery]
  Discover --> Host[AnalysisHost]
  Host --> Check[Core diagnostics]
  Check --> Emit[Core lowering and LuaEmission]
  Emit --> Write[CLI output and source maps]
  Assets[Asset roots] --> Copy[CLI asset copy]
  Copy --> Write
```

The build path is split by responsibility:

1. The CLI loads configuration, discovers project files, and updates one
   `AnalysisHost`.
2. Core analysis checks the same sources and dependencies used by editor
   features.
3. Core lowering returns `LuaEmission`: generated Lua plus source provenance.
4. The CLI writes Lua, helper runtimes that are actually required, assets, Tua
   provenance maps, and standard debugger-compatible source maps.
5. Runtime traceback mapping consumes compiler provenance instead of guessing
   line offsets from generated text.

The VS Code LOVE debug command consumes those build artifacts rather than
adding debugger semantics to the compiler or language server. It launches an
external Lua debug adapter against generated Lua and lets the adapter resolve
Tua breakpoints and stack frames through the standard maps.

Watch mode keeps a long-lived host while project configuration remains stable.
File events update the VFS, invalidate affected importers, and rebuild from the
new snapshot. Filesystem watching and output suppression stay in `tua-cli`;
semantic invalidation stays in `tua-core`.

## Semantic Ownership

Use this map when deciding where a change belongs:

| Concern | Primary modules |
| --- | --- |
| Surface syntax, normalization, comments, and spans | `parser`, parser source models |
| Declarations, expressions, scopes, and semantic identity | `hir`, `scope`, `semantic_index` |
| Type representation, inference, assignability, and project exports | `types`, `type_database` |
| User-facing errors, warnings, inline ignores, and lints | `check`, `diagnostic`, `inline_ignore`, `lint` |
| Roots, files, module paths, imports, and project identity | `project_model`, `vfs`, `module_graph`, `module_index`, `module_link` |
| Revisioned inputs, cached products, recovery, and public queries | `analysis` |
| Completion, hover, navigation, symbols, refactors, and editor data | Corresponding `tua-core` feature module, exposed through `analysis` |
| Lowering, helper selection, generated Lua, and provenance | `emit` |
| LOVE, assets, sprites, Signals, stores, input, ECS, pools, and vectors | Shared domain modules and analysis queries |

Domain support follows the same architecture as language support. LOVE and Lua
catalogs contribute typed facts; asset and gameplay helpers contribute indexed
summaries; editor features consume those facts through shared queries. Custom
VS Code views render structured results but do not derive them.

## Architecture Invariants

These are review requirements, not future goals:

- Core IDE queries do not read files directly from disk.
- LSP handlers and extension code do not duplicate semantic analysis.
- Diagnostics use the shared diagnostic model and stable Tua codes.
- A snapshot never observes half of a newer revision.
- Invalid-source support uses shared recovery rather than one-off parsers.
- Cross-file features use normalized project identities and indexes.
- Dynamic project code is never executed to answer an editor query.
- Generated output remains portable Lua 5.1-compatible code with source
  provenance.
- Optional helper runtimes are emitted only when lowering requires them.

The external tooling bridge is not implemented. Its design is documented in
[External API Bridge](external-api-bridge.md); an implementation must remain
local-only, disabled by default, and backed by the same snapshot queries.

## Change Workflows

### Add Or Change Language Behavior

1. Update parser normalization and source models where syntax changes.
2. Represent declarations and expressions in shared HIR and scope data.
3. Implement type behavior in the shared environment and type database.
4. Add checker diagnostics and lowering from the same semantics.
5. Expose reusable facts through analysis queries for editor features.
6. Add focused parser, checker, emitter, and analysis tests, then update the
   language documentation and examples.

### Add An IDE Or LSP Feature

1. Define a protocol-neutral result type and query in `tua-core`.
2. Reuse semantic files, project indexes, and shared resolvers.
3. Add focused core tests for valid, incomplete, and cross-file source.
4. Add a thin `tua-lsp` conversion and handler with protocol tests.
5. Change `editors/vscode` only when VS Code needs custom UI or configuration.

### Add A Domain Inspector

1. Extract a compact summary while analyzing each file.
2. Aggregate summaries in a revisioned snapshot graph.
3. Expose structured graph data through a core query.
4. Route a custom LSP request only if standard LSP cannot represent it.
5. Keep rendering and interaction in the extension.

### Change Build Or Watch Behavior

Put language lowering and provenance in `tua-core`. Put source discovery,
filesystem events, generated-file writes, asset copying, and terminal reporting
in `tua-cli`. Test invalidation with an existing host rather than rebuilding a
new analysis world for every event.

## Verification

Test at the layer that owns the behavior, then run the integration boundary it
affects:

| Change | Focused verification |
| --- | --- |
| Parser, types, diagnostics, IDE facts, or emission | `cargo test -p tua-core` |
| LSP routing, freshness, protocol conversion, or scheduling | `cargo test -p tua-lsp` |
| CLI commands, builds, source maps, or watch mode | `cargo test -p tua-cli` |
| Real editor and closed-file behavior | `make editor-regression-smoke` |
| Documentation structure and examples | `make docs-build` |
| Main repository checks | `make verify` |

Use the smallest focused test while iterating. Before merging a cross-layer
change, include the relevant integration test and run `make diff-check`.

## Non-Goals

- Tua does not run user code to improve static analysis.
- Tua does not make the VS Code extension a second language engine.
- Tua does not require an LSP process to compile a project.
- Tua does not optimize architecture around giant-workspace machinery before
  measurements justify the complexity.
- Tua does not hide generated output behind an opaque runtime; emitted Lua is a
  readable, debuggable build artifact.

## Related Guides

- [Language and Toolchain Reference](specification.md)
- [Configuration](configuration.md)
- [Lua Interoperability](lua-interop.md)
- [Diagnostics](luahelper-code-matrix.md)
- [Completion Benchmarks](completion-benchmarks.md)
- [External API Bridge](external-api-bridge.md)

---
icon: lucide/workflow
---

# Architecture

This page is the contributor guide for Tua's compiler and language-server
architecture. It also maps rust-analyzer's crate architecture to Tua's current
shape and the Phase 12 roadmap. The goal is not to copy rust-analyzer directly.
The goal is to borrow boundaries that make a Lua/LOVE-sized language tool
faster, easier to reason about, and less likely to grow one-off feature paths.

Language behavior belongs in the root
[SPEC](https://github.com/Kyonru/tua-lang/blob/main/SPEC.md). Commit-sized
planning belongs in the root
[ROADMAP](https://github.com/Kyonru/tua-lang/blob/main/ROADMAP.md). This page
explains where code belongs and how data should flow through the implementation.

Reference point: the current rust-analyzer `crates/` tree includes architecture
families such as `base-db`, `ide-db`, `vfs`, `vfs-notify`, `project-model`,
`parser`, `syntax`, `hir`, `hir-def`, `hir-expand`, `hir-ty`, `ide`,
`ide-assists`, `ide-completion`, `ide-diagnostics`, `test-fixture`,
`test-utils`, and `profile`. The [gap matrix](#gap-matrix) near the end of this
guide maps those ideas to Tua's smaller architecture.

## Current Tua Shape

```mermaid
flowchart TD
  CLI[tua-cli] --> Core[tua-core]
  LSP[tua-lsp] --> Core
  VSCode[editors/vscode] --> LSP

  Core --> Parser[parser]
  Core --> Types[types and SemanticModel]
  Core --> Check[check and lint]
  Core --> Emit[emit]
  Core --> IDE[hover, completion, definition, references, rename, symbols]
  Core --> Project[module index and workspace helpers]

  CLI --> Build[build, check, watch, init]
  LSP --> Protocol[LSP protocol conversion and server state]
```

The current implementation already has a broad feature surface: diagnostics,
hover, completion, signature help, go to definition, references, rename,
document symbols, workspace symbols, document links, code actions, call
hierarchy, inlay hints, semantic tokens, LuaDoc support, LOVE helpers, and
watched-file updates.

The main architectural gap is not missing editor surface. It is that too much
semantic knowledge still lives in helper APIs and feature-specific paths instead
of a single dependency-tracked analysis model.

## Crates And Ownership

`crates/tua-cli` owns command-line behavior: argument parsing, project
initialization, `check`, `build`, `watch`, process exit codes, and user-facing
diagnostic printing. It should call `tua-core` for language work. It should not
implement parser, type, module, or IDE behavior locally.

`crates/tua-core` owns the language implementation. Parser, source models, HIR,
type representation, type checking, diagnostics, lowering, emission, project
modeling, VFS, module graphs, LOVE/Lua catalogs, and pure IDE queries all live
here. When a behavior must be shared by CLI and LSP, it belongs in this crate.

`crates/tua-lsp` owns JSON-RPC/LSP transport, server state, capability
advertising, protocol conversion, and workspace/document notifications. Request
handlers should mostly convert LSP positions and URIs into core query inputs,
read an `AnalysisSnapshot` or a current-file `AnalysisHost` query, then convert
core results back into LSP shapes.

`editors/vscode` owns editor packaging: language registration, grammar,
extension activation, bundled/server binary discovery, debug launch tasks,
configuration settings, and VS Code smoke scripts. It should not duplicate core
language rules in TypeScript.

External tooling that needs structured project facts should follow the
[external API bridge design](external-api-bridge.md). That bridge is not
implemented today; any future implementation should stay disabled by default,
local-only, and backed by the same `AnalysisHost` / `AnalysisSnapshot` queries as
CLI and LSP features.

## Analysis Data Flow

Project-aware feature requests should follow the same path:

1. The CLI or LSP updates `AnalysisHost` inputs: project config, workspace
   folders, source roots, ignored paths, disk files, open-document overlays, and
   builtin catalogs.
2. The caller takes an immutable `AnalysisSnapshot`.
3. The snapshot reads normalized files through the VFS and project model.
4. Cached per-file queries parse source, extract typed source models, lower HIR,
   build scopes, collect diagnostics, collect exports, and resolve type data.
5. Project queries build require edges, module exports, symbol indexes,
   project-aware type information, and lazy reverse-reference indexes from
   indexed files.
6. IDE queries such as hover, completion, definition, references, rename,
   inlay hints, semantic tokens, document symbols, call hierarchy, and code
   actions read from the snapshot instead of rereading files or rebuilding
   project state during a request.

Request handlers should be deterministic. If two handlers need the same
semantic fact, that fact should move into `tua-core` or the analysis snapshot
instead of being recomputed independently.

Current-file IDE requests that do not need project indexes should stay on the
lighter `AnalysisHost` path. Document symbols, document links, document
highlights, selection ranges, folding ranges, and semantic tokens can reuse the
same cached per-file analysis without building the module graph, project type
database, workspace symbol index, or call-hierarchy maps. This keeps editor
startup and typing-adjacent requests responsive while preserving the snapshot
path for project-aware features.

## Where Code Belongs

Parser and syntax recovery code belongs under `crates/tua-core/src/parser`.
Keep raw syntax parsing, Tua normalization, typed source extraction, LuaDoc
annotation parsing, and source-span utilities separate enough that incomplete
editor text can still produce best-effort IDE results.

Type representation and assignability belongs under `crates/tua-core/src/types`.
Project-aware type lookup belongs in the type database and should use module
aliases, require edges, LuaDoc annotations, LOVE/Lua catalogs, and HIR symbols
through the snapshot. Avoid adding hover-only or completion-only type logic.

Diagnostics belong in the shared diagnostic/check/lint pipeline. Parser,
project, type, lint, asset, and emission diagnostics should use stable codes and
flow through the same CLI and LSP formatting paths.

Emission belongs under `crates/tua-core/src/emit` and build orchestration belongs
in CLI build code. The emitter should preserve readable Lua 5.1/LuaJIT output
and should not depend on LSP-only concepts.

Project and file identity code belongs in the project model, VFS, module graph,
and analysis host. Do not resolve `require(...)`, static `import(...)`,
ignored paths, output roots, or open unsaved buffers separately in feature
handlers.

IDE features should have a pure core query first and a protocol conversion
layer second. For example, go-to-definition logic belongs in `tua-core`; LSP
code should only map URI/position inputs and `Location` outputs.

LOVE and Lua builtin behavior belongs in curated catalogs and shared semantic
resolution paths. A LOVE method return type, signature, or completion should be
visible to hover, completion, signature help, diagnostics, and inlay hints
through the same resolver.

Performance-sensitive IDE features should reuse snapshot semantic facts rather
than rebuilding type environments in each feature. Completion, completion
resolve, hover, signature help, and inlay hints should prefer cached
project-aware environments and only fall back to standalone helpers for
parse-recovery or test-style source inputs.

Open and changed documents should publish fast current-file diagnostics first,
then queue full project-aware diagnostics behind a short debounce. The stdio
server drains slow diagnostics one document at a time during idle ticks so
foreground requests such as completion, hover, symbols, and semantic tokens can
preempt background diagnostic work.

`AnalysisHost` memoizes the latest `AnalysisSnapshot` by revision. Repeated
requests against unchanged inputs should reuse the warm snapshot, including
shared lazy indexes such as semantic facts, completion lists, definition lookup
results, and reverse references. Any file, open-document, config, builtin, root,
or asset-root change increments the revision and forces the next snapshot to
rebuild from the file-granular cache.

## Adding A Core Query

1. Define the core result type near the domain that owns the behavior.
2. Add a current-file host query for file-local IDE features, or a snapshot
   method when the query depends on project indexes.
3. Reuse HIR, scope maps, the module graph, the symbol index, the
   reverse-reference index, and the type database before adding a syntax walk.
4. Return `any` or `None` consistently for dynamic Lua that cannot be proven.
5. Add focused core tests, preferably with the fixture DSL when the query needs
   multiple files, cursor markers, or expected diagnostics.
6. Update examples or docs when the query changes what users can observe in the
   editor.

Compatibility helpers such as `*_tua_source(...)` may stay for small unit tests
and CLI-like one-file calls, but new project-aware behavior should route through
`AnalysisSnapshot`.

## Adding An LSP Feature

1. Advertise the capability in the LSP capability builder.
2. Add a handler that translates LSP URI, range, position, or params into a
   snapshot query.
3. Keep request-time work inside protocol conversion plus one core query. Avoid
   disk reads, ad hoc require resolution, or local type inference in handlers.
4. Convert core ranges using the shared 1-based core to 0-based LSP helpers.
5. Add LSP tests for capability advertisement, normal results, unresolved
   results, and behavior after open-document edits or watched-file changes when
   relevant.
6. If the feature has user-visible behavior, add or update an example with `--`
   comments naming the feature being demonstrated.

## VFS Overlay Example

The core analysis layer now keeps file identity in a small virtual file system.
Disk files and open editor documents share the same normalized path, so an
unsaved editor buffer can temporarily replace disk content without changing the
file's identity.

```rust
use tua_core::{AnalysisHost, AnalysisSourceKind, SourcePosition};

let path = "game/src/player.tua";
let mut host = AnalysisHost::new();

host.set_source_roots([std::path::PathBuf::from("game/src")]);
host.set_output_roots([std::path::PathBuf::from("game/build")]);
host.set_file_with_kind(
    path,
    "type Player = { hp: number }\nlocal player: Player = { hp = 10 }\n",
    AnalysisSourceKind::Tua,
);

host.open_document_with_kind(
    "game\\src\\player.tua",
    "type Enemy = { hp: number }\nlocal enemy: Enemy = { hp = 5 }\n",
    AnalysisSourceKind::Tua,
);

let snapshot = host.snapshot();

assert!(snapshot.is_open_document("./game/src/player.tua"));
assert!(snapshot
    .hover("game/src/player.tua", SourcePosition { line: 2, column: 14 })
    .unwrap()
    .contents
    .contains("Enemy"));

host.close_document("game/src/player.tua");

let snapshot = host.snapshot();

assert!(!snapshot.is_open_document("game/src/player.tua"));
assert!(snapshot
    .hover("game/src/player.tua", SourcePosition { line: 2, column: 15 })
    .unwrap()
    .contents
    .contains("Player"));
```

The same VFS helpers normalize `file://` URIs with spaces, backslash-separated
paths from Windows-style clients, source roots, build output roots, ignored
paths, and module-relative paths such as `entities.player` or
`entities/player`.

### Project Model Example

Project modeling now sits between raw config parsing and request handlers. A
handler or CLI command should ask the model for roots, targets, and require
paths instead of rebuilding those rules locally.

```rust
use tua_core::{ProjectModel, ProjectModelOptions};

let model = ProjectModel::load_with_options(ProjectModelOptions {
    // Each workspace folder keeps its own tua.toml and roots.
    workspace_folders: vec!["examples/project-model".into()],

    // Editor settings can add associations without changing tua.toml.
    file_associations: vec![],

    ..ProjectModelOptions::default()
})?;

let workspace = model.primary_workspace().unwrap();

// The CLI emits only the primary source root by default, but the model can
// represent multiple roots for future indexing and generated-source workflows.
assert_eq!(workspace.primary_source_root().ends_with("src"), true);

// Require resolution is rooted in the project model, not the current file's
// parent directory.
let main = workspace.primary_source_root().join("main.tua");
assert_eq!(
    model.resolve_required_module(&main, "entities.player").unwrap(),
    workspace.primary_source_root().join("entities/player.tua")
);
```

### Query Cache Example

The analysis cache is intentionally file-granular for now. It is not yet a full
incremental query database, but it gives the LSP and future IDE queries a stable
place to reuse expensive per-file work.

```rust
use tua_core::AnalysisHost;

let mut host = AnalysisHost::new();

host.set_file("src/main.tua", r#"local value: number = 1"#);
host.set_file("src/player.tua", "return { hp = 100 }");

// First snapshot parses files and builds derived per-file query products:
// type environments, diagnostics, module exports, require edges, and symbols.
let first = host.snapshot();
assert!(first.parse("src/main.tua").is_some());

let stats_after_first = host.cache_stats();

// The second snapshot reuses the same revision-cached snapshot. It does not
// need to walk files or bump per-file cache counters at all.
let _second = host.snapshot();
assert_eq!(host.cache_stats(), stats_after_first);

// Editing a file invalidates that file. When require edges resolve to
// project-backed files, editing an exported module also invalidates known
// importers through the cached dependency graph. Unrelated files stay reusable.
host.set_file("src/player.tua", "return { hp = 200 }");
let _third = host.snapshot();
assert!(host.cache_stats().invalidations > 0);
```

### Parser Layer Example

Parser output is split into explicit layers while `ParsedSource` remains the
compatibility aggregate used by older emit/type/check code.

```rust
use tua_core::{extract_tua_source_model, normalize_tua_source, parse_raw_tua_source};

let normalized = normalize_tua_source("type Slot = { name?: string }\n");
assert_eq!(normalized.parser_source, "type Slot = { name: string? }\n");

let raw = parse_raw_tua_source("src/slot.tua", normalized.original)?;

// Raw syntax is Lua/Luau-compatible parser output with source spans.
assert!(raw.span.is_some());

// Typed extraction is a separate semantic-source pass over the raw tree.
let typed = extract_tua_source_model(&raw)?;
assert_eq!(typed.type_aliases[0].name, "Slot");
```

### HIR Module Example

The HIR-like layer lowers a parsed file into stable same-file symbols. IDE
features should prefer this model for declarations and same-file symbol identity
before falling back to syntax walks or dynamic Lua behavior.

```rust
use tua_core::{lower_parsed_source, parse_tua_source, HirExportValue, HirSymbolKind};

let parsed = parse_tua_source(
    "src/player.tua",
    r#"
type Player = { hp: number }
local M = {}
return { player = M, dynamic = make_player() }
"#,
)?;

let module = lower_parsed_source(&parsed);

// Symbols have stable ids and origin spans for hover, navigation, edits, and
// semantic highlighting.
let player = module.definition_for_type_alias("Player").unwrap();
assert_eq!(player.kind, HirSymbolKind::TypeAlias);

// Simple exports point at known symbols. Dynamic exports stay unknown instead
// of being guessed.
assert!(matches!(module.exports[0].value, HirExportValue::Symbol(_)));
assert_eq!(module.exports[1].value, HirExportValue::Unknown);
```

### Require Graph Example

The module graph is built from the snapshot's indexed files, HIR modules, and
project roots. It resolves `require(...)` and static `import(...)` edges,
keeps partial results for missing modules and cycles, and indexes public exports
without rereading files during hover or go-to-definition requests.

```rust
use tua_core::{
    AnalysisHost, AnalysisSourceKind, DiagnosticCode, SourcePosition,
};

let mut host = AnalysisHost::new();

host.set_source_roots(["src".into()]);
host.set_file_with_kind(
    "src/main.tua",
    r#"local Player = require("player")
local player = Player.new()
"#,
    AnalysisSourceKind::Tua,
);
host.set_file_with_kind(
    "src/player.tua",
    r#"local function new()
  return { hp = 100 }
end

return { new = new }
"#,
    AnalysisSourceKind::Tua,
);

let snapshot = host.snapshot();
let graph = snapshot.module_graph();

assert!(graph.export("src/player.tua", "new").is_some());
assert_eq!(graph.diagnostics("src/main.tua"), []);

let definition = snapshot
    .definition("src/main.tua", SourcePosition { line: 2, column: 22 })
    .unwrap();

assert_eq!(definition.path.ends_with("src/player.tua"), true);

host.set_file_with_kind(
    "src/broken.tua",
    r#"local Missing = require("missing")
"#,
    AnalysisSourceKind::Tua,
);

let snapshot = host.snapshot();
assert!(snapshot
    .module_graph()
    .diagnostics("src/broken.tua")
    .iter()
    .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_MISSING_REQUIRE)));
```

### Scope Map Example

Scope maps are the reusable lexical layer above HIR. They track locals,
parameters, module aliases, globals, table-shape fields, declaration spans, use
spans, shadowing, and unresolved names through one lookup path.

```rust
use tua_core::{
    parse_tua_source, scope_map_parsed_source_with_globals, ScopeDeclarationKind,
    ScopeGlobalKind,
};

let parsed = parse_tua_source(
    "src/main.tua",
    r#"
local Player = require("player")
local value = 1

do
  local value = 2
  print(Player.make(), value, configured)
end
"#,
)?;
let scopes = scope_map_parsed_source_with_globals(&parsed, ["configured"]);

let values = scopes
    .declarations()
    .iter()
    .filter(|declaration| declaration.name == "value")
    .collect::<Vec<_>>();

// Same-name locals are separate symbols, and the inner one records that it
// shadows the outer one.
assert_eq!(values.len(), 2);
assert_eq!(values[1].shadows, Some(values[0].id));

let player = scopes
    .declarations()
    .iter()
    .find(|declaration| declaration.name == "Player")
    .unwrap();

assert_eq!(
    player.kind,
    ScopeDeclarationKind::ModuleAlias {
        module: "player".to_owned()
    }
);
assert_eq!(
    scopes.globals().get("configured").map(|global| global.kind),
    Some(ScopeGlobalKind::Configured)
);
```

### Project Type Database Example

The project type database is built from snapshot-indexed files, cached
per-file type environments, lexical module aliases, and the module graph. It
resolves type aliases, LuaDoc-provided types, exported function signatures, and
module field chains across indexed files without reading from disk during a
request.

```rust
use tua_core::{AnalysisHost, AnalysisSourceKind, TableField, Type};

let mut host = AnalysisHost::new();

host.set_source_roots(["src".into()]);
host.set_file_with_kind(
    "src/main.tua",
    r#"local Types = require("types")
local player: Types.Player = nil
"#,
    AnalysisSourceKind::Tua,
);
host.set_file_with_kind(
    "src/types.tua",
    r#"type Player = { hp: number }

return {}
"#,
    AnalysisSourceKind::Tua,
);

let snapshot = host.snapshot();
let binding = snapshot
    .type_database()
    .resolve_type_name(snapshot.module_graph(), "src/main.tua", "Types.Player")
    .unwrap();

assert_eq!(
    binding.ty,
    Type::table([("hp", TableField::required(Type::number()))])
);
assert!(snapshot.diagnostics("src/main.tua").is_empty());
```

## Gap Matrix

| rust-analyzer area | Purpose in rust-analyzer | Tua equivalent today | Status | Roadmap |
| --- | --- | --- | --- | --- |
| `base-db` | Stores stable inputs, file IDs, crate graph, and query base data. | `AnalysisHost`/`AnalysisSnapshot`, VFS, project model inputs, and a file-granular query cache. | Partial | 53, 56 |
| `vfs`, `vfs-notify` | Tracks files, changes, paths, and file watching. | Core VFS plus LSP watched-file handling and CLI source discovery. | Partial | 54 |
| `project-model` | Models Cargo workspaces, packages, targets, cfg, and sysroot. | Core `ProjectModel` for workspace folders, source/out roots, require roots, file associations, Lua target, and LOVE target. | Existing | 55 |
| `parser` | Parses token streams into syntax. | Raw Lua/Tua parser APIs over full_moon plus normalization for Tua-only syntax. | Existing | 57 |
| `syntax` | Typed syntax tree and syntax utilities. | Raw parsed source, normalized source, and typed source extraction models in `tua-core::parser`. | Partial | 57 |
| `hir`, `hir-def`, `hir-expand` | Lowers syntax into semantic definitions and handles expansion. | `HirModule` with stable same-file symbols, declarations, require bindings, and simple module exports. | Partial | 58 |
| `hir-ty` | Type inference and type checking over HIR. | `types`, `check`, `SemanticModel`, project type database, and Lua/LOVE builtin catalogs. | Partial | 61, 62 |
| `ide-db` | Shared IDE database and symbol indexes. | `AnalysisSnapshot`, file-granular cache, module graph, export index, lexical scope maps, project type database, workspace symbol scans, lazy reverse-reference index, and LSP analysis helpers. | Partial | 59, 60, 61, 64, 65, 70.20 |
| `ide` | User-facing IDE queries. | Core modules for hover, completion, definition, references, rename, symbols, etc. | Existing | 65 |
| `ide-completion` | Completion-specific query and rendering logic. | `completion` module and LSP completion conversion. | Existing | 64, 65 |
| `ide-diagnostics` | Diagnostic query layer. | Parser/check/lint/project diagnostics called by CLI and LSP. | Partial | 63 |
| `ide-assists` | Code actions and refactor assists. | `code_action` module with conservative quick fixes, source import actions, and safe file fix-all. | Partial | 66, 89 |
| `ide-ssr` | Structural search and replace. | No equivalent planned for v1. | Out of scope | None |
| `test-fixture`, `test-utils` | Fixture DSLs and shared test support. | Many focused tests with ad hoc fixtures and temp projects. | Partial | 52 |
| `profile` | Performance tracing and benchmark support. | Opt-in `TUA_PROFILE` tracing, LSP latency benchmark fixtures, completion-specific profiling, and cache stats reporting. | Existing | 69, 77.5 |
| `proc-macro-*`, `mbe`, `tt` | Rust macro expansion infrastructure. | No equivalent; Tua plugins must not become arbitrary macro expansion. | Out of scope | None |
| `load-cargo`, `toolchain` | Rust toolchain and Cargo loading. | No equivalent; Tua project model stays Lua/LOVE-focused. | Out of scope | None |

## Phase 12 Architecture Path

Phase 12 landed in this order, with some rust-analyzer-like pieces intentionally
kept smaller than their Rust equivalents:

1. Add fixture DSL support so architecture changes can be tested clearly.
2. Introduce `AnalysisHost` and immutable `AnalysisSnapshot`.
3. Add VFS/open-file overlays and a project model.
4. Add dependency-tracked query caching.
5. Split parser output from the semantic source model.
6. Lower files into HIR-like modules with stable IDs.
7. Build require graphs, scope maps, project-aware type data, and symbol indexes.
8. Route diagnostics and IDE features through the snapshot.
9. Move code actions and call hierarchy onto the shared model.
10. Add selection/folding ranges, profiling, and this contributor architecture
    guide.

Deferred precision work remains in later roadmap items: richer flow-sensitive
narrowing, broader LuaLS/LuaCATS annotation compatibility, and future LOVE
domain extensions.

## Non-Goals

- Do not create a Rust clone. Tua targets Lua 5.1/LuaJIT/LOVE projects.
- Do not add a macro system equivalent to Rust macros.
- Do not run project code to discover symbols or assets.
- Do not make plugin support a back door for arbitrary expression rewriting.
- Do not replace readable Lua output with optimized or minified compiler output.
- Do not move everything into separate crates until the boundary has earned its
  cost.

## Commit Mapping

| Roadmap item | Architecture gap |
| --- | --- |
| 51 | This matrix and architecture gap documentation. |
| 52 | Test fixture and project fixture foundation. |
| 53 | Analysis host/snapshot boundary. |
| 54 | VFS and open-document overlays. |
| 55 | Project model for roots, targets, config, and require paths. |
| 56 | Dependency-tracked query cache. |
| 57 | Syntax tree versus semantic source model split. |
| 58 | HIR-like semantic module lowering. |
| 59 | Require graph and export index. |
| 60 | Lexical scope maps. |
| 61 | Project-aware type database. |
| 62 | Flow-sensitive narrowing. |
| 63 | Incremental diagnostics pipeline. |
| 64 | Shared symbol index. |
| 65 | LSP feature routing through `AnalysisSnapshot`. |
| 66 | Code action framework over snapshot data. |
| 67 | Selection and folding ranges. |
| 68 | Call hierarchy over require graph and symbols. |
| 69 | Analysis tracing and benchmarks. |
| 70 | Final contributor architecture guide after implementation. |

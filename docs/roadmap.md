# Roadmap

The canonical roadmap lives at the repository root:

- [ROADMAP.md](https://github.com/Kyonru/tua-lang/blob/main/ROADMAP.md)

It is organized into commit-sized slices. Each roadmap item has a proposed
commit title, scope, and acceptance criteria.

## Current Focus

The v1 implementation covers the compiler, CLI, examples, VS Code extension,
LOVE-focused helpers, LuaDoc-aware Lua interop, and a broad language-server
surface.

The recent roadmap focus landed the Phase 12 architecture spine:

- rust-analyzer-inspired analysis database, VFS, HIR, and query caching;
- stronger project modeling for require graphs, source roots, and exports;
- shared IDE routing for navigation, symbols, call hierarchy, selection ranges,
  folding ranges, and profiling.
- cached semantic facts for autocomplete, hover, signature help, and inlay hints
  so inferred types are reused across fast editor queries.
- warm snapshot reuse for repeated navigation requests, including a dedicated
  `definitionAgain` profiling row for cmd+click-style latency checks.

See [Architecture](architecture.md) for the contributor guide and Phase 12
rust-analyzer gap matrix.

Current future work is now more about precision and domain depth:

- flow-sensitive type narrowing and richer LuaLS/LuaCATS annotation support;
- deeper table, array, index, and loop-variable inference;
- more LuaHelper parity fixtures and stub coverage;
- LuaHelper check-code parity gaps tracked by the
  [LuaHelper Code Matrix](luahelper-code-matrix.md);
- larger refactor support where the shared model can prove edits are safe.

After that, Phase 13 explores LOVE domain extensions:

- a conservative plugin architecture for domain declarations and tooling,
  starting with the [Domain Plugins](plugins.md) design;
- built-in vector ergonomics, starting with `Vec2` and `vec2(x, y)` plain-table
  emission;
- a prototype sprite/animation DSL;
- richer asset validation and editor metadata;
- optional Feather/tooling integration hooks.

## Working Rule

Each commit should leave the repository buildable and tested. Behavior changes
should include focused core or LSP tests in the same commit.

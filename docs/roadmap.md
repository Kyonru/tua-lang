# Roadmap

The canonical roadmap lives at the repository root:

- [ROADMAP.md](https://github.com/Kyonru/tua-lang/blob/main/ROADMAP.md)

It is organized into commit-sized slices. Each roadmap item has a proposed
commit title, scope, and acceptance criteria.

## Current Focus

The v1 implementation covers the compiler, CLI, examples, VS Code extension,
LOVE-focused helpers, LuaDoc-aware Lua interop, and a broad language-server
surface.

The active roadmap focus is now deeper architecture and precision:

- rust-analyzer-inspired analysis database, VFS, HIR, and query caching;
- stronger project modeling for require graphs, source roots, and exports;
- flow-sensitive type narrowing and richer project-aware type information;
- more LuaHelper parity fixtures and stub coverage;
- editor assists such as selection/folding ranges and larger refactor support.

See [Architecture](architecture.md) for the Phase 12 rust-analyzer gap matrix.

After that, Phase 13 explores LOVE domain extensions:

- a conservative plugin architecture for domain declarations and tooling;
- built-in vector ergonomics;
- a prototype sprite/animation DSL;
- richer asset validation and editor metadata;
- optional Feather/tooling integration hooks.

## Working Rule

Each commit should leave the repository buildable and tested. Behavior changes
should include focused core or LSP tests in the same commit.

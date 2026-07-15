---
icon: lucide/map
---

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
- built-in vector ergonomics, starting with `Vec2` and `vec2(x, y)`
  BrineVector-backed emission;
- opt-in LuaJIT guidance for stable local/table shapes in hot LOVE code;
- opt-in tiny-ecs-shaped ECS domain support through the built-in plugin
  registry path;
- a prototype sprite/animation DSL;
- richer asset validation and editor metadata, with the first shared metadata
  slice now powering asset hovers and path completion;
- optional Feather/tooling integration hooks.

Phase 14 tracks completion intelligence as a compiler-first pipeline. It now
generates parsed identifier candidates from scope visibility, keeps field
completions backed by type, module, LOVE catalog, and table-shape facts, and
ranks semantic candidates with local/project usage signals when the receiver is
known:

- deterministic semantic candidate generation;
- same-file proximity, call-site frequency, project member usage, and
  declaration-quality ranking signals;
- optional project-adaptive and model-assisted ranking paths behind explicit
  configuration;
- completion latency and ranking-overhead benchmarks.

Phase 15 tracks TypeScript Go-inspired LSP and VS Code parity gaps:

- editor lifecycle UX first: restart/output commands, status, server selection,
  logging, profiling, and support commands;
- performance-oriented protocol support includes pull diagnostics,
  range/on-type formatting, and semantic token delta;
- deeper navigation and refactor parity after that: type definition,
  implementation, code lens references, completion label details, signature
  retriggers, and broader refactor/source-action families beyond the safe import
  and fix-all actions already implemented;
- TypeScript Go-style proposed editor features are evaluated conservatively:
  Tua keeps verbose hover, source definition, multi-document highlight, and auto
  insert off custom/proposed protocols unless a stable or preview-gated Lua/Tua
  workflow is proven.
- The optional external API bridge is design-only today. It should remain
  disabled by default, local-only, authenticated for pipe/socket use, and
  separate from LSP startup until a concrete consumer exists.

## Working Rule

Each commit should leave the repository buildable and tested. Behavior changes
should include focused core or LSP tests in the same commit.

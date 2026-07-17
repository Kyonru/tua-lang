# Changelog

All notable changes to Tua are documented here. Releases follow semantic
versioning while the project is pre-1.0: minor releases may deliberately evolve
language or editor behavior, and migration notes call out those changes.

## [Unreleased]

### Added

- A zero-setup `ecs` module with a bundled data-oriented `tuaecs.lua` runtime,
  documented completion, hover, named signatures, world queries, and systems.
- A zero-setup `objectPool({...})` helper with item-aware typing, lifecycle
  completion and hover, prewarming, guarded release, cleanup hooks, and an
  on-demand bundled `tuaobjectpool.lua` runtime.
- A VS Code quick-pick and editor-toolbar command for Full, Compact, Types
  Only, Parameters Only, and Off inlay-hint modes. Compact restores the exact
  previous Tua-specific hint-length preference when another mode is selected.
- A compiler-backed `Explain Type / Why Is This Any?` command with source-linked
  annotation, inference, widening, narrowing, module, field, and dynamic
  fallback steps.
- Generated LOVE callback assistance with `TL5006` typo/signature diagnostics,
  typo rename fixes, and a source action for missing core lifecycle callbacks.
- Compiler-owned asset inventory and a VS Code Tua Assets Explorer grouped by
  type and static usage, with reference navigation, dimensions, refresh,
  reveal, and LSP-aware file rename.
- Asset path casing warning `TL2008` and conservative unused-asset hint
  `TL2009`; dynamic asset loading remains supported.
- A compiler-backed VS Code module dependency explorer for static `require` and
  `import` relationships, unresolved resolutions, cycles, reverse dependencies,
  exported surfaces, and source-linked "Why Is This Module Included?" paths.
- A compiler-backed VS Code ECS Explorer for project components, systems,
  queries, conservative callback read/write access, exact source navigation,
  and non-diagnostic allocation insights.
- Shared VS Code webview, URI/range, error, and document-response utilities now
  own CSP nonces, safe JSON embedding, HTML escaping, location opening, and
  stale-response checks across editor domains.

### Changed

- ECS analysis is enabled by default and can still be disabled through
  `[ecs].enabled` or the matching editor setting.
- VS Code syntax highlighting now covers the full Lua/Tua and game-helper
  surface, while semantic highlighting uses standard theme-compatible symbol
  types and declaration, readonly, deprecated, modification, and library
  modifiers.
- The VS Code extension now requires VS Code 1.94 or newer for native compact
  inlay-hint presentation.

## [0.1.0] - 2026-07-16

### Added

- Typed Lua parsing, checking, readable Lua 5.1/LuaJIT emission, source maps,
  formatting, linting, watch builds, and project-aware module analysis.
- A preemptible language server with completion, hover, signature help,
  navigation, refactors, semantic tokens, inlay hints, diagnostics, and LOVE
  catalog intelligence.
- LOVE asset helpers, generated LOVE 11.x API data, Vec2, SignalBus, ECS, and
  preview-gated anim8-backed sprite-sheet support.
- Platform-specific VSIX packages containing matching Tua server binaries and
  standalone server archives with SHA-256 checksums.

### Compatibility

- Tua 0.1 targets Lua 5.1-style Lua, LuaJIT, and LOVE 11.x.
- The CLI, language server, Rust crates, and VS Code extension now share version
  `0.1.0` from the Rust workspace metadata.

### Migration

- Replace any development VSIX named `tua-0.0.1.vsix` with the VSIX matching
  your operating system and architecture.
- Project source and `tua.toml` files require no migration for this coordinated
  release.

[Unreleased]: https://github.com/Kyonru/tua-lang/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Kyonru/tua-lang/releases/tag/v0.1.0

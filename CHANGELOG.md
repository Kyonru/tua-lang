# Changelog

All notable changes to Tua are documented here. Releases follow semantic
versioning while the project is pre-1.0: minor releases may deliberately evolve
language or editor behavior, and migration notes call out those changes.

## [Unreleased]

No unreleased changes are recorded yet.

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

# Changelog

All notable changes to Tua are documented here. Releases follow semantic
versioning while the project is pre-1.0: minor releases may deliberately evolve
language or editor behavior, and migration notes call out those changes.

## [Unreleased]

### Added

- Resolved `---@deprecated` API guidance through standard lint `TL4056`,
  module exports, hover, completion tags, and deprecated semantic tokens.
- Standard lint rules for `tua lint` and the default-on
  `diagnostics.rules` editor/check setting, including symbol, declaration,
  control-flow, format-string, table-operation, and expression guidance.
- Source-mapped LOVE debugging through **Tua: Debug LOVE Project**. Builds now
  emit standard `*.lua.map` sidecars alongside Tua traceback maps, allowing the
  Second Local Lua Debugger extension to bind `.tua` breakpoints and show
  mapped Tua stack frames while debugging generated Lua. Game workspaces can
  start the same prepared workflow from `.vscode/launch.json` with
  `tuaProject: true`.
- A zero-setup Baton-backed `input({...})` helper with derived control and pair
  names, LOVE input-source completion and validation, name-dependent return
  signatures, active-device typing, and an on-demand bundled `baton.lua`
  runtime.
- An explicit zero-setup `store({...})` helper with inferred state reads,
  typed replacement and mutation, whole-store and keyed subscriptions,
  idempotent unsubscribe functions, and an on-demand `tuastore.lua` runtime.
- A zero-setup `stateMachine({...})` helper backed by lua-state-machine, with
  derived state/event unions, generated event methods, callback and value
  completion, and an on-demand bundled `tuastatemachine.lua` runtime.
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
- A compiler-backed VS Code Object Pool Explorer for pool declarations,
  lifecycle operations, item types, conservative unreleased-acquire warnings,
  and source navigation, plus a quick fix for repeated zero-argument
  constructors.
- Conservative refactor assists for extracting selected expressions to locals
  or parameterized local functions, and inlining stable local bindings across
  resolved reads.
- A quick fix that generates a named Tua table type from stable local table
  literals and annotates the local binding in the same workspace edit.
- SignalBus graph filters for event, bus, file, listener, and emitter views,
  plus direct schema-conflict display and actions to create typed listeners or
  declarations from compiler metadata.
- Shared VS Code webview, URI/range, error, and document-response utilities now
  own CSP nonces, safe JSON embedding, HTML escaping, location opening, and
  stale-response checks across editor domains.

### Changed

- `MisleadingAndOr` now identifies the exact `false` or `nil` alternative,
  points at the complete expression, and recommends a LuaJIT-safe `if`
  statement assignment instead of Luau-only conditional-expression syntax.
- `DuplicateCondition` now structurally compares expressions, flattens grouped
  boolean chains, preserves runtime literal identity, suppresses the common Lua
  and-or ternary idiom, and points diagnostics at the repeated condition with
  its origin.
- `TableOperations` now resolves Lua builtins, detects expanding final
  `table.insert` calls, and uses shared table types for conservative `#` and
  `ipairs` guidance, including `__len` and `__ipairs` support.
- Native standalone dictionary annotations such as `{ [string]: Value }` now
  lower to dictionary types instead of losing their index signature.
- `DuplicateLocal` now reports repeated local and parameter names uniformly as
  `TL4043`, recognizes implicit method `self`, identifies the original
  declaration, and accepts repeated `_` placeholders.
- `ImportUnused` now recognizes dynamic calls to the global `require`, excludes
  lexically shadowed calls, counts qualified type references, includes `_`
  guidance, and backs import source actions with the same resolved usage facts.
- `FunctionUnused` now reports direct local and user-global function
  declarations as `TL4055`, with safe editor removal for complete declarations.
- Standard lint rules are enabled by default for `tua check` and editor
  diagnostics. Set `diagnostics.rules = false` to opt out; `tua lint` continues
  to enable the standard set for its own run.
- `UnknownGlobal` now recognizes explicit global assignments and simple global
  function declarations anywhere in the current Tua file, regardless of source
  order.
- Formatting and linting are now independent toolchain modules: `tua fmt` and
  `fmt --check` own canonical layout, while `tua lint` reports syntax/lint
  categories and `tua check` owns the complete semantic diagnostic pipeline.
- ECS analysis is enabled by default and can still be disabled through
  `[ecs].enabled` or the matching editor setting.
- VS Code syntax highlighting now covers the full Lua/Tua and game-helper
  surface, while semantic highlighting uses standard theme-compatible symbol
  types and declaration, readonly, deprecated, modification, and library
  modifiers.
- The VS Code extension now requires VS Code 1.94 or newer for native compact
  inlay-hint presentation.

### Migration

- Replace `TL4008` suppressions or filters with `TL4043`; the
  `duplicate_parameters` setting remains available and now emits `TL4043`.
- Projects that need the previous lint-free `tua check` and editor behavior can
  set `[diagnostics].rules = false`.
- Remove `diagnostics.formatting_lints` and the
  `tua.diagnostics.formattingLints` editor override. Use `tua fmt --check` for
  formatting enforcement and `diagnostics.rules` for the standard lint rules.

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

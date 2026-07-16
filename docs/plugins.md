---
icon: lucide/puzzle
---

# Domain Plugins

Tua's domain plugin model is a design direction for LOVE-focused extensions. It
is not a general macro system. The goal is to let Tua understand small,
tooling-friendly domains such as assets, sprites, animation, vectors, and editor
metadata while keeping normal Lua/Tua semantics predictable.

This page defines the intended v1 boundaries and the current registry shape.
Third-party executable plugins are not available today.

## Current Implementation

Tua currently has an experimental Rust-only validation registry in `tua-core`.
`AnalysisHost` owns an optional `PluginRegistry`; the effective registry always
reserves the built-in `tua.sprite` validator and adds `tua.ecs` when
`[ecs].enabled = true`.

The executable `TuaPlugin` trait currently contributes diagnostics only. A
plugin receives the current path, source, parsed source when available, and
indexed asset metadata. It runs while the cached `AnalysisFile` is built, not
from an LSP request handler. The same file and analysis revision therefore reuse
the same plugin diagnostics.

Ownership and ordering are explicit:

- non-empty plugin names are unique;
- duplicate registration is rejected;
- built-in `tua.*` names are registered before configured Rust registries and
  cannot be replaced;
- plugins run in lexical name order, preserving each plugin's diagnostic order;
- diagnostics are constrained to the current file;
- missing or non-plugin codes become `TL5001`, while stable codes in the
  reserved `TL5000`-`TL5999` range are preserved;
- normal diagnostic deduplication resolves identical diagnostic keys (file,
  position, severity, and code) using that deterministic order.

Sprite parsing, typing, completion, hover, and anim8-backed emission are core
language services over the ordinary `spriteSheet({...})` helper call. The
`tua.sprite` plugin owns only asset and animation-range validation. Likewise,
the typed `ecs` API is provided through the shared type environment, while the
opt-in `tua.ecs` plugin owns ECS-specific validation.

## Goals

Domain plugins should make game code easier to validate, complete, inspect, and
lower to readable Lua. A plugin should provide static facts that the compiler,
language server, and editor can share.

Good validation plugin features today:

- validate static inputs such as asset paths, frame sizes, and mode names;
- consume parser and project metadata already owned by Tua;
- return deterministic diagnostics with stable spans and plugin-range codes;
- remain side-effect free and fast enough to run during cached file analysis.

Completion, hover, declarations, editor metadata, and structured lowering are
possible future surfaces. They should not return to `TuaPlugin` until they have
shared cached analysis queries, conflict rules, and a Tua-owned lowering IR.

Bad plugin features:

- arbitrary expression rewrites;
- changing core Lua evaluation rules;
- executing the user's game code during analysis;
- hiding large runtime behavior behind Tua syntax;
- returning raw Lua strings as the main lowering API.

## Current Extension Surface

The current executable surface is one validation pass over parser-owned syntax
and project metadata. Plugins do not add syntax, globals, types, completion,
hover, editor commands, or emitted code. Recognized helpers such as
`spriteSheet({...})` must first exist as ordinary core language APIs.

Potential future extension points include cached editor metadata and structured
lowering requests, but those are design directions rather than public trait
methods today.

Validation plugins may not:

- rewrite arbitrary Lua/Tua expressions;
- add new expression operators;
- change how locals, functions, tables, metatables, or `require` work;
- execute project runtime code or call LOVE APIs during analysis;
- read arbitrary files outside configured project roots;
- emit raw Lua strings as the primary lowering API;
- depend on network access during normal check/build/LSP requests.

Validation plugins are additive. Disabling one removes its domain diagnostics;
it does not change parsing, typing, completion, hover, or emission.

## Plugin Kinds

Tua should support implementation choices in this order.

### Rust Built-In Plugins

Rust built-ins are the safest v1 starting point. They compile with Tua and share
existing parser, diagnostic, and asset metadata without executing project code.

Use Rust built-ins for:

- deterministic domain validation;
- LOVE asset validation;
- sprite/animation experiments;
- features that must be fast and deterministic during editor analysis.

Tradeoff: users cannot author plugins without changing Tua itself.

### Static Manifest Plugins

Static manifest plugins provide metadata only. They do not execute code. A
manifest can contribute known assets, object types, enum values, completions,
hover docs, and editor metadata.

Example shape:

```toml
[plugin]
name = "game.assets"
version = "0.1.0"

[[asset_kind]]
name = "portrait"
extensions = ["png", "jpg"]
root = "assets/portraits"
```

Use manifests for:

- project-specific asset categories;
- completion catalogs;
- documentation overlays;
- editor-only metadata.

Tradeoff: manifests cannot express complex validation or lowering.

### Lua-Authored Plugins Hosted By Rust

Lua-authored plugins are attractive because Tua users already know Lua, but they
are not a safe default for v1. If Tua eventually supports them, Rust should host
them through a restricted API with explicit capabilities.

Required restrictions:

- no direct file-system access except Tua-provided project APIs;
- no network access;
- no `require` of arbitrary host libraries;
- no access to the user's game runtime;
- deterministic inputs and outputs;
- bounded execution time;
- stable serialized AST/IR formats;
- plugin diagnostics tied to real source spans.

Lua plugins should return structured data, not raw Lua strings:

```lua
return {
  name = "tua.anim8",

  helpers = { "spriteSheet" },

  validate = function(node, ctx)
    if not ctx.asset_exists(node.image) then
      ctx.error(node.image_span, "missing sprite image: " .. node.image)
    end
  end,

  lower = function(node, ctx)
    return {
      kind = "local",
      name = node.name,
      value = {
        kind = "call",
        callee = "anim8.newAnimation",
        args = {
          { kind = "identifier", name = node.grid_name },
          { kind = "number", value = node.fps },
        },
      },
    }
  end,
}
```

Tradeoff: this is the most flexible route and the highest security risk. It
should stay disabled until the registry, IR, sandboxing, and diagnostics model
are proven by built-in plugins.

## Structured Lowering

Structured plugin lowering is not implemented. Before lowering returns to the
plugin API, Tua should define a small owned IR and make the emitter consume it
through the same cached analysis snapshot.

Useful IR nodes:

- `require(module)`;
- `local name = value`;
- table constructors;
- function and method calls;
- string, number, boolean, and nil literals;
- plain identifiers and field chains;
- generated comments for traceability.

Raw Lua string emission should not be a plugin API. String emission is difficult
to format, hard to map back to source spans, and easy to make unsafe.

## Sprite And Animation Example

Core Tua owns one experimental ordinary helper-call shape:

```tua
local Player = spriteSheet({
  image = "assets/player.png",
  frame = { width = 16, height = 16 },
  animations = {
    idle = { from = 0, to = 3, fps = 8 },
    run = { from = 4, to = 9, fps = 12, loop = false },
  },
})
```

Static model:

- helper: `spriteSheet`;
- symbol name: `Player`;
- asset dependency: `assets/player.png`;
- frame size: `16x16`;
- animations: `idle`, `run`;
- generated runtime dependency: the bundled `anim8.lua` module, unless the
  project provides its own `anim8.lua`.

Validation examples:

- `assets/player.png` must exist under configured asset roots;
- image extension must be supported by LOVE;
- frame width and height must be positive;
- animation frame ranges must be ordered from low to high;
- animation FPS must be greater than zero.

Editor examples:

- top-level completion includes a `spriteSheet` helper snippet;
- sprite image paths participate in asset metadata hovers;
- normal parser-native syntax keeps inlay hints, hover, formatting, rename, and
  document symbols on ordinary Tua nodes;
- VS Code can preview animations from the active document with the
  preview-gated `Tua: Preview Sprite Animations` command, including the command
  link shown when hovering a parsed sprite sheet binding.

Current emitted Lua shape:

```lua
local PlayerImage = love.graphics.newImage("assets/player.png")
local PlayerGrid = anim8.newGrid(16, 16, PlayerImage:getWidth(), PlayerImage:getHeight())
local Player = {
  image = PlayerImage,
  grid = PlayerGrid,
  frame = { width = 16, height = 16 },
  animations = {
    idle = anim8.newAnimation(PlayerGrid("1-4", 1), 1 / 8),
    run = anim8.newAnimation(PlayerGrid("5-10", 1), 1 / 12, "pauseAtEnd"),
  },
}
```

The parser records sprite metadata, the `tua.sprite` validator reports static
asset/range errors, and the core emitter produces the Lua above. The exact
output can still change before stabilization, but it must remain readable and
ordinary. When generated Lua needs `anim8`, `tua build` writes the bundled
`anim8.lua` runtime into the output directory unless the project already
provides that module. There is no hidden Tua runtime.

## Asset Validation Example

A built-in asset plugin can start smaller than a sprite DSL. It can add project
metadata and diagnostics around existing Tua/Lua code.

Example source:

```tua
local player = image "assets/player.png"
local jump = sound "assets/jump.wav"
```

Plugin facts:

- `image` expects a LOVE-supported image extension;
- `sound` expects a LOVE-supported audio extension;
- paths resolve against configured asset roots;
- the returned types are `Image` and `Source`;
- diagnostics use stable source spans on the string literal.

Editor metadata:

- path completions under configured asset roots;
- hover with file kind and resolved path;
- diagnostics for missing paths and unsupported extensions;
- optional preview metadata for images or audio waveforms in editor adapters.

This style should be preferred before larger DSLs. It proves the same plugin
pipeline: static project metadata, validation, type facts, completion, hover,
and deterministic diagnostics.

## Security And Determinism

Plugin execution must be deterministic from the analysis inputs:

- source text;
- project configuration;
- indexed files and open-document overlays;
- built-in catalogs;
- explicit plugin manifests;
- configured asset roots.

Plugins must not depend on:

- wall-clock time;
- random numbers;
- network calls;
- process environment not passed through config;
- game runtime state;
- mutable global host state;
- file reads outside the project model.

Diagnostics must be stable:

- same input produces same diagnostics in the same order;
- diagnostics include file, line, column, severity, message, and code;
- plugin diagnostic codes use a reserved range or stable plugin namespace;
- dynamic uncertainty should stay quiet instead of guessing.

For Lua-authored plugins, Tua should additionally require:

- a sandboxed Lua VM;
- instruction or time limits;
- memory limits where practical;
- explicit capability declarations;
- no arbitrary `require`;
- no direct OS, IO, debug, package, or FFI access.

## LSP Integration

Plugin diagnostics flow through the same cached analysis files and snapshots as
core diagnostics. LSP handlers do not call plugins directly during a request.

Future plugin-provided editor data could include:

- completions;
- hover text;
- diagnostics;
- semantic-token classes;
- document symbols;
- document links for assets;
- code actions where edits are mechanical;
- future inspector metadata for editor or Feather-style tooling.

Request handlers should stay mostly protocol conversion plus snapshot access.
These are not current `TuaPlugin` methods. If a future plugin feature needs
project-wide state, the analysis host must index it before the request and own
its ordering and conflict behavior.

## V1 Recommendation

The recommended implementation path is:

1. A Rust-only validation registry in `tua-core`. Status: done.
2. Built-in sprite and opt-in ECS validators over shared parser/project facts.
   Status: done.
3. Shared cached query types for any future completion, hover, or inspector
   contributions.
4. A structured lowering IR before exposing plugin emission hooks.
5. No third-party executable plugins.

Static manifest plugins can follow once the registry exists. Lua-authored
plugins should wait until the sandbox, capability model, and structured IR are
stable enough to keep Tua predictable.

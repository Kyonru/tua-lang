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

Tua currently has an experimental Rust-only plugin registry in `tua-core`.
`AnalysisHost` owns an optional `PluginRegistry`; the default registry is empty,
so ordinary projects behave exactly as before.

Registered Rust plugins can currently contribute:

- declaration metadata;
- diagnostics through the normal analysis snapshot pipeline;
- completion items;
- hover snippets;
- structured lowering requests.

Only diagnostics are wired into the default analysis pipeline today. The other
contribution types are exposed by the registry so future built-in domain plugins
can route them through completion, hover, and emission without inventing another
plugin API. Plugin diagnostics use the reserved `TL5000`-`TL5999` range.

The first built-in consumer is the opt-in ECS plugin. When `[ecs].enabled =
true`, `AnalysisHost` adds the Rust `tua.ecs` plugin to the effective registry.
That plugin contributes ECS declaration metadata and diagnostics, while the
typed `ecs` API surface is provided through the shared type environment so
completion, hover, and signature help use the same semantic path as other Tua
builtins.

## Goals

Domain plugins should make game code easier to validate, complete, inspect, and
lower to readable Lua. A plugin should provide static facts that the compiler,
language server, and editor can share.

Good plugin features:

- describe domain declarations with stable spans;
- validate static inputs such as asset paths, frame sizes, and mode names;
- lower domain syntax through structured compiler metadata;
- provide completions, hover text, diagnostics, and editor metadata;
- emit Lua that remains LuaJIT and LOVE compatible;
- avoid hidden runtime requirements unless the project explicitly imports a
  normal Lua dependency.

Bad plugin features:

- arbitrary expression rewrites;
- changing core Lua evaluation rules;
- executing the user's game code during analysis;
- hiding large runtime behavior behind Tua syntax;
- returning raw Lua strings as the main lowering API.

## V1 Extension Surface

V1 plugins may extend only declaration-shaped parts of the language. This keeps
the parser, type checker, LSP, and emitter deterministic.

Allowed extension points:

- **Top-level declarations**: for example `sprite Player ... end`.
- **Domain blocks**: structured blocks owned by a registered declaration kind.
- **Validation passes**: diagnostics over parsed plugin declarations and project
  metadata.
- **Structured lowering metadata**: requests to emit calls, tables, assignments,
  module requires, or generated constants through Tua-owned lowering APIs.
- **Editor metadata**: completions, hover, diagnostics, document symbols,
  semantic-token hints, asset previews, and future inspector data.

V1 plugins may not:

- rewrite arbitrary Lua/Tua expressions;
- add new expression operators;
- change how locals, functions, tables, metatables, or `require` work;
- execute project runtime code or call LOVE APIs during analysis;
- read arbitrary files outside configured project roots;
- emit raw Lua strings as the primary lowering API;
- depend on network access during normal check/build/LSP requests.

Plugins should be additive. If a plugin is disabled, ordinary Lua/Tua files
should still parse, check, and build as before, except for files that use that
plugin's domain syntax.

## Plugin Kinds

Tua should support implementation choices in this order.

### Rust Built-In Plugins

Rust built-ins are the safest v1 starting point. They compile with Tua, use
normal Rust types for AST and lowering IR, and can share existing parser,
diagnostic, asset, and LSP infrastructure.

Use Rust built-ins for:

- proving the plugin registry;
- LOVE asset validation;
- sprite/animation experiments;
- richer typed vector helpers if they need parser or lowering support beyond
  the built-in `Vec2`/`vec2(x, y)` BrineVector-backed helper;
- features that must be fast and deterministic in the LSP.

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

  declarations = { "sprite" },

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

Plugins should lower to a small Tua-owned IR. The emitter then turns that IR
into readable Lua.

Useful IR nodes:

- `require(module)`;
- `local name = value`;
- table constructors;
- function and method calls;
- string, number, boolean, and nil literals;
- plain identifiers and field chains;
- generated comments for traceability.

Raw Lua string emission should be reserved for an escape hatch, not the primary
API. String emission is difficult to format, hard to map back to source spans,
and easy to make unsafe.

## Sprite And Animation Example

A future sprite plugin could own a declaration like this:

```tua
sprite Player
  image "assets/player.png"
  frame 16x16
  anim idle frames 0..3 fps 8
  anim run frames 4..9 fps 12
end
```

Static model:

- declaration kind: `sprite`;
- symbol name: `Player`;
- asset dependency: `assets/player.png`;
- frame size: `16x16`;
- animations: `idle`, `run`;
- generated runtime dependency: a normal Lua module such as `anim8`.

Validation examples:

- `assets/player.png` must exist under configured asset roots;
- image extension must be supported by LOVE;
- frame width and height must be positive;
- animation frame ranges must fit the image dimensions when metadata is known;
- duplicate animation names produce deterministic diagnostics;
- unknown animation references can power completions and diagnostics.

Editor examples:

- completion for animation names after `Player.animations.`;
- hover showing frame size, fps, and source image;
- document symbols for `Player`, `idle`, and `run`;
- optional editor metadata for a future sprite preview panel.

Possible structured lowering:

```lua
local PlayerImage = love.graphics.newImage("assets/player.png")
local PlayerGrid = anim8.newGrid(16, 16, PlayerImage:getWidth(), PlayerImage:getHeight())
local Player = {
  image = PlayerImage,
  animations = {
    idle = anim8.newAnimation(PlayerGrid("1-4", 1), 1 / 8),
    run = anim8.newAnimation(PlayerGrid("5-10", 1), 1 / 12),
  },
}
```

The exact emitted Lua can change, but it must remain readable and ordinary. The
plugin should not require a hidden Tua runtime.

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

Plugin facts should flow through the same analysis snapshot as core facts. LSP
handlers should not call plugins directly during a request.

Plugin-provided editor data can include:

- completions;
- hover text;
- diagnostics;
- semantic-token classes;
- document symbols;
- document links for assets;
- code actions where edits are mechanical;
- future inspector metadata for editor or Feather-style tooling.

Request handlers should stay mostly protocol conversion plus snapshot access.
If a plugin feature needs project-wide state, the analysis host should index it
before the request.

## V1 Recommendation

The recommended implementation path is:

1. A Rust-only plugin registry in `tua-core`. Status: done.
2. One built-in asset validation plugin.
3. One small experimental declaration plugin, such as a sprite metadata parser,
   behind an explicit config flag.
4. Structured lowering IR for any emitted code.
5. No third-party executable plugins.

Static manifest plugins can follow once the registry exists. Lua-authored
plugins should wait until the sandbox, capability model, and structured IR are
stable enough to keep Tua predictable.

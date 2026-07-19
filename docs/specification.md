---
icon: lucide/book-open-check
---

# Tua Language And Toolchain Reference

This page documents the implemented behavior shared by the Tua compiler, CLI,
language server, and VS Code extension. Detailed syntax examples live in
[Language](language.md). Editor behavior is documented in
[Language Server](lsp.md), and project settings live in
[Configuration](configuration.md).

!!! info "Implemented behavior, not a design plan"
    This reference describes the current contract. Pending repository work is
    tracked separately from the public documentation.

## Overview

Tua is a gradual typed Lua superset that emits readable Lua 5.1-compatible
source for LuaJIT and LOVE 11.x projects. It is source-to-source tooling, not a
new VM. Types improve diagnostics and editor intelligence and are erased before
runtime.

Tua preserves ordinary Lua workflows:

- normal Lua control flow, tables, functions, metatables, and `require` remain
  valid foundations;
- emitted code remains inspectable and debuggable as Lua;
- `.lua` modules and LuaDoc annotations interoperate with `.tua` modules;
- dynamic code remains possible through `any` and conservative analysis;
- language additions emit LuaJIT- and LOVE-compatible code.

## Runtime And Emission

The default target is `love11-luajit`. Generated code uses Lua 5.1-compatible
syntax and avoids Lua 5.2+ runtime-only behavior.

Tua's type syntax is erased. Straightforward Tua code requires no Tua runtime.
Some source-used built-ins lower to ordinary Lua APIs backed by vendored
modules:

- `vec2(...)` emits a `brinevector` require when used;
- `signal()` emits a `tuasignal` require when used;
- `spriteSheet({...})` emits an `anim8` require when used;
- the built-in `ecs` module emits a `tuaecs` require when used;
- `objectPool({...})` emits a `tuaobjectpool` require when used.

The build writes a required bundled module only when emitted code uses that
feature, and it does not overwrite a project-provided module with the same
name. This helper model is part of Tua's build output, not a replacement VM or
an always-present runtime.

Lowering preserves source provenance. Source maps and `tua traceback` translate
generated Lua locations back to Tua source locations where mappings are
available.

## Project Model

The default project layout is:

```text
.
├── src/
│   └── main.tua
├── build/
│   └── main.lua
└── tua.toml
```

By default:

- `.tua` files under `source` are parsed, checked, lowered, and emitted under
  `out` with the configured extension changed to `.lua`;
- the configured `main` source file also emits to `<out>/main.lua` for LOVE
  entrypoint loading while retaining its normal mirrored output path;
- non-Tua files are copied as assets when source and output roots differ;
- `.lua` files participate in project analysis when their LuaDoc or inferred
  exports provide stable facts;
- source/output collisions produce diagnostics rather than silently selecting
  ambiguous inputs;
- watch mode updates a long-lived project analysis session and rechecks affected
  reverse dependencies.

`tua.toml` is parsed as TOML through a typed schema. Unknown settings produce
diagnostics with suggestions. [Configuration](configuration.md) is the
generated user-facing setting reference.

## Language Surface

Tua accepts Lua syntax plus these erased or LuaJIT-safe additions:

- local, parameter, return, and field type annotations;
- type aliases, table shapes, optional fields, arrays, dictionaries, tuples,
  unions, literal types, and native function types;
- typed varargs and default parameters;
- classes, inheritance, final classes/methods, and enums;
- compound assignment operators;
- runtime `is` checks for supported primitive, alias, metatable, and class
  identities;
- static imports that lower to ordinary `require` calls;
- LOVE-focused built-ins expressed as ordinary calls or declarations.

Within class initializers and instance methods, `self` has the enclosing class
type, including inherited fields and methods.

The semantic type model is gradual:

- `any` is the explicit dynamic escape hatch;
- annotated values are checked against their declared contracts;
- inferred Tua locals keep a stable widened contract across reassignment;
- control-flow narrowing applies where the compiler can prove the condition;
- unknown dynamic fields and metatable behavior remain conservative rather
  than guessed;
- ordinary `.lua` sources remain permissive unless annotations establish a
  contract.

Generics are not part of Tua. Native function types and LuaDoc `fun(...)`
annotations share one semantic representation. Arrays use native `T[]` syntax;
grouped union and function elements use `(T | U)[]` and
`((value: T) -> U)[]`. An empty table literal can initialize an explicitly
typed array or dictionary.

Class fields accept the same native type surface. Constructors return the
declared class type, method signatures are checked exactly, and class exports
retain constructor/member types through static project modules. Classes are
closed by default: only `open class` declarations can be extended.

## Modules And Lua Interoperability

Normal `require("module")` is the runtime module mechanism. Tua resolves static
module strings through configured project roots and indexes exports for
checking and editor navigation.

Static `import` syntax is convenience syntax that lowers to `require`; it does
not introduce a separate module runtime. Tua does not add JavaScript-style
`export` declarations or type-only runtime modules.

LuaDoc/LuaCATS annotations support practical aliases, classes, fields,
functions, overloads, typed varargs, and module exports. Unsupported annotations
degrade conservatively, and semantic analysis does not execute project code.

## LOVE Semantics

LOVE 11.x APIs are generated from one pinned `love-api` revision. The generated
catalog owns function signatures, overloads, enums, callback identity, object
methods, and inheritance used by checking and editor features.

Top-level `love.<callback>` declarations use that catalog for snippets,
documentation, and validation. Lua callbacks can omit unused parameters. The
checker reports declarations that require values LOVE does not supply or use
explicit parameter annotations incompatible with the generated callback type.
Close callback-name typos use stable domain diagnostic `TL5006`.

LOVE asset paths are resolved relative to configured asset roots. Supported
loader calls provide diagnostics, completion, hover metadata, definition, and
references without executing project code.

Domain helpers lower to ordinary Lua-compatible APIs. `spriteSheet({...})`,
`signal()`, and the built-in `ecs` module are first-class compiler domains
rather than executable third-party plugins. Experimental editor presentation
is explicitly preview-gated.

The sprite domain describes anim8 grid cell size, left/top origin, border
spacing, zero-based row/column ranges, per-animation grid overrides, numeric or
table durations, and `onLoop` method strings or callbacks. One shared semantic
model supplies these facts to diagnostics, completion, inspection, preview,
and Lua emission.

## CLI

The `tua` executable provides:

- `init` for project scaffolding;
- `check` for project diagnostics;
- `fmt` and `fmt --check` for formatting;
- `lint` and `lint --strict` for lint profiles;
- `build` and `build --emit-on-error` for Lua emission;
- `watch` for incremental builds;
- `traceback` for source-map remapping;
- `lsp` for stdio language-server operation;
- `--version` for the coordinated toolchain version.

A successful default build does not emit files that have blocking errors.
`--emit-on-error` is an explicit recovery mode and still exits unsuccessfully
when errors exist.

## Editor Integration

The language server is the semantic authority for diagnostics, completion,
hover, signature help, definitions, references, rename, symbols, semantic
tokens, inlay hints, code actions, formatting, file operations, and domain
inspection.

The VS Code extension presents those results without duplicating Tua parsing or
type inference. Requests capture a document revision; stale background results
are cancelled or discarded after newer edits. Compiler facts determine valid
candidates before deterministic ranking signals reorder completion.

No source content or completion-learning data is sent off-device by default.
Completion providers are local-only. The model-integration setting is an inert
policy gate and does not enable a network-backed provider.

## Diagnostics

Diagnostics use stable `TLxxxx` codes grouped by syntax, type, asset/build,
lint/project, and domain families. Suppression comments can target exact codes
or documented groups but cannot alter inference, lowering, or runtime behavior.

The compiler and language server use the same semantic diagnostics for an
identical project snapshot. The editor presents those diagnostics without a
parallel type-checking path.

## Compatibility And Boundaries

Compatibility targets are Lua 5.1-style syntax, LuaJIT, and LOVE 11.x.
Pre-1.0 releases follow the migration policy in [Releases](releases.md).

Tua does not provide:

- a new Lua VM or Lua 5.2+ output target;
- a package manager;
- whole-program optimization or minification;
- arbitrary execution of project code for semantic discovery;
- a Lua debugger or runtime profiler;
- perfect proof of dynamic keys, metatables, or shape-changing tables;
- an executable third-party plugin registry;
- network-required core completion;
- full LuaHelper or Luau compatibility.

---
icon: lucide/book-open-check
---

# Tua Language And Toolchain Specification

This is the canonical specification for current Tua behavior. It defines the
contract shared by the compiler, CLI, language server, and editor extension.
Detailed syntax examples live in [Language](language.md), and implemented
surface status lives in [Capabilities](capabilities.md).

## Purpose

Tua is a gradual typed Lua superset that emits readable Lua 5.1-compatible
source for LuaJIT and LOVE 11.x projects. It is source-to-source tooling, not a
new VM. Types improve diagnostics and editor intelligence and are erased before
runtime.

Tua must preserve ordinary Lua workflows:

- normal Lua control flow, tables, functions, metatables, and `require` remain
  valid foundations;
- emitted code remains inspectable and debuggable as Lua;
- `.lua` modules and LuaDoc annotations interoperate with `.tua` modules;
- dynamic code remains possible through `any` and conservative analysis;
- language additions must remain compatible with LuaJIT and LOVE.

## Runtime And Emission

The default target is `love11-luajit`. Generated code must avoid Lua 5.2+
runtime-only syntax and behavior.

Tua's type syntax is erased. Straightforward Tua code requires no Tua runtime.
Some source-used built-ins lower to ordinary Lua APIs backed by vendored
modules:

- `vec2(...)` can emit a `brinevector` require;
- `signal()` can emit a `tuasignal` require;
- `spriteSheet({...})` can emit an `anim8` require;
- the built-in `ecs` module can emit a `tuaecs` require.
- `objectPool({...})` can emit a `tuaobjectpool` require.

The build writes a required bundled module only when emitted code uses that
feature, and it does not overwrite a project-provided module with the same
name. This helper model is part of Tua's build output, not a replacement VM or
an always-present runtime.

Lowering must preserve source provenance. Source maps and `tua traceback`
translate generated Lua locations back to Tua source locations where mappings
are available.

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

The semantic type model is gradual:

- `any` is the explicit dynamic escape hatch;
- annotated values are checked against their declared contracts;
- inferred Tua locals keep a stable widened contract across reassignment;
- control-flow narrowing applies where the compiler can prove the condition;
- unknown dynamic fields and metatable behavior remain conservative rather
  than guessed;
- ordinary `.lua` sources remain permissive unless annotations establish a
  contract.

Generics are not part of the current language. Native function types and
LuaDoc `fun(...)` annotations share one semantic representation.

## Modules And Lua Interoperability

Normal `require("module")` is the runtime module mechanism. Tua resolves static
module strings through configured project roots and indexes exports for
checking and editor navigation.

Static `import` syntax is convenience syntax and must lower to `require`; it
does not introduce an incompatible module runtime. Tua does not add JavaScript-
style `export` declarations or type-only runtime modules.

LuaDoc/LuaCATS annotations support practical aliases, classes, fields,
functions, overloads, typed varargs, and module exports. Unsupported annotations
must degrade conservatively and must not execute project code.

## LOVE Semantics

LOVE 11.x APIs are generated from one pinned `love-api` revision. The generated
catalog owns function signatures, overloads, enums, callback identity, object
methods, and inheritance used by checking and editor features.

Top-level `love.<callback>` declarations use that catalog for snippets,
documentation, and validation. Lua callbacks may omit unused parameters, but a
declaration must not require values LOVE does not supply, and explicit
parameter annotations must accept the corresponding generated type. Close
callback-name typos use stable domain diagnostic `TL5006`.

LOVE asset paths are resolved relative to configured asset roots. Supported
loader calls can provide diagnostics, completion, hover metadata, definition,
and references without executing project code.

Domain helpers must prefer ordinary Lua-compatible APIs. `spriteSheet({...})`,
`signal()`, and the built-in `ecs` module are first-class compiler domains
rather than executable third-party plugins. Experimental editor presentation
remains explicitly preview-gated.

## CLI Contract

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

## Editor Contract

The language server is the semantic authority for diagnostics, completion,
hover, signature help, definitions, references, rename, symbols, semantic
tokens, inlay hints, code actions, formatting, file operations, and domain
inspection.

Editor clients may present those results but must not duplicate Tua parsing or
type inference. Requests capture a document revision; stale background results
are cancelled or discarded after newer edits. Compiler facts determine valid
candidates before deterministic ranking signals can reorder completion.

No source content or completion-learning data is sent off-device by default.
Any future model integration requires explicit configuration and policy gates.

## Diagnostics

Diagnostics use stable `TLxxxx` codes grouped by syntax, type, asset/build,
lint/project, and domain families. Suppression comments can target exact codes
or documented groups but cannot alter inference, lowering, or runtime behavior.

The compiler and language server must use the same semantic diagnostics for an
identical project snapshot. Editor-only presentation must not invent a parallel
type-checking path.

## Compatibility And Non-Goals

Current compatibility targets are Lua 5.1-style syntax, LuaJIT, and LOVE 11.x.
Pre-1.0 releases follow the migration policy in [Releases](releases.md).

The following are not current goals:

- a new Lua VM or Lua 5.2+ output target;
- a package manager;
- whole-program optimization or minification;
- arbitrary execution of project code for semantic discovery;
- perfect proof of dynamic keys, metatables, or shape-changing tables;
- an executable third-party plugin registry;
- network-required core completion;
- full LuaHelper or Luau compatibility.

The historical initial v1 contract remains available in
[Historical Tua v1 Specification](archive/specification-v1-initial.md). It is a
record of earlier decisions, not the current normative source.

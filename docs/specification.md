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
- `objectPool({...})` emits a `tuaobjectpool` require when used;
- `stateMachine({...})` emits a `tuastatemachine` require when used;
- `store({...})` emits a `tuastore` require when used;
- `input({...})` emits a `baton` require when used.

The build writes a required bundled module only when emitted code uses that
feature, and it does not overwrite a project-provided module with the same
name. This helper model is part of Tua's build output, not a replacement VM or
an always-present runtime.

Lowering preserves source provenance. Tua writes its `*.lua.map.json` format
for navigation and traceback remapping plus standard `*.lua.map` v3 sidecars
for compatible Lua debuggers. Both translate generated Lua locations back to
Tua source where mappings are available.

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

Configured declaration-only type libraries recursively index local `.lua`
declarations. A declaration `---@meta` identity is mapped from its configured
declaration prefix to the runtime prefix used by `require(...)`. Declaration
files contribute types and editor semantics only: they are never emitted,
copied, executed, packaged, or fetched from the network. Static `import(...)`
continues to require a real project module.

LuaDoc/LuaCATS annotations support practical aliases, classes, fields,
functions, overloads, typed varargs, and module exports. Unsupported annotations
degrade conservatively, and semantic analysis does not execute project code.
Explicit `---@deprecated` metadata follows resolved local functions, fields, and
module exports into diagnostics and editor presentation; Tua does not guess
deprecation from API documentation prose.
Recursive aliases preserve their outer declared shape while the repeated alias
on the active expansion path degrades to `any`, keeping external declaration
libraries finite and editor-safe.

An expected table type also drives constructor completion. Known union members
contribute fields and string-literal values; existing literal fields narrow
discriminated unions; array, dictionary, tuple, and named-field steps propagate
the expectation into nested constructors; and expected function fields provide
callback parameter types.

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
`signal()`, `stateMachine({...})`, `store({...})`, `input({...})`, and the
built-in `ecs` module are first-class compiler domains rather than executable
third-party plugins. Experimental editor presentation is explicitly
preview-gated.

Static `store({...})` calls derive a structural state type. The type is reused
by reads, replacements, mutators, whole-store subscriptions, keyed watcher
overloads, completion, hover, signature help, checking, and module exports.
Stores are explicit runtime instances and have no built-in LOVE filesystem or
serialization behavior.

Static state-machine configuration derives exact state and event literal
unions, event methods, callback names, and state/event argument completion.
Computed configuration remains valid through the generic gradual
`StateMachine` surface; analysis does not execute configuration code to infer
dynamic names.

Static `input({...})` configuration derives exact Baton control and pair names.
Control lookups return one number, pair lookups return x and y, active-device
queries return `"kbm" | "joy" | "none"`, and LOVE-backed source strings receive
completion and validation. Computed configuration remains valid through the
generic gradual `Input` surface.

The sprite domain describes anim8 grid cell size, left/top origin, border
spacing, zero-based row/column ranges, per-animation grid overrides, numeric or
table durations, and `onLoop` method strings or callbacks. One shared semantic
model supplies these facts to diagnostics, completion, inspection, preview,
and Lua emission.

## CLI

The `tua` executable provides:

- `init` for project scaffolding;
- `check` for project diagnostics;
- `fmt` and `fmt --check` for canonical formatting, independently of diagnostics;
- `lint` for the standard lint rules and `lint --strict` for those rules
  plus every Tua-specific optional lint;
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

LOVE debugging remains Lua debugging. The extension builds generated Lua,
prepares a build-output-only debugger bootstrap, and launches the optional
Second Local Lua Debugger adapter. Tua source maps bind `.tua` breakpoints and
map stopped stack frames back to source; the external adapter owns execution,
stepping, variables, watches, and evaluation. VS Code `lua-local` launch
configurations opt into Tua's build and map preparation with
`tuaProject: true`.

No source content or completion-learning data is sent off-device by default.
Completion providers are local-only. The model-integration setting is an inert
policy gate and does not enable a network-backed provider.

## Diagnostics

Diagnostics use stable `TLxxxx` codes grouped by syntax, type, asset/build,
lint/project, and domain families. Suppression comments can target exact codes
or documented groups but cannot alter inference, lowering, or runtime behavior.
The standard lint rules are enabled by default for checking and editor
diagnostics; projects can disable that set without disabling type checking.
The unknown-global lint recognizes explicit global assignments and simple
global function declarations anywhere in the current Tua file; runtime-provided
and cross-file globals remain explicit project configuration.
Builtin-global-write guidance resolves lexical bindings and protects direct
writes to the Lua 5.1/LuaJIT, LOVE, and Tua helper globals modeled by the target.
The exact `_` binding is a write-only placeholder: writes are accepted without
global-local guidance, while reads recommend choosing a named variable.
Global-local guidance follows common lexical ancestry across nested functions
and remains conservative around module-scope and conditionally initialized
globals.
Local-shadow guidance is limited to used bindings in the same function and
user globals referenced in the file; runtime-provided globals and nested
function boundaries remain permissive.
Table-literal guidance compares literal keys by LuaJIT runtime identity,
including escaped strings, signed numbers, and implicit list indices. The same
diagnostic reports repeated named fields in Tua table type shapes when the
later field would otherwise silently replace the earlier contract.
Duplicate-local guidance uses `TL4043` for repeated names in local declarations
and parameter lists, accounts for implicit method `self`, and exempts the exact
placeholder `_`.
Duplicate-function guidance compares local, global, and qualified declarations
within each lexical block and cites the original definition for every repeat.
Uninitialized-local guidance tracks each binding's initializer separately,
allows a final direct call or varargs to supply additional values, and treats a
compound assignment as a read of the previous value rather than initialization.
Unbalanced-assignment guidance compares explicit expression counts and remains
permissive for direct-call, vararg, and intentional-`nil` tails.
Implicit-return guidance recognizes guaranteed `repeat` bodies and unbreakable
literal infinite loops, while reachable `break` paths preserve fallthrough.
Unknown-runtime-type guidance follows resolved builtin identity: LuaJIT
`type(...)` names and statically known LOVE object `type`/`typeOf` names are
validated against their target catalogs, while named runtime checks remain the
responsibility of Tua's `is` checker.
Numeric-for guidance evaluates constant LuaJIT bounds with the implicit `1`
step, keeps dynamic bounds permissive, and defers to any explicitly written
step.
Format-string guidance resolves the builtin `string` library before validating
literal LuaJIT/Lua 5.1 printf formats, patterns, captures, and `gsub`
replacements. It also recognizes literal `("..."):format(...)` calls, skips
plain `string.find` searches, and leaves host-specific LuaJIT
`os.date`/`strftime` conversions permissive.
Unused-local analysis includes reads from default-parameter expressions and
nested closures, excludes loop bindings and `_`-prefixed names, and only offers
declaration removal when the initializer is safe to discard.
Unused-function analysis covers direct local and unqualified user-global
declarations, uses its own stable diagnostic code, and offers complete
declaration removal for compiler-proven source ranges.
Unused-import analysis distinguishes the global `require` from lexically
shadowed functions, accepts computed module names, and counts qualified type
references as uses. Import source actions operate only on compiler-resolved
static bindings and preserve dynamic or shadowed calls.

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
- a bundled Lua debugger, VM, or runtime profiler;
- perfect proof of dynamic keys, metatables, or shape-changing tables;
- an executable third-party plugin registry;
- network-required core completion;
- full compatibility with other Lua language servers or typed Lua dialects.

---
icon: lucide/circle-alert
---

# Diagnostic Codes

This page defines Tua's stable `TLxxxx` error, warning, and hint codes. The
compiler, CLI, language server, and VS Code extension use the same codes for the
same semantic conditions.

!!! info "Canonical user-facing reference"
    The implementation registry lives in
    `crates/tua-core/src/diagnostic/mod.rs`. This page defines what each
    registered code means and how users control it.

## Reading A Diagnostic

CLI diagnostics use this shape:

```text
src/player.tua:12:15 error TL1003: type mismatch: expected number, found string
```

- The path and position identify the primary source range.
- The severity is `error`, `warning`, `info`, or `hint`.
- The `TLxxxx` code identifies the condition independently of wording.
- The message explains the values, names, paths, or constraints involved.

Errors block normal build emission. Warnings and hints do not block emission,
but `check` still fails when enabled diagnostics are present. `lint` considers
syntax and `TL4xxx` lint diagnostics only.

!!! note "A code can have more than one severity"
    The code identifies a condition, not a fixed severity. For example,
    `TL2003` is an error when two Tua sources target the same output, but a
    warning when a copied Lua asset would overwrite generated Lua. `TL5006` is
    a warning for a likely callback-name typo and an error for an incompatible
    callback declaration.

## Code Ranges

| Range | Category | Covers |
| --- | --- | --- |
| `TL0001-TL0999` | Syntax | Parsing, Tua syntax normalization, and malformed annotations. |
| `TL1000-TL1999` | Type | Type names, assignments, calls, classes, and runtime checks. |
| `TL2000-TL2999` | Build and assets | Lowering, emission, filesystem output, and asset validation. |
| `TL3000-TL3999` | Project configuration | Reading and validating `tua.toml`. |
| `TL4000-TL4999` | Lint and guidance | Optional correctness/style checks, module diagnostics, and LuaJIT guidance. |
| `TL5000-TL5999` | Game domains | LOVE callbacks, sprites, signals, ECS, and object pools. |

Unused numbers are reserved. A number appearing inside a category range is not
valid unless it is listed below.

## Syntax Codes

| Code | Meaning | Control |
| --- | --- | --- |
| `TL0001` | Source cannot be parsed as valid Lua/Tua syntax. | Always enabled. |
| `TL0002` | Compound-assignment syntax or its target is invalid. | Always enabled. |
| `TL0003` | A supported LuaDoc annotation is malformed. | `diagnostics.doc_annotations` |

## Type Codes

| Code | Meaning | Control |
| --- | --- | --- |
| `TL1001` | A referenced type name is unknown in the current scope. | Always enabled. |
| `TL1002` | A type alias or class-backed type name is declared more than once. | Always enabled. |
| `TL1003` | A value is not assignable to its declared or inferred type contract. | Always enabled. |
| `TL1004` | A statically known function call has the wrong argument count. | Always enabled for core typed calls. |
| `TL1005` | A comparison is provably impossible because the operand types do not overlap. | Always enabled; warning. |
| `TL1006` | LuaDoc declarations conflict while building one type environment. | Always enabled. |
| `TL1007` | A runtime `is` check cannot be represented safely for the requested type. | Always enabled. |
| `TL1008` | A class declaration, inheritance relation, override, constructor, or member access violates class rules. | Always enabled. |

`TL1003` is the general type-contract error used for annotated assignments,
stable inferred locals, fields, arguments, returns, and compatible container
operations. More specialized optional lint codes such as `TL4016` and
`TL4017` are used by the configurable lint pipeline.

## Build And Asset Codes

| Code | Meaning | Control |
| --- | --- | --- |
| `TL2001` | A compound assignment parsed successfully but cannot be lowered safely. | Always enabled. |
| `TL2002` | Tua could not read or write a source, asset, map, or emitted output file. | Always enabled. |
| `TL2003` | Multiple inputs target one output path, or a copied file would overwrite generated Lua. | Always enabled; severity depends on the collision. |
| `TL2004` | A statically known asset path is empty or does not exist. | Always enabled for supported asset loaders. |
| `TL2005` | An asset extension is unsupported by the selected loader. | Always enabled for supported asset loaders. |
| `TL2006` | An asset exists but belongs to the wrong asset kind for the selected loader. | Always enabled for supported asset loaders. |
| `TL2007` | Internal lowering operations overlap or cannot be applied safely. | Always enabled. |
| `TL2008` | Asset-path letter casing differs from the filesystem's canonical path. | Always enabled; warning. |
| `TL2009` | A statically indexed asset has no known source reference. | Asset Explorer analysis; hint. |

## Configuration Codes

| Code | Meaning | Control |
| --- | --- | --- |
| `TL3001` | `tua.toml` is malformed, contains an unknown setting, or uses an invalid value. | Always enabled. |
| `TL3002` | `tua.toml` could not be read. | Always enabled. |

## Lint And Guidance Codes

The standard `TL4xxx` lint set is enabled by default through
`diagnostics.rules`; Tua-specific guidance outside that set remains opt-in
through project/editor settings. The `tua lint` command always enables the
standard set for its own run. Formatting has no diagnostic codes and is
enforced separately with `tua fmt --check`.

| Code | Meaning | Configuration |
| --- | --- | --- |
| `TL4001` | A literal table declares the same key more than once. | `duplicate_keys` |
| `TL4002` | A static `require` or `import` target cannot be resolved. | `missing_requires` |
| `TL4003` | An assignment writes a value back to the same variable or field. | `self_assignments` |
| `TL4004` | A variable read has no known declaration or allowed global. | `undefined_variables` |
| `TL4005` | A non-function local is declared but never used. | `unused_locals` |
| `TL4006` | A function parameter is never used. | `unused_parameters` |
| `TL4007` | A local is read before its declaration. | `use_before_definition` |
| `TL4008` | A function declares the same parameter more than once. | `duplicate_parameters` |
| `TL4009` | A lint-known call has the wrong argument count. | `wrong_argument_count` |
| `TL4010` | A static module specifier is invalid or resolves through an unsupported form. | `bad_requires` |
| `TL4011` | A boolean expression is provably constant or impossible. | `impossible_boolean_expressions` |
| `TL4012` | An `if` or `elseif` chain repeats the same condition. | `duplicate_if_conditions` |
| `TL4013` | A suspicious binary expression repeats the same operand. | `duplicate_binary_operands` |
| `TL4014` | Code compares a floating-point literal for exact equality. | `float_equality` |
| `TL4015` | Code assigns to a name recognized as constant. | `const_assignments` |
| `TL4016` | A known function argument does not match its parameter type. | `parameter_type_mismatches` |
| `TL4017` | A function return does not match its declared return type. | `return_type_mismatches` |
| `TL4018` | A known LOVE object method uses `.` instead of `:`, so `self` would not be passed. | Always enabled; warning with quick fix. |
| `TL4019` | More than one project file can satisfy the same module specifier. | `bad_requires` |
| `TL4020` | The static module graph contains a require/import cycle. | `bad_requires` |
| `TL4025` | An inline-ignore directive, scope, group, or code is invalid. | `inline_ignores` |
| `TL4026` | An inline-ignore directive did not suppress any matching diagnostic. | `inline_ignores` |
| `TL4027` | Assignment target count differs from the statically known value count. | `assignment_arity` |
| `TL4028` | Local declaration count differs from the statically known initializer count. | `local_declaration_arity` |
| `TL4029` | A value assigned to a local is never read. | `unused_assignments` |
| `TL4030` | A binary operator receives statically incompatible operand types. | `binary_operator_type_mismatches` |
| `TL4031` | A local function is referenced but never directly called. | `uncalled_local_functions` |
| `TL4032` | A local changes between LuaJIT-unfriendly value types. | `jit.enabled` and `jit.variable_type_drift` |
| `TL4033` | A table gains late fields or changes known field types in a LuaJIT-unfriendly way. | `jit.enabled` and `jit.table_shape_drift` |
| `TL4034` | A hot loop writes through a dynamic table key. | `jit.enabled` and `jit.hot_loop_hints` |
| `TL4035` | A used local shadows a declaration in the same function or a user global referenced in the file. | `rules` |
| `TL4036` | Adjacent statements share a line without an explicit separator. | `rules` |
| `TL4037` | A continuation line is not indented relative to its statement. | `rules` |
| `TL4038` | A direct write overwrites a protected LuaJIT, LOVE, or Tua builtin global. | `rules` |
| `TL4039` | Placeholder `_` is read as a value. | `rules` |
| `TL4040` | A statement is unreachable. | `rules` |
| `TL4041` | A `type`/`typeof` comparison uses an unknown runtime type name. | `rules` |
| `TL4042` | A numeric `for` range likely needs a different bound or explicit step. | `rules` |
| `TL4043` | One local declaration repeats a name. | `rules` |
| `TL4044` | A static format string, pattern, or date format is invalid. | `rules` |
| `TL4045` | A local is read but never initialized. | `rules` |
| `TL4046` | A function name is defined more than once in one block. | `rules` |
| `TL4047` | A `table.insert` or `table.remove` index is invalid, redundant, or suspicious. | `rules` |
| `TL4048` | `a and false/nil or b` cannot preserve the false/nil result. | `rules` |
| `TL4049` | A `--!` comment directive is unsupported by Tua. | `rules` |
| `TL4050` | An integer literal loses precision as a Lua number. | `rules` |
| `TL4051` | Comparison precedence is ambiguous or behaves unlike a chained comparison. | `rules` |
| `TL4052` | A global can be local to its common enclosing function, or every read follows a definite function-local write. | `rules` |
| `TL4053` | A function returns values on one path and implicitly returns none on another. | `rules` |
| `TL4054` | A direct global `require` or static `import` binding is unused. | `rules` |
| `TL4055` | A direct local or user-global function declaration is never used. | `unused_locals` |

Configuration keys in this table live under `[diagnostics]` unless they start
with `jit.`. VS Code can override matching diagnostic settings for the running
language server without rewriting `tua.toml`.

## Domain Codes

| Code | Meaning | Control |
| --- | --- | --- |
| `TL5002` | An ECS component value does not match its declared static schema. | Built-in ECS analysis. |
| `TL5003` | An ECS query or system references an unknown or invalid component. | Built-in ECS analysis. |
| `TL5004` | A `spriteSheet({...})` declaration has invalid frame, animation, duration, selector, or loop configuration. | Sprite-sheet analysis. |
| `TL5005` | One SignalBus event is used with conflicting payload schemas. | Signal analysis. |
| `TL5006` | A LOVE callback name is likely misspelled or its declared signature is incompatible with LOVE. | LOVE callback analysis; severity depends on the condition. |
| `TL5007` | A directly visible object-pool acquisition has no later release or pool clear in the same file. | Object-pool analysis; conservative warning. |
| `TL5008` | An `input({...})` declaration has an invalid Baton source, pair, control reference, duplicate name, or deadzone. | Baton input analysis. |

## Enable Optional Diagnostics

Optional lints can be enabled individually:

```toml title="tua.toml"
[diagnostics]
undefined_variables = true
unused_locals = true
missing_requires = true
parameter_type_mismatches = true
```

Use `tua lint --strict` to enable every optional lint for that command. Inline
ignore behavior remains controlled by project or editor configuration.

See [Configuration](configuration.md#diagnostics) for every setting and its
VS Code override.

## Suppress A Diagnostic

Inline suppression is disabled by default. Enable it first:

```toml title="tua.toml"
[diagnostics]
inline_ignores = true
```

Then target one code:

```tua
-- tua-ignore-next-line TL1003
local legacyCount: number = dynamicValue()

player.setFilter("nearest") -- tua-ignore-line TL4018
```

Directives can also target category groups such as `TL1000`, `TL2000`,
`TL3000`, `TL4000`, and `TL5000`, or names such as `syntax`, `type`,
`lowering`, `config`, `lint`, and `domain`. `TL0000` is the numeric syntax
group.

!!! warning "Suppressions only hide diagnostics"
    An inline ignore never changes inference, lowering, emitted Lua, or runtime
    behavior. Invalid directives report `TL4025`, and directives that match
    nothing report `TL4026`.

## Quick Fixes

Some diagnostics expose conservative editor fixes:

| Code | Available fix |
| --- | --- |
| `TL4001` | Rename a duplicate table key. |
| `TL4002` | Create a missing project module. |
| `TL4003` | Remove a self-assignment. |
| `TL4005` | Remove an unused local. |
| `TL4018` | Replace a LOVE dot-method call with colon syntax. |
| `TL5006` | Rename a close LOVE callback typo. |

Not every diagnostic has a safe mechanical fix. Tua omits an action when the
correct edit depends on runtime intent.

## Maintaining The Registry

When adding or changing a diagnostic:

1. Add or update the `DiagnosticCode` constant and `is_known` registry in
   `crates/tua-core/src/diagnostic/mod.rs`.
2. Keep the code inside its category range and preserve existing code meanings.
3. Add focused core tests for code, severity, range, and message behavior.
4. Update this page and any related configuration or feature documentation.
5. Add exact LSP or code-action tests when the editor surface changes.

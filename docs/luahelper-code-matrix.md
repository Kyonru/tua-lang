---
icon: lucide/circle-alert
---

# Diagnostic Codes

This page defines Tua's stable `TLxxxx` error, warning, and hint codes. The
compiler, CLI, language server, and VS Code extension use the same codes for the
same semantic conditions.

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
| `TL0001` | Source cannot be parsed as valid Lua/Tua syntax, including reserved keywords where an identifier is expected. | Always enabled. |
| `TL0002` | Compound-assignment syntax or its target is invalid. | Always enabled. |
| `TL0003` | A supported LuaDoc annotation is malformed. | `diagnostics.doc_annotations` |
| `TL0004` | The parser recognized a Luau grammar form that is not part of Tua. | Always enabled; blocks analysis and emission. |

## Type Codes

| Code | Meaning | Control |
| --- | --- | --- |
| `TL1001` | A referenced type name is unknown in the current scope. | Always enabled. |
| `TL1002` | A type alias or class-backed type name is declared more than once. | Always enabled. |
| `TL1003` | A value is not assignable to its declared or inferred type contract. | Always enabled. |
| `TL1004` | A statically known function call has the wrong argument count. A `.` call missing only `self` names the `:` call to use. | Always enabled for core typed calls. |
| `TL1005` | A comparison is provably impossible because the operand types do not overlap. | Always enabled; warning. |
| `TL1006` | LuaDoc declarations conflict while building one type environment. | Always enabled. |
| `TL1007` | A runtime `is` check cannot be represented safely for the requested type. | Always enabled. |
| `TL1008` | A class declaration, member syntax, inheritance relation, field initialization, override, constructor, or member access violates class rules. | Always enabled. |
| `TL1009` | An imported type collides with another visible type name. | Always enabled. |
| `TL1010` | Imported type aliases form a cycle. | Always enabled. |
| `TL1011` | A field, index, method, or call operation is not supported by every member of a union. Narrow the value before using the operation. | Always enabled. |
| `TL1012` | A statically resolved module table has no exported member with that name. Unresolved, dynamic, and `any` modules stay permissive. | Always enabled. |
| `TL1013` | A required plain Lua module does not parse, so its exports are untyped. The message names the first syntax error. | Always enabled; warning. |

`TL1003` is the general type-contract error used for annotated assignments,
stable inferred locals, fields, arguments, returns, and compatible container
operations. Nested failures report the field, dictionary, tuple, callback
parameter, or callback-return path and the incompatible leaf types; table
literals highlight the deepest corresponding value Tua can identify. Use an
exact `diagnostics.severity.TL1003` override to tune this shared contract
diagnostic instead of enabling a duplicate lint.

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
| `TL2011` | An asset-pipeline configuration, importer execution, manifest, cache entry, diagnostic, or generated output is invalid. | Enabled for configured asset pipelines. |
| `TL2012` | Built-in audio decoding or analysis failed, or its timing/channel metadata needs attention. | Enabled for `audio-analysis` pipelines. |
| `TL2009` | A statically indexed asset has no known source reference. | Asset Explorer analysis; hint. |
| `TL2010` | Completed compiler output failed the final Lua 5.1 safety validation. | Always enabled; blocks emission. |

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
| `TL4001` | A table constructor repeats a runtime-equivalent literal key, or a table type repeats a named field. | `duplicate_keys` |
| `TL4002` | A literal `require` or static `import` target cannot be resolved. | `missing_requires`; enabled by default |
| `TL4003` | An assignment writes a value back to the same variable or field. | `self_assignments` |
| `TL4004` | A variable read has no known declaration or allowed global. Error in Tua; warning in Lua. | `undefined_variables` |
| `TL4005` | A non-function local or loop binding is declared but never used. `_`-prefixed bindings are exempt. | `unused_locals`; local/loop checks also belong to `rules` |
| `TL4006` | A named function parameter or declared varargs are never used. `_`-prefixed named parameters are exempt. | `unused_parameters` |
| `TL4007` | A local is read before its declaration. | `use_before_definition` |
| `TL4008` | Legacy duplicate-parameter code retained for source compatibility; new diagnostics use `TL4043`. | No longer emitted. |
| `TL4010` | A static module specifier is malformed or resolves through an unsupported form. | Always enabled for malformed static imports |
| `TL4011` | A boolean expression is provably constant or impossible. | `impossible_boolean_expressions` |
| `TL4012` | An `if` or `elseif` chain structurally repeats a condition. | `rules`; can also use `duplicate_if_conditions` |
| `TL4013` | A flattened `and`/`or` chain or optional suspicious binary check structurally repeats an operand. | `rules`; broader operators can also use `duplicate_binary_operands` |
| `TL4014` | Code compares a floating-point literal for exact equality. | `float_equality` |
| `TL4015` | Code assigns to a name recognized as constant. | `const_assignments` |
| `TL4018` | A known LOVE object method uses `.` instead of `:`, so `self` would not be passed. | Always enabled; warning with quick fix. |
| `TL4019` | More than one project file can satisfy the same module specifier. | `bad_requires` |
| `TL4020` | The static module graph contains a require/import cycle. | `bad_requires` |
| `TL4025` | An inline-ignore directive, scope, placement, or exact code is invalid. | `inline_ignores` |
| `TL4026` | An inline-ignore directive did not suppress any matching diagnostic. | `inline_ignores` |
| `TL4027` | Assignment target count differs from the explicit value count, excluding multi-value and intentional-`nil` tails. | `assignment_arity` |
| `TL4028` | Local count differs from the explicit initializer count, excluding multi-value and intentional-`nil` tails. | `local_declaration_arity` |
| `TL4029` | An initializer, parameter value, or reassignment is overwritten before a read, or a final local assignment is never read. | `unused_assignments` |
| `TL4030` | A binary operator receives statically incompatible operand types. | `binary_operator_type_mismatches` |
| `TL4031` | A local function is referenced but never directly called; callback uses still count as uncalled. | `uncalled_local_functions`; opt-in even under `--strict` |
| `TL4032` | A local changes between LuaJIT-unfriendly value types. | `jit.enabled` and `jit.variable_type_drift` |
| `TL4033` | A table gains late fields or changes known field types in a LuaJIT-unfriendly way. | `jit.enabled` and `jit.table_shape_drift` |
| `TL4034` | A hot loop writes through a dynamic table key. | `jit.enabled` and `jit.hot_loop_hints` |
| `TL4035` | A used local shadows a declaration in the same function or a user global referenced in the file. | `rules` |
| `TL4036` | Adjacent statements share a line without an explicit separator. | `rules` |
| `TL4037` | A continuation line is not indented relative to its statement. | `rules` |
| `TL4038` | A direct write overwrites a protected LuaJIT, LOVE, or Tua builtin global. | `rules` |
| `TL4039` | Placeholder `_` is read as a value. | `rules` |
| `TL4040` | The first statement in a region is unreachable after paths that always return, break, or call a resolved terminating builtin. | `rules` |
| `TL4041` | A resolved LuaJIT `type(...)` comparison or known LOVE object `type`/`typeOf` call uses an unknown target-specific type name. | `rules` |
| `TL4042` | A numeric `for` range likely needs a different bound or explicit step. | `rules` |
| `TL4043` | One local declaration or function parameter list repeats a non-placeholder name, including an explicit method `self`. | `rules`; parameters can also use `duplicate_parameters` |
| `TL4044` | A literal LuaJIT/Lua 5.1 printf format, Lua pattern, capture, or `gsub` replacement is invalid. | `rules` |
| `TL4045` | A local is read without its own initializer or any plain assignment; a final direct call or varargs can initialize remaining bindings. | `rules` |
| `TL4046` | A function name is defined more than once in one block. | `rules` |
| `TL4047` | A resolved table operation uses a suspicious index or expanding final call, or known `#`/`ipairs` usage has no compatible array part. | `rules` |
| `TL4048` | An `and`-`or` expression cannot preserve its literal `false` or `nil` first alternative; use an `if` statement assignment. | `rules` |
| `TL4049` | A `--!` comment directive is unsupported by Tua. | `rules` |
| `TL4050` | An integer literal loses precision as a Lua number. | `rules` |
| `TL4051` | Comparison precedence is ambiguous or behaves unlike a chained comparison. | `rules` |
| `TL4052` | A global can be local to its common enclosing function, or every read follows a definite function-local write. | `rules` |
| `TL4053` | A function returns values on one path and has a reachable fallthrough path, including through a breakable loop. | `rules` |
| `TL4054` | A direct global `require`, static runtime import, or type-only import binding is unused. | `rules` |
| `TL4055` | A direct local or user-global function declaration is never used. | `unused_locals` |
| `TL4056` | A resolved function or field explicitly marked `---@deprecated` is used. | `rules` |
| `TL4057` | An explicit child constructor can return without calling its parent constructor. | `rules` |
| `TL4058` | An open class constructor directly calls an overridable method on `self`. | `rules` |
| `TL4059` | A type-only child field declaration redundantly repeats an inherited field contract. | `rules` |
| `TL4060` | `#` is applied to a built-in `Vec2`, which is `0` when vectors are plain tables; use `.length`. | `rules` |
| `TL4061` | A call begins on a new line and Lua parses it as continuation of the preceding expression. | `rules`; enabled by default |
| `TL4062` | A table field begins with call syntax after a missing comma, so Lua parses it as a call on the preceding field. | `rules`; enabled by default |
| `TL4063` | A plain module-scope assignment or function declaration creates an unscoped global in a Tua file. | `rules`; enabled by default |
| `TL4064` | Equality compares two separately created table or function values, making the result constant by identity. | `rules`; enabled by default |
| `TL4065` | A direct `if` and `else` execute identical non-empty runtime tokens. | `rules`; enabled by default |
| `TL4066` | A control-flow or named-function body is empty without a comment or conventional stub exemption. | `rules`; enabled by default |
| `TL4067` | A loop unconditionally terminates during its first iteration. | `rules`; enabled by default |
| `TL4068` | A direct call discards the result of a resolved function or module export marked `---@nodiscard`. | `rules`; enabled by default |
| `TL4069` | Two static module references resolve to the same file but use different final Lua runtime keys, which can duplicate module state. | `rules`; enabled by default |
| `TL4070` | A function exceeds the configured cyclomatic-complexity maximum. | `max_cyclomatic_complexity`; zero disables it |
| `TL4071` | Division uses a literal-zero denominator without the intentional literal `0 / 0` NaN form. | `divide_by_zero`; opt-in |
| `TL4072` | Two adjacent simple assignments look like a failed swap and reuse an overwritten value. | `almost_swapped`; opt-in |
| `TL4073` | A table literal mixes sequential entries with keyed fields. | `mixed_tables`; opt-in |
| `TL4074` | A write targets a field absent from a statically known record shape. | `unknown_record_fields`; opt-in |
| `TL4075` | A private type alias is not reachable from an annotation, class, or exported type. | `rules`; enabled by default |

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
| `TL5009` | An `audioSystem({...})` declaration has an invalid bus graph or unknown bus reference. | Audio-system analysis. |

## Enable Optional Diagnostics

Optional lints can be enabled individually:

```toml title="tua.toml"
[diagnostics]
undefined_variables = true
unused_locals = true
missing_requires = true

[diagnostics.severity]
TL1003 = "warning"
```

Use `tua lint --strict` to enable the broadly applicable optional lints for that
command. Direct-call-only and project-policy rules (`TL4031`, `TL4070` through
`TL4074`) remain opt-in. Inline ignore behavior remains controlled by project
or editor configuration.

See [Configuration](configuration.md#diagnostics) for every setting and its
VS Code override.

## Suppress A Diagnostic

Inline suppression is enabled by default and always targets exact diagnostic
codes:

```tua
-- tua-ignore-file TL4048

-- tua-ignore-next-line TL1003
local legacyCount: number = dynamicValue()

player.setFilter("nearest") -- tua-ignore-line TL4018
```

`tua-ignore-file` must be the first nonblank line. Every directive requires at
least one exact `TLxxxx` code; category names, numeric groups such as `TL4000`,
and empty directives report `TL4025`. Set `diagnostics.inline_ignores = false`
to forbid source suppressions, or restrict `inline_ignore_scopes` to selected
directive forms.

!!! warning "Suppressions only hide diagnostics"
    An inline ignore never changes inference, lowering, emitted Lua, or runtime
    behavior. Invalid directives report `TL4025`, and directives that match
    nothing report `TL4026`.

## Quick Fixes

Some diagnostics expose conservative editor fixes:

| Code | Available fix |
| --- | --- |
| `TL4001` | Manually rename a duplicate table key; excluded from safe fix-all because the intended key is ambiguous. |
| `TL4002` | Create a missing project module. |
| `TL4003` | Remove a self-assignment. |
| `TL4005` | Remove an unused local. |
| `TL4018` | Replace a LOVE dot-method call with colon syntax. |
| `TL5006` | Rename a close LOVE callback typo. |

Not every diagnostic has a safe mechanical fix. Tua omits an action when the
correct edit depends on runtime intent.

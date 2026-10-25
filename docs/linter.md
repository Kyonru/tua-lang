---
icon: lucide/list-checks
---

# Linter

The linter owns code-quality guidance. Its role is comparable to ESLint: it
reports suspicious constructs without rewriting presentation. `tua lint`
enables Tua's standard rules automatically and reports syntax and lint
diagnostics; use `tua check` for type, project, and domain diagnostics.

```bash
tua lint
tua lint --strict
```

`tua lint` fails on warning and error diagnostics after applying the project's
exact-code severity policy. By contrast, `tua check` fails only on errors; use
`tua check --deny-warnings` for a warnings-as-errors CI gate.

`--strict` adds the broadly applicable Tua-specific optional lints to the
standard rules. It deliberately leaves project-policy rules such as
`uncalled_local_functions`, complexity, literal-zero division, failed swaps,
and mixed tables opt-in. The standard rules are also enabled by default in
`tua check` and the editor. A project can opt out of that default without
changing `tua lint`:

```toml title="tua.toml"
[diagnostics]
rules = false
```

Individual diagnostic codes can be hidden or assigned a different severity:

```toml title="tua.toml"
[diagnostics.severity]
TL4035 = "allow"
TL4045 = "error"
TL4056 = "info"
```

Accepted levels are `allow`, `hint`, `info`, `warning`, and `error`. Overrides
apply consistently to the CLI and editor after inline suppression. Unknown
codes and levels are configuration errors, preventing misspelled policy from
silently doing nothing.

## Rules

Tua's standard rules cover portable Lua/Tua code-quality checks. Several rules
share an existing Tua code because they are the same analysis.

| Rule | Tua diagnostic |
| --- | --- |
| `UnknownGlobal` | `TL4004` |
| `GlobalUsedAsLocal` | `TL4052` |
| `LocalShadow` | `TL4035` |
| `SameLineStatement` | `TL4036` |
| `MultiLineStatement` | `TL4037` |
| `LocalUnused` | `TL4005` (locals and loop bindings) |
| `ParameterUnused` | `TL4006` (named parameters and varargs) |
| `FunctionUnused` | `TL4055` |
| `DeprecatedApi` | `TL4056` |
| `ParentConstructorPath` | `TL4057` |
| `VirtualCallInInit` | `TL4058` |
| `RedundantInheritedField` | `TL4059` |
| `ImportUnused` | `TL4054` (runtime and type-only imports) |
| `TypeUnused` | `TL4075` |
| `LoveCallbackOrder` | `TL4076` (its own `love_callback_order` switch) |
| `BuiltinGlobalWrite` | `TL4038` |
| `PlaceholderRead` | `TL4039` |
| `UnreachableCode` | `TL4040` |
| `UnknownType` | `TL4041` |
| `ForRange` | `TL4042` |
| `UnbalancedAssignment` | `TL4027`, `TL4028` |
| `ImplicitReturn` | `TL4053` |
| `DuplicateLocal` | `TL4043` |
| `FormatString` | `TL4044` |
| `TableLiteral` | `TL4001` |
| `UninitializedLocal` | `TL4045` |
| `DuplicateFunction` | `TL4046` |
| `TableOperations` | `TL4047` |
| `DuplicateCondition` | `TL4012`, `TL4013` |
| `MisleadingAndOr` | `TL4048` |
| `CommentDirective` | `TL4049` |
| `IntegerParsing` | `TL4050` |
| `ComparisonPrecedence` | `TL4051` |
| `VectorLength` | `TL4060` |
| `NewlineCall` | `TL4061` |
| `NewfieldCall` | `TL4062` |
| `UnscopedGlobalWrite` | `TL4063` |
| `ConstantIdentity` | `TL4064` |
| `IdenticalBranches` | `TL4065` |
| `EmptyBlock` | `TL4066` |
| `SingleIterationLoop` | `TL4067` |
| `MustUseResult` | `TL4068` |
| `CanonicalModuleSpelling` | `TL4069` |

The following opinionated rules are available only through their explicit
project or editor settings:

| Rule | Tua diagnostic |
| --- | --- |
| `HighCyclomaticComplexity` | `TL4070` |
| `DivideByZero` | `TL4071` |
| `AlmostSwapped` | `TL4072` |
| `MixedTable` | `TL4073` |
| `UnknownRecordField` | `TL4074` |

`UnknownGlobal` considers plain global assignments and simple global function
declarations anywhere in the current Tua file to be definitions, regardless of
source order. Qualified writes still require a known root, and compound
assignment reads its target before writing it. Runtime-provided or cross-file
globals should be listed in [`diagnostics.globals`](configuration.md#diagnostics).
In `.tua` files, `UnscopedGlobalWrite` separately warns on the first plain
module-scope assignment or unqualified function declaration that creates a
global. Builtins, configured globals, and qualified `love.*` callbacks are
exempt. Ordinary `.lua` files remain permissive, and an explicit `_G.name`
write continues to communicate deliberate global state.

`CanonicalModuleSpelling` compares the final keys passed to Lua's runtime
`require`, after static `import` calls have been resolved and lowered. It warns
when two spellings resolve to the same file but would occupy different
`package.loaded` entries, which can initialize stateful modules twice. It does
not compare raw import text, so a child-relative `import("./shared")` and its
lowered `require("shared")` key agree.

`NewlineCall` reports a call whose argument opener begins on a later line than
the expression it calls. Lua treats the lines as one chained expression even
when they look like separate statements. `NewfieldCall` is the table-field
variant, where a missing comma can silently turn adjacent-looking fields into
one call. The editor offers `;` or `,` separator fixes when they are safe to
place; both diagnostics always offer the standard exact-code suppression.

`ConstantIdentity` reports equality and inequality comparisons between two
fresh table or function literals. Lua compares those values by reference, so
separately created values cannot have the same identity. Comparisons through
variables remain permissive because the values may share a reference.

`IdenticalBranches` reports a direct `if`/`else` whose non-empty branches have
the same runtime tokens. Comments and presentation whitespace do not hide the
duplicate behavior. Chains containing `elseif` remain permissive until the
linter can compare their complete control flow without misleading advice.

`EmptyBlock` reports empty `if`, `elseif`, `else`, and loop bodies, plus empty
named functions. A comment anywhere in the construct records that the empty
body is deliberate. Anonymous callbacks, LOVE callbacks, underscore-prefixed
functions, class methods, and functions named `noop` or `no_op` are also
treated as deliberate stubs.

`SingleIterationLoop` reports a non-empty loop whose body unconditionally
returns, breaks, or calls a resolved terminating builtin during its first
iteration. Conditional exits and loops with a reachable continuing path remain
valid.

`HighCyclomaticComplexity` starts each function at one and counts `if` and
`elseif` branches, loops, `and`/`or` decision points, and Tua conditional
expressions. Nested functions are measured independently. Set
`max_cyclomatic_complexity` to a positive threshold to enable it; zero disables
the rule.

`DivideByZero` reports `/` with a literal-zero denominator when the numerator
is not also literal zero. `0 / 0` remains available as an intentional NaN
idiom, and dynamic denominators remain permissive.

`AlmostSwapped` recognizes adjacent, single-name assignments such as `a = b`
followed by `b = a`, where the second statement reads the already overwritten
value. It stays conservative around fields, indexes, calls, and multi-target
assignments.

`MixedTable` reports a literal that contains both implicit sequential entries
and keyed fields. Mixed tables are valid and common Lua, so this is a
project-policy warning rather than a standard rule.

`UnknownRecordField` reports writes such as `game.socre = 10` when the
receiver has a known non-empty record shape without that field. Empty tables,
dictionaries, dynamic `__index` tables, classes, and exact module exports keep
their existing semantics. Enable `unknown_record_fields` when a project treats
record-shaped tables as closed for mutation.

`LoveCallbackOrder` reports a top-level `love.load`, `love.update`, or
`love.draw` defined after a callback that LÖVE runs later. `love.run` calls
`love.load` once, then `love.update` and `love.draw` every frame. Both
`function love.draw()` and `love.draw = ...` count. Definitions inside
functions or conditional blocks are ignored. Order has no runtime effect, so
this is a readability warning. It is enabled by default; set
`love_callback_order = false` to turn it off.

`BuiltinGlobalWrite` reports direct assignments, compound assignments, and
simple global function declarations that overwrite protected Lua 5.1/LuaJIT,
LOVE, or Tua helper globals. It resolves lexical bindings first, so assigning a
predeclared local or parameter with builtin-like spelling remains valid. Field
writes such as `math.custom = value` do not replace the builtin root and are not
reported. The protected set follows Tua's LuaJIT/LOVE target rather than
completion-only entries from other Lua versions.

`PlaceholderRead` treats the exact name `_` as a write-only placeholder for
locals, parameters, loop bindings, and globals. Plain assignments can discard a
value without producing unused or global-local guidance. Reading `_`, including
through a compound assignment or as the base of a field access, reports the
warning and recommends a named variable. Longer `_`-prefixed names retain their
normal value semantics while remaining exempt from unused-binding guidance.

`UnreachableCode` reports the first statement in each unreachable region and
identifies whether the preceding path always returns, breaks, errors, or merges
those outcomes. Control-flow termination propagates through nested blocks even
when they already contain unreachable statements. Direct calls to the resolved
Lua builtins `error(...)` and `assert(false)` count as errors; lexically shadowed,
configured, or assigned globals with those names remain ordinary calls.

`UnknownType` validates string names compared with the resolved LuaJIT builtin
`type(...)`. Its accepted results are `nil`, `boolean`, `userdata`, `number`,
`string`, `table`, `function`, `thread`, and LuaJIT's `cdata`; Luau-only names
such as `vector` and `buffer` are not part of Tua's target. Tua does not invent a
global `typeof`, and lexically shadowed, configured, or assigned `type` functions
remain ordinary calls. For a statically known LOVE object, the same rule checks
`object:type()` comparisons and literal `object:typeOf(...)` arguments against
Tua's generated LOVE 11.x type catalog. Use Tua's `value is Type` syntax for
named metatable, class, and alias checks; unsupported `is` targets are reported
separately as `TL1007`.

`ForRange` checks numeric `for` loops that use Lua's implicit `1` step. It
reports constant bounds that count downward or cannot land exactly on the limit,
including decimal and hexadecimal-integer LuaJIT literals, and recognizes the
common `#items, 1` and `#items, 0` array mistakes. Dynamic bounds remain
permissive, and writing any explicit step silences this opinionated guidance.

`FormatString` validates literal LuaJIT/Lua 5.1 `string.format` formats, Lua
patterns, capture references, and literal `string.gsub` replacements on calls
to the resolved builtin `string` library. Literal receivers also support the
standard `("..."):format(...)` form. `string.find(..., true)` is a plain search,
and an unknown `plain` flag stays permissive. Pattern methods such as
`value:match(...)` are not checked because the receiver is not resolved as the
builtin library. `os.date` formats remain host-defined by LuaJIT's C `strftime`
integration and are therefore not labelled invalid by this portable rule.

`UnbalancedAssignment` compares explicit targets and values without guessing a
function's return count. A shortage ending in a direct call or varargs remains
valid Lua multi-return code, including intentional selection of a call's first
result. A trailing `nil` also records that missing values are intentional.
Unambiguous shortages recommend adding `nil`; excess values warn that some
results are unused.

`UnusedAssignment` follows straight-line reads and writes in source order. It
reports an initializer, parameter value, or reassignment that is overwritten
before any read, plus a final assignment whose value is never read. Conditional
paths are handled conservatively: Tua stays quiet unless the dead value is
proven on the analyzed path. The diagnostic describes the unused value only;
it does not imply that a right-hand-side call or other side effect can be
deleted.

`ImplicitReturn` reports a function that explicitly returns one or more values
on one path but can still reach its end without a return. Bare `return` and
`return nil` both make a path explicit, and resolved `error(...)` or
`assert(false)` calls terminate it. A `repeat` body that always terminates and
an unbreakable `while true` or `repeat ... until false` loop cannot fall
through; a reachable `break` restores the possible fallthrough. The diagnostic
names declared functions, cites the first valued-return line, and points to the
last statement on the fallthrough path.

`GlobalUsedAsLocal` follows the common enclosing function across nested
closures, so a value shared only by children of one function can still become a
local in that parent. It also reports globals whose reads in separate functions
all follow an unconditional assignment. Module-scope definitions and reads that
may occur before a conditional assignment remain valid global patterns.

`LocalShadow` reports a used local that hides an earlier declaration in the
same function, including a parameter or a sequential declaration in the same
block. It also reports locals that hide a user global referenced anywhere in
the file. Shadowing across function boundaries, unused locals, duplicate
declarations, Lua/LOVE builtins, and configured runtime globals do not produce
this warning.

`DuplicateLocal` reports repeated names in one local declaration or function
parameter list as `TL4043`. It identifies the original declaration by line or
column and recognizes the implicit `self` parameter of colon methods. Repeating
the exact placeholder name `_` remains valid. The `duplicate_parameters`
setting can enable the parameter check without enabling the complete standard
rule set.

`DuplicateFunction` reports repeated local, global, and qualified function
declarations in one lexical block. Separate branch and nested blocks remain
independent, while dot and colon declarations of the same method share one
qualified name. Every later duplicate identifies the original definition.

`DuplicateCondition` compares runtime expression structure rather than source
text. It flattens grouped `and` and `or` chains, treats equivalent numeric and
escaped-string literals consistently, and preserves meaningful whitespace
inside strings. The repeated condition is highlighted and identifies the
original line or column. The common Lua `value and value or fallback` ternary
idiom remains quiet. `TL4012` covers `if`/`elseif` chains, while `TL4013`
covers boolean chains and the optional broader `duplicate_binary_operands`
checks.

`MisleadingAndOr` reports `condition and false or fallback` and the equivalent
literal-`nil` form because Lua's `or` always selects the fallback instead of
preserving that first alternative. The diagnostic identifies the exact value,
points at the complete expression, and recommends an ordinary `if` statement
assignment so the rewrite remains valid LuaJIT/Lua 5.1 output:

```lua
local result
if condition then
  result = false
else
  result = fallback
end
```

Truthy alternatives such as `true` and `0` remain valid. Parenthesizing the
left side, as in `(condition and false) or fallback`, records intentional use
of Lua's fallback behavior and silences this opinionated warning.

`ComparisonPrecedence` reports an unparenthesized `not` on the left side of a
comparison and comparison chains such as `minimum <= value <= maximum`.
Equality warnings suggest the inverse operator, while relational chains suggest
the explicit Lua form `minimum <= value and value <= maximum`. The complete
expression is highlighted. Comparing two explicitly negated values, such as
`not left == not right`, remains a valid intentional boolean comparison, and
parentheses silence the warning when the original grouping is deliberate.

`VectorLength` reports `#` applied to a built-in `Vec2`, such as `#velocity`.
BrineVector uses FFI structs only when LuaJIT's JIT compiler is on; otherwise
vectors are plain tables, and LuaJIT ignores `__len` on tables, so `#velocity`
is `0` instead of the vector's length. The warning names the replacement, such
as `velocity.length` or `(-velocity).length`, and the editor offers it as a
quick fix. A project that declares its own `type Vec2` is not affected. See
[Vectors](vectors.md#operators).

`TableLiteral` compares literal keys by their LuaJIT runtime value, so escaped
strings and signed numeric spellings cannot hide a duplicate. It also reports
explicit numeric keys that overlap list entries and repeated named fields in
Tua table type shapes. Diagnostics identify the earlier field or the conflicting
list entry. The dedicated `duplicate_keys` setting enables this complete check
even when the standard rule set is disabled.

`TableOperations` checks calls to the resolved LuaJIT `table` and `ipairs`
builtins. It reports zero-based or redundant `table.insert`/`table.remove`
indices and a direct final `table.insert` call that can expand to multiple Lua
values; parentheses around the call explicitly keep its first result. With
known types, `#` and `ipairs` also report record tables without an array part
and dictionaries with string keys. Arrays, numeric dictionaries, empty tables,
`any`, and uncertain unions stay permissive. A known `__len` or `__ipairs`
metamethod enables its matching operation, and lexically shadowed builtins are
never inspected. LuaJIT/Lua 5.1 does not provide Luau's `table.create` or
`table.move`, so Tua does not invent guidance for them.

`UninitializedLocal` tracks each binding in a multi-local declaration
independently. Missing initializer slots remain uninitialized unless the final
expression is a direct function call or varargs, which can supply additional
Lua values. A plain assignment or function declaration initializes a local;
compound assignment does not, because it reads the previous value first. The
analysis follows source order and only carries an assignment past an `if` when
every branch assigns it. Assignments inside a `while` or `for` do not initialize
the value after the loop, while `repeat` assignments do because its body runs
at least once. Assignments captured inside a nested function do not initialize
the outer value merely because the function is declared. The diagnostic points
to the first unsafe read, identifies the declaration line, and can be silenced
by explicitly initializing the binding with `nil`.

`LocalUnused` treats reads in default-parameter expressions and nested closures
as uses, and reports unused numeric or generic `for` bindings. Prefix an
intentionally unused local or loop binding with `_` to silence the warning. The
editor only offers to remove an unused declaration when its initializer is safe
to discard; calls and other potentially effectful expressions are preserved.

`ParameterUnused` also reports a declared `...` when the function body never
reads its varargs. Named parameters retain the `_`-prefix exemption; Lua does
not give a vararg declaration a name, so an intentionally ignored `...` should
either be removed or the diagnostic should be configured by exact code.

`FunctionUnused` reports direct local and unqualified user-global function
declarations that are never read. Nested declarations participate in their
lexical scope, while `_`-prefixed names remain intentionally unused. The editor
can remove the complete unused declaration, including a multiline body.

`ImportUnused` reports unused bindings initialized by a direct call to the
global `require`, including computed module names, or by Tua's static
`import(...)` form. Calls to a lexically shadowed `require` remain ordinary
local initializers. Qualified type references and nested value references count
as uses, while `_`-prefixed bindings remain intentionally unused. Import source
actions only rewrite compiler-resolved static bindings; dynamic `require`
calls and shadowed functions are preserved.

`DeprecatedApi` reports resolved uses of functions and fields explicitly marked
with `---@deprecated`. Optional text after the tag becomes replacement guidance:

```lua
---@deprecated use newApi
local function oldApi() end

oldApi() -- TL4056: API `oldApi` is deprecated; use newApi
```

The metadata follows exports from required `.lua` and `.tua` modules, while
lexical shadowing prevents unrelated bindings with the same spelling from
warning. Hover presents the guidance, completion marks deprecated candidates,
and semantic tokens mark resolved declarations and uses. Tua does not infer
LOVE deprecations from prose in the generated API catalog.

Class correctness and class style are separate. Invalid inheritance, missing or
spurious `override`, incompatible override signatures, unknown members, and
uninitialized required fields remain always-on `TL1008` errors. The following
class lints are suppressible standard rules and disappear when
`diagnostics.rules = false`:

- `ParentConstructorPath` (`TL4057`) reports an explicit child `init` that can
  return without calling `super(...)`. A child with no explicit constructor
  inherits the parent initializer automatically and remains quiet.
- `VirtualCallInInit` (`TL4058`) reports a direct `self` call to an overridable
  method from an open class's `init`, because a subclass override can observe
  incomplete subclass state. Final methods, final classes, and calls inside
  deferred function bodies remain quiet.
- `RedundantInheritedField` (`TL4059`) reports a type-only child field
  declaration that repeats an inherited field's resolved type without providing
  a new default.

These rules analyze source class declarations, resolved inheritance, lexical
bindings, and constructor flow. They do not inspect emitted Lua. Tua does not
warn merely because an override omits its parent method, has an empty body, is
exported but unused locally, or belongs to an open class with no known local
subclass.

`DeprecatedGlobal` still depends on a host-provided deprecated-global catalog,
which Tua does not invent. Likewise, dialect-specific table APIs are ignored on
Tua's LuaJIT target. Tua does report unsupported `--!` directives so foreign
configuration comments never silently appear to affect Tua. In particular,
Luau's `--!nolint` reports `TL4049` and points to Tua's header-only
`-- tua-ignore-file TLxxxx` syntax. Same-line and next-line forms use
`tua-ignore-line` and `tua-ignore-next-line`; every suppression requires an
exact diagnostic code.

In editors, diagnostics with eligible exact codes offer quick fixes to
suppress that code for the current line or the whole file. Tua preserves an
existing trailing comment by using a next-line directive, and extends an
existing file header instead of adding a second `tua-ignore-file` comment.
Suppression quick fixes respect `inline_ignores`, `inline_ignore_scopes`, and
`inline_ignore_max_per_file`, and are never included in safe fix-all.

The repository's `examples/linter` fixture contains an intentionally broken
case for every rule Tua can currently emit in the standard set. Running
`tua lint` there is expected to exit with status `1`.

## Formatter Boundary

`SameLineStatement` and `MultiLineStatement` remain lints because they identify
ambiguous statement structure. Whitespace, wrapping, quote style, indentation
width, and empty-line normalization belong only to the [Formatter](formatter.md).

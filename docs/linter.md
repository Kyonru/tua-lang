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

`--strict` adds every Tua-specific optional lint to the standard rules. The
standard rules are also enabled by default in `tua check` and the editor. A
project can opt out of that default without changing `tua lint`:

```toml title="tua.toml"
[diagnostics]
rules = false
```

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
| `LocalUnused` | `TL4005` |
| `FunctionUnused` | `TL4055` |
| `ImportUnused` | `TL4054` |
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

`UnknownGlobal` considers plain global assignments and simple global function
declarations anywhere in the current Tua file to be definitions, regardless of
source order. Qualified writes still require a known root, and compound
assignment reads its target before writing it. Runtime-provided or cross-file
globals should be listed in [`diagnostics.globals`](configuration.md#diagnostics).

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

`TableLiteral` compares literal keys by their LuaJIT runtime value, so escaped
strings and signed numeric spellings cannot hide a duplicate. It also reports
explicit numeric keys that overlap list entries and repeated named fields in
Tua table type shapes. Diagnostics identify the earlier field or the conflicting
list entry. The dedicated `duplicate_keys` setting enables this complete check
even when the standard rule set is disabled.

`LocalUnused` treats reads in default-parameter expressions and nested closures
as uses, but does not report unused numeric or generic `for` bindings. Prefix an
intentionally unused local with `_` to silence the warning. The editor only
offers to remove an unused declaration when its initializer is safe to discard;
calls and other potentially effectful expressions are preserved.

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

`DeprecatedGlobal` depends on a host-provided deprecated-global catalog, and
`DeprecatedApi` depends on deprecation metadata for the resolved external API.
Tua does not invent either catalog, so those two rules are not emitted yet.
Likewise, dialect-specific table APIs are ignored on Tua's LuaJIT target. Tua
does report unsupported `--!` directives so foreign configuration comments
never silently appear to affect Tua.

The repository's `examples/linter` fixture contains an intentionally broken
case for every rule Tua can currently emit in the standard set. Running
`tua lint` there is expected to exit with status `1`.

## Formatter Boundary

`SameLineStatement` and `MultiLineStatement` remain lints because they identify
ambiguous statement structure. Whitespace, wrapping, quote style, indentation
width, and empty-line normalization belong only to the [Formatter](formatter.md).

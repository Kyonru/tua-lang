# Tua linting review and enhancement plan

## Verdict

Tua's lint catalog is already unusually broad for a young Lua-family language,
especially around types, classes, JIT behavior, LÖVE APIs, table operations,
and runtime-specific mistakes. The main weakness is not the number of rules:
several foundational analyses are too permissive, three documented options do
nothing, and configuration is not granular enough to support safely adding
more opinionated rules.

Fix the semantic and configuration issues before expanding the catalog.

## Confirmed findings

### Blocker: unsafe union access is accepted

This currently passes without diagnostics:

```tua
type Player = { x: number }
local player: Player | nil = nil
print(player.x)

type Either = { x: number } | { y: number }
local either: Either = { y = 1 }
print(either.x)

local callable: ((number) -> number) | nil = nil
print(callable(1))
```

This is more serious than a missing lint: it is an unsoundness in the type
checker.

For field access, union resolution silently discards members that do not
contain the requested field:

- `crates/tua-core/src/semantic.rs`, `resolve_table_field_type`
- `crates/tua-core/src/check/mod.rs`, `resolve_table_field_type`

Call resolution similarly selects the first callable union member while
ignoring `nil` or other non-callable alternatives in `callable_from_type`.

This should be an error in Tua and probably a warning in permissive Lua files.
Field, index, method, and call operations should require every relevant union
member to support the operation unless control-flow narrowing has removed the
unsafe members.

A shared union-operation resolver is important here. Fixing only checker field
access would leave hover, completion, calls, and indexing inconsistent.

### Major: three documented lint settings are no-ops

These settings exist, are exposed through the LSP, are enabled by
`tua lint --strict`, and are documented:

- `wrong_argument_count`
- `parameter_type_mismatches`
- `return_type_mismatches`

They are declared in `crates/tua-core/src/config/mod.rs`, but none is read by
lint implementation code. `TL4009`, `TL4016`, and `TL4017` are declared but
never emitted.

The test `checker_backed_function_lints_do_not_duplicate_type_diagnostics` in
`crates/tua-core/src/linter/tests.rs` effectively confirms that enabling them
still emits only the always-on type diagnostics `TL1004` and `TL1003`.
`docs/configuration.md` nevertheless claims they control diagnostics.

Recommended resolution: deprecate and remove these options and codes. Argument
and return contracts are type-system errors, not optional lint policy. If
configurability is desired, introduce severity overrides for `TL1003` and
`TL1004` instead of duplicate lint codes.

### Major: `UninitializedLocal` is not definite-assignment analysis

These accesses receive no warning:

```tua
local before
print(before)
before = 1

local conditional
if condition then
  conditional = 1
end
print(conditional)
```

The implementation tracks only `initialized`, `assigned`, and `first_read` in
`crates/tua-core/src/linter/mod.rs`. Any assignment anywhere suppresses the
warning, regardless of source order or control flow. The behavior is explicitly
encoded by the test
`uninitialized_local_respects_shadowing_and_plain_assignments_anywhere`.

This should become forward definite-assignment analysis:

- Assigned before every reachable read: safe.
- Assigned in only one branch: possibly nil.
- Assigned after the read: does not help.
- `local value = nil`: explicit and intentional, so it remains quiet.

This can eventually share a small control-flow/dataflow layer with unreachable
code, implicit returns, dead stores, and narrowing.

### Major: ambiguous newline calls are missed

Both examples lint clean:

```tua
print("first")
  ("second"):upper()

local callbacks = {
  print
  ("third")
}
```

Lua parses these differently from what many users visually expect. They can
become chained calls or table function calls and lead to runtime failures or
silently different values.

The existing multiline-layout rule in
`crates/tua-core/src/linter/rules.rs` only warns when the continuation is not
indented, and table expressions are excluded. Indentation is not enough to
distinguish an intentional chain from accidental newline-call syntax. LuaLS
has dedicated `newline-call` and `newfield-call` diagnostics in its
[official diagnostic catalog](https://luals.github.io/wiki/diagnostics/).

Tua should add an enabled-by-default warning based on token boundaries, with
fixes such as adding a semicolon or comma.

### Major: lint policy is too coarse

`rules = true` enables a large monolithic group. Most standard rules cannot
then be disabled individually because implementation checks commonly use
`config.rules || individual_flag`.

There is also no per-code severity policy. This matters because `tua check`
fails when any diagnostic is returned, including warnings.

LuaLS supports severity configuration in its
[settings](https://luals.github.io/wiki/settings/), while Selene supports
individual lint configuration in its
[configuration model](https://github.com/Kampfkarren/selene/blob/main/docs/src/usage/configuration.md).

Before adding many rules, Tua should support something resembling:

```toml
[diagnostics.rules]
TL4035 = "allow"
TL4045 = "error"
newline_call = "warning"
```

Then separate CI behavior:

- Normal `tua check`: fail on errors.
- `tua lint`: fail on configured warning/error policy.
- `--deny-warnings`: optional strict CI mode.

This will scale better than adding dozens of booleans.

### Major: `uncalled_local_functions` is intentionally noisy

The rule warns for callback-shaped code:

```tua
local function callback() end
print(callback)
```

That behavior is explicitly expected by the test
`reports_referenced_local_functions_that_are_never_directly_called`.

Passing functions to event dispatchers, `table.sort`, signals, ECS systems,
LÖVE APIs, and other higher-order functions is normal Lua. Enabling this
through `--strict` therefore creates predictable false positives.

Recommendations:

- Remove it from the strict preset, or
- Rename it to make its "never directly called" limitation explicit, and
- Eventually add escape/callback analysis.

`TL4055` already covers functions that are genuinely unused.

### Medium: missing static modules should be reported by default

`missing_requires` and `bad_requires` default to false in
`crates/tua-core/src/config/mod.rs`. Project diagnostics are explicitly
filtered by those settings in
`crates/tua-core/src/analysis/project_queries.rs`.

A literal missing `require("player")` is a high-confidence runtime failure.
Recommended behavior:

- Enable `missing_requires` by default.
- Enable malformed static paths by default.
- Keep dynamic requires permissive.
- Keep cycles and ambiguous candidates optional initially.

### Medium: `unused_assignments` is not dead-store analysis

The rule only reports assignments when the local is never read at all. It
misses:

```tua
local x = expensive()
x = 2
print(x)
```

The result of `expensive()` is overwritten before use. Luacheck distinguishes
unused values and overwritten assignments in its
[warning catalog](https://luacheck.readthedocs.io/en/stable/warnings.html).

This should eventually use reaching-use or liveness analysis and distinguish:

- Unused binding.
- Unused initial value.
- Dead reassignment.
- Parameter overwritten before first read.

### Medium: accidental module-scope globals are accepted

Tua deliberately treats plain global assignments at module scope as
definitions. That means a typo such as `plaeyr = player` silently creates a new
global.

Luacheck has undefined-global-set warnings; Selene has `unscoped_variables`;
LuaLS has `global-element`. Tua should add an `unscoped_global_write` rule:

- Warning by default for `.tua`.
- Optional or warning for `.lua`.
- Exempt configured globals, builtins, generated declarations, and intended
  LÖVE roots.
- Prefer qualified LÖVE callbacks rather than broadly allowing arbitrary
  globals.

### Minor: documentation overstates two rules

`float_equality` says it checks "known numeric values," but implementation only
recognizes float literals. `unused_assignments` similarly sounds like general
dead-store analysis but is much narrower.

Document these rules accurately until their implementations improve.

## Missing rules worth adding

Compared with the official catalogs for
[Luau](https://luau.org/lint/),
[LuaLS](https://luals.github.io/wiki/diagnostics/),
[Selene](https://kampfkarren.github.io/selene/lints/index.html), and
[Luacheck](https://luacheck.readthedocs.io/en/stable/warnings.html), prioritize:

1. `newline_call` / `newfield_call` — default warning.
2. `unscoped_global_write` — default warning in Tua.
3. Constant table/function identity comparison: `{}` == `{}` is always false.
4. Identical `then` and `else` branches.
5. Empty `if`, loop, or function blocks, with exemptions for deliberate stubs.
6. Loop guaranteed to stop on its first iteration.
7. `---@nodiscard` / must-use results. Tua already parses the annotation but
   treats it as metadata only.
8. Same canonical module required under different specifiers, which can create
   duplicate stateful module instances.
9. Unused loop bindings and unused varargs.
10. Optional cyclomatic-complexity and suspicious reverse-loop rules.

Lower-priority, opt-in rules include divide-by-literal-zero, almost-swapped
sequential assignments, mixed array/map tables, and naming conventions.
Formatting-related rules should generally remain the formatter's
responsibility.

## Recommended implementation order

1. Fix union field/index/call safety.
2. Remove or redesign the three no-op diagnostic settings.
3. Add per-rule severity and presets.
4. Replace `UninitializedLocal` with definite-assignment analysis.
5. Add newline-call and unscoped-global-write diagnostics.
6. Enable literal missing-module diagnostics by default.
7. Remove `uncalled_local_functions` from strict mode.
8. Build reusable control-flow facts for dead stores, empty/identical branches,
   and one-iteration loops.
9. Add `nodiscard` and canonical-module-spelling diagnostics.

## Verification performed for this review

- Ran all 131 focused linter tests: all passed.
- Ran `examples/linter`; it emitted 33 distinct standard lint codes.
- Reproduced unsafe nil/partial-union field access and nil-callable access with
  `tua check`; all incorrectly passed.
- Reproduced both newline-call ambiguities with `tua lint`; both passed
  silently.
- Reproduced read-before-assignment and branch-only assignment; neither
  produced `TL4045`.
- Confirmed the three no-op settings have no implementation reads.
- Confirmed the worktree was clean before adding this report.

The current tests are substantial and the rule catalog is strong. The next
quality step should be deeper shared semantic/control-flow correctness and
configurable policy, not simply adding more isolated AST checks.

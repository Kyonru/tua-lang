# Configuration

Tua projects use `tua.toml`.

## Default Config

```toml
source = "src"
out = "build"
target = "love11-luajit"
extension = "tua"
emit_maps = true

[emit]
annotation_mode = "off"

[jit]
enabled = false
variable_type_drift = true
table_shape_drift = true
hot_loop_hints = true

[ecs]
enabled = false
backend = "tiny-ecs"

[diagnostics]
duplicate_keys = false
missing_requires = false
self_assignments = false
doc_annotations = false
undefined_variables = false
unused_locals = false
unused_parameters = false
use_before_definition = false
duplicate_parameters = false
wrong_argument_count = false
bad_requires = false
impossible_boolean_expressions = false
duplicate_if_conditions = false
duplicate_binary_operands = false
float_equality = false
const_assignments = false
parameter_type_mismatches = false
return_type_mismatches = false
assignment_arity = false
local_declaration_arity = false
unused_assignments = false
binary_operator_type_mismatches = false
uncalled_local_functions = false
formatting_lints = false
inline_ignores = false
inline_ignore_scopes = ["tua-ignore-line", "tua-ignore-next-line"]
inline_ignore_max_per_file = 0
globals = []
```

## Root Options

`source`
: Directory containing Tua source files.

`out`
: Directory where emitted Lua files are written.

`target`
: Runtime target label. The default is `love11-luajit`.

`extension`
: Source extension to discover. The default is `tua`.

`emit_maps`
: Reserved for source-map-style output. Current emitted Lua stays readable even
without runtime metadata.

## Emit

`annotation_mode`
: Controls annotation-preserving emission for emitted Lua.

- `off` (default): erase all Tua-only types and keep current minimal output.
- `luacats`: emit LuaCATS-compatible comments derived from available type
  information.

LuaCATS output includes:

- `---@alias` for exported type aliases.
- `---@type` for annotated locals.
- `---@param` and `---@return` for function signatures.
- `---@field` for stable table/object fields.

## LuaJIT Guidance

LuaJIT guidance is opt-in and diagnostic-only. It encourages stable local and
table shapes for LOVE projects that care about desktop LuaJIT behavior, but it
does not rewrite code, add runtime checks, or change emitted Lua.

```toml
[jit]
enabled = true
variable_type_drift = true
table_shape_drift = true
hot_loop_hints = true
```

`enabled`
: Enables the LuaJIT guidance pass. The default is `false`.

`variable_type_drift`
: Reports clear local value-type drift such as assigning a `string` into a
local previously inferred as `number`. Values annotated or inferred as `any`
remain an intentional dynamic escape hatch.

`table_shape_drift`
: Reports clear table-shape drift such as adding late fields to a known table
constructor or changing a known field from one concrete type to another.

`hot_loop_hints`
: Reports dynamic table-key writes inside loops when Tua can see the pattern.
These are hints for reviewing hot code paths, not proof that a loop is slow.

## ECS Domain Support

ECS support is opt-in and currently shaped around tiny-ecs-compatible Lua
projects:

```toml
[ecs]
enabled = true
backend = "tiny-ecs"
```

`enabled`
: Enables the built-in ECS domain plugin, the typed `ecs` helper module, and ECS
diagnostics. The default is `false`.

`backend`
: Names the intended runtime adapter. The first supported profile is
`tiny-ecs`; Tua still emits ordinary Lua and does not vendor or execute an ECS
runtime.

When enabled, Tua recognizes:

- `ecs.world(): EcsWorld`;
- `ecs.component(name, schema): EcsComponent`;
- `ecs.system(world, components, update): EcsSystem`;
- world methods such as `world:create()`, `world:add(...)`, and
  `world:query(...)`.

The initial diagnostics cover clear, static patterns:

- mismatched or missing fields in one-line `world:add(entity, Component, { ... })`
  calls;
- unknown components in one-line `ecs.system(world, { ... }, ...)` or
  `world:query({ ... })` calls;
- empty component query lists.

Dynamic ECS construction remains permissive.

## Diagnostics

Optional lints are disabled by default.

```toml
[diagnostics]
duplicate_keys = true
missing_requires = true
self_assignments = true
doc_annotations = true
undefined_variables = true
unused_locals = true
unused_parameters = true
use_before_definition = true
duplicate_parameters = true
wrong_argument_count = true
bad_requires = true
impossible_boolean_expressions = true
duplicate_if_conditions = true
duplicate_binary_operands = true
float_equality = true
const_assignments = true
parameter_type_mismatches = true
return_type_mismatches = true
assignment_arity = true
local_declaration_arity = true
unused_assignments = true
binary_operator_type_mismatches = true
uncalled_local_functions = true
formatting_lints = true
inline_ignores = true
inline_ignore_scopes = ["tua-ignore-line", "tua-ignore-next-line"]
inline_ignore_max_per_file = 0
globals = ["love", "_G"]

[emit]
annotation_mode = "luacats"
```

`duplicate_keys`
: Reports repeated literal keys in table constructors.

`missing_requires`
: Reports `require("module")` calls that Tua cannot resolve from the current
project layout.

`self_assignments`
: Reports assignments such as `player.hp = player.hp`.

`doc_annotations`
: Reports malformed supported LuaHelper/LuaLS-style doc annotations such as
`---@param` without a parameter type.

`undefined_variables`
: Reports variable reads that are not known locals, parameters, configured
globals, Lua builtins, LOVE globals, or known module exports.

`unused_locals`
: Reports local bindings that are never read.

`unused_parameters`
: Reports function parameters that are never read.

`use_before_definition`
: Reports local reads before the local declaration when Tua can prove the
ordering.

`duplicate_parameters`
: Reports repeated parameter names in a function declaration.

`wrong_argument_count`
: Reports calls to known functions with the wrong number of arguments.

`bad_requires`
: Reports require paths that cannot be resolved by project/module discovery.

`impossible_boolean_expressions`
: Reports boolean/equality expressions that Tua can prove are impossible.

`duplicate_if_conditions`
: Reports repeated conditions in one `if`/`elseif` chain.

`duplicate_binary_operands`
: Reports suspicious repeated operands such as `value == value`.

`float_equality`
: Reports direct equality checks between known numeric values.

`const_assignments`
: Reports assignments to values Tua treats as constant.

`parameter_type_mismatches`
: Reports known function argument type mismatches through the lint pipeline.

`return_type_mismatches`
: Reports known function return type mismatches through the lint pipeline.

`assignment_arity`
: Reports multiple assignments where Tua can prove the number of values does not
match the number of targets. Unknown multi-return calls and varargs remain
permissive.

`local_declaration_arity`
: Reports local declarations where Tua can prove the initializer count does not
match the number of declared locals.

`unused_assignments`
: Reports assignments to locals that are never otherwise read. This complements
`unused_locals`, which reports the declaration itself.

`binary_operator_type_mismatches`
: Reports binary operators used with statically incompatible operand types, such
as `1 + "hp"`. Dynamic or `any` operands remain permissive.

`uncalled_local_functions`
: Reports local functions that are referenced as values but never directly
called. Indirect callback patterns can stay quiet by leaving this opt-in lint
disabled.

`formatting_lints`
: Reports conservative Lua/Tua formatting consistency issues. The baseline is
intentionally small and opt-in: trailing whitespace, tabs or odd-space
indentation, obvious spacing around `then`/`do`, comma spacing,
parenthesis/bracket padding, and assignment-style token spacing. These lints
help mixed `.lua` and `.tua` code stay readable across contributors without
rewriting files or enforcing a full formatter.

`inline_ignores`
: Enables diagnostic suppression comments. This is disabled by default so
ordinary diagnostics stay visible unless a project explicitly chooses gradual
migration comments.

Supported comments are:

```lua
-- tua-ignore-next-line TL4018
player.setFilter("nearest")

local legacy: number = "soon" -- tua-ignore-line TL1003
```

`tua-ignore-next-line` applies to the immediately following line.
`tua-ignore-line` applies to diagnostics on the same line. `tua-ignore-file`
exists for migration-heavy files, but it must be added to
`inline_ignore_scopes` before it is accepted.

Codes may be exact (`TL1003`) or category groups (`TL1000`, `TL4000`, `type`,
`lint`). If no code is supplied, the comment suppresses all non-syntax
diagnostics for that target. Invalid codes and unused suppressions produce
warnings, and suppression never changes type inference or emitted Lua.

`inline_ignore_scopes`
: Lists the suppression directives accepted by this project. The default allows
only line-targeted suppressions.

`inline_ignore_max_per_file`
: Limits the number of accepted suppression comments per file. `0` means
unlimited.

`globals`
: Adds project-specific global names that should be accepted by diagnostics and
linting.

## Workspace Resolution

The LSP chooses the nearest workspace folder or ancestor containing `tua.toml`
when resolving diagnostic settings. If no config is found, defaults are used.

## Editor Settings

The LSP also accepts editor-provided settings through
`workspace/didChangeConfiguration`.

Configuration precedence is:

1. Built-in defaults.
2. The nearest applicable `tua.toml`.
3. Editor settings sent by the LSP client.

Editor settings are an in-memory overlay for the running language server. They
do not rewrite `tua.toml`.

For diagnostic toggles, use `true` or `false` to override the project config.
Leave the value unset or `null` to inherit from `tua.toml`.

Supported editor settings:

- `tua.diagnostics.*`: overrides matching `[diagnostics]` keys.
- `tua.diagnostics.globals`: replaces the configured global allow-list.
- `tua.fileAssociations`: maps glob patterns to `lua` or `tua` so extra file
  extensions can be indexed.
- `tua.jit.enabled`: overrides `[jit].enabled` for opt-in LuaJIT guidance.
- `tua.ecs.enabled`: overrides `[ecs].enabled` for opt-in ECS domain support.
- `tua.ignoredPaths`: excludes matching paths from indexing and diagnostics.
- `tua.requirePathSeparator`: controls suggested require paths, either `.` or
  `/`.
- `tua.preview.enabled`: reserves an editor-facing preview toggle for
  experimental features.

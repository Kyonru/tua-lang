# Configuration

Tua projects use `tua.toml`.

## Default Config

```toml
source = "src"
out = "build"
target = "love11-luajit"
extension = "tua"
emit_maps = true

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
formatting_lints = false
inline_ignores = false
inline_ignore_scopes = ["tua-ignore-line", "tua-ignore-next-line"]
inline_ignore_max_per_file = 0
globals = []
[emit]
annotation_mode = "off"
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

Planned LuaCATS output includes:

- `---@alias` for exported type aliases.
- `---@type` for annotated locals.
- `---@param` and `---@return` for function signatures.
- `---@field` for stable table/object fields.

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
formatting_lints = true
inline_ignores = false
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

`formatting_lints`
: Reports conservative Lua/Tua formatting consistency issues. The baseline is
  intentionally small and opt-in: trailing whitespace, tabs or odd-space
  indentation, obvious spacing around `then`/`do`, comma spacing,
  parenthesis/bracket padding, and assignment-style token spacing. These lints
  help mixed `.lua` and `.tua` code stay readable across contributors without
  rewriting files or enforcing a full formatter.

`inline_ignores`
: Enables line-level diagnostic suppression comments.

`inline_ignore_scopes`
: Limits which suppression comment forms are accepted. Valid values are comment
  tags such as `tua-ignore-line`, `tua-ignore-next-line`, and
  `tua-ignore-file`.

`inline_ignore_max_per_file`
: Controls suppression-policy size. A value of `0` means unlimited suppressions.
  Any positive value caps how many comments can be used in one file.

Inline suppression examples:

```lua
-- tua-ignore-line TL4018
player:setFilter("nearest")

-- tua-ignore-next-line TL4001 TL4018
local x = some_dynamic_access()
```

If an ignore comment lists no matching diagnostic in scope, the checker can emit a
special warning diagnostic so suppression is explicit instead of silent.

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
- `tua.ignoredPaths`: excludes matching paths from indexing and diagnostics.
- `tua.requirePathSeparator`: controls suggested require paths, either `.` or
  `/`.
- `tua.preview.enabled`: reserves an editor-facing preview toggle for
  experimental features.

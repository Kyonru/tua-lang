---
icon: lucide/settings
---

# Configuration

Tua projects use `tua.toml`.

Configuration is decoded through a typed Serde schema and a standards-compliant
TOML parser. Normal TOML quoting, escapes, multiline arrays, dotted keys, and
inline tables are supported. Unknown sections and keys are rejected with a
nearby-name suggestion when one is unambiguous, and editor diagnostics select
the precise offending key or value range.

The language server attaches to `tua.toml` files in VS Code and provides
schema-backed key completion, section completion, enum/default value
completion, setting hover documentation, and quick fixes for unknown-key
diagnostics that include a suggestion.

Generated config templates live in `config/templates/`:

- `default.toml`: the full default `tua.toml`, generated from
  `config/schema.json` and checked against `DEFAULT_CONFIG_TEXT`.
- `minimal.toml`: the smallest root-level project template for common new
  projects.

## Default Config

```toml
source = "src"
out = "build"
main = "main.tua"
target = "love11-luajit"
extension = "tua"
emit_maps = true
type_libraries = []
asset_pipelines = []

[emit]
annotation_mode = "off"

[format]
indent_width = 2
line_width = 100
quote_style = "preserve"
final_newline = true

[jit]
enabled = false
variable_type_drift = true
table_shape_drift = true
hot_loop_hints = true

[ecs]
enabled = true
backend = "tua"

[diagnostics]
rules = true
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
inline_ignores = true
inline_ignore_scopes = ["tua-ignore-line", "tua-ignore-next-line", "tua-ignore-file"]
inline_ignore_max_per_file = 0
globals = []
```

## Schema Reference

This compact reference is generated from the same metadata used for config-key
validation and editor-setting drift checks. The detailed sections below explain
the behavior and tradeoffs of each option.

<!-- tua-config-schema:start -->
| TOML path | Type | Default | VS Code override |
| --- | --- | --- | --- |
| `source` | string | `"src"` | - |
| `out` | string | `"build"` | - |
| `main` | string | `"main.tua"` | - |
| `target` | string | `"love11-luajit"` | - |
| `extension` | string | `"tua"` | - |
| `emit_maps` | boolean | `true` | - |
| `type_libraries` | array | `[]` | - |
| `asset_pipelines` | array | `[]` | - |
| `emit.annotation_mode` | string | `"off"` | - |
| `format.indent_width` | integer | `2` | `tua.format.indentWidth` |
| `format.line_width` | integer | `100` | `tua.format.lineWidth` |
| `format.quote_style` | string | `"preserve"` | `tua.format.quoteStyle` |
| `format.final_newline` | boolean | `true` | `tua.format.finalNewline` |
| `jit.enabled` | boolean | `false` | `tua.jit.enabled` |
| `jit.variable_type_drift` | boolean | `true` | - |
| `jit.table_shape_drift` | boolean | `true` | - |
| `jit.hot_loop_hints` | boolean | `true` | - |
| `ecs.enabled` | boolean | `true` | `tua.ecs.enabled` |
| `ecs.backend` | string | `"tua"` | - |
| `diagnostics.rules` | boolean | `true` | `tua.diagnostics.rules` |
| `diagnostics.duplicate_keys` | boolean | `false` | `tua.diagnostics.duplicateKeys` |
| `diagnostics.missing_requires` | boolean | `false` | `tua.diagnostics.missingRequires` |
| `diagnostics.self_assignments` | boolean | `false` | `tua.diagnostics.selfAssignments` |
| `diagnostics.doc_annotations` | boolean | `false` | `tua.diagnostics.docAnnotations` |
| `diagnostics.undefined_variables` | boolean | `false` | `tua.diagnostics.undefinedVariables` |
| `diagnostics.unused_locals` | boolean | `false` | `tua.diagnostics.unusedLocals` |
| `diagnostics.unused_parameters` | boolean | `false` | `tua.diagnostics.unusedParameters` |
| `diagnostics.use_before_definition` | boolean | `false` | `tua.diagnostics.useBeforeDefinition` |
| `diagnostics.duplicate_parameters` | boolean | `false` | `tua.diagnostics.duplicateParameters` |
| `diagnostics.wrong_argument_count` | boolean | `false` | `tua.diagnostics.wrongArgumentCount` |
| `diagnostics.bad_requires` | boolean | `false` | `tua.diagnostics.badRequires` |
| `diagnostics.impossible_boolean_expressions` | boolean | `false` | `tua.diagnostics.impossibleBooleanExpressions` |
| `diagnostics.duplicate_if_conditions` | boolean | `false` | `tua.diagnostics.duplicateIfConditions` |
| `diagnostics.duplicate_binary_operands` | boolean | `false` | `tua.diagnostics.duplicateBinaryOperands` |
| `diagnostics.float_equality` | boolean | `false` | `tua.diagnostics.floatEquality` |
| `diagnostics.const_assignments` | boolean | `false` | `tua.diagnostics.constAssignments` |
| `diagnostics.parameter_type_mismatches` | boolean | `false` | `tua.diagnostics.parameterTypeMismatches` |
| `diagnostics.return_type_mismatches` | boolean | `false` | `tua.diagnostics.returnTypeMismatches` |
| `diagnostics.assignment_arity` | boolean | `false` | `tua.diagnostics.assignmentArity` |
| `diagnostics.local_declaration_arity` | boolean | `false` | `tua.diagnostics.localDeclarationArity` |
| `diagnostics.unused_assignments` | boolean | `false` | `tua.diagnostics.unusedAssignments` |
| `diagnostics.binary_operator_type_mismatches` | boolean | `false` | `tua.diagnostics.binaryOperatorTypeMismatches` |
| `diagnostics.uncalled_local_functions` | boolean | `false` | `tua.diagnostics.uncalledLocalFunctions` |
| `diagnostics.inline_ignores` | boolean | `true` | `tua.diagnostics.inlineIgnores` |
| `diagnostics.inline_ignore_scopes` | string-array | `["tua-ignore-line","tua-ignore-next-line","tua-ignore-file"]` | `tua.diagnostics.inlineIgnoreScopes` |
| `diagnostics.inline_ignore_max_per_file` | integer | `0` | `tua.diagnostics.inlineIgnoreMaxPerFile` |
| `diagnostics.globals` | string-array | `[]` | `tua.diagnostics.globals` |
<!-- tua-config-schema:end -->

## Root Options

`source`
: Directory containing Tua source files.

`out`
: Directory where emitted Lua files are written.

`main`
: Source file under `source` that also emits to `<out>/main.lua` for LOVE
entrypoint loading. The source file still keeps its normal mirrored output
path, so `main = "love/boot.tua"` emits both `build/love/boot.lua` and
`build/main.lua`. If another source would also mirror to `<out>/main.lua`,
`tua build` reports an output collision until one of the sources is renamed or
`main` is changed.

`target`
: Runtime target label. The default is `love11-luajit`.

`extension`
: Source extension to discover. The default is `tua`.

`emit_maps`
: When `true`, `tua build` writes `*.lua.map.json` Tua provenance and standard
`*.lua.map` v3 sidecars next to each generated `.lua` file. They support
generated-Lua navigation, `tua traceback`, and source-mapped Lua debugging;
they are not loaded by the game at runtime.

## Declaration-Only Type Libraries

`type_libraries` indexes third-party LuaCATS declarations without treating
them as source files or runtime dependencies. Replace the default
`type_libraries = []` line with one array entry per installed library:

```toml title="tua.toml"
[[type_libraries]]
path = "lib/feel/types"
declaration_prefix = "feel"
runtime_prefix = "lib.feel"
```

Each entry has three required fields:

- `path` is a directory, relative to the project root unless absolute. Tua
  recursively indexes its `.lua` files as declarations.
- `declaration_prefix` is the module namespace used by `---@meta`, such as
  `feel` and `feel.love`.
- `runtime_prefix` is the namespace used by project `require(...)` calls. The
  example maps `---@meta feel` to `require("lib.feel")` and
  `---@meta feel.love` to `require("lib.feel.love")`.

Declaration libraries are analysis-only. Tua never copies, emits, executes, or
packages their files, and the editor performs no network fetching. A static
`import(...)` still resolves a real project module because it represents
runtime/build code; only `require(...)` receives the declaration overlay.
Changes under configured roots are watched by `tua watch`. The language server
indexes them during workspace reindexing and refreshes changes reported by the
editor.

The installed library owns its declarations. Updating a vendored or
package-managed Feel checkout therefore updates Tua completion without a Tua
release and without bundling Feel itself.

## Asset Pipelines

`asset_pipelines` configures opt-in standalone importers that run before
`check`, `build`, and `watch`. Each importer is cached by its configuration and
input contents, writes a versioned manifest, and can generate typed `.tua` or
declaration-only `.d.tua` modules under its dedicated source directory.

```toml title="tua.toml"
[[asset_pipelines]]
name = "world-maps"
inputs = ["maps/**/*.ldtk", "tools/import_maps.lua"]
command = ["lua", "tools/import_maps.lua"]
output = "src/generated/world-maps"
version = "1"
```

An asset pipeline may use `builtin = "audio-analysis"` instead of `command`.
The two fields are mutually exclusive. Built-in-specific scalar settings live
under `[asset_pipelines.options]`; see [Audio Systems](audio.md#cached-audio-analysis).

See [Asset Pipelines](asset-pipelines.md) for field definitions, importer
environment variables, the manifest protocol, diagnostics, caching, watch
behavior, and security guidance.

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

## Format

The built-in formatter is Rust-native and is used by `tua fmt` plus LSP
`textDocument/formatting`, `textDocument/rangeFormatting`, and
`textDocument/onTypeFormatting`. It removes leading and trailing empty lines
and collapses consecutive empty lines between code sections to one. Empty
lines inside multiline strings and block comments are preserved.

```toml
[format]
indent_width = 2
line_width = 100
quote_style = "preserve"
final_newline = true
```

`indent_width`
: Spaces per indentation level.

`line_width`
: Preferred maximum line width for line-aware groups.

`quote_style`
: Currently `preserve`, which keeps existing string quote style.

`final_newline`
: Writes a final LF newline when formatting.

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
: Retained for LuaJIT-guidance configuration compatibility. Incompatible
inferred-local reassignment in `.tua` is now a core `TL1003` error regardless
of JIT settings, so the JIT pass does not duplicate that diagnostic. `any` and
unknown dynamic values remain escape hatches.

`table_shape_drift`
: Reports clear table-shape drift such as adding late fields to a known table
constructor or changing a known field from one concrete type to another.

`hot_loop_hints`
: Reports dynamic table-key writes inside loops when Tua can see the pattern.
These are hints for reviewing hot code paths, not proof that a loop is slow.

## ECS Domain Support

ECS support and the bundled Tua runtime are enabled by default. The section can
be omitted:

```toml
[ecs]
enabled = true
backend = "tua"
```

`enabled`
: Enables the built-in `ecs` module, first-class ECS analysis, ECS diagnostics,
and automatic bundled runtime emission. The default is `true`; set it to
`false` to opt out.

`backend`
: Names the runtime profile. The default `tua` profile emits the bundled
`tuaecs.lua` runtime only when a source file uses the built-in `ecs` module.

When enabled, Tua recognizes:

- `ecs.world(): EcsWorld`;
- `ecs.component(name, schema): EcsComponent`;
- `ecs.system(world, components, update): EcsSystem`;
- world methods such as `world:create()`, `world:add(...)`, and
  `world:query(...)`.

For static component declarations, the schema is retained in the component's
semantic type. `world:get`, `world:add`, and `world:remove` therefore return the
matching payload shape, even when the component is imported from another Tua
module. Dynamic component construction continues to return `any`.

The initial diagnostics cover clear, static patterns:

- mismatched or missing fields in one-line `world:add(entity, Component, { ... })`
  calls;
- unknown components in one-line `ecs.system(world, { ... }, ...)` or
  `world:query({ ... })` calls;
- empty component query lists.

Dynamic ECS construction remains permissive.

An explicit binding such as `local ecs = require("project_ecs")` overrides the
built-in runtime for that file. See [Entity Component Systems](ecs.md) for the
runtime model and guidance on when ECS is appropriate.

## State Store Support

`store({...})` is available without configuration. A source file that uses it
emits a `require("tuastore")`, and the build copies the bundled runtime only
when required. There is no global store setting: state ownership, persistence,
save formats, and migrations belong to each explicit store and project. See
[State Stores](stores.md) for the API and LOVE filesystem adapter pattern.

## Object Pool Support

`objectPool({...})` is available without configuration. A source file that
uses it emits a `require("tuaobjectpool")`, and the build copies the bundled
LuaJIT-compatible runtime only when required. The compiler models lifecycle
methods and preserves an annotated factory return type through `acquire()`.

There is no pool-wide project setting. Pool size and lifecycle belong at each
call site through `initial`, `prewarm`, `reset`, and `dispose`. See
[Object Pooling](object-pooling.md) for the full API and performance guidance.

## Input Support

`input({...})` is available without configuration. A source file that uses it
emits `require("baton").new`, and the build copies the bundled MIT-licensed
`baton.lua` only when required. A project-provided `src/baton.lua` is copied
instead and is not overwritten.

There is no global input setting. Controls, pairs, joystick selection,
deadzones, and player ownership belong to each `input` call. See
[Input](input.md) for the full Baton-backed API.

## Diagnostics

Formatting and linting are independent: `[format]` controls `tua fmt`, while
`[diagnostics]` controls code-quality guidance. `tua lint` always enables the
standard rules for that command. Those rules are enabled by default for
`tua check` and editor diagnostics; set `rules = false` to opt out there.
`tua lint --strict` adds every Tua-specific optional lint. See
[Formatter](formatter.md), [Linter](linter.md), and
[Diagnostic Codes](luahelper-code-matrix.md).

```toml
[diagnostics]
rules = true
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
inline_ignores = true
inline_ignore_scopes = ["tua-ignore-line", "tua-ignore-next-line", "tua-ignore-file"]
inline_ignore_max_per_file = 0
globals = ["love", "_G"]

[emit]
annotation_mode = "luacats"
```

`duplicate_keys`
: Reports runtime-equivalent literal keys in table constructors, explicit
  indices that collide with list entries, and repeated named fields in table
  type shapes.

`rules`
: Controls the standard lint rules for `tua check` and language server
diagnostics. They are enabled by default. `tua lint` enables these rules for
its own run regardless of the configured value. See [Linter](linter.md) for
the rule map and target-specific exclusions.

`missing_requires`
: Reports `require("module")` and static `import(...)` bindings that Tua cannot
resolve from the current project layout.

`self_assignments`
: Reports assignments such as `player.hp = player.hp`.

`doc_annotations`
: Reports malformed supported LuaHelper/LuaLS-style doc annotations such as
`---@param` without a parameter type.

`undefined_variables`
: Reports variable reads that are not known locals, parameters, configured
globals, Lua builtins, LOVE globals, or known module exports.

`unused_locals`
: Reports local bindings and direct user-global function declarations that are
never read.

`unused_parameters`
: Reports function parameters that are never read.

`use_before_definition`
: Reports local reads before the local declaration when Tua can prove the
ordering.

`duplicate_parameters`
: Reports repeated parameter names as `TL4043` without enabling every standard
rule.

`wrong_argument_count`
: Reports calls to known functions with the wrong number of arguments.

`bad_requires`
: Reports invalid require/import paths, including `import("../...")` parent
traversal and malformed import specifiers.

`impossible_boolean_expressions`
: Reports boolean/equality expressions that Tua can prove are impossible.

`duplicate_if_conditions`
: Reports structurally repeated conditions in one `if`/`elseif` chain.

`duplicate_binary_operands`
: Reports structurally repeated operands in boolean chains and suspicious
  expressions such as `value == value`.

`float_equality`
: Reports direct equality checks between known numeric values.

`const_assignments`
: Reports assignments to values Tua treats as constant.

`parameter_type_mismatches`
: Reports known function argument type mismatches through the lint pipeline.

`return_type_mismatches`
: Reports known function return type mismatches through the lint pipeline.

`assignment_arity`
: Reports assignments whose explicit value count does not match the target
count. A shortage ending in a direct call or varargs remains permissive because
the final expression may produce multiple values. Ending a shortage with
explicit `nil` also silences the warning and records that the missing values are
intentional.

`local_declaration_arity`
: Applies the same explicit-value-count check to local declarations.

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

`inline_ignores`
: Enables exact-code diagnostic suppression comments. This is enabled by
default; set it to `false` when a project wants to forbid source suppressions.

Supported comments are:

```lua
-- tua-ignore-file TL4048

-- tua-ignore-next-line TL4018
player.setFilter("nearest")

local legacy: number = "soon" -- tua-ignore-line TL1003
```

`tua-ignore-next-line` applies to the immediately following line.
`tua-ignore-line` applies to diagnostics on the same line. `tua-ignore-file`
applies throughout the file and must be its first nonblank line.

Every directive requires at least one exact code such as `TL1003` or `TL4048`.
Multiple exact codes may be listed. Empty directives, category names, and broad
groups such as `TL4000` report `TL4025` instead of hiding unrelated diagnostics.

!!! warning "Suppressions do not change program behavior"
    Invalid codes and unused suppressions produce warnings. A suppression only
    hides matching diagnostics; it never changes type inference or emitted Lua.

`inline_ignore_scopes`
: Lists the suppression directives accepted by this project. The default allows
same-line, next-line, and header-only file suppressions.

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

!!! info "Editor settings are temporary overlays"
    Editor settings apply in memory to the running language server and do not
    rewrite `tua.toml`. For diagnostic toggles, use `true` or `false` to
    override project configuration; leave a value unset or `null` to inherit
    from `tua.toml`.

Supported editor settings:

- `tua.diagnostics.*`: overrides matching `[diagnostics]` keys.
- `tua.diagnostics.globals`: replaces the configured global allow-list.
- `tua.fileAssociations`: maps glob patterns to `lua` or `tua` so extra file
  extensions can be indexed.
- `tua.jit.enabled`: overrides `[jit].enabled` for opt-in LuaJIT guidance.
- `tua.ecs.enabled`: overrides `[ecs].enabled` for built-in ECS support.
- `tua.completion.sequenceRanking.enabled`: enables the experimental local
  contextual sequence ranker for completion ordering. It defaults to `false`
  and never adds non-compiler candidates.
- `tua.completion.sequenceRanking.timeoutMs`: maximum sequence-ranking budget
  in milliseconds. A timeout keeps the pre-sequence completion order.
- `tua.completion.sequenceRanking.maxCandidates`: maximum compiler candidates
  sent to the sequence ranker.
- `tua.completion.multiToken.enabled`: enables the experimental multi-token
  completion lane. It defaults to `false`.
- `tua.completion.multiToken.advanced`: allows multi-token suggestions even when
  normal compiler completion has ordinary confidence.
- `tua.completion.multiToken.allowModelIntegration`: explicit opt-in policy
  gate for local Ollama inline completions. It defaults to `false`.
- `tua.completion.multiToken.maxSuggestions`: maximum validated multi-token
  suggestions appended to completion results.
- `tua.completion.inline.enabled`: enables experimental deterministic inline
  ghost text. It defaults to `false`.
- `tua.completion.inline.sources.semantic`: enables unique source-only
  statement snippets for deterministic inline completion. It defaults to
  `true`, but has no effect while the parent feature is disabled.
- `tua.completion.inline.sources.currentFile`: enables conservative
  single-line recall from repeated lines in the active document. It defaults
  to `true`, but has no effect while the parent feature is disabled.
- `tua.completion.ollama.endpoint`: machine-scoped loopback endpoint; defaults
  to `http://127.0.0.1:11434`.
- `tua.completion.ollama.model`: machine-scoped model name; defaults to
  `qwen2.5-coder:1.5b-base`.
- `tua.completion.ollama.timeoutMs`: generation timeout; defaults to `5000`.
- `tua.completion.ollama.keepAlive`: Ollama residency hint; defaults to `10m`.
- `tua.completion.ollama.contextTokens`: requested Ollama context window;
  defaults to `4096`.
- `tua.completion.ollama.maxTokens`: user generation ceiling; defaults to
  `256`. Tua uses at most 64 tokens for local expressions and 512 for function
  bodies.
- `tua.ignoredPaths`: excludes matching paths from indexing and diagnostics.
- `tua.requirePathSeparator`: controls suggested require paths, either `.` or
  `/`. Static `import(...)` lowering uses the same separator when mapping a
  source path back to the emitted Lua `require("...")` module name.
- `tua.preview.enabled`: enables explicit experimental editor previews such as
  `Tua: Preview Sprite Animations` and `Tua: Preview State Machine`. Preview
  metadata comes from the local Tua language server through `tua/inspect`; no
  project source or asset metadata is sent off-device. It does not execute
  state-machine code or enable proposed VS Code APIs,
  source-definition/multi-document-highlight/auto-insert behavior, or the
  external API or MCP processes.

Run **Tua: Toggle Experimental Completions** to switch both
`tua.completion.sequenceRanking.enabled` and
`tua.completion.multiToken.enabled` for the current workspace. If either
feature is off, the command enables both; if both are on, it disables both.
Advanced multi-token and model-integration policy settings are not changed.
Use **Tua: Toggle Local Ollama Completions** for the separate model-integration
gate and consent flow.

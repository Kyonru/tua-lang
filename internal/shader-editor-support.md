# Shader editor support plan

## Goal

Make LÖVE shaders as checkable and editable as Tua source. Mistakes that LÖVE
only reports when the game runs should surface in the editor and in
`tua check`. Shader edits should reach a running game without a restart.

The work spans four layers:

- **Uniform contract**: what a `.glsl` file declares, and whether the Tua code
  that sends values matches it.
- **Editor features**: completion, hover, and go-to-definition across the
  Tua/GLSL boundary.
- **GLSL compilation**: real compiler errors for LÖVE's dialect, produced by
  the user's own LÖVE (which contains glslang) or `glslangValidator`, rather
  than a GLSL front end inside Tua.
- **Workflow**: a setup step that installs shader tooling, and hot reload for
  shader files in `tua dev`.

## Non-goals

- A GLSL language server. Tua checks what it can verify about LÖVE shaders and
  leaves general GLSL editing to other extensions.
- Bundling a GLSL compiler. Validation uses the LÖVE executable the project
  already runs on, or `glslangValidator` where LÖVE cannot open a window.
  Everything else works without either.
- Uniform blocks and macro-generated declarations. LÖVE 11 cannot `send` to
  uniform blocks, and parsing macro output would require a preprocessor.
  Shaders that use either stay unchecked, as they are today.

## Implemented compilation slice (Unreleased)

The LÖVE-only compilation path is implemented: `[shaders]`, isolated CLI
validation, and a persistent idle LSP worker with content/target caching.
Diagnostics use `TL2016` on shader files and loader calls. The editor forwards
unsaved shader buffers without taking over GLSL language features. Validation
requires a client workspace-trust signal, and no executable paths are read from
project configuration. Discovery currently uses CLI `--love`, `TUA_LOVE`, PATH,
and the macOS app; it does not yet use `tua.love.command` for the server.

Fake-process timeout/protocol tests, editor synchronization tests, and a blocked
validator/interactive-hover regression cover the boundaries. Real LÖVE 11.5 on
macOS was tested; Windows `lovec.exe` remains unverified. Phase 1 editor features
and Phase 2 value-shape diagnostics are also implemented below. The
glslangValidator fallback and setup tooling remain planned. Phase 5 shader
hot reload is implemented.

## Current state

Shipped in `84e18f4` and `7d91af0` (asset module: `shader_uniforms`,
`shader_send_call`; checker: `check_shader_send_call`):

| Code | Severity | Reports |
| --- | --- | --- |
| `TL2013` | error | `send`/`sendColor` names a uniform the shader does not declare. Names built in numeric `for` loops with literal bounds are expanded and checked one by one. |
| `TL2014` | warning | A sent uniform is declared but never read, so the GLSL compiler removes it and LÖVE raises `does not exist`. |

Existing infrastructure this plan reuses:

- Shader assets resolve through the asset index (`validate_asset_with_index`,
  `resolved_asset_path`). Path completion and missing-file errors already
  exist.
- The language server registers file watchers for `**/*.glsl`, `.frag`, `.fs`,
  `.vert`, and `.vs` (`tua-lsp/src/server/handlers.rs`), so saved shader
  edits already trigger reanalysis.
- `tua watch` copies changed assets into the output directory. The hot-reload
  runtime (`vendor/runtime/tuahotreload.lua`) only reloads Lua modules whose
  emitted source changed, so a shader edit never reaches the running game.
- The VS Code setup walkthrough (`tua.setup`) already installs an optional
  extension on demand (Material Icons) and records reversible settings
  (`src/setup/presets.js`).

The shader parser now records types, array lengths, original declaration
positions, entrypoints, and read status in the shared cached uniform model.

## Constraints

### LÖVE's dialect

LÖVE does not compile shader files as written. It prepends a header that
defines `extern` as `uniform`, `number` as `float`, `Image` as `sampler2D`, and
`Texel` as the texture lookup. The header also declares built-ins such as
`love_ScreenSize`, `TransformProjectionMatrix`, and `VaryingTexCoord`, and adds
a `main()` that calls the user's `effect` or `position`. A single file can hold
both stages under `#ifdef VERTEX` and `#ifdef PIXEL`.

Generic GLSL linters therefore report false errors on almost every LÖVE
shader. Any compile check must reproduce LÖVE's wrapping. In LÖVE 11.5 the
wrapping lives in `src/modules/graphics/wrap_GraphicsShader.lua` (not
`Shader.cpp`), vendored unmodified at `vendor/love/11.5/`. See
[Phase 0](#phase-0-findings) for how it selects stages, versions, and line
numbering.

### Trust

Running a validator is running an executable on workspace content. A cloned
repository must not be able to choose what Tua executes:

- `tua.toml` never names an executable. It may only enable or disable
  validation and choose targets.
- The LÖVE executable comes from the same sources as `tua dev`: `--love`,
  `TUA_LOVE`, `tua.love.command`, `love` on `PATH`, then
  `/Applications/love.app` on macOS. `glslangValidator` comes from
  `TUA_GLSLANG`, a new `tua.shaders.glslangPath` setting, or `PATH`.
- `tua.love.command` is already in the extension's `restrictedConfigurations`,
  so VS Code ignores a workspace value until the workspace is trusted.
  `tua.shaders.glslangPath` joins that list.
- No validation runs in untrusted workspaces.

### Performance

The checker already reads shader files synchronously when a local is assigned
from a shader asset. Every new feature must reuse one parsed result per file
and content hash, not reread or reparse per call site. Validator runs happen
off the request path, with a timeout, and their results are cached by content
hash, validator version, and target.

## Shared model: `ShaderUniformSet` v2

Extend the parser once and drive every feature from it:

```rust
pub(crate) struct ShaderUniform {
    pub(crate) name: String,
    pub(crate) glsl_type: String,     // "vec4", "number", "Image", ...
    pub(crate) array_len: Option<usize>,
    pub(crate) declarations: Vec<SourceSpan>, // one per stage section
    pub(crate) read: bool,
}

pub(crate) struct ShaderUniformSet {
    pub(crate) uniforms: BTreeMap<String, ShaderUniform>,
    pub(crate) has_effect: bool,   // `vec4 effect(`
    pub(crate) has_position: bool, // `vec4 position(`
}
```

Requirements:

- Positions are line and column in the original file, after comment stripping
  keeps offsets. The current stripper replaces block comments with one space,
  which shifts columns and must preserve length instead.
- A per-analysis cache keyed by resolved path and content hash, shared by the
  checker, completion, hover, and definition. The checker's `shader_locals`
  map holds handles into this cache rather than copies.
- Unit tests cover multi-stage files, arrays, precision qualifiers,
  `#pragma language glsl3`, comments next to declarations, and CRLF.

## Phase 0: Findings

Done. Recorded on macOS (Apple Silicon) with LÖVE 11.5 and glslang 16.6.0.
Linux and Windows results are still needed; rerun the probes there before
Phase 3 depends on them.

### Artifacts

- Fixture corpus: `crates/tua-core/src/asset/fixtures/shaders/`. It holds 11
  shaders: pixel-only, vertex and pixel in one file, `glsl3`, LÖVE built-ins,
  arrays and matrices, an unread uniform, a compile error, a GLES-only error,
  a missing entry point, a multi-canvas `void effect()`, and a water shader
  that sends suffixed uniforms from loops. It was written for Tua rather than
  copied from a user project. Each file states its expected LÖVE result in a
  leading comment. `parses_the_shader_fixture_corpus` checks the uniform
  parser against all of them.
- LÖVE's wrapper: `vendor/love/11.5/wrap_GraphicsShader.lua`, unmodified, with
  its zlib header.
- Probes, rerunnable on any platform:
  - `love scripts/love-shader-facts [fixtures-dir]` validates every fixture with
    `love.graphics.validateShader` on desktop and GLES, then runs the
    `Shader:send` cases below.
  - `scripts/love-shader-facts/glslang.sh [fixtures-dir] [lua]` wraps each
    fixture with the vendored LÖVE code and runs `glslangValidator`.

### LÖVE as the validator

`love.graphics.validateShader(gles, code)` runs LÖVE's bundled glslang on the
CPU, using LÖVE's exact header for the installed version. This is the most
faithful validator available, and every LÖVE user already has it.

- It requires a window. With `t.window = false` it raises `love.graphics
  cannot function without a window!`, and SDL's `offscreen` video driver is
  not available in the macOS build. A 1×1 borderless window at `(-4000, -4000)`
  works. Whether a dock or taskbar entry flashes must be checked on each
  platform.
- Starting LÖVE takes about 0.6 s. Each validation takes 0.3–3.6 ms. Validate
  every shader in one process per `tua check`, and keep one process alive for
  the language server.
- Reported lines are user lines (`ERROR: 0:4:` for line 4) on both targets.
  Only the first error per stage is reported; glslang then prints
  `compilation terminated`.
- A shader without an entry point raises `missing 'position' or 'effect'
  function?` instead of returning a result.
- It cannot run in headless CI without a display. CI uses `glslangValidator`,
  or LÖVE under a virtual display such as `xvfb-run`.

### `glslangValidator`

- `--stdin -S frag|vert` works. The exit code is 0 for success and 2 for
  compile errors. Errors print as `ERROR: 0:LINE: message`. About 20 ms per
  stage, including wrapping.
- It agrees with LÖVE on all 11 fixtures once the code is wrapped with the
  vendored wrapper.
- Line numbers: for GLSL 1 code, LÖVE's desktop wrapper uses `#version 330
  core` with `LOVE_GLSL1_ON_GLSL3` and inserts `#line 0`. glslang 16 then
  reports user line 4 as line 3. With `#line 1`, the reported lines match on
  both targets. Tua's wrapper must emit `#line 1`, and the fixtures pin the
  mapping.
- Install: Homebrew `glslang` (16.6.0, bottled), `glslang-tools` on Debian and
  Ubuntu, or the Vulkan SDK.

### How LÖVE wraps a shader (11.5)

- Stages are detected by pattern. A file is vertex code if it contains
  `vec4 position(` and pixel code if it contains `vec4 effect(`, or
  `void effect(` for the custom multi-canvas form. One file can be both; it is
  compiled once per stage with `VERTEX` or `PIXEL` defined.
- The language is `glsl1` unless the code starts with `#pragma language glsl3`.
  On systems that support GLSL 3, `glsl1` code compiles as `#version 330 core`
  with `LOVE_GLSL1_ON_GLSL3`. GLES uses `#version 100` or `#version 300 es`.
- The header order is: version, stage define, optional defines, `SYNTAX`,
  stage `HEADER`, `UNIFORMS`, `FUNCTIONS`, stage `FUNCTIONS`, stage `MAIN`,
  the `#line` directive, then user code.

### What `Shader:send` rejects at runtime

| Case | LÖVE 11.5 |
| --- | --- |
| Unknown or unread uniform | error: `Shader uniform 'x' does not exist` |
| number ← number | ok |
| number ← table | error: `number expected, got table` |
| number ← two numbers | ok, the extra value is ignored |
| vec4 ← 4 numbers | ok |
| vec4 ← 3 numbers | error: `bad argument #7 (number expected, got nil)` |
| vec4 ← 5 numbers | ok, the extra value is ignored |
| vec4 ← number | error: `table expected, got number` |
| `sendColor` vec3 ← 3 or 4 components | ok |
| `sendColor` on a number uniform | error: `sendColor can only be used on vec3 or vec4 uniforms` |
| array[2] ← 2 or 3 values | ok, extra values are ignored |
| Image ← Canvas | ok |
| Image ← number | error: `Texture expected, got number` |
| mat2 ← rows or flat table | ok |
| mat2 ← `Transform` | error: `table expected, got userdata` |
| mat4 ← `Transform` | ok |
| bool ← boolean | ok |
| bool ← number | error: `boolean expected, got number` |
| No entry point | `newShader` error: `missing 'position' or 'effect' function?` |

### GLSL Lint

`dtoplak.vscode-glsllint` 1.9.2 (source: `hsimpson/vscode-glsllint`) has no
setting that turns it off. It activates on `*` and lints documents whose
language ID is `glsl`:

- A `.glsl` file has no stage mapping, so every open and save shows an error
  message asking for `glsllint.fallBackStage` or
  `glsllint.additionalStageAssociations`.
- `.frag` and `.vert` files are linted without LÖVE's header, so LÖVE code
  gets false errors.

It skips documents with any other language ID. Giving LÖVE shaders a Tua
language ID keeps GLSL Lint away from them, and gives the language server a
document to synchronize for live diagnostics. See Phase 4.

## Phase 1: Editor features from the uniform model

Implemented (Unreleased). Core and LSP tests cover completion ranking and
unfinished calls, hover, both-stage definitions, lexical shadowing, disk
invalidation, and unsaved overlays. The checker and editor share immutable
asset-index models. Queries perform no shader file reads. Macro-generated
uniforms and uniform blocks remain unsupported.

All three features key off `shader_send_call`'s first argument position and
the receiver's shader local.

### Completion

Inside `shader:send("|")` and `shader:sendColor("|")`, offer the declared
uniforms:

- Label is the name; detail is the GLSL type (`vec4[4]`); sorted by name.
- Unread uniforms are marked in the detail and ranked last.
- `sendColor` ranks `vec3`/`vec4` uniforms first.
- Works for partially typed names and for the literal prefix of a
  concatenation (`"duckRipple" .. i`).

Implementation: a completion context in `crates/tua-core/src/completion/`
detecting a string literal as the first argument of a recognized send call,
next to the existing asset-path completion (`asset::complete_asset_paths`).

### Hover

Hovering the uniform-name string shows the declaration line from the shader,
for example `extern vec4 duckRipple1;`, plus the file and line, the expected
Lua value (see Phase 2), and whether it is unread. Pattern names list the
matching uniforms.

### Go to definition

From the uniform string to its declaration in the `.glsl` file. A name declared
in both stage sections returns both locations. This uses the existing
definition plumbing that already resolves asset paths to files
(`DefinitionKind::Asset`).

Tests: core completion, hover, and definition tests, plus LSP protocol tests
for the URI and range of a definition in a non-Tua file.

## Phase 2: Value shape checks (`TL2015`)

Implemented (Unreleased), with core and LSP regression coverage. The expanded
LÖVE 11.5 probe confirms short vectors/matrices and wrong component types are
rejected, while numeric strings, matrix layout selectors, and raw Data are
accepted. Unknown values, lengths, heterogeneous dynamic name patterns, raw
Data sizes, and texture dimensions remain unchecked. Extra ignored values are
not diagnosed. No external validator is needed for these checks.
The initial expanded scalar/vector/matrix probe passed on macOS; subsequent
launches for the additional `bvec3`/`ivec2` cases timed out before producing any
probe output. Those cases have core regressions but still need a completed
native probe run.

Check the values passed to `send` against the uniform's GLSL type, which LÖVE
enforces at runtime:

| GLSL type | Accepted Lua value |
| --- | --- |
| `float`, `number`, `int` | number |
| `bool` | boolean |
| `vec2`/`vec3`/`vec4` (and `ivec`, `bvec`) | table of at least 2/3/4 numbers (booleans for `bvec`) |
| `mat2`/`mat3` | table of rows or a flat table |
| `mat4` | table of rows, a flat table, or a `Transform` |
| `Image`, `sampler2D`, `ArrayImage`, `CubeImage`, `VolumeImage` | a LÖVE texture: the matching image type or a `Canvas` |

Rules, from the Phase 0 table:

- Report only what LÖVE rejects. A vector table with fewer components than
  the type is an error; extra components are ignored by LÖVE and are not
  reported. A non-table sent to a vector, a table sent to a scalar, a number
  sent to a texture, a non-boolean sent to a `bool`, and a `Transform` sent to
  anything but `mat4` are errors.
- Array uniforms accept extra values silently, so array length is not checked.
- `sendColor` on a uniform that is not `vec3`/`vec4` is an error. Three or four
  components are both accepted.
- Only report what the types prove. A table whose length is unknown is not
  checked for component count.
- Error severity, since LÖVE raises in every reported case. Each rule is
  backed by a row in the Phase 0 table and rerun with the probe.

This depends on the `table` parameter fix (`b054c6b`) so that arrays reach the
check as `number[]` rather than an empty table.

## Phase 3: GLSL compilation (`TL2016`)

### Validators

1. **LÖVE** (preferred): the project's LÖVE executable, found as in
   [Trust](#trust). It runs a small bundled Lua program, like
   `scripts/love-shader-facts`, that reads shader sources and targets on stdin
   and writes one result per stage. It uses LÖVE's own header, so it follows
   the installed LÖVE version exactly.
2. **`glslangValidator`** (fallback): used when LÖVE cannot open a window, as in
   headless CI, or when the user selects it. Tua wraps the code itself.

### Ownership

`tua-core` stays free of process spawning. It provides pure functions:

- `shader_compile_units(source, target) -> Vec<CompileUnit>` detects stages
  with LÖVE's patterns and wraps each one with the vendored header text,
  ending in `#line 1`. Only the `glslangValidator` path needs this.
- `parse_shader_errors(output) -> Vec<ShaderDiagnostic>` reads
  `ERROR: 0:LINE: message` from either validator. It reports errors in the
  header lines as an internal problem rather than as a user error.

`tua-cli` and `tua-lsp` run the validator, cache results by content hash,
validator, version, and target, and publish diagnostics.

### Targets

- `desktop` (default): LÖVE's desktop GLSL version, and GLSL 3.30 for files
  with `#pragma language glsl3`.
- `gles` (opt-in): GLSL ES 1.00/3.00. It catches shaders that work on desktop
  but fail on mobile or the web, such as missing precision or implicit
  int/float conversion.

### Configuration

```toml
[shaders]
validate = "auto"        # "auto", "love", "glslang", or "off"
targets = ["desktop"]    # add "gles" for mobile/web builds
```

`auto` uses LÖVE when it can open a window and `glslangValidator` otherwise.
Executable paths come only from the sources listed under [Trust](#trust).

### Diagnostics

- Published on the `.glsl` file's URI, with the original line and column.
- One summary diagnostic at the `shader "…"` call in Tua when its shader fails
  to compile, so `tua check` output and Tua files show the problem.
- When no validator is found, `tua check` prints a single note, not an error.
  `--deny-warnings` is unaffected.

### Language server

- Validate on open and on save of shader files, and after the file watcher
  reports a change.
- Live validation of unsaved shader edits needs the client to sync shader
  documents. Synchronize the `love-glsl` language from Phase 4 (and `glsl`
  when the user keeps that association), used only for shader features.
- Keeps one LÖVE validator process per workspace, restarted if it exits. Runs
  off the main scheduler queue, cancels stale runs on edit, has a timeout, and
  is skipped in untrusted workspaces.

## Phase 4: VS Code setup step

Add a step to the `tua.setup` walkthrough, after the icon step:

**Shader support (optional)**

- **A LÖVE shader language.** The extension contributes a `love-glsl` language
  whose grammar includes `source.glsl` when a GLSL grammar is installed and
  highlights LÖVE's `extern`, `number`, `Image`, `Texel`, `effect`, and
  `position`. The step maps the project's shader files to it with a
  workspace-level `files.associations` entry, so GLSL Lint, which only lints
  `glsl` documents, no longer reports false errors on them. The mapping is
  reversible like the other presets.
- **Install highlighting.** Installs `slevesque.shader` ("Shader languages
  support for VS Code", which provides the GLSL grammar) through
  `workbench.extensions.installExtension`, as the Material Icons step does.
- **Find a validator.** Reports whether Tua found LÖVE, the version, and
  whether it can validate. When only headless validation is possible, or the
  user wants it, shows the `glslangValidator` install command:
  `brew install glslang` on macOS, `apt install glslang-tools` on Debian and
  Ubuntu, or the Vulkan SDK on Windows. Offers a file picker for
  `tua.shaders.glslangPath`.
- **Duplicate checks.** If GLSL Lint is installed, explains why LÖVE shaders
  moved to `love-glsl`. Tua does not install GLSL Lint.
- **Reversible.** Every setting written is backed up under
  `tua.setup.previous.v1.shaders` and has a Restore action.

Completion event: the shader association is applied, or the user chose to
skip.

Tests: extend `scripts/manifest-check.js` and the activation smoke test.

## Phase 5: Shader hot reload in `tua dev`

Implemented (Unreleased). The watch build emits a complete shader-to-output-module
map in each reload signal; the runtime compares shader content so skipped
generations cannot lose changes. It preflights changed shaders before any reload
or restart, releases temporary shader objects, preserves the running game after
shader errors, and replaces exported shader objects. Core asset references drive
the map. CLI signal integration and plain-Lua runtime regressions cover the flow.
Dynamic paths and assets outside the copied source tree remain untracked.

Design:

1. `tua dev` knows which modules load each shader asset from the analysis
   asset references. When a build changes only shader files, it writes the
   complete shader-to-module map into the reload signal. The runtime detects
   changed shader content and selects the affected loaded modules.
2. The runtime reloads those modules even though their Lua is unchanged.
   Module reload re-runs the file, so `local water = shader "…"` builds a new
   `Shader` from the new source. The new module functions capture it.
3. Before reloading, the runtime compiles the new source with
   `love.graphics.newShader` inside `pcall`. On failure it prints the GLSL error
   to the console and the overlay and keeps the last working shader and module.
4. State caveat, documented in `docs/hot-reload.md`: values derived from the
   shader in module-level locals reset, as for any module reload.

Tests: an integration test in `tua-cli` for the signal contents, and a
plain-Lua runtime test for reload-on-asset with a stub `love.graphics`.

## Phase 6: Further checks

- **Missing entry point (`TL2017`)**: a shader with no `effect` (pixel) or
  `position` (vertex) fails in `newShader`. Cheap to check from the uniform
  model; reported at the `shader "…"` call. Useful without a validator.
- **Uniforms never sent (`TL2018`, opt-in)**: a uniform that is read but sent
  by no Tua file keeps its default of zero. This needs project-wide asset
  references and is a lint, not a type check.
- **Inline shaders**: apply the uniform model, Phases 1–3, and `TL2017` to
  `love.graphics.newShader([[ … ]])` with literal source. Diagnostics map into
  the long string's lines.

## Diagnostic codes

| Code | Phase | Severity | Meaning |
| --- | --- | --- | --- |
| `TL2013` | done | error | Unknown uniform name |
| `TL2014` | done | warning | Uniform declared but never read |
| `TL2015` | 2 | error | Sent value does not match the uniform's GLSL type |
| `TL2016` | 3 | error | Shader does not compile for a target |
| `TL2017` | 6 | error | Shader lacks the entry point for its stage |
| `TL2018` | 6 | warning, opt-in | Uniform is read but never sent |

Each new code goes into `DiagnosticCode::is_known`,
`docs/luahelper-code-matrix.md`, and the shader section of
`docs/language.md`.

## Recommended order

1. Phase 0 (done on macOS; repeat the probes on Linux and Windows).
2. Phase 1 together with the `ShaderUniformSet` v2 model: small, visible, and
   needs no external tools.
3. Phase 5: the largest day-to-day improvement for shader iteration.
4. Phase 2.
5. Phase 3, then Phase 4, so the setup step can point at a working feature.
6. Phase 6 items as they become useful.

Each phase lands as separate commits, one per feature, with regression tests,
CHANGELOG entries, and documentation.

## Open questions

- LÖVE 12 changes shader syntax and adds features such as uniform buffers. Do
  we target LÖVE 11.5 only, matching the current API catalog, and add 12 when
  the catalog moves?
- Should `TL2016` block `tua build` like other errors, or stay advisory? A
  validator can disagree with a specific GPU driver.
- Does the off-screen 1×1 LÖVE window flash a dock or taskbar entry on macOS,
  Windows, or Linux? If it does, the language server should use
  `glslangValidator` when available and LÖVE only on save or on request.

## References

- GLSL Lint, `dtoplak.vscode-glsllint`: <https://marketplace.visualstudio.com/items?itemName=dtoplak.vscode-glsllint>
- Shader languages support, `slevesque.shader`: <https://marketplace.visualstudio.com/items?itemName=slevesque.shader>
- glslang reference front end: <https://github.com/KhronosGroup/glslang>
- GLSL Lint source: <https://github.com/hsimpson/vscode-glsllint>
- LÖVE 11.5 shader wrapper: <https://github.com/love2d/love/blob/11.5/src/modules/graphics/wrap_GraphicsShader.lua>
- LÖVE `love.graphics.validateShader`: <https://love2d.org/wiki/love.graphics.validateShader>
- LÖVE `Shader:send`: <https://love2d.org/wiki/Shader:send>

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
- **GLSL compilation**: real compiler errors for LÖVE's dialect, produced by an
  external validator rather than a GLSL front end inside Tua.
- **Workflow**: a setup step that installs shader tooling, and hot reload for
  shader files in `tua dev`.

## Non-goals

- A GLSL language server. Tua checks what it can verify about LÖVE shaders and
  leaves general GLSL editing to other extensions.
- Bundling a GLSL compiler. Validation uses `glslangValidator` when the user
  has one; everything else works without it.
- Uniform blocks and macro-generated declarations. LÖVE 11 cannot `send` to
  uniform blocks, and parsing macro output would require a preprocessor.
  Shaders that use either stay unchecked, as they are today.

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

The shader parser (`shader_uniforms`) knows declared names and which are unread.
It does not yet record types, array lengths, or declaration positions.

## Constraints

### LÖVE's dialect

LÖVE does not compile shader files as written. It prepends a header that
defines `extern` as `uniform`, `number` as `float`, `Image` as `sampler2D`, and
`Texel` as the texture lookup. The header also declares built-ins such as
`love_ScreenSize`, `TransformProjectionMatrix`, and `VaryingTexCoord`, and adds
a `main()` that calls the user's `effect` or `position`. A single file can hold
both stages under `#ifdef VERTEX` and `#ifdef PIXEL`.

Generic GLSL linters therefore report false errors on almost every LÖVE
shader. Any compile check must reproduce LÖVE's wrapping. Phase 0 must pin the
exact header for LÖVE 11.5 from `src/modules/graphics/Shader.cpp`, and record
how `#pragma language glsl3` and GLSL ES targets change it.

### Trust

Running a validator is running an executable on workspace content. The
executable path may come only from user-level settings (`tua.shaders.validatorPath`)
or `PATH`, never from `tua.toml`. A cloned repository must not be able to
choose what Tua executes. `tua.toml` may only enable or disable validation and
choose targets. The VS Code extension does not run the validator in untrusted
workspaces.

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

## Phase 0: Fixtures and facts

Produce evidence before building:

- A fixture corpus under `crates/tua-core/tests/fixtures/shaders/` (or the
  asset test helpers): a pixel-only shader, a vertex+pixel shader, a
  `glsl3` shader, one using LÖVE built-ins, one with arrays, one with an
  unread uniform, one with a compile error, and the zombie project's
  `water.glsl` (with permission) as a realistic case.
- The exact LÖVE 11.5 header and footer text per stage and target, vendored
  with the zlib license notice.
- `glslangValidator` facts on macOS, Linux, and Windows: stdin support
  (`--stdin -S frag`), the error output format, whether `#line 1` after the
  header makes reported lines match the user file, the exit codes, and how
  long one compile takes.
- Whether GLSL Lint (`dtoplak.vscode-glsllint`) can be turned off per
  workspace or per file extension. This decides how the setup step avoids
  duplicate, false diagnostics.
- A small LÖVE script that confirms each runtime failure this plan claims to
  prevent: unknown uniform, unread uniform, wrong value shape, wrong array
  length, missing entry point. `84e18f4` already confirmed the unread case.

Exit criteria: fixtures committed, header vendored, validator behavior
recorded in this document.

## Phase 1: Editor features from the uniform model

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

Check the values passed to `send` against the uniform's GLSL type, which LÖVE
enforces at runtime:

| GLSL type | Accepted Lua value |
| --- | --- |
| `float`, `number`, `int` | number |
| `bool` | boolean |
| `vec2`/`vec3`/`vec4` (and `ivec`, `bvec`) | table of 2/3/4 numbers (booleans for `bvec`) |
| `mat2`/`mat3`/`mat4` | table of rows, or a flat table, or a `Transform` (confirm in Phase 0) |
| `Image`, `sampler2D`, `ArrayImage`, `CubeImage`, `VolumeImage` | the matching LÖVE texture type, or a `Canvas` |
| arrays `T[n]` | up to `n` values of the element shape |

Rules:

- Only report what the types prove. A table literal with three numbers sent to
  a `vec4` is an error. An array of unknown length is not.
- `sendColor` requires `vec3`/`vec4` targets and colors in the 0–1 range when
  literal.
- Error severity, since LÖVE raises in every reported case. Each case gets a
  LÖVE fixture from Phase 0.

This depends on the `table` parameter fix (`b054c6b`) so that arrays reach the
check as `number[]` rather than an empty table.

## Phase 3: GLSL compilation through `glslangValidator` (`TL2016`)

### Ownership

`tua-core` stays free of process spawning. It provides pure functions:

- `shader_compile_units(source, target) -> Vec<CompileUnit>` wraps the file for
  each stage present (`VERTEX`, `PIXEL`) with LÖVE's header and footer and a
  `#line 1` directive before the user code.
- `parse_glslang_output(output, unit) -> Vec<ShaderDiagnostic>` maps messages
  back to the original file, and drops or rewrites errors located in the
  header.

`tua-cli` and `tua-lsp` run the validator, cache results, and publish
diagnostics.

### Targets

- `desktop` (default): LÖVE's desktop GLSL version, and GLSL 3.30 for files
  with `#pragma language glsl3`.
- `gles` (opt-in): GLSL ES 1.00/3.00. It catches shaders that work on desktop
  but fail on mobile or the web, such as missing precision or implicit
  int/float conversion.

### Configuration

```toml
[shaders]
validate = "auto"        # "auto" (when a validator is found), "off"
targets = ["desktop"]    # add "gles" for mobile/web builds
```

The validator path comes from the `TUA_GLSLANG` environment variable,
`tua.shaders.validatorPath` in user settings, or `PATH`, in that order.

### Diagnostics

- Published on the `.glsl` file's URI, with the original line and column.
- One summary diagnostic at the `shader "…"` call in Tua when its shader fails
  to compile, so `tua check` output and Tua files show the problem.
- When no validator is found, `tua check` prints a single note, not an error.
  `--deny-warnings` is unaffected.

### Language server

- Validate on open and on save of shader files, and after the file watcher
  reports a change.
- Live validation of unsaved shader edits needs the client to sync `.glsl`
  documents. Add a document selector for `glsl` (and the shader file
  extensions) used only for synchronization, not for Tua features.
- Runs off the main scheduler queue, cancels stale runs on edit, has a timeout,
  and is skipped in untrusted workspaces.

## Phase 4: VS Code setup step

Add a step to the `tua.setup` walkthrough, after the icon step:

**Shader support (optional)**

- **Install highlighting.** Installs `slevesque.shader` ("Shader languages
  support for VS Code", which gives GLSL highlighting) through
  `workbench.extensions.installExtension`, as the Material Icons step does.
- **Find a validator.** Detects `glslangValidator` and shows its version. When
  missing, shows the platform command: `brew install glslang` on macOS,
  `apt install glslang-tools` on Debian/Ubuntu, or the Vulkan SDK or
  `winget` on Windows. Offers a file picker that writes
  `tua.shaders.validatorPath` at user level.
- **Avoid duplicate checks.** If GLSL Lint is installed, explains that it does
  not understand LÖVE's dialect. If Phase 0 finds a supported way to disable it
  for the workspace, offers that as a reversible preset. Tua does not install
  GLSL Lint.
- **Reversible.** Like the other presets, every setting written is backed up
  under `tua.setup.previous.v1.shaders` and has a Restore action.

Completion event: the validator was found, or the user chose to skip.

Tests: extend `scripts/manifest-check.js` and the activation smoke test.

## Phase 5: Shader hot reload in `tua dev`

Today a changed `.glsl` is copied to the output, but the game keeps the old
shader.

Design:

1. `tua dev` knows which modules load each shader asset from the analysis
   asset references. When a build changes only shader files, it writes the
   affected module names into the reload signal.
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

1. Phase 0, since every later phase depends on its fixtures and facts.
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
- Is the zombie project's `water.glsl` acceptable as a test fixture, or should
  we write an equivalent?

## References

- GLSL Lint, `dtoplak.vscode-glsllint`: <https://marketplace.visualstudio.com/items?itemName=dtoplak.vscode-glsllint>
- Shader languages support, `slevesque.shader`: <https://marketplace.visualstudio.com/items?itemName=slevesque.shader>
- glslang reference front end: <https://github.com/KhronosGroup/glslang>
- LÖVE `Shader:send`: <https://love2d.org/wiki/Shader:send>

---
icon: lucide/package
---

# Shaders

Tua understands shader assets loaded by `shader "path.glsl"` and
`love.graphics.newShader("path.glsl")`. It can validate shader code, check
uniform names and values, and provide uniform completion, hover, and navigation
in the editor. See [shader uniform editing](lsp.md#shader-uniform-editing) and
[shader compilation diagnostics](lsp.md#shader-compilation-diagnostics).

## Split shader source across files

LÖVE does not expand GLSL `#include` directives itself. The opt-in
`ShaderScan` module expands them before passing the code to LÖVE:

```tua
local shaders = ShaderScan.new()
function love.load()
    shaders:load_shader("water", "shaders/water.glsl")
end

function love.update(dt)
    shaders:update(dt)
end

function love.draw()
    love.graphics.setShader(shaders.s.water)
    love.graphics.rectangle("fill", 0, 0, 800, 600)
    love.graphics.setShader()
end
```

Use paths relative to the running LÖVE game root. With `source = "src"` and
`out = "build"`, `src/shaders/water.glsl` is copied to
`build/shaders/water.glsl`; both `load_shader` and `#include` use
`"shaders/..."`. A source path such as `"src/shaders/water.glsl"` exists while
editing but does not exist inside the built game. ShaderScan suggests the
build-relative path when it finds that file at runtime. Tua also reports
`TL2019` on a literal `load_shader` path when the file exists under the
configured source root but the literal path will disappear after build. The
diagnostic names the path to use. Dynamic paths cannot be checked this way.

For example:

```glsl
// shaders/water.glsl
#include "shaders/lib/noise.glsl"

vec4 effect(vec4 color, Image texture, vec2 uv, vec2 pixel) {
    return Texel(texture, uv + noise(uv)) * color;
}
```

Included files are expanded once per shader. This makes shared includes and
cycles finite and prevents a shared declaration from being duplicated. If an
include is missing or escapes the game root, the load error shows the
including file and line. LÖVE compiler errors are mapped from the expanded
source back to the original included file and line.

`ShaderScan` is a built-in module: using it in Tua adds its runtime to the
project build. The implementation is independent and follows the feature ideas
of the [MIT-licensed LÖVE ShaderScan project](https://github.com/idbrii/love-shaderscan).
The [feature-tour example](https://github.com/Kyonru/tua-lang/blob/main/examples/features/src/shaders.tua)
shows the loading, update, and drawing flow with a bundled shader asset.
The scanner and its `s` collection keep their method and `Shader` lookup types
when exported from a Tua module. A lookup can be `nil` before its first
successful load, so check it before drawing.

Call `shaders:update(dt)` from `love.update` to poll the shader and its includes
for edits. A successful compile replaces `shaders.s.water`; a compile failure
keeps the previous shader active and prints a source-mapped error. Use the
shader through `shaders.s.<name>` so drawing code sees the replacement rather
than a shader value cached before the edit. Polling checks file contents every
250 ms and does not require `tua dev`.

Set `scanner.on_compiled` before loading when a shader needs textures or
uniforms restored. The callback receives `(name: string, path: string,
shader: Shader, previous: Shader | nil)` after compilation succeeds but before
`scanner.s[name]` changes. `previous` is `nil` on the first successful load;
on reload it is the still-active shader. For example:

```tua
local shaders = ShaderScan.new()
local elapsed = 0
shaders.on_compiled = function(name: string, path: string, replacement: Shader, previous: Shader | nil)
    if name == "water" then
        replacement:send("time", elapsed)
        replacement:send("noiseTexture", noiseTexture)
    end
end
shaders:load_shader("water", "shaders/water.glsl")
```

The callback runs for both direct `load_shader` calls and successful automatic
reloads. If it throws, ShaderScan releases the new candidate and keeps the
last working shader active. A direct load reports the error to its caller;
an automatic reload prints it once. An automatic retry occurs when the shader
or an included file changes again. Fixing setup state without editing a file
requires another explicit `load_shader` call. A successful replacement releases
the previous shader after the callback returns. The callback's return value
does not affect activation.

If the first load fails during development, catch the load error so the game
continues to call `update`. The scanner remembers the failed shader and retries
when its file or an included file changes:

```tua
local ok, message = pcall(function()
    shaders:load_shader("water", "shaders/water.glsl")
end)
if not ok then print(message) end
```

## Preload shaders for a loading screen

Synchronous `load_shader(name, path)` remains available. To spread a named list
across frames, call `scanner:preload(entries, on_progress?, on_complete?)` and
keep calling `scanner:update(dt)`. Each entry is `{ name = "...", path = "..." }`.
The returned job has these fields:

| Field | Meaning |
| --- | --- |
| `total` | Number of entries submitted. |
| `completed` | Entries attempted; always `successful + failed`. |
| `successful`, `failed` | Compile and setup results. |
| `current_name`, `current_path` | First queued entry before the first update, then the most recently attempted entry. |
| `errors` | `{ name, path, message }` for each failed entry, including source mapped include and compiler errors. |
| `done`, `cancelled` | Whether the job ended, and whether it ended by cancellation. |

`on_progress(job)` runs after each attempt. `on_complete(job)` runs exactly once
after the last entry, after `job:cancel()`, or immediately for an empty list.
Progress or completion callback errors are printed and do not change shader
results or counts. Only one preload job can run per scanner; starting another
before the first finishes raises an error. Empty names or paths and duplicate
names are rejected when the list is submitted.

For a complete loading screen with progress, error display, retry, and a
transition into gameplay, see
[`examples/features/src/shader_preload.tua`](https://github.com/Kyonru/tua-lang/blob/main/examples/features/src/shader_preload.tua).
Wire it into a LÖVE entry point like this:

```tua
local Loading = require("shader_preload")
local screen = Loading.begin()

function love.update(dt)
    screen.update(dt)
end
function love.draw()
    screen.draw()
end
function love.keypressed(key)
    if key == "r" then screen.retry() end
    if key == "escape" then screen.cancel() end
end
```

Each `update` call attempts at most one queued shader. A failure is recorded in
`job.errors`, and later entries still run. The scanner uses the same include
expansion, source mapped diagnostics, and `on_compiled` callback as direct
loads. Successfully preloaded shaders are immediately available through
`scanner.s[name]` and continue to hot reload after the job ends. A failed job
keeps its final counts; start a new job after fixing a shader to retry it.

`job:cancel()` skips entries not yet attempted and preserves completed shaders.
Outside a compilation it notifies completion immediately. If called from
`on_compiled` during a compilation, it waits for that entry's result and
progress notification before completing. `cancelled` is true when entries were
skipped; cancelling after the final entry has completed leaves it false. It
cannot interrupt a GPU compilation on the main thread. There is no per-frame time
budget: limiting the queue to one compilation per `update` lets the game draw
between entries, but a single GPU compilation can still stall that frame.
Some shaders also incur a separate first-draw GPU cost; ShaderScan does not
warm them up by drawing, so a game that needs that can render with the compiled
shader during its loading phase.

Tua expands includes for the shared shader index. Uniforms
declared in included files participate in `Shader:send` diagnostics, completion,
hover, go to definition, and LOVE-backed shader validation. Diagnostics and
definitions retain the included file's original path and line. This analysis
also applies to shaders loaded through the built-in `shader` helper and
`love.graphics.newShader` when the shader is inside a configured asset root.

## Optional uniforms

For a uniform that can be optimized out or vary by renderer, use
`ShaderScan.safe_send`:

```tua
ShaderScan.safe_send(water, "desktopOnlyGlow", glow)
```

It calls `Shader:hasUniform` first, skips the send if the uniform is absent,
and prints one warning per shader and uniform name. It returns `true` after a
send and `false` when it skips. For required uniforms, use `:send` so Tua can
report misspellings and incompatible values.

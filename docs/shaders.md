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

Use project-root-relative include paths:

```glsl
// shaders/water.glsl
#include "shaders/lib/noise.glsl"

vec4 effect(vec4 color, Image texture, vec2 uv, vec2 pixel) {
    return Texel(texture, uv + noise(uv)) * color;
}
```

Included files are expanded once per shader. This makes shared includes and
cycles finite and prevents a shared declaration from being duplicated. If an
include is missing or escapes the project root, the load error shows the
including file and line. LÖVE compiler errors are mapped from the expanded
source back to the original included file and line.

`ShaderScan` is a built-in module: using it in Tua adds its runtime to the
project build. The implementation is independent and follows the feature ideas
of the [MIT-licensed LÖVE ShaderScan project](https://github.com/idbrii/love-shaderscan).
The [feature-tour example](https://github.com/Kyonru/tua-lang/blob/main/examples/features/src/shaders.tua)
shows the loading, update, and drawing flow with a bundled shader asset.

Call `shaders:update(dt)` from `love.update` to poll the shader and its includes
for edits. A successful compile replaces `shaders.s.water`; a compile failure
keeps the previous shader active and prints a source-mapped error. Use the
shader through `shaders.s.<name>` so drawing code sees the replacement rather
than a shader value cached before the edit. Polling checks file contents every
250 ms and does not require `tua dev`.

If the first load fails during development, catch the load error so the game
continues to call `update`. The scanner remembers the failed shader and retries
when its file or an included file changes:

```tua
local ok, message = pcall(function()
    shaders:load_shader("water", "shaders/water.glsl")
end)
if not ok then print(message) end
```

Tua expands project-root-relative includes for the shared shader index. Uniforms
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

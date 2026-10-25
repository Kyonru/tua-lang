# LÖVE shader support

Apply shader associations to use **LÖVE Shader** for `.glsl`, `.vert`, `.frag`,
`.vs`, and `.fs` files in this workspace. Restore keeps later manual changes.
The optional Shader languages extension supplies the full GLSL grammar.
Tua supplies LÖVE keywords and validation; it does not install GLSL Lint.

Validation prefers LÖVE. When LÖVE is unavailable, install `glslangValidator`:

- macOS: `brew install glslang`
- Debian / Ubuntu: `sudo apt install glslang-tools`
- Windows: install the Vulkan SDK and put `glslangValidator.exe` on PATH.

The fallback targets LÖVE 11.5 headers. LÖVE validation can briefly open a
small graphics window. Validation runs in a background worker, not while
handling completion requests.

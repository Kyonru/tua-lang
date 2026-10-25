#!/usr/bin/env python3
"""Generate headless-validation prefixes from the vendored LOVE 11.5 wrapper."""
import json
import os
import pathlib
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
SCRIPT = r'''
R = function() end
love = { graphics = {
 getSupported = function() return { glsl3 = arg[1] == "desktop" or arg[2] == "glsl3" } end,
 isGammaCorrect = function() return false end,
 _setDefaultShaderCode = function() end,
} }
assert(loadfile("vendor/love/11.5/wrap_GraphicsShader.lua"))()
local f = assert(io.open(arg[4], "rb")); local source = f:read("*a"); f:close()
local vertex, pixel = love.graphics._shaderCodeToGLSL(arg[1] == "gles", source)
local code = arg[3] == "vertex" and vertex or pixel
io.write((code:gsub("\n#line 0\n", "\n#line 1\n")))
'''

def generate():
    vendor = (ROOT / "vendor/love/11.5/wrap_GraphicsShader.lua").read_text()
    notice = vendor.split("--[[", 1)[1].split("--]]", 1)[0].strip()
    headers = {"_notice": "Generated and adapted from LOVE 11.5 (source-string line markers changed).\n\n" + notice}
    with tempfile.TemporaryDirectory(prefix="tua-shader-headers-") as temp:
        script = pathlib.Path(temp) / "generate.lua"
        source_file = pathlib.Path(temp) / "source.glsl"
        script.write_text(SCRIPT)
        for target in ("desktop", "gles"):
            for language in ("glsl1", "glsl3"):
                for stage, body in {
                    "vertex": "vec4 position(mat4 m, vec4 p) { return m * p; }",
                    "pixel": "vec4 effect(vec4 c, Image t, vec2 u, vec2 p) { return c; }",
                    "custom": "void effect() { love_PixelColor = vec4(1.0); }",
                    "multicanvas": "void effect() { love_Canvases[0] = vec4(1.0); }",
                }.items():
                    source = f"#pragma language {language}\n{body}\n"
                    source_file.write_text(source)
                    wrapped = subprocess.check_output(
                        [os.environ.get("LUA", "lua"), str(script), target, language, stage, str(source_file)],
                        cwd=ROOT, text=True,
                    )
                    assert wrapped.endswith(source)
                    headers[f"{target}/{language}/{stage}"] = wrapped[:-len(source)].replace("#line 1\n", "#line 1 1\n")
    return json.dumps(headers, indent=2, sort_keys=True) + "\n"

if __name__ == "__main__":
    import sys
    output = ROOT / "crates/tua-core/src/shader_validation/generated/love-11.5.json"
    generated = generate()
    if "--check" in sys.argv:
        if output.read_text() != generated:
            raise SystemExit("Shader headers are stale: run scripts/generate-shader-headers.py")
    else:
        output.write_text(generated)

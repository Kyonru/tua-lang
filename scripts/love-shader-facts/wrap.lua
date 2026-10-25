-- Prints the stage code LOVE 11.5 compiles for a shader file, using LOVE's
-- own wrapper from vendor/love/11.5. Runs under plain Lua 5.1+.
--
-- Usage: lua scripts/love-shader-facts/wrap.lua <desktop|gles> <vertex|pixel> <file>
--
-- LOVE inserts `#line 0` before GLSL 1 code compiled as GLSL 3 on desktop.
-- glslang 16 then numbers the first user line 0, so this script uses
-- `#line 1`, which reports user lines exactly on both targets.

local target, stage, path = arg[1], arg[2], arg[3]
local root = arg[0]:gsub("scripts/love%-shader%-facts/wrap%.lua$", "")

R = function() end -- the file opens with R"luastring"--( for C++ embedding
love = {
  graphics = {
    getSupported = function() return { glsl3 = true } end,
    isGammaCorrect = function() return false end,
    _setDefaultShaderCode = function() end,
  },
}

local function read(file)
  local handle = assert(io.open(file, "rb"))
  local text = handle:read("*a")
  handle:close()
  return text
end

local wrapper = read(root .. "vendor/love/11.5/wrap_GraphicsShader.lua")
assert((loadstring or load)(wrapper, "=wrap_GraphicsShader.lua"))()

local vertex, pixel = love.graphics._shaderCodeToGLSL(target == "gles", read(path))
local code
if stage == "vertex" then
  code = vertex
else
  code = pixel
end
if code then
  io.write((code:gsub("\n#line 0\n", "\n#line 1\n")), "\n")
end

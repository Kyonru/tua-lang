-- A stub LOVE filesystem whose files the test edits between polls.
local files = {}
local restarted = false
_G.loadstring = _G.loadstring or load
_G.love = {
  filesystem = {
    getInfo = function(path) return files[path] and { type = "file" } or nil end,
    read = function(path) return files[path] end,
  },
  event = { quit = function(reason) restarted = reason == "restart" end },
}

local function tick()
  love.update(0.25)
end

files["main.lua"] = "-- main"
files[".tuahotreload"] = "1\n"
files["game/enemy.lua"] = "local M = { hp = 3 }\nfunction M.speed() return 1 end\nfunction M.damage(amount) M.hp = M.hp - amount return M.hp end\nreturn M\n"
package.loaded["game.enemy"] = assert(loadstring(files["game/enemy.lua"]))()
local enemy = package.loaded["game.enemy"]
enemy.hp = 7

local updates = 0
function love.update() updates = updates + 1 end

local hot = require("tuahotreload")
hot.install()
tick()
assert(updates == 1, "the game's update still runs")

-- A later assignment replaces the game's callback, not the wrapper.
function love.update() updates = updates + 10 end
tick()
assert(updates == 11, "reassigned update is called through the wrapper")

-- An edit swaps new functions into the same table and keeps its data.
files["game/enemy.lua"] = "local M = { hp = 3 }\nfunction M.speed() return 2 end\nfunction M.damage(amount) M.hp = M.hp - amount return M.hp end\nreturn M\n"
files[".tuahotreload"] = "2\n"
tick()
assert(package.loaded["game.enemy"] == enemy, "module identity is kept")
assert(enemy.speed() == 2, "new function is used")
assert(enemy.hp == 7, "existing data is kept")
assert(enemy.damage(2) == 5 and enemy.hp == 5,
  "new functions keep using the preserved module table")

-- A broken reload pauses the game's callbacks instead of ending it.
files["game/enemy.lua"] = "local M = {\nreturn M\n"
files[".tuahotreload"] = "3\n"
local before = updates
tick()
tick()
assert(updates == before, "callbacks pause while a reload error is shown")
assert(enemy.speed() == 2, "old code keeps running after a failed reload")

-- Saving a fix resumes the game.
files["game/enemy.lua"] = "local M = { hp = 3 }\nfunction M.speed() return 3 end\nfunction M.damage(amount) M.hp = M.hp - amount return M.hp end\nreturn M\n"
files[".tuahotreload"] = "4\n"
tick()
assert(enemy.speed() == 3 and updates > before, "a fixed reload resumes the game")

-- Shader edits reload unchanged Lua, preserve data, and replace exported shaders.
local function hex(value) return (value:gsub(".", function(c) return string.format("%02x", c:byte()) end)) end
local function shader_signal(number, module)
  return number .. "\nshader\t" .. hex("water.glsl") .. "\t" .. hex(module or "game/water.lua") .. "\n"
end
local releases = 0
love.graphics = {
  newShader = function(source)
    source = files[source] or source
    assert(source ~= "broken", "GLSL syntax error")
    return { source = source, typeOf = function(_, kind) return kind == "Shader" end,
      release = function() releases = releases + 1 end }
  end,
}
files["water.glsl"] = "version one"
files["game/water.lua"] = 'local shader = love.graphics.newShader("water.glsl"); local M = { count = 1, shader = shader }; function M.source() return shader.source end; return M'
package.loaded["game.water"] = assert(loadstring(files["game/water.lua"]))()
local water = package.loaded["game.water"]
water.count = 8
files[".tuahotreload"] = shader_signal(5)
tick()
assert(water.source() == "version one")
files["water.glsl"] = "version two"
-- Two generations can be skipped; the complete map still identifies the edit.
files[".tuahotreload"] = shader_signal(8)
tick()
assert(water.source() == "version two" and water.count == 8)
assert(water.shader.source == "version two", "exported shaders must be replaced")
assert(releases > 0, "preflight shaders are released")

files["water.glsl"] = "broken"
files[".tuahotreload"] = shader_signal(9)
local before_shader_failure = updates
tick()
assert(water.source() == "version two", "failed shader keeps previous module")
assert(updates > before_shader_failure, "shader errors do not pause the last good game")
files["water.glsl"] = "version three"
files[".tuahotreload"] = shader_signal(10)
tick()
assert(water.source() == "version three", "fixed shader is retried")

-- A shader error must not resume callbacks paused by an earlier Lua error.
files["game/enemy.lua"] = "local M = {\nreturn M\n"
files[".tuahotreload"] = shader_signal(20)
tick()
local paused = updates
files["water.glsl"] = "broken"
files[".tuahotreload"] = shader_signal(21)
tick()
tick()
assert(updates == paused, "a later shader error keeps a blocking reload error paused")
files["water.glsl"] = "version four"
files["game/enemy.lua"] = "local M = { hp = 3 }\nfunction M.speed() return 4 end\nreturn M\n"
files[".tuahotreload"] = shader_signal(22)
tick()
assert(enemy.speed() == 4 and updates > paused, "fixing both errors resumes the game")

-- Invalid shaders loaded by main must not trigger a restart either.
files["water.glsl"] = "broken"
files[".tuahotreload"] = shader_signal(11, "main.lua")
tick()
assert(not restarted, "preflight precedes restart")
files["water.glsl"] = "valid entry shader"
files[".tuahotreload"] = shader_signal(12, "main.lua")
tick()
assert(restarted, "entry shader changes restart after successful preflight")
restarted = false

-- A changed entry point restarts the game.
files["main.lua"] = "-- main v2"
files[".tuahotreload"] = "5\n"
tick()
assert(restarted, "main.lua changes restart the game")

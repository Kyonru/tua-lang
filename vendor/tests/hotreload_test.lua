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
files["game/enemy.lua"] = "local M = { hp = 3 }\nfunction M.speed() return 1 end\nreturn M\n"
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
files["game/enemy.lua"] = "local M = { hp = 3 }\nfunction M.speed() return 2 end\nreturn M\n"
files[".tuahotreload"] = "2\n"
tick()
assert(package.loaded["game.enemy"] == enemy, "module identity is kept")
assert(enemy.speed() == 2, "new function is used")
assert(enemy.hp == 7, "existing data is kept")

-- A broken reload pauses the game's callbacks instead of ending it.
files["game/enemy.lua"] = "local M = {\nreturn M\n"
files[".tuahotreload"] = "3\n"
local before = updates
tick()
tick()
assert(updates == before, "callbacks pause while a reload error is shown")
assert(enemy.speed() == 2, "old code keeps running after a failed reload")

-- Saving a fix resumes the game.
files["game/enemy.lua"] = "local M = { hp = 3 }\nfunction M.speed() return 3 end\nreturn M\n"
files[".tuahotreload"] = "4\n"
tick()
assert(enemy.speed() == 3 and updates > before, "a fixed reload resumes the game")

-- A changed entry point restarts the game.
files["main.lua"] = "-- main v2"
files[".tuahotreload"] = "5\n"
tick()
assert(restarted, "main.lua changes restart the game")

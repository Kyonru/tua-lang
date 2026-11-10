-- Run from build/: `lua ../smoke.lua`.
local circles = 0
local labels = {}
love = {
  keyboard = { isDown = function() return false end },
  graphics = {
    clear = function() end,
    setColor = function() end,
    circle = function() circles = circles + 1 end,
    print = function(text) labels[#labels + 1] = text end,
  },
  window = { setMode = function() end, setTitle = function() end },
}

require("main")
love.load()
love.update(0.5)
love.update(0.5)
love.draw()
assert(circles == 5, "all protocol actors must draw")
assert(labels[2] == "Survived: 1", "generic score value must survive emitted Lua")
love.keypressed("r")
labels = {}
love.draw()
assert(labels[2] == "Survived: 0", "reset must clear score")
print("arena smoke passed")

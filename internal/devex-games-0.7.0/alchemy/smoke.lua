love = {
  window = { setTitle = function() end, setMode = function() end },
  graphics = {
    clear = function() end,
    setColor = function() end,
    print = function() end,
    rectangle = function() end,
  },
}

local game = require("main")
love.load()
assert(#game.elements == 4)
game.choose("fire")
game.choose("water")
assert(game.elements[5] == "steam")
assert(game.discoveries[1] == "steam")
game.choose("fire")
game.choose("water")
assert(#game.discoveries == 1)
game.choose("earth")
game.choose("water")
assert(game.elements[6] == "mud")
game.choose("fire")
game.choose("mud")
assert(game.elements[7] == "brick")
love.mousepressed(40, 105, 1)
love.keypressed("escape")
love.draw()
print("alchemy smoke passed")

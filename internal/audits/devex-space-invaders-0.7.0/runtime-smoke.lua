local draw_calls = 0
local text_calls = 0
local keys_down = {}

love = {
  keyboard = {
    isDown = function(key)
      return keys_down[key] == true
    end,
  },
  graphics = {
    rectangle = function()
      draw_calls = draw_calls + 1
    end,
    print = function()
      text_calls = text_calls + 1
    end,
  },
}

package.path = "./build/?.lua;./build/?/init.lua;" .. package.path

require("main")
local game = require("game")
love.load()
assert(game.state.wave == 1)

for frame = 1, 120 do
  keys_down.space = true
  keys_down.right = frame <= 5
  love.update(1 / 60)
end

assert(game.state.score > 0, "shots should hit and score against protocol-typed invaders")
assert(game.state.lives == 3)

love.draw()
assert(draw_calls > 0, "the game should draw the player and remaining invaders")
assert(text_calls == 1, "the game should draw its HUD once")

print(("runtime smoke passed: score=%d wave=%d"):format(game.state.score, game.state.wave))

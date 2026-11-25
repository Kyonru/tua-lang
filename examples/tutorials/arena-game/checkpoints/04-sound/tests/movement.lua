local Movement = require("movement")
local function simulate(steps)
  local player = { x = 800, y = 600 }
  for index = 1, steps do Movement.move(player, 1, 0, 180, 1 / steps) end
  return player
end
assert(math.abs(simulate(30).x - simulate(144).x) < 0.000001)
local player = { x = 800, y = 600 }
Movement.move(player, 0, 0, 180, 1)
assert(player.x == 800 and player.y == 600)
Movement.move(player, -1, -1, 180, 100)
assert(player.x == 32 and player.y == 48)
Movement.move(player, 1, 1, 180, 100)
assert(player.x == 1568 and player.y == 1168)
print("CHARACTER_MOVEMENT_OK step sizes, stopped input, world bounds")

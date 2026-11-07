local __tua_store = require("tuastore")
local player = __tua_store({ score = 10 })
function love.update(dt)
  local state = player:get()
  state.score = state.score + 1
end

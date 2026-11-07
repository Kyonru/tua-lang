local game = require("game").game
local transitions = 0
local stop = game:watch("score", function(value, previous, state)
  transitions = transitions + 1
  assert(value == 10)
  assert(previous == 0)
  assert(state.paused == false)
end)
game:update(function(state)
  state.score = state.score + 10
end)
local score = game:get().score
assert(score == 10)
assert(transitions == 1)
stop()
stop()
game:set({ score = 20, paused = true })
assert(transitions == 1)
print("PASS imported store, update inference, watch, set, idempotent unsubscribe")

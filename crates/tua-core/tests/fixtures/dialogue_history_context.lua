do
  local context = require("context")
  local scene = require("scene")
  local player = context.new(scene, { coins = 5, unlocked = true }, {
    history = { capacity = 8 },
    transitionBudget = 8,
  })
  player:start()
  assert(player:history().entries[1].text == "You have 5 coins.")
  player:updateContext({ coins = 10, unlocked = true })
  assert(player:history().entries[1].text == "You have 5 coins.")
  player:advance()
  assert(not player:choose("shop.buy"))
  assert(not player:choose("shop.bonus"))
  assert(#player:history().entries == 1)
  player:updateContext({ coins = 25, unlocked = true })
  assert(player:choose("shop.buy"))
  local entries = player:history().entries
  assert(#entries == 3 and entries[2].choiceId == "shop.buy")
  assert(entries[3].text == "Ask the host to complete the purchase.")
  player:start("loop")
  assert(player:snapshot().state == "faulted")
  assert(#player:history().entries == 3)
  player:dispose()
  print("typed context dialogue history passed")
end

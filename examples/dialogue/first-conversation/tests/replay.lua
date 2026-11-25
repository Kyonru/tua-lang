local player
local function setup(context)
  player = require("__tua_replay_game")
  love.window.setMode(800, 600, { vsync = 0 })
  assert(player:snapshot().node.textId == "intro.hello")
  love.keypressed("1")
  assert(player:snapshot().state == "awaiting_advance", "reply keys do not skip lines")
end
local function beforeFrame(context)
  local frame = context.frame
  if frame == 21 then
    love.keypressed("space")
    assert(player:snapshot().state == "awaiting_choice")
    love.keypressed("space")
    assert(player:snapshot().state == "awaiting_choice", "Space does not select a reply")
  end
  if frame == 41 then
    love.keypressed("1")
    assert(player:snapshot().node.textId == "intro.welcome")
  end
  if frame == 61 then
    love.keypressed("space")
    assert(player:snapshot().state == "completed")
  end
  if frame == 81 then
    love.keypressed("r")
    love.keypressed("space")
    love.keypressed("2")
    assert(player:snapshot().node.textId == "intro.farewell")
  end
  if frame == 101 then
    love.keypressed("space")
    assert(player:snapshot().state == "completed")
  end
end
local function verify(context)
  assert(player:snapshot().state == "completed")
  assert(#player:history().entries == 0, "history is opt-in")
  print("FIRST_CONVERSATION_OK both replies, final acknowledgement, restart, defaults")
end
return { setup = setup, beforeFrame = beforeFrame, verify = verify }

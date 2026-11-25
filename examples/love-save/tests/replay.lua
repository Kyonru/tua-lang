local session
local function setup(context)
  session = require("__tua_replay_game")
  assert(session.snapshot().score == 0 and session.issue() == nil)
end
local function beforeFrame(context)
  local frame = context.frame
  if frame >= 10 and frame <= 15 then love.keypressed("space") end
  if frame == 31 then love.keypressed("s") end
  if frame == 61 then love.keypressed("d") end
  if frame == 76 then
    assert(session.ui.confirmReset)
    love.keypressed("space")
    assert(session.snapshot().score == 6, "reset prompt owns input")
    love.keypressed("escape")
    assert(not session.ui.confirmReset and session.snapshot().score == 6)
  end
  if frame == 91 then love.keypressed("d") end
  if frame == 106 then
    love.keypressed("return")
    assert(session.snapshot().score == 0 and session.issue() == nil)
    love.keypressed("s")
  end
end
local function verify(context)
  assert(session.snapshot().score == 0 and not session.ui.confirmReset)
  print("SAVE_UI_OK points, checkpoint, reset confirmation, cancel and input ownership")
end
return { setup = setup, beforeFrame = beforeFrame, verify = verify }

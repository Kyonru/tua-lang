local host
local function setup(context)
  host = require("__tua_replay_game")
  love.window.setMode(1120, 720, { vsync = 0 })
  host.settings.reducedMotion = true
  love.keypressed("f3")
  for index = 1, 8 do host.advance() end
  love.keypressed("tab")
end
local function beforeFrame(context)
  local frame = context.frame
  if frame == 31 then
    love.keypressed("]")
    assert(host.settings.historyScroll > 0)
  end
  if frame == 61 then
    love.window.setMode(640, 480, { vsync = 0 })
    host.settings.historyScroll = 0
  end
  if frame == 91 then
    love.mousepressed(530, 390, 1)
    assert(host.settings.historyScroll > 0)
  end
  if frame == 121 then
    love.keypressed("[")
    assert(host.settings.historyScroll == 0)
  end
  if frame == 151 then
    host.reset()
    host.player():clearHistory()
  end
end
local function verify(context)
  assert(host.settings.transcript)
  assert(host.settings.historyScroll == 0)
  print("SHOWCASE_TRANSCRIPT_OK brackets, buttons, narrow layout and empty history")
end
return { setup = setup, beforeFrame = beforeFrame, verify = verify }

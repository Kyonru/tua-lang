local host
local modes = { "panel", "bubble", "radio", "subtitles", "mythic", "tactics", "terminal" }
local function setup(context)
  host = require("__tua_replay_game")
  love.window.setMode(1120, 720, { vsync = 0 })
  love.keypressed("f3")
  assert(host.settings.entry == "rich")
end
local function beforeFrame(context)
  local frame = context.frame
  if frame <= 140 and (frame - 1) % 20 == 0 then
    host.settings.mode = modes[math.floor((frame - 1) / 20) + 1]
    host.settings.reducedMotion = true
    host.player():start("@line:rich.clues")
    host.update(0)
  end
  if frame == 141 then
    host.settings.mode = "bubble"
    host.settings.reducedMotion = false
    host.player():start("@line:rich.motion")
    host.update(0)
  end
  if frame == 180 then host.advance() end
  if frame == 201 then
    love.keypressed("m")
    assert(host.view().complete)
  end
  if frame == 221 then
    love.window.setMode(640, 480, { vsync = 0 })
    host.locale()
    host.player():start("@line:rich.clues")
    host.update(0)
    love.keypressed("pagedown")
  end
  if frame == 241 then
    host.settings.mode = "mythic"
    host.player():start("@line:rich.nested")
    host.update(0)
  end
  if frame == 261 then
    host.locale()
    host.settings.reducedMotion = false
    host.player():start("@line:rich.timing")
    host.update(0)
  end
  if frame == 330 then
    host.settings.reducedMotion = true
    host.player():start("rich")
    host.update(0)
  end
end
local function verify(context)
  assert(host.player():snapshot().state ~= "faulted")
  print("SHOWCASE_RICH_NATIVE_OK seven styles, timing, motion, nested marks, French, narrow wrap")
end
return { setup = setup, beforeFrame = beforeFrame, verify = verify }

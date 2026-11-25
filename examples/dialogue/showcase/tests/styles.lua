local host
local styles = { "panel", "bubble", "radio", "subtitles" }
local portraits = { "left", "right", "above", "none" }
local function setup(context)
  host = require("__tua_replay_game")
  host.settings.reducedMotion = true
  love.window.setMode(1120, 720, { vsync = 0 })
end
local function beforeFrame(context)
  local frame = context.frame
  if frame <= 256 and (frame - 1) % 8 == 0 then
    local combination = math.floor((frame - 1) / 8)
    local narrow = combination >= 16
    local styleIndex = math.floor((combination % 16) / 4) + 1
    local portraitIndex = combination % 4 + 1
    love.window.setMode(narrow and 640 or 1120, narrow and 480 or 720, { vsync = 0 })
    host.settings.mode = styles[styleIndex]
    host.settings.portrait = portraits[portraitIndex]
    host.player():start(styleIndex == 3 and "signal_reply" or "start")
    host.update(0)
  end
  if frame == 257 then
    host.settings.mode = "panel"
    host.player():start("decision")
    host.update(0)
    host.advance()
    host.settings.selected = 2
  end
  if frame == 269 then
    love.keypressed("down")
    love.keypressed("down")
    assert(host.settings.selected == 4)
  end
  if frame == 281 then love.keypressed("h") end
  if frame == 293 then
    love.keypressed("h")
    host.settings.mode = "bubble"
    host.settings.portrait = "left"
    host.reset()
    host.locale()
    assert(host.enqueue("radio"))
  end
  if frame == 305 then love.keypressed("tab") end
  if frame == 317 then love.keypressed("tab") end
end
local function verify(context)
  assert(host.player():snapshot().state ~= "faulted")
  print("SHOWCASE_STYLES_OK 32 style/portrait/size combinations; choices, French, radio, transcript")
end
return { setup = setup, beforeFrame = beforeFrame, verify = verify }

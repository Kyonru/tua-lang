local host
local entries = {
  "staging",
  "@line:staging.ivo.right",
  "@line:staging.beacon.radio",
  "staging_chart",
  "@line:staging.mara.return",
}
local function setup(context)
  host = require("__tua_replay_game")
  love.window.setMode(1120, 720, { vsync = 0 })
  love.keypressed("f4")
  host.settings.reducedMotion = true
end
local function beforeFrame(context)
  local frame = context.frame
  if frame == 101 then
    love.window.setMode(640, 480, { vsync = 0 })
    host.locale()
  end
  if frame <= 200 and (frame - 1) % 20 == 0 then
    host.player():start(entries[math.floor((frame - 1) / 20) % 5 + 1])
    host.update(0)
  end
  if frame == 201 then
    host.player():start("@line:staging.mara.question")
    host.update(0)
    host.advance()
  end
  if frame == 221 then
    love.keypressed("down")
    love.keypressed("return")
  end
end
local function verify(context)
  assert(host.settings.entry == "staging")
  assert(host.player():snapshot().state ~= "faulted")
  print("SHOWCASE_STAGING_NATIVE_OK portraits, bubbles, overrides, French, narrow choices")
end
return { setup = setup, beforeFrame = beforeFrame, verify = verify }

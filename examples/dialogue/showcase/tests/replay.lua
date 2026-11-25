local host
local function press(context, key)
  context.key(key, true)
  context.key(key, false)
end
local function setup(context)
  host = require("__tua_replay_game")
  love.window.setMode(640, 480, { vsync = 0 })
end
local function beforeFrame(context)
  local frame = context.frame
  if frame == 5 then
    for index = 1, 6 do host.enqueue("radio") end
    assert(host.metrics().messages == 4 and host.metrics().rejected == 2)
  end
  if frame == 25 then press(context, "m") end
  if frame == 30 then press(context, "space") end
  if frame == 40 then press(context, "space") end
  if frame == 45 then
    assert(host.player():snapshot().state == "awaiting_choice")
    host.player():updateContext({ name = "Ada", permit = false })
    press(context, "return")
    assert(host.player():snapshot().state == "awaiting_choice")
    assert(host.metrics().rewards == 0)
    host.player():updateContext({ name = "Ada", permit = true })
    press(context, "down")
    press(context, "up")
    press(context, "return")
  end
  if frame == 65 then
    assert(host.metrics().rewards == 1)
    assert(host.player():snapshot().nodeId == "done")
    press(context, "f5")
  end
  if frame == 70 then press(context, "space") end
  if frame == 75 then
    press(context, "f9")
    assert(host.metrics().rewards == 1)
  end
  if frame == 80 then
    press(context, "2")
    press(context, "r")
    press(context, "l")
    love.window.setMode(1280, 720, { vsync = 0 })
    assert(host.enqueue("radio"))
  end
  if frame == 100 then press(context, "p") end
  if frame == 110 then press(context, "p") end
  if frame == 120 then
    press(context, "3")
    press(context, "m")
    press(context, "r")
    press(context, "v")
    press(context, "f")
  end
  if frame == 135 then press(context, "s") end
  if frame == 140 then press(context, "s") end
  if frame == 145 then
    host.settings.reducedMotion = true
    host.reset()
    host.advance()
    host.advance()
    assert(host.player():snapshot().state == "awaiting_choice")
    press(context, "down")
  end
  if frame == 146 then press(context, "pagedown") end
  if frame == 147 then
    assert(host.settings.scroll > 0, "radio choices expose keyboard scrolling")
  end
  if frame >= 150 and frame < 170 then host.reset() end
  if frame == 175 then
    press(context, "1")
    host.reset()
  end
end
local function verify(context)
  assert(host.metrics().rewards == 1)
  assert(host.metrics().messages == 0)
  assert(host.metrics().conversations == 0)
  assert(host.player():history().capacity == 8)
  print("SHOWCASE_NATIVE_OK rewards=1 queues=bounded restore=quiet")
end
return { setup = setup, beforeFrame = beforeFrame, verify = verify }

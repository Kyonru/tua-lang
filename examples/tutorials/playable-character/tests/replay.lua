local host
local frozenX
local frozenY
local hudDraws = 0
local function setup(context)
  host = require("__tua_replay_game")
  local originalPrint = love.graphics.print
  love.graphics.print = function(text, x, y, ...)
    if text == "A walk around the harbor" then
      local sx, sy = love.graphics.transformPoint(x, y)
      assert(math.abs(sx - 24) < 0.000001 and math.abs(sy - 19) < 0.000001,
        "world transform does not leak into HUD")
      hudDraws = hudDraws + 1
    end
    return originalPrint(text, x, y, ...)
  end
  love.window.setMode(960, 640, { vsync = 0 })
  context.key("d", true)
  host.update(1)
  assert(math.abs(host.player.x - 980) < 0.000001)
  host.reset()
  context.key("s", true)
  host.update(1)
  local dx, dy = host.player.x - 800, host.player.y - 600
  assert(math.abs(math.sqrt(dx * dx + dy * dy) - 180) < 0.000001, "diagonal speed is normalized")
  context.key("d", false)
  context.key("s", false)
  host.reset()
end
local function beforeFrame(context)
  local frame = context.frame
  if frame == 1 then context.key("d", true) end
  if frame == 6 then
    assert(host.hero.animation.position == 2, "repeated play preserves walk progress")
  end
  if frame == 31 then
    assert(host.hero.name == "walk")
    assert(host.camera.x > 800 and host.camera.x < host.player.x, "camera follows with damping")
    context.key("s", true)
  end
  if frame == 61 then
    love.focus(false)
    frozenX, frozenY = host.player.x, host.player.y
  end
  if frame == 76 then
    assert(host.player.x == frozenX and host.player.y == frozenY, "focus loss freezes movement")
    assert(host.hero.name == "idle")
    context.key("d", false)
    context.key("s", false)
    love.focus(true)
  end
  if frame == 91 then
    assert(host.player.x == frozenX and host.player.y == frozenY, "released input stays stopped")
    context.key("a", true)
  end
  if frame == 121 then
    context.key("a", false)
    love.keypressed("r")
    assert(host.player.x == 800 and host.player.y == 600)
    assert(host.hero.name == "idle")
  end
end
local function verify(context)
  assert(host.player.x == 800 and host.player.y == 600)
  assert(hudDraws > 0)
  print("CHARACTER_NATIVE_OK diagonal input, animation, camera, focus, reset, HUD transform")
end
return { setup = setup, beforeFrame = beforeFrame, verify = verify }

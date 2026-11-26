local host
local frozenX
local frozenCamera
local frozenFrame
local hudDraws = 0
local function setup(context)
  host = require("__tua_replay_game")
  local original = love.graphics.print
  love.graphics.print = function(text, x, y, ...)
    if text == "ARENA GAME / CHAPTER 01" then
      local sx, sy = love.graphics.transformPoint(x, y)
      assert(sx == 32 and sy == 28, "HUD must stay in screen space")
      hudDraws = hudDraws + 1
    end
    return original(text, x, y, ...)
  end
end
local function beforeFrame(context)
  local frame = context.frame
  if frame == 10 then
    context.key("return", true)
    context.key("d", true)
  end
  if frame == 11 then
    assert(host.player.x == 800, "start consumes its frame")
    context.key("return", false)
  end
  if frame == 40 then
    context.key("escape", true)
  end
  if frame == 41 then
    context.key("escape", false)
    assert(host.session.flow:is("paused"))
    frozenX = host.player.x
    frozenCamera = host.camera.x
    frozenFrame = host.hero.animation.position
  end
  if frame == 60 then
    assert(host.player.x == frozenX and host.camera.x == frozenCamera)
    assert(host.hero.animation.position == frozenFrame)
    context.key("return", true)
  end
  if frame == 61 then
    assert(host.player.x == frozenX, "resume consumes its frame")
    context.key("return", false)
    context.key("s", true)
  end
  if frame == 80 then
    love.focus(false)
    frozenX = host.player.x
  end
  if frame == 90 then
    love.focus(true)
  end
  if frame == 95 then
    assert(host.session.flow:is("paused") and host.player.x == frozenX)
    context.key("d", false)
    context.key("s", false)
    context.key("r", true)
  end
  if frame == 96 then
    context.key("r", false)
    assert(host.player.x == 800 and host.player.y == 600)
    assert(host.hero.name == "idle" and host.camera.x == 800)
  end
  if frame == 100 then
    love.window.setMode(640, 480)
  end
  if frame == 110 then
    context.key("backspace", true)
  end
  if frame == 111 then
    context.key("backspace", false)
  end
end
local function verify(context)
  assert(host.session.flow:is("title") and hudDraws > 0)
  print(
    "ARENA_NATIVE_OK transition input, movement, animation/camera pause, focus, reset, title, HUD"
  )
end
return { setup = setup, beforeFrame = beforeFrame, verify = verify }

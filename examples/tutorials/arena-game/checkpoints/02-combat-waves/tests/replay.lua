local host
local function tap(context, key)
  context.key(key, true)
  host.update(0)
  context.key(key, false)
  host.update(0)
end
local function aimAtFirst(context)
  local selected
  host.combat.enemies:each(function(id)
    if not selected or id < selected then selected = id end
  end)
  if selected then
    local position = host.combat.readPosition(selected)
    local x, y = host.camera:toScreen(position.x, position.y)
    context.pointer(x, y, false)
  end
end
local function burst(context, mode)
  tap(context, "r")
  host.feedback.options.enabled = mode ~= "off"
  host.feedback.options.reducedMotion = mode == "reduced"
  context.key("space", true)
  for _ = 1, 90 do
    aimAtFirst(context)
    host.update(1 / 30)
  end
  context.key("space", false)
  local result = { host.combat.state.shots, host.combat.state.hits, host.combat.state.defeated }
  assert(result[3] > 0)
  return result
end
local function setup(context)
  host = require("__tua_replay_game")
  tap(context, "return")
  local baseline = burst(context, "on")
  for _, mode in ipairs({ "off", "reduced" }) do
    local result = burst(context, mode)
    for i = 1, 3 do assert(result[i] == baseline[i], "feedback must not affect damage") end
  end
  tap(context, "r")
  host.feedback.options.enabled = true
  host.feedback.options.reducedMotion = false
end
local shots
local bulletX
local framePosition
local function beforeFrame(context)
  local frame = context.frame
  aimAtFirst(context)
  if frame == 1 then context.key("space", true) end
  if frame == 25 then
    tap(context, "escape")
    shots = host.combat.state.shots
    bulletX = host.combat.bullets[1] and host.combat.bullets[1].x
    framePosition = host.hero.animation.position
  end
  if frame == 40 then
    assert(host.combat.state.shots == shots and host.hero.animation.position == framePosition)
    if bulletX then assert(host.combat.bullets[1].x == bulletX) end
    tap(context, "return")
  end
  if frame == 70 then
    love.focus(false)
    shots = host.combat.state.shots
  end
  if frame == 75 then love.focus(true) end
  if frame == 80 then
    assert(host.combat.state.shots == shots and host.session.flow:is("paused"))
    context.key("space", false)
    tap(context, "r")
    assert(#host.combat.bullets == 0 and host.combat.enemies:count() == 8)
    assert(host.feedback.values.impact == 0)
    context.key("space", true)
  end
  if frame == 100 then
    context.key("space", false)
    love.window.setMode(640, 480)
  end
end
local function verify(context)
  assert(host.combat.state.hits > 0)
  context.key("space", true)
  for _ = 1, 240 do
    aimAtFirst(context)
    host.update(1 / 30)
  end
  context.key("space", false)
  for _ = 1, 45 do host.update(1 / 30) end
  assert(host.combat.enemies:count() == 0 and #host.combat.bullets == 0)
  tap(context, "n")
  assert(host.combat.state.wave == 2 and host.combat.enemies:count() == 8)
  tap(context, "backspace")
  local ticks = host.combat.state.ticks
  host.update(1)
  assert(host.combat.state.ticks == ticks)
  print("ARENA_COMBAT_NATIVE_OK feedback parity, aimed hits, pause/focus, reset, clear, next wave, title")
end
return { setup = setup, beforeFrame = beforeFrame, verify = verify }

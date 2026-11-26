local host
local function tap(context, key)
  context.key(key, true)
  host.update(0)
  context.key(key, false)
  host.update(0)
end
local function clearWave()
  local ids = {}
  host.combat.enemies:each(function(id)
    ids[#ids + 1] = id
  end)
  for _, id in ipairs(ids) do
    host.combat.destroyEnemy(id)
  end
end
local function setup(context)
  host = require("__tua_replay_game")
  tap(context, "return")
  clearWave()
  local ticks = host.combat.state.ticks
  tap(context, "n")
  assert(host.conversation.state.active)
  context.key("space", true)
  context.key("d", true)
  host.update(0.25)
  assert(host.combat.state.ticks == ticks and host.player.x == 800 and host.combat.state.shots == 0)
  context.key("space", false)
  context.key("d", false)
  local before = host.conversation.player:snapshot()
  tap(context, "tab")
  local after = host.conversation.player:snapshot()
  assert(after.visit == before.visit and after.generation == before.generation)
  assert(after.node.speaker == "Gardienne du port")
  love.focus(false)
  tap(context, "return")
  assert(host.conversation.player:snapshot().visit == after.visit)
  love.focus(true)
  tap(context, "return")
  assert(
    host.conversation.player:snapshot().visit == after.visit,
    "focus resume consumes its input"
  )
  tap(context, "return")
  assert(host.conversation.player:snapshot().state == "awaiting_choice")
  tap(context, "down")
  assert(host.conversation.state.selected == 2)
  tap(context, "tab")
  assert(host.conversation.state.selected == 2)
  tap(context, "tab")
  love.window.setMode(640, 480)
end
local function beforeFrame(context)
  if context.frame == 25 then
    tap(context, "return")
    tap(context, "return")
    assert(not host.conversation.state.active and host.combat.state.wave == 1)
  end
  if context.frame == 30 then
    tap(context, "n")
  end
  if context.frame == 35 then
    tap(context, "return")
  end
  if context.frame == 40 then
    tap(context, "return")
  end
  if context.frame == 45 then
    local ticks = host.combat.state.ticks
    tap(context, "tab")
    tap(context, "return")
    assert(host.combat.state.wave == 2 and not host.conversation.state.active)
    assert(host.combat.state.ticks == ticks)
  end
  if context.frame == 50 then
    tap(context, "r")
    clearWave()
    tap(context, "n")
    tap(context, "return")
    tap(context, "return")
    tap(context, "escape")
    assert(not host.conversation.state.active and host.combat.state.wave == 1)
  end
end
local function verify(context)
  assert(host.conversation.state.wavesStarted == 1 and host.combat.state.wave == 1)
  print(
    "ARENA_DIALOGUE_NATIVE_OK modal input, focus resume, French wrapping, locale identity, rest/start/cancel"
  )
end
return { setup = setup, beforeFrame = beforeFrame, verify = verify }

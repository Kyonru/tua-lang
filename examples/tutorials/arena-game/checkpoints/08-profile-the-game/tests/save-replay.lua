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
  assert(love.filesystem.getIdentity():match("^tua%-replay%-"))
  host = require("__tua_replay_game")
  tap(context, "tab")
  tap(context, "m")
  tap(context, "0")
  tap(context, "g")
  tap(context, "return")
  host.combat.state.health = 7
  host.session.trigger.held = true
  host.combat.update(1 / 120, 0, 0, true, 1030, 572)
  clearWave()
  host.update(0)
  assert(#host.combat.bullets > 0 and host.progress.snapshot().clearedWave == 0)
  host.session.trigger.held = false
  for _ = 1, 7 do
    host.update(0.25)
  end
  assert(host.progress.snapshot().clearedWave == 1)
  assert(host.progress.snapshot().health == 7 and host.progress.snapshot().locale == "fr")
  tap(context, "n")
  assert(host.conversation.state.active)
  tap(context, "return")
  tap(context, "return")
  tap(context, "return")
  assert(host.combat.state.wave == 2 and host.progress.snapshot().clearedWave == 1)
  tap(context, "backspace")
  tap(context, "f9")
  assert(host.combat.state.wave == 1 and host.combat.state.health == 7)
  assert(host.combat.enemies:count() == 0 and #host.combat.bullets == 0)
  assert(host.session.state.time == 0 and host.combat.state.shots == 0)
  assert(host.soundscape.options.muted and host.feedback.options.reducedMotion)
  assert(not host.hitShader.state.enabled and not host.conversation.state.active)
  tap(context, "backspace")
  tap(context, "delete")
  assert(host.progress.ui.confirmReset)
  tap(context, "escape")
  assert(not host.progress.ui.confirmReset and host.progress.snapshot().clearedWave == 1)
  love.window.setMode(640, 480)
end
local function beforeFrame(context)
  if context.frame == 20 then
    tap(context, "delete")
  end
  if context.frame == 30 then
    tap(context, "return")
    assert(host.progress.snapshot().clearedWave == 0 and not host.progress.ui.confirmReset)
    assert(host.session.flow:is("title"), "confirmation must not start gameplay")
    assert(not host.soundscape.options.muted and not host.feedback.options.reducedMotion)
    tap(context, "f9")
    assert(host.session.flow:is("title"), "no checkpoint after reset")
  end
end
local function verify()
  print("ARENA_SAVE_NATIVE_OK boundary, bullets, preferences, restore, cancel and confirmed reset")
end
return { setup = setup, beforeFrame = beforeFrame, verify = verify }

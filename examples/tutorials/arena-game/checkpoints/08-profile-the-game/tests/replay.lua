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
  host.update(0.25)
  local snapshot = host.audioBackend.snapshot()
  assert(snapshot.music.state == "combat")
  host.audioBackend.stopEffects()
  assert(love.audio.getActiveSourceCount() == 1, "one native music Source must play")
  host.audioBackend.pauseMusic(true)
  host.audioBackend.update(0)
  local pausedPosition = host.audioBackend.snapshot().music.position
  love.timer.sleep(0.08)
  host.audioBackend.update(0.08)
  assert(
    host.audioBackend.snapshot().music.position == pausedPosition,
    "paused Source clock must stop"
  )
  host.audioBackend.pauseMusic(false)
  love.timer.sleep(0.08)
  host.audioBackend.update(0.08)
  assert(
    host.audioBackend.snapshot().music.position > pausedPosition,
    "resumed Source clock must advance"
  )
end
local shots
local bulletX
local framePosition
local function beforeFrame(context)
  local frame = context.frame
  if frame == 10 then
    tap(context, "1")
    tap(context, "4")
    local snapshot = host.audioBackend.snapshot()
    assert(math.abs(snapshot.buses.music.volume - 0.25) < 0.00001)
    assert(math.abs(snapshot.buses.sfx.volume - 0.7) < 0.00001)
    tap(context, "0")
    assert(host.audioBackend.snapshot().buses.master.muted)
  end
  if frame == 15 then tap(context, "0") end
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
  -- Let the live wave approach and attack without firing.
  tap(context, "r")
  for _ = 1, 360 do host.update(1 / 30) end
  assert(host.session.flow:is("gameover") and host.combat.state.health == 0)
  local ticks = host.combat.state.ticks
  host.update(1)
  assert(host.combat.state.ticks == ticks)
  tap(context, "return")
  assert(host.session.flow:is("playing") and host.combat.state.health == 10)
  assert(host.combat.enemies:count() == 8 and #host.combat.bullets == 0)
  for _ = 1, 30 do
    tap(context, "r")
    assert(host.audioBackend.snapshot().buses.music.activeVoices == 1)
  end
  host.audioBackend.shot()
  host.audioBackend.hit()
  host.update(0.1)
  assert(love.audio.getActiveSourceCount() > 0)
  host.soundscape.dispose()
  host.soundscape.dispose()
  assert(love.audio.getActiveSourceCount() == 0, "shutdown must stop all owned playback")
  print("ARENA_AUDIO_NATIVE_OK real Sources, pause/resume, gains/mute, 30 restarts, shutdown")
  print("ARENA_DECISIONS_NATIVE_OK feedback parity, combat pause/focus, natural death, restart")
end
return { setup = setup, beforeFrame = beforeFrame, verify = verify }

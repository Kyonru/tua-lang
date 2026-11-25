local Save = require("tuasave")
local Profile = require("profile")
local Progress = require("progress")
local Session = require("session")
local paths = { "__tua_saves/arena.a", "__tua_saves/arena.b" }
local function bytes()
  return love.filesystem.read(paths[1]), love.filesystem.read(paths[2])
end
local function unchanged(a, b)
  local currentA, currentB = bytes()
  assert(a == currentA and b == currentB, "save files changed unexpectedly")
end
local function clear()
  assert(love.filesystem.getIdentity():match("^tua%-arena%-save%-test%-"))
  for _, path in ipairs(paths) do
    if love.filesystem.getInfo(path) then assert(love.filesystem.remove(path)) end
  end
end
local options = {
  locale = "fr",
  music = 0.2,
  effects = 0.4,
  muted = true,
  feedback = false,
  reducedMotion = true,
  shader = false,
}
local function blocked(progress, code)
  assert(progress.blocked())
  if code then assert(progress.issue().code == code) end
  local a, b = bytes()
  progress.preferences(options)
  assert(not progress.checkpoint(3, 7) and not progress.save())
  progress.update(20)
  progress.requestReset()
  progress.cancelReset()
  unchanged(a, b)
  assert(not progress.dispose())
  unchanged(a, b)
end
local function run(phase)
  assert(love.filesystem.getIdentity():match("^tua%-arena%-save%-test%-"))
  if phase == "1" then
    clear()
    local progress = Progress.new()
    progress.update(20)
    unchanged(nil, nil)
    progress.preferences(options)
    assert(progress.checkpoint(4, 7))
    assert(progress.dispose())
    print("SAVE_PHASE_1_OK saved cleared wave and preferences")
    return
  end
  local a, b = bytes()
  local progress = Progress.new()
  local saved = progress.snapshot()
  assert(saved.clearedWave == 4 and saved.health == 7 and saved.locale == "fr")
  assert(saved.music == 0.2 and saved.effects == 0.4 and saved.muted)
  assert(not saved.feedback and saved.reducedMotion and not saved.shader)
  unchanged(a, b)
  local session = Session.new()
  assert(session.resumeCheckpoint(saved.clearedWave, saved.health))
  assert(session.flow:is("playing") and session.combat.state.wave == 4)
  assert(session.combat.state.health == 7 and session.combat.enemies:count() == 0)
  assert(#session.combat.bullets == 0 and session.combat.state.ticks == 0)
  assert(session.player.x == 800 and not session.trigger.held)
  assert(not session.resumeCheckpoint(4, 7), "restore requires title")
  assert(session.combat.nextWave() and session.combat.state.wave == 5)
  assert(session.combat.enemies:count() == 8)
  session.combat.dispose()
  assert(progress.dispose())
  unchanged(a, b)

  clear()
  progress = Progress.new()
  assert(not progress.checkpoint(1.5, 5) and not progress.checkpoint(1, 0))
  progress.preferences(options)
  progress.update(0.5)
  unchanged(nil, nil)
  progress.update(0.6)
  assert(Profile.new():load().locale == "fr")
  a, b = bytes()
  progress.ui.status = "UI only"
  progress.preferences(options)
  progress.update(20)
  unchanged(a, b)
  options.music = 0.7
  progress.preferences(options)
  assert(progress.dispose(), "quit flushes pending preferences")
  assert(Profile.new():load().music == 0.7)

  clear()
  local old = Save.contract({
    slot = "arena",
    version = 1,
    schema = Save.record({ clearedWave = Save.number(0), health = Save.number(10) }),
  })
  assert(old:save({ clearedWave = 3, health = 6 }))
  a, b = bytes()
  progress = Progress.new()
  assert(not progress.blocked() and progress.snapshot().clearedWave == 3)
  assert(progress.snapshot().locale == "en" and progress.snapshot().music == 0.35)
  unchanged(a, b)
  assert(progress.dispose(), "quit commits the validated migration")
  assert(Profile.new():load().clearedWave == 3)

  clear()
  old = Save.contract({
    slot = "arena",
    version = 1,
    schema = Save.record({ clearedWave = Save.string(""), health = Save.number(10) }),
  })
  assert(old:save({ clearedWave = "wrong", health = 7 }))
  blocked(Progress.new(), "invalid_migration_input")

  clear()
  old = Save.contract({
    slot = "arena",
    version = 3,
    schema = Save.record({ future = Save.boolean(true) }),
  })
  assert(old:save({ future = true }))
  blocked(Progress.new(), "future_version")

  clear()
  local writer = Profile.new()
  local invalid = writer:defaults()
  invalid.clearedWave = -1
  assert(writer:save(invalid))
  blocked(Progress.new())

  clear()
  assert(love.filesystem.write(paths[1], "corrupt envelope"))
  progress = Progress.new()
  assert(progress.blocked() and not progress.recover())
  assert(not progress.confirmReset(), "reset needs confirmation")
  progress.requestReset()
  local remove = love.filesystem.remove
  love.filesystem.remove = function() return nil, "injected reset failure" end
  assert(not progress.confirmReset() and progress.ui.confirmReset)
  love.filesystem.remove = remove
  assert(progress.confirmReset() and not progress.blocked())
  unchanged(nil, nil)
  assert(progress.snapshot().clearedWave == 0)
  progress.dispose()

  clear()
  writer = Profile.new()
  local first = writer:defaults()
  first.clearedWave = 2
  assert(writer:save(first))
  first.clearedWave = 3
  assert(writer:save(first))
  assert(love.filesystem.write(paths[2], "damaged newer copy"))
  progress = Progress.new()
  assert(progress.issue().code == "damaged_slot_unverified")
  assert(progress.snapshot().clearedWave == 2)
  a, b = bytes()
  progress.update(20)
  assert(not progress.save())
  unchanged(a, b)
  assert(progress.recover() and progress.save())
  progress.dispose()
  local recovered, issue = Profile.new():load()
  assert(recovered.clearedWave == 2 and issue == nil)

  -- Partial deletion remains blocked even after dismissing confirmation.
  progress = Progress.new()
  progress.requestReset()
  love.filesystem.remove = function(path)
    if path == paths[2] then return nil, "second deletion failed" end
    return remove(path)
  end
  assert(not progress.confirmReset())
  love.filesystem.remove = remove
  progress.cancelReset()
  assert(progress.blocked() and not progress.save())
  progress.requestReset()
  assert(progress.confirmReset())
  progress.dispose()

  clear()
  progress = Progress.new()
  local write = love.filesystem.write
  love.filesystem.write = function() return nil, "injected write failure" end
  assert(not progress.checkpoint(5, 8))
  assert(progress.ui.status == "failed" and progress.snapshot().clearedWave == 5)
  love.filesystem.write = write
  assert(progress.save())
  assert(Profile.new():load().clearedWave == 5)
  progress.dispose()
  clear()
  love.filesystem.remove("__tua_saves")
  print("SAVE_PHASE_2_OK relaunch, restore, debounce, quit, migration, corruption, recovery, reset, retry")
end
return { run = run }

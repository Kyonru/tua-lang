local Save = require("tuasave")
local Profile = require("profile")
local Session = require("session")
local paths = { "__tua_saves/profile.a", "__tua_saves/profile.b" }
local function bytes()
  return love.filesystem.read(paths[1]), love.filesystem.read(paths[2])
end
local function unchanged(a, b)
  local currentA, currentB = bytes()
  assert(currentA == a and currentB == b, "unexpected write or deletion")
end
local function clear()
  assert(
    love.filesystem.getIdentity():match("^tua%-save%-tutorial%-"),
    "fixtures require isolated identity"
  )
  for _, path in ipairs(paths) do
    if love.filesystem.getInfo(path) then assert(love.filesystem.remove(path)) end
  end
end
local function oldContract(version, schema)
  return Save.contract({ slot = "profile", version = version, schema = schema })
end
local function blocked(session, code)
  assert(session.issue() and session.issue().code == code)
  local a, b = bytes()
  assert(not session.score())
  assert(not session.save())
  session.update(20)
  session.requestReset()
  session.cancelReset()
  assert(not session.acceptRecovery() or code == "damaged_slot_unverified")
  unchanged(a, b)
end
local function run(phase)
  assert(love.filesystem.getIdentity():match("^tua%-save%-tutorial%-"))
  if phase == "1" then
    clear()
    local session = Session.new()
    assert(session.issue() == nil and session.snapshot().score == 0)
    for index = 1, 8 do assert(session.score()) end
    assert(session.save())
    assert(session.dispose(false))
    print("SAVE_PHASE_1_OK persisted score=8")
    return
  end
  -- This is a second OS process with the same isolated identity.
  local a, b = bytes()
  local session = Session.new()
  assert(session.issue() == nil and session.snapshot().score == 8)
  unchanged(a, b)
  session.dispose(false)

  clear()
  session = Session.new()
  session.update(20)
  unchanged(nil, nil)
  assert(session.score())
  session.update(0.5)
  unchanged(nil, nil)
  session.update(0.6)
  assert(Profile.new():load().score == 1)
  a, b = bytes()
  session.ui.notice = "UI-only change"
  session.update(20)
  unchanged(a, b)
  assert(session.score())
  assert(session.dispose(), "quit flushes pending progress")
  assert(Profile.new():load().score == 2)

  clear()
  local old = oldContract(1, Save.record({ points = Save.number(0) }))
  assert(old:save({ points = 27 }))
  a, b = bytes()
  session = Session.new()
  assert(session.issue() == nil and session.snapshot().score == 27 and session.snapshot().musicOn)
  unchanged(a, b)
  assert(session.save())
  session.dispose(false)
  assert(Profile.new():load().score == 27)

  clear()
  old = oldContract(1, Save.record({ points = Save.string("") }))
  assert(old:save({ points = "invalid" }))
  session = Session.new()
  blocked(session, "invalid_migration_input")
  a, b = bytes()
  assert(not session.dispose())
  unchanged(a, b)

  clear()
  local future = oldContract(
    3,
    Save.record({ score = Save.number(0), musicOn = Save.boolean(true) })
  )
  assert(future:save({ score = 99, musicOn = true }))
  session = Session.new()
  blocked(session, "future_version")
  a, b = bytes()
  assert(not session.dispose())
  unchanged(a, b)

  clear()
  assert(love.filesystem.write(paths[1], "damaged envelope"))
  session = Session.new()
  blocked(session, "corrupt_envelope")
  assert(not session.confirmReset(), "reset requires confirmation")
  a, b = bytes()
  session.requestReset()
  session.update(20)
  unchanged(a, b)
  local remove = love.filesystem.remove
  love.filesystem.remove = function() return nil, "injected reset failure" end
  assert(not session.confirmReset())
  assert(session.ui.confirmReset and session.ui.notice:find("injected reset failure", 1, true))
  love.filesystem.remove = remove
  unchanged(a, b)
  assert(session.confirmReset())
  unchanged(nil, nil)
  assert(session.snapshot().score == 0 and session.issue() == nil)
  assert(session.save())
  session.dispose(false)

  clear()
  local writer = Profile.new()
  assert(writer:save({ score = 10, musicOn = true }))
  assert(writer:save({ score = 20, musicOn = true }))
  assert(love.filesystem.write(paths[2], "damaged newer generation"))
  session = Session.new()
  assert(session.snapshot().score == 10)
  assert(session.issue().code == "damaged_slot_unverified")
  a, b = bytes()
  assert(not session.save() and not session.score())
  session.update(20)
  unchanged(a, b)
  assert(session.acceptRecovery())
  assert(session.score() and session.save())
  session.dispose(false)
  local recovered, issue = Profile.new():load()
  assert(recovered.score == 11 and issue == nil)

  clear()
  writer = Profile.new()
  assert(writer:save({ score = 3, musicOn = true }))
  assert(writer:save({ score = 4, musicOn = true }))
  session = Session.new()
  session.requestReset()
  remove = love.filesystem.remove
  love.filesystem.remove = function(path)
    if path == paths[2] then return nil, "second deletion failed" end
    return remove(path)
  end
  assert(not session.confirmReset())
  love.filesystem.remove = remove
  assert(session.issue().code == "reset_failed")
  assert(not love.filesystem.getInfo(paths[1]) and love.filesystem.getInfo(paths[2]))
  session.cancelReset()
  a, b = bytes()
  assert(not session.score() and not session.save())
  session.update(20)
  unchanged(a, b)
  session.requestReset()
  assert(session.confirmReset())
  session.dispose(false)

  clear()
  session = Session.new()
  assert(session.score())
  local write = love.filesystem.write
  love.filesystem.write = function() return nil, "injected write failure" end
  assert(not session.save())
  assert(session.snapshot().score == 1 and session.ui.notice:find("injected", 1, true))
  love.filesystem.write = write
  assert(session.save(), "explicit retry succeeds without losing in-memory progress")
  session.dispose(false)
  assert(Profile.new():load().score == 1)
  clear()
  love.filesystem.remove("__tua_saves")
  print("SAVE_PHASE_2_OK restart, autosave, UI isolation, quit, migration, future, corruption, recovery, reset, retry")
end
return { run = run }

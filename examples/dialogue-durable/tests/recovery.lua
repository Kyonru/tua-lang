package.path = "../tests/?.lua;" .. package.path
local Memory = require("memory")
local Journals = require("journal")
local Sessions = require("session")
local Profile = require("profile")
local function fails(fn, fragment)
  local ok, message = pcall(fn)
  assert(not ok, "expected rejection")
  if fragment then assert(tostring(message):find(fragment, 1, true), tostring(message)) end
end
local function setup()
  local storage = Memory.new()
  local journal = Journals.new(storage.backend)
  fails(journal.load, "initialize")
  fails(function() Sessions.new(storage.backend).recover() end, "initialize")
  journal.initialize()
  return storage, journal, Sessions.new(storage.backend)
end
local function counters(journal, coins, shells)
  local value = journal.load()
  assert(value.coins == coins and value.shells == shells,
    "persisted counters must change exactly once per successful operation")
end
local function finish(storage, journal)
  local recovered = assert(Sessions.new(storage.backend).recover())
  local before = storage.writes()
  local committed = journal.receipt(recovered.key).committed
  assert(recovered.player:snapshot().state == "awaiting_action")
  counters(journal, committed and 5 or 10, committed and 1 or 0)
  assert(recovered.player:runAction())
  counters(journal, 5, 1)
  if committed then assert(storage.writes() == before, "receipt replay never writes a reward") end
  assert(recovered.player:snapshot().nodeId == "done")
  recovered.acknowledge()
  assert(Sessions.new(storage.backend).recover() == nil)
  return recovered.key
end

-- Restart after durable intent but before dispatch; recovery itself has no effect.
do
  local storage, journal, sessions = setup()
  local active = sessions.begin()
  assert(active.key == "demo:1")
  counters(journal, 10, 0)
  fails(sessions.begin, "outstanding")
  fails(active.acknowledge, "consume")
  fails(function() active.player:checkpoint() end, "checkpoint")
  active.player:dispose()
  finish(storage, journal)
end

-- Commit succeeded before the handler returned or the player advanced.
do
  local storage, journal, sessions = setup()
  local active = sessions.begin()
  local request = active.player:snapshot().action
  assert(journal.commit(active.key, { item = "shell" }, request))
  assert(active.player:snapshot().state == "awaiting_action")
  counters(journal, 5, 1)
  active.player:dispose()
  finish(storage, journal)
end

-- Failure before a write, after bytes land, or during read-back is not proof of rollback.
for _, mode in ipairs({ "before", "after", "readback" }) do
  local storage, journal, sessions = setup()
  storage.fail(mode)
  fails(sessions.begin)
  local active = Sessions.new(storage.backend).recover()
  if mode == "before" then
    assert(active == nil)
    active = sessions.begin()
  else
    assert(active ~= nil)
  end
  active.player:dispose()
  finish(storage, journal)

  storage, journal, sessions = setup()
  active = sessions.begin()
  storage.fail(mode)
  assert(not active.player:runAction())
  assert(active.player:snapshot().state == "faulted")
  active.player:dispose()
  finish(storage, journal)

  storage, journal, sessions = setup()
  active = sessions.begin()
  assert(active.player:runAction())
  storage.fail(mode)
  fails(active.acknowledge)
  active.player:dispose()
  counters(journal, 5, 1)
  if mode == "before" then
    finish(storage, journal)
  else
    assert(Sessions.new(storage.backend).recover() == nil)
    fails(function() journal.receipt(active.key) end, "stale")
  end
end

-- Damaged newer data is quarantined, never automatically accepted or defaulted.
for _, phase in ipairs({ "prepare", "commit", "acknowledge" }) do
  local storage, journal, sessions = setup()
  local active
  if phase ~= "prepare" then active = sessions.begin() end
  if phase == "acknowledge" then assert(active.player:runAction()) end
  storage.fail("torn")
  if phase == "prepare" then fails(sessions.begin)
  elseif phase == "commit" then assert(not active.player:runAction())
  else fails(active.acknowledge) end
  local writes = storage.writes()
  fails(journal.load, "damaged_slot_unverified")
  fails(journal.initialize, "damaged_slot_unverified")
  fails(function() Sessions.new(storage.backend).recover() end, "damaged_slot_unverified")
  assert(storage.writes() == writes, "recovery issues never trigger automatic writes")
  if active then active.player:dispose() end
end

-- A completed receipt is replayable; a new occurrence gets a new key. False is durable too.
do
  local storage, journal, sessions = setup()
  local oldKey
  for operation = 1, 3 do
    local active = sessions.begin()
    assert(active.key == "demo:" .. operation)
    local request = active.player:snapshot().action
    if oldKey then
      fails(function() journal.commit(oldKey, { item = "shell" }, request) end, "stale")
    end
    fails(function() journal.commit(active.key, { item = "wrong" }, request) end, "payload")
    assert(active.player:runAction())
    fails(function() active.player:checkpoint() end, "registries")
    local expected = operation < 3
    assert(journal.receipt(active.key).success == expected)
    local writes = storage.writes()
    for _ = 1, 3 do
      local replay = assert(Sessions.new(storage.backend).recover())
      assert(replay.player:runAction())
      assert(replay.player:snapshot().nodeId == (expected and "done" or "declined"))
      replay.player:dispose()
    end
    assert(storage.writes() == writes)
    fails(function() journal.abandon(active.key) end, "committed")
    oldKey = active.key
    active.acknowledge()
  end
  counters(journal, 0, 2)
  assert(journal.load().nextOperation == 4)
  journal.initialize()
  counters(journal, 0, 2)
end

-- Cancelling playback does not discard intent. Explicit prepared-only abandonment does.
do
  local storage, journal, sessions = setup()
  local active = sessions.begin()
  assert(active.player:cancel())
  assert(journal.load().intent ~= nil)
  journal.abandon(active.key)
  assert(sessions.recover() == nil)
  local nextSession = sessions.begin()
  assert(nextSession.key == "demo:2")
  fails(function() journal.receipt(active.key) end, "stale")
  journal.abandon(nextSession.key)
  counters(journal, 10, 0)
  active.player:dispose()
  nextSession.player:dispose()
end

-- A shape-valid but incompatible record is still rejected before dispatch.
for _, mutate in ipairs({
  function(value) value.intent.contentVersion = "future" end,
  function(value) value.intent.key = "other:1" end,
  function(value) value.intent.price = 6 end,
  function(value) value.intent.nodeId = "done" end,
  function(value) value.intent.actionId = "other" end,
  function(value) value.intent.operation = 9 end,
  function(value) value.intent.success = true end,
  function(value) value.coins = -1 end,
}) do
  local storage, journal, sessions = setup()
  local active = sessions.begin()
  local raw = Profile.open(storage.backend)
  local value = assert(raw:load())
  mutate(value)
  assert(raw:save(value))
  local writes = storage.writes()
  fails(function() Sessions.new(storage.backend).recover() end)
  assert(storage.writes() == writes)
  active.player:dispose()
end
do
  local storage, journal, sessions = setup()
  local active = sessions.begin()
  local scene = require("scene")
  local target = scene.nodes.buy.success
  scene.nodes.buy.success = "declined"
  local writes = storage.writes()
  fails(function() Sessions.new(storage.backend).recover() end, "authored action changed")
  assert(storage.writes() == writes)
  counters(journal, 10, 0)
  scene.nodes.buy.success = target
  active.player:dispose()
  finish(storage, journal)
end
print("durable dialogue crash-window recovery passed")

local Memory = dofile("../tests/memory.lua")
local Session = require("saved_session")
local Profile = require("persistence")

local memory = Memory.new()
local original = Session.new(10, memory.backend)
assert(not original.load())
assert(original.player:snapshot().state == "idle")
assert(memory.writes() == 0)
original.player:start()
original.player:setLocals({ visits = 3 })
original.save()
original.player:dispose()

local restored = Session.new(20, memory.backend)
assert(restored.load())
assert(restored.player:getLocals().visits == 3)
assert(restored.player:getContext().coins == 20)
assert(restored.player:snapshot().revealComplete)
assert(restored.player:snapshot().restored)
assert(restored.player:snapshot().locale == "en")
restored.player:advance()
restored.save()
local blocked = Session.new(0, memory.backend)
local writes = memory.writes()
assert(not pcall(blocked.load))
assert(blocked.player:snapshot().state == "idle")
assert(memory.writes() == writes)
blocked.player:updateContext({ coins = 5 })
assert(blocked.load())
assert(blocked.player:choose("harbor.context.buy"))

local waiting = Session.new(10, memory.backend)
waiting.player:start("tide")
waiting.player:update(0.5, false)
waiting.player:suspend()
waiting.save()
local resumed = Session.new(10, memory.backend)
assert(resumed.load())
resumed.player:update(10, false)
assert(resumed.player:snapshot().state == "waiting")
resumed.player:resume()
resumed.player:update(1.5, false)
assert(resumed.player:snapshot().nodeId == "done")

-- Schema-valid but graph-incompatible data must still fail transactionally.
for _, field in ipairs({ "version", "contentVersion", "graph", "nodeId", "elapsed" }) do
  waiting.save()
  local contract = Profile.open(memory.backend)
  local saved = assert(contract:load())
  saved.checkpoint[field] = (field == "version" or field == "elapsed") and 999 or "changed"
  assert(contract:save(saved))
  local fresh = Session.new(10, memory.backend)
  local count = memory.writes()
  assert(not pcall(fresh.load))
  assert(fresh.player:snapshot().state == "idle")
  assert(memory.writes() == count)
  fresh.player:dispose()
end

for _, mode in ipairs({ "before", "after", "readback", "torn" }) do
  local store = Memory.new()
  local writer = Session.new(10, store.backend)
  writer.player:start()
  writer.save()
  writer.player:setLocals({ visits = 7 })
  store.fail(mode)
  assert(not pcall(writer.save))
  local fresh = Session.new(20, store.backend)
  local count = store.writes()
  if mode == "torn" then
    assert(not pcall(fresh.load))
    assert(fresh.player:snapshot().state == "idle")
    assert(not pcall(writer.save))
  else
    assert(fresh.load())
    assert(fresh.player:getLocals().visits == (mode == "before" and 0 or 7))
  end
  assert(store.writes() == count)
  writer.player:dispose()
  fresh.player:dispose()
end
-- Locale comes from the checkpoint, not the new player's default.
waiting.save()
local localeContract = Profile.open(memory.backend)
local localized = assert(localeContract:load())
localized.checkpoint.locale = "fr"
assert(localeContract:save(localized))
local french = Session.new(10, memory.backend)
assert(french.load())
assert(french.player:snapshot().locale == "fr")
french.player:dispose()

-- Save validates fields before persistence; schema defaults never repair them.
local invalid = assert(Profile.open(memory.backend):load())
invalid.checkpoint.locals.visits = "wrong"
local count = memory.writes()
assert(not Profile.open(memory.backend):save(invalid))
assert(memory.writes() == count)

-- Neither damaged slot is treated as a new conversation.
for _, slot in ipairs({ "a", "b" }) do
  memory.backend.write("__tua_saves/dialogue_checkpoint_demo." .. slot, "broken")
end
local damaged = Session.new(10, memory.backend)
count = memory.writes()
assert(not pcall(damaged.load))
assert(damaged.player:snapshot().state == "idle")
assert(not pcall(waiting.save))
assert(memory.writes() == count)
damaged.player:dispose()

for _, session in ipairs({ restored, blocked, waiting, resumed }) do
  session.player:dispose()
end
print("Save-backed dialogue checkpoint tests passed")

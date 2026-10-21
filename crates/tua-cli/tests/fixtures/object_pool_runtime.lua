local root = assert(arg[1])
local objectPool = assert(loadfile(root .. "/vendor/runtime/tuaobjectpool.lua"))()

-- Counts, LIFO reuse, ownership checks, and reset behavior share one state map.
do
  local created = 0
  local reset = 0
  local disposed = {}
  local pool = objectPool({
    create = function()
      created = created + 1
      return { id = created, dirty = true }
    end,
    reset = function(object)
      reset = reset + 1
      object.dirty = false
    end,
    dispose = function(object) disposed[object] = true end,
    initial = 3,
  })

  assert(pool:capacity() == 3 and pool:availableCount() == 3 and pool:activeCount() == 0)
  local first = pool:acquire()
  local second = pool:acquire()
  assert(first.id == 3 and second.id == 2, "acquire must retain LIFO reuse")
  assert(pool:owns(first) and pool:activeCount() == 2 and pool:availableCount() == 1)
  assert(pool:release(first) and reset == 1 and not first.dirty)
  assert(not pool:release(first), "double release must fail")
  assert(not pool:release({}), "foreign release must fail")
  assert(pool:activeCount() == 1 and pool:availableCount() == 2)
  assert(pool:acquire() == first, "the most recently released object must be reused")

  local createdByPrewarm = pool:prewarm(4)
  assert(createdByPrewarm == 3 and pool:availableCount() == 4 and pool:capacity() == 6)
  pool:clear()
  local disposedCount = 0
  for _ in pairs(disposed) do disposedCount = disposedCount + 1 end
  assert(disposedCount == 6, "clear must dispose active and available objects")
  assert(pool:capacity() == 0 and pool:availableCount() == 0 and pool:activeCount() == 0)
  assert(not pool:owns(first) and not pool:release(second), "clear must discard ownership")
end

-- A failing reset leaves the object checked out and never exposes a partially
-- reset value through the free stack.
do
  local pool = objectPool({
    create = function() return {} end,
    reset = function() error("expected reset failure") end,
  })
  local object = pool:acquire()
  local ok, message = pcall(function() pool:release(object) end)
  assert(not ok and tostring(message):find("expected reset failure", 1, true))
  assert(pool:owns(object) and pool:activeCount() == 1 and pool:availableCount() == 0)
end

-- Factories cannot return an identity the pool already owns, regardless of
-- whether that identity is active or available.
do
  local shared = {}
  local pool = objectPool({ create = function() return shared end, initial = 1 })
  pool:acquire()
  local ok, message = pcall(function() pool:acquire() end)
  assert(not ok and tostring(message):find("already owned", 1, true))
end

-- A warmed acquire/release loop stays allocation-free.
do
  local pool = objectPool({ create = function() return {} end, initial = 1 })
  local object = pool:acquire()
  assert(pool:release(object))

  collectgarbage("collect")
  collectgarbage("stop")
  local before = collectgarbage("count")
  for _ = 1, 10000 do
    object = pool:acquire()
    pool:release(object)
  end
  local allocatedKb = collectgarbage("count") - before
  collectgarbage("restart")

  assert(allocatedKb < 32, string.format(
    "10,000 object-pool cycles should allocate under 32 KB (got %.1f KB)", allocatedKb))
end

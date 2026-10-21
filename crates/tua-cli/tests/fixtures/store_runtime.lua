local root = assert(arg[1])
local runtime = assert(loadfile(root .. "/vendor/runtime/tuastore.lua"))()

local failures = 0

local function check(condition, message)
  if not condition then
    failures = failures + 1
    io.stderr:write("FAIL: " .. message .. "\n")
  end
end

-- Stores without observers take the allocation-free update path.
do
  local state = {}
  for index = 1, 2000 do
    state["field" .. index] = index
  end
  local store = runtime(state)
  local function increment(current)
    current.field1 = current.field1 + 1
  end

  store:update(increment)
  collectgarbage("collect")
  collectgarbage("stop")
  local before = collectgarbage("count")
  for _ = 1, 200 do
    store:update(increment)
  end
  local allocatedKb = collectgarbage("count") - before
  collectgarbage("restart")
  check(store:get().field1 == 202, "unobserved updates mutate state")
  check(allocatedKb < 32, string.format(
    "200 unobserved updates allocate under 32 KB (got %.1f KB)", allocatedKb))
end

-- With keyed watchers but no whole-store subscriber, only watched values need
-- to be retained. Unwatched changes and unchanged watched values stay silent.
do
  local state = {}
  for index = 1, 2000 do
    state["field" .. index] = index
  end
  local store = runtime(state)
  local changes = {}
  store:watch("field1", function(value, previous)
    changes[#changes + 1] = { value, previous }
  end)

  store:update(function(current) current.field2 = current.field2 + 1 end)
  store:update(function(current) current.field1 = current.field1 end)
  store:update(function(current) current.field1 = current.field1 + 10 end)
  check(#changes == 1, "a watcher runs only when its value changes")
  check(changes[1][1] == 11 and changes[1][2] == 1, "a watcher receives current and previous values")

  local function increment(current)
    current.field1 = current.field1 + 1
  end
  collectgarbage("collect")
  collectgarbage("stop")
  local before = collectgarbage("count")
  for _ = 1, 50 do
    store:update(increment)
  end
  local allocatedKb = collectgarbage("count") - before
  collectgarbage("restart")
  check(allocatedKb < 256, string.format(
    "50 watched updates of a 2,000-field store allocate under 256 KB (got %.1f KB)", allocatedKb))
end

-- Whole-store subscribers retain the documented full previous state and run
-- after every update, including a no-op update.
do
  local store = runtime({ score = 1, paused = false })
  local calls = 0
  local previousScore
  store:subscribe(function(_, previous)
    calls = calls + 1
    previousScore = previous.score
    check(previous.paused == false, "whole-store subscribers receive the complete previous state")
  end)
  store:update(function() end)
  check(calls == 1 and previousScore == 1, "whole-store subscribers run after a no-op update")
end

-- Listener mutations are copy-on-write: the current notification sees its
-- original listener list, while the next notification sees the mutations.
do
  local store = runtime({ value = 0 })
  local calls = {}
  local added = false
  local stopSecond

  store:subscribe(function()
    calls[#calls + 1] = "first"
    stopSecond()
    if not added then
      added = true
      store:subscribe(function() calls[#calls + 1] = "third" end)
    end
  end)
  stopSecond = store:subscribe(function() calls[#calls + 1] = "second" end)

  store:update(function(state) state.value = 1 end)
  check(table.concat(calls, ",") == "first,second", "subscriber mutations do not alter the active notification")
  calls = {}
  store:update(function(state) state.value = 2 end)
  check(table.concat(calls, ",") == "first,third", "subscriber mutations apply to the next notification")
end

-- Keyed watchers have the same copy-on-write behavior.
do
  local store = runtime({ value = 0 })
  local calls = {}
  local added = false
  local stopSecond

  store:watch("value", function()
    calls[#calls + 1] = "first"
    stopSecond()
    if not added then
      added = true
      store:watch("value", function() calls[#calls + 1] = "third" end)
    end
  end)
  stopSecond = store:watch("value", function() calls[#calls + 1] = "second" end)

  store:update(function(state) state.value = 1 end)
  check(table.concat(calls, ",") == "first,second", "watcher mutations do not alter the active notification")
  calls = {}
  store:update(function(state) state.value = 2 end)
  check(table.concat(calls, ",") == "first,third", "watcher mutations apply to the next notification")
end

-- A watcher added by a whole-store subscriber starts with the next
-- notification; the captured watcher-map version stays stable for this one.
do
  local store = runtime({ value = 0 })
  local watcherCalls = 0
  local added = false
  store:subscribe(function()
    if not added then
      added = true
      store:watch("value", function() watcherCalls = watcherCalls + 1 end)
    end
  end)
  store:update(function(state) state.value = 1 end)
  check(watcherCalls == 0, "new watcher keys do not join the active notification")
  store:update(function(state) state.value = 2 end)
  check(watcherCalls == 1, "new watcher keys join the next notification")
end

-- A listener error must not corrupt the immutable subscription version, so
-- later subscription changes and notifications still behave normally.
do
  local store = runtime({ value = 0 })
  local stop = store:subscribe(function() error("expected listener failure") end)
  check(not pcall(function()
    store:update(function(state) state.value = 1 end)
  end), "listener errors propagate")
  stop()
  local calls = 0
  store:subscribe(function() calls = calls + 1 end)
  store:update(function(state) state.value = 2 end)
  check(calls == 1, "listener collections recover after an error")
end

if failures > 0 then
  os.exit(1)
end

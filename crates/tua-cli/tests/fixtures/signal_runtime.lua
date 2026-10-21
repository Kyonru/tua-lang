local root = assert(arg[1])
package.path = root .. "/vendor/runtime/?.lua;" .. package.path

local signal = require("tuasignal")

-- A stable listener set emits without allocating a snapshot each time.
do
  local events = signal()
  local calls = 0
  events:on("hit", function() calls = calls + 1 end)
  local stableList = events._signals.hit
  local temporary = events:on("hit", function() end)
  assert(events._signals.hit == stableList, "subscribing outside emit mutates in place")
  events:off("hit", temporary)
  assert(events._signals.hit == stableList, "unsubscribing outside emit mutates in place")
  events:emit("hit")

  collectgarbage("collect")
  collectgarbage("stop")
  local before = collectgarbage("count")
  for _ = 1, 10000 do events:emit("hit") end
  local allocatedKb = collectgarbage("count") - before
  collectgarbage("restart")

  assert(calls == 10001)
  assert(allocatedKb < 32, string.format(
    "10,000 stable signal emits should allocate under 32 KB (got %.1f KB)", allocatedKb))
end

-- Subscription changes are copy-on-write: an outer emit finishes the version
-- it started with, while a nested emit sees the newly subscribed listener.
do
  local events = signal()
  local calls = {}
  local nested = false
  local added = false

  events:on("hit", function()
    calls[#calls + 1] = nested and "nested:first" or "outer:first"
    if not added then
      added = true
      events:on("hit", function()
        calls[#calls + 1] = nested and "nested:third" or "outer:third"
      end)
      nested = true
      events:emit("hit")
      nested = false
    end
  end)
  events:on("hit", function()
    calls[#calls + 1] = nested and "nested:second" or "outer:second"
  end)

  local outerList = events._signals.hit
  events:emit("hit")
  assert(events._signals.hit ~= outerList, "subscribing during emit copies the active version")
  assert(table.concat(calls, ",") ==
    "outer:first,nested:first,nested:second,nested:third,outer:second")
end

-- Removing during an emit has the same versioned behavior, including once:
-- the current emit keeps its original listeners and nested emits use the new
-- list where removed listeners are already absent.
do
  local events = signal()
  local calls = {}
  local second
  local nested = false

  events:on("hit", function()
    calls[#calls + 1] = nested and "nested:first" or "outer:first"
    if not nested then
      events:off("hit", second)
      nested = true
      events:emit("hit")
      nested = false
    end
  end)
  second = events:on("hit", function() calls[#calls + 1] = "outer:second" end)

  events:emit("hit")
  assert(table.concat(calls, ",") == "outer:first,nested:first,outer:second")

  local onceCalls = 0
  events:once("once", function()
    onceCalls = onceCalls + 1
    events:emit("once")
  end)
  events:emit("once")
  assert(onceCalls == 1, "once must be absent from a nested emit")
end

-- A throwing handler leaves a marked immutable version, so a later mutation
-- detaches safely without wrapping errors or changing their propagation.
do
  local events = signal()
  local bad = events:on("hit", function() error("expected signal failure") end)
  local ok, message = pcall(function() events:emit("hit") end)
  assert(not ok and tostring(message):find("expected signal failure", 1, true))
  events:off("hit", bad)
  local calls = 0
  events:on("hit", function() calls = calls + 1 end)
  events:emit("hit")
  assert(calls == 1, "subscription mutation after a handler error must recover")
end

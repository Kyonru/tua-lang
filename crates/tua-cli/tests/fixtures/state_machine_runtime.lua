local root = assert(arg[1])
local stateMachine = assert(loadfile(root .. "/vendor/runtime/tuastatemachine.lua"))()

-- Synchronous transitions preserve callback order and payload forwarding.
do
  local calls = {}
  local machine = stateMachine.create({
    initial = "closed",
    events = { { name = "open", from = "closed", to = "open" } },
    callbacks = {
      onbeforeopen = function(_, event, from, to, payload)
        calls[#calls + 1] = table.concat({ "before", event, from, to, payload }, ":")
      end,
      onleaveclosed = function() calls[#calls + 1] = "leave" end,
      onenteropen = function() calls[#calls + 1] = "enter" end,
      onafteropen = function() calls[#calls + 1] = "after" end,
      onstatechange = function() calls[#calls + 1] = "change" end,
    },
  })

  assert(machine:open("actor"))
  assert(machine.current == "open" and machine.asyncState == machine.NONE)
  assert(table.concat(calls, ",") ==
    "before:open:closed:open:actor,leave,enter,after,change")
end

-- Invalid events return before constructing their callback-parameter table.
-- A warmed loop therefore remains effectively allocation-free.
do
  local machine = stateMachine.create({
    initial = "idle",
    events = { { name = "stop", from = "running", to = "idle" } },
  })
  assert(not machine:stop("ignored"))

  collectgarbage("collect")
  collectgarbage("stop")
  local before = collectgarbage("count")
  for _ = 1, 10000 do assert(not machine:stop("ignored")) end
  local allocatedKb = collectgarbage("count") - before
  collectgarbage("restart")

  assert(allocatedKb < 32, string.format(
    "10,000 rejected transitions should allocate under 32 KB (got %.1f KB)", allocatedKb))
end

-- Returning false from a before or leave callback still cancels cleanly.
do
  local before = stateMachine.create({
    initial = "idle",
    events = { { name = "start", from = "idle", to = "running" } },
    callbacks = { onbeforestart = function() return false end },
  })
  assert(not before:start() and before.current == "idle")
  assert(before.currentTransitioningEvent == nil and before.asyncState == before.NONE)

  local leave = stateMachine.create({
    initial = "idle",
    events = { { name = "start", from = "idle", to = "running" } },
    callbacks = { onleaveidle = function() return false end },
  })
  assert(not leave:start() and leave.current == "idle")
  assert(leave.currentTransitioningEvent == nil and leave.asyncState == leave.NONE)
end

-- Deferred leave and enter phases retain the original payload when resumed.
do
  local calls = {}
  local leavePending = true
  local enterPending = true
  local machine = stateMachine.create({
    initial = "idle",
    events = { { name = "start", from = "idle", to = "running" } },
    callbacks = {
      onleaveidle = function(self, _, _, _, payload)
        calls[#calls + 1] = "leave:" .. payload
        if leavePending then leavePending = false; return self.ASYNC end
      end,
      onenterrunning = function(self, _, _, _, payload)
        calls[#calls + 1] = "enter:" .. payload
        if enterPending then enterPending = false; return self.ASYNC end
      end,
      onafterstart = function(_, _, _, _, payload)
        calls[#calls + 1] = "after:" .. payload
      end,
    },
  })

  assert(machine:start("payload"))
  assert(machine.current == "idle" and machine.asyncState == "startWaitingOnLeave")
  assert(machine:transition("start"))
  assert(machine.current == "running" and machine.asyncState == "startWaitingOnEnter")
  assert(machine:transition("start"))
  assert(machine.asyncState == machine.NONE and machine.currentTransitioningEvent == nil)
  assert(table.concat(calls, ",") ==
    "leave:payload,enter:payload,after:payload")
end

-- Cancelling a deferred transition leaves the source state selected and allows
-- the event to start again with a new payload.
do
  local payloads = {}
  local pending = true
  local machine = stateMachine.create({
    initial = "idle",
    events = { { name = "start", from = "idle", to = "running" } },
    callbacks = {
      onleaveidle = function(self, _, _, _, payload)
        payloads[#payloads + 1] = payload
        if pending then pending = false; return self.ASYNC end
      end,
    },
  })

  assert(machine:start("first"))
  machine:cancelTransition("start")
  assert(machine.current == "idle" and machine.currentTransitioningEvent == nil)
  assert(machine:start("second") and machine.current == "running")
  assert(table.concat(payloads, ",") == "first,second")
end

-- Successful transitions without extra arguments call handlers directly, so a
-- warmed loop of valid transitions is effectively allocation-free too.
do
  local entered = 0
  local machine = stateMachine.create({
    initial = "playing",
    events = {
      { name = "pause", from = "playing", to = "paused" },
      { name = "resume", from = "paused", to = "playing" },
    },
    callbacks = { onenterpaused = function() entered = entered + 1 end },
  })
  assert(machine:pause() and machine:resume())

  collectgarbage("collect")
  collectgarbage("stop")
  local before = collectgarbage("count")
  for _ = 1, 10000 do
    machine:pause()
    machine:resume()
  end
  local allocatedKb = collectgarbage("count") - before
  collectgarbage("restart")

  assert(entered == 10001 and machine.current == "playing")
  assert(allocatedKb < 32, string.format(
    "10,000 valid transition pairs should allocate under 32 KB (got %.1f KB)", allocatedKb))
end

-- Extra arguments are forwarded by count, so nil holes and trailing values
-- survive both synchronous calls and deferred resumption.
do
  local seen = {}
  local pending = true
  local machine = stateMachine.create({
    initial = "idle",
    events = { { name = "start", from = "idle", to = "running" } },
    callbacks = {
      onleaveidle = function(self, _, _, _, first, second, third)
        seen[#seen + 1] = tostring(first) .. ":" .. tostring(second) .. ":" .. tostring(third)
        if pending then pending = false; return self.ASYNC end
      end,
      onenterrunning = function(_, _, _, _, ...)
        seen[#seen + 1] = "enter:" .. select("#", ...)
      end,
    },
  })

  assert(machine:start(nil, "b", nil))
  assert(machine:transition("start"))
  assert(machine.current == "running")
  assert(table.concat(seen, ",") == "nil:b:nil,enter:3")
end

-- Starting another event while one is deferred abandons the deferred one and
-- reports success, whether or not the new event can run.
do
  local machine = stateMachine.create({
    initial = "idle",
    events = {
      { name = "start", from = "idle", to = "running" },
      { name = "abort", from = "idle", to = "stopped" },
      { name = "finish", from = "running", to = "done" },
    },
    callbacks = { onleaveidle = function(self, event) if event == "start" then return self.ASYNC end end },
  })

  assert(machine:start())
  assert(machine.asyncState == "startWaitingOnLeave")
  assert(machine:abort("why"))
  assert(machine.current == "stopped" and machine.asyncState == machine.NONE)

  local blocked = stateMachine.create({
    initial = "idle",
    events = {
      { name = "start", from = "idle", to = "running" },
      { name = "finish", from = "running", to = "done" },
    },
    callbacks = { onleaveidle = function(self) return self.ASYNC end },
  })
  assert(blocked:start())
  assert(blocked:finish())
  assert(blocked.current == "idle" and blocked.asyncState == blocked.NONE)
end

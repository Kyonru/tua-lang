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

-- Baseline workloads, not a new profiler. All timing/assertions use emitted runtime data.
local Dialogue = require("tuadialogue")
local document = require("scene")
local function sorted(values)
  local result = {}
  for index, value in ipairs(values) do result[index] = value end
  table.sort(result)
  return result
end
local function report(name, unit, values)
  local ordered = sorted(values)
  local median = ordered[math.ceil(#ordered / 2)]
  local p95 = ordered[math.ceil(#ordered * 0.95)]
  print(string.format("METRIC\t%s\t%s\t%.9f\t%.9f\t%d\t%.9f\t%.9f",
    name, unit, median, p95, #values, ordered[1], ordered[#ordered]))
end
local function run()
  local player = Dialogue.new(document)
  player:start()
  player:update(1, true)
  player:update(1, true)
  local operations = {
    {
      name = "idle_update",
      count = 2000,
      call = function()
        assert(#player:update(1 / 60, true) == 0)
      end,
    },
    {
      name = "snapshot",
      count = 2000,
      call = function()
        local view = player:snapshot()
        assert(view.node.text == "Welcome aboard." and #view.node.events == 2)
      end,
    },
    {
      name = "full_session",
      count = 500,
      call = function()
        local instance = Dialogue.new(document)
        instance:start()
        assert(#instance:update(0.2, false) == 1)
        instance:update(0, true)
        assert(#instance:update(0.25, true) == 1)
        assert(instance:advance() and instance:choose("baseline.leave"))
        assert(instance:advance() and instance:snapshot().state == "completed")
        instance:dispose()
      end,
    },
  }
  -- CPU: warmed batches with GC running. p95 is over batch averages, not frames.
  for _, operation in ipairs(operations) do
    for _ = 1, 200 do operation.call() end
    operation.samples = {}
  end
  for trial = 1, 7 do
    for offset = 1, #operations do
      local index = trial % 2 == 0 and #operations - offset + 1 or offset
      local operation = operations[index]
      collectgarbage("collect")
      collectgarbage("restart")
      local began = os.clock()
      for _ = 1, operation.count do operation.call() end
      operation.samples[#operation.samples + 1] = (os.clock() - began) * 1000 / operation.count
    end
  end
  for _, operation in ipairs(operations) do
    report(operation.name, "cpu_ms_per_operation_batch_average", operation.samples)
    -- Separate allocation proxy; not the CPU trial and not C/GPU allocation.
    local growth, retained = {}, {}
    for _ = 1, 7 do
      collectgarbage("collect")
      local before = collectgarbage("count")
      collectgarbage("stop")
      for _ = 1, 200 do operation.call() end
      local after = collectgarbage("count")
      collectgarbage("restart")
      collectgarbage("collect")
      local surviving = collectgarbage("count")
      growth[#growth + 1] = (after - before) / 200
      retained[#retained + 1] = surviving - before
    end
    report(operation.name, "gc_stopped_lua_kib_per_operation", growth)
    report(operation.name, "retained_lua_kib_per_batch", retained)
  end
  player:dispose()
end
return { run = run, report = report }

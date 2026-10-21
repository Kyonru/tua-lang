-- Compares two tuastore.lua runtimes under the selected Lua interpreter.
--
--   lua benchmarks/store-runtime.lua <before/tuastore.lua> [after/tuastore.lua]

local beforePath = assert(arg[1], "usage: lua benchmarks/store-runtime.lua <before> [after]")
local afterPath = arg[2] or "vendor/runtime/tuastore.lua"
local SAMPLES = 7

local function loadRuntime(path)
  local chunk, message = loadfile(path)
  if not chunk then
    error("could not load runtime " .. path .. ": " .. tostring(message))
  end
  return chunk()
end

local function largeState()
  local state = {}
  for index = 1, 2000 do
    state["field" .. index] = index
  end
  return state
end

local workloads = {
  {
    name = "unobserved update, 2k fields",
    iterations = 5000,
    setup = function(createStore)
      local store = createStore(largeState())
      local function update(state) state.field1 = state.field1 + 1 end
      return function(count)
        for _ = 1, count do store:update(update) end
      end, function() return store:get().field1 end
    end,
  },
  {
    name = "one watcher, 2k fields",
    iterations = 3000,
    setup = function(createStore)
      local store = createStore(largeState())
      local notifications = 0
      store:watch("field1", function() notifications = notifications + 1 end)
      local function update(state) state.field1 = state.field1 + 1 end
      return function(count)
        for _ = 1, count do store:update(update) end
      end, function() return store:get().field1 + notifications end
    end,
  },
  {
    name = "100 subscribers, 4 fields",
    iterations = 10000,
    setup = function(createStore)
      local store = createStore({ value = 0, a = 1, b = 2, c = 3 })
      local notifications = 0
      for _ = 1, 100 do
        store:subscribe(function() notifications = notifications + 1 end)
      end
      local function update(state) state.value = state.value + 1 end
      return function(count)
        for _ = 1, count do store:update(update) end
      end, function() return store:get().value + notifications end
    end,
  },
  {
    name = "one subscriber, 2k fields",
    iterations = 3000,
    setup = function(createStore)
      local store = createStore(largeState())
      local notifications = 0
      store:subscribe(function() notifications = notifications + 1 end)
      local function update(state) state.field1 = state.field1 + 1 end
      return function(count)
        for _ = 1, count do store:update(update) end
      end, function() return store:get().field1 + notifications end
    end,
  },
}

local function median(values)
  table.sort(values)
  return values[math.floor((#values + 1) / 2)]
end

local function measureGarbage(createStore, workload)
  local run = workload.setup(createStore)
  run(10)
  collectgarbage("collect")
  collectgarbage("stop")
  local before = collectgarbage("count")
  run(100)
  local garbage = (collectgarbage("count") - before) / 100
  collectgarbage("restart")
  collectgarbage("collect")
  return garbage
end

local function measure(runtimes, workload)
  local samples = { {}, {} }
  local checksums = {}
  for sample = 1, SAMPLES do
    local order = sample % 2 == 1 and { 1, 2 } or { 2, 1 }
    for _, index in ipairs(order) do
      local run, readChecksum = workload.setup(runtimes[index].createStore)
      run(100)
      collectgarbage("collect")
      local started = os.clock()
      run(workload.iterations)
      samples[index][sample] = (os.clock() - started) * 1000000 / workload.iterations
      checksums[index] = readChecksum()
    end
  end

  return {
    {
      micros = median(samples[1]),
      garbage = measureGarbage(runtimes[1].createStore, workload),
      checksum = checksums[1],
    },
    {
      micros = median(samples[2]),
      garbage = measureGarbage(runtimes[2].createStore, workload),
      checksum = checksums[2],
    },
  }
end

local runtimes = {
  { label = "before", createStore = loadRuntime(beforePath) },
  { label = "after", createStore = loadRuntime(afterPath) },
}

print(_VERSION .. (jit and " / " .. jit.version or ""))
print("before: " .. beforePath)
print("after:  " .. afterPath)
print(string.format("%-31s %10s %10s %8s %12s %12s %9s",
  "workload", "before us", "after us", "speedup", "before KB/u", "after KB/u", "checksum"))

local allMatch = true
for _, workload in ipairs(workloads) do
  local results = measure(runtimes, workload)
  local before, after = results[1], results[2]
  local matches = before.checksum == after.checksum
  allMatch = allMatch and matches
  print(string.format("%-31s %10.3f %10.3f %7.2fx %12.3f %12.3f %9s",
    workload.name, before.micros, after.micros, before.micros / after.micros,
    before.garbage, after.garbage, matches and "match" or "DIFFER"))
end

if not allMatch then
  os.exit(1)
end

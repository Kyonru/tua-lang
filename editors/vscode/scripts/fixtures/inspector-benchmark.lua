-- Deterministic runtime workload. The frame timer includes the inspector's
-- post-frame tick but excludes pacing sleep; no renderer or game input is used.
local ecs = require("tuaecs")
local store = require("tuastore")
local pool = require("tuaobjectpool")
local signals = require("tuasignal")
local machines = require("tuastatemachine")
local heavy = os.getenv("TUA_BENCH_PROFILE") == "heavy"
local warmup = tonumber(os.getenv("TUA_BENCH_WARMUP")) or 3
local duration = tonumber(os.getenv("TUA_BENCH_SECONDS")) or 8
local entityCount, storeCount, fieldCount, emissions = heavy and 2000 or 200, heavy and 48 or 8, heavy and 32 or 16, heavy and 128 or 8
local world, component = ecs.world(), ecs.component("Position", {})
local positions, stores, pools, states = {}, {}, {}, {}
for i = 1, entityCount do
  local position = { x = i, y = i % 100, vx = 2, vy = 3 }
  positions[i] = position
  world:add(world:create(), component, position)
end
for i = 1, storeCount do
  local value = {}
  for field = 1, fieldCount do value["field" .. field] = field end
  stores[i] = store(value)
  stores[i]:subscribe(function() end)
  stores[i]:watch("field1", function() end)
end
for i = 1, 4 do
  pools[i] = pool({ create = function() return { x = 0 } end, initial = 64 })
  states[i] = machines.create({ initial = "idle", events = {
    { name = "move", from = "idle", to = "moving" }, { name = "rest", from = "moving", to = "idle" },
  } })
end
local bus = signals.new()
bus:on("frame", function() end)
local function mutate(value) value.field1 = value.field1 + 1 end
local frame = 0
local function update()
  frame = frame + 1
  for _, position in ipairs(positions) do
    position.x = (position.x + position.vx / 60) % 1000
    position.y = (position.y + position.vy / 60) % 1000
  end
  for _, value in ipairs(stores) do value:update(mutate) end
  for _, value in ipairs(pools) do
    for i = 1, 8 do local item = value:acquire(); item.x = frame; value:release(item) end
  end
  for i = 1, emissions do bus:emit("frame", frame) end
  if frame % 60 == 0 then
    for _, value in ipairs(states) do if value.current == "idle" then value:move() else value:rest() end end
  end
end

function love.run()
  local started = love.timer.getTime()
  local previousStart, measuring, measureStart, cpuStart
  local samples, heap, heapPeak, heapSamples = {}, 0, 0, 0
  return function()
    local now = love.timer.getTime()
    if measuring and previousStart then
      samples[#samples + 1] = (now - previousStart) * 1000
      local memory = collectgarbage("count")
      heap = heap + memory; heapPeak = math.max(heapPeak, memory); heapSamples = heapSamples + 1
    end
    if not measuring and now - started >= warmup then
      collectgarbage("collect")
      measuring, measureStart, cpuStart = true, love.timer.getTime(), os.clock()
      print("BENCH_START")
    elseif measuring and now - measureStart >= duration then
      local elapsed, cpu = now - measureStart, os.clock() - cpuStart
      local times = {}; for i, value in ipairs(samples) do times[i] = string.format("%.9f", value) end
      collectgarbage("collect")
      print(string.format('BENCH_RESULT {"elapsed":%.6f,"cpuSeconds":%.6f,"heapMeanKiB":%.3f,"heapPeakKiB":%.3f,"heapRetainedKiB":%.3f,"frames":[%s]}', elapsed, cpu, heap / math.max(1, heapSamples), heapPeak, collectgarbage("count"), table.concat(times, ",")))
      return 0
    end
    -- Pace at ~60 Hz, outside the measured frame work. The preceding interval
    -- includes the real shim tick that runs after this function returns.
    if previousStart then love.timer.sleep(math.max(0, 1 / 60 - (love.timer.getTime() - previousStart))) end
    previousStart = love.timer.getTime()
    update()
    assert(world and bus and states[1]) -- Keep all observed objects reachable.
  end
end

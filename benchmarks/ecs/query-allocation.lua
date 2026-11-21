-- lua benchmarks/ecs/query-allocation.lua path/to/tuaecs.lua
-- Also callable with dofile from LÖVE after setting arg[1]. CPU timings belong
-- in a separate GC-running run; this probe measures allocation, not live growth.
local ecs = assert(loadfile(assert(arg[1], 'provide a runtime path')))()
print(_VERSION, jit and jit.version or 'no LuaJIT', 'JIT', jit and jit.status() or false)
for _, width in ipairs({1, 3, 4, 5, 8}) do
  local world, components = ecs.world(), {}
  for index = 1, width do components[index] = ecs.component('C' .. index) end
  for _ = 1, 12 do
    local id = world:create()
    for _, component in ipairs(components) do world:add(id, component, false) end
  end
  local query, visits = world:query(components), 0
  local function visit() visits = visits + 1 end
  for _ = 1, 100 do query:each(visit) end
  collectgarbage('collect')
  collectgarbage('stop')
  local before = collectgarbage('count')
  for _ = 1, 10000 do query:each(visit) end
  local allocated = collectgarbage('count') - before
  collectgarbage('restart')
  assert(visits == 121200)
  print(string.format('width=%d warmup=100 repetitions=10000 entities=12 gc=stopped allocated_kib=%.3f', width, allocated))
end

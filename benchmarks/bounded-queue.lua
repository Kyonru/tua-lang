-- Run from the repository root: lua benchmarks/bounded-queue.lua
-- Optional: a replacement runtime path, then repetitions (default 9).
local Queue = dofile(arg[1] or 'vendor/runtime/tuaboundedqueue.lua')
local trials = tonumber(arg[2]) or 9
local steps, warmup = 100000, 10000

local function workload(kind, depth)
  local initial = {}
  for index = 1, depth do initial[index] = index end
  if kind == 'ring' then
    local queue = Queue.new(depth, initial)
    return function(count)
      local sum = 0
      for _ = 1, count do
        local value = queue.pop()
        sum = sum + value
        assert(queue.push(value))
      end
      assert(queue.count() == depth)
      return sum
    end
  end
  return function(count)
    local sum = 0
    for _ = 1, count do
      local value = table.remove(initial, 1)
      sum = sum + value
      initial[#initial + 1] = value
    end
    assert(#initial == depth)
    return sum
  end
end

local function percentile(values, fraction)
  table.sort(values)
  return values[math.ceil(#values * fraction)]
end

print(string.format('runtime=%s jit=%s enabled=%s steps=%d warmup=%d trials=%d',
  _VERSION, jit and jit.version or 'unavailable',
  jit and tostring(jit.status()) or 'unavailable', steps, warmup, trials))
print('CPU: GC running, os.clock seconds, uninstrumented; allocations: separate GC-stopped KiB growth')
for _, depth in ipairs({ 1, 16, 128 }) do
  local results = { array = {}, ring = {} }
  local expected
  for trial = 1, trials do
    local order = trial % 2 == 1 and { 'array', 'ring' } or { 'ring', 'array' }
    for _, kind in ipairs(order) do
      collectgarbage('restart')
      collectgarbage('collect')
      local run = workload(kind, depth)
      run(warmup)
      local started = os.clock()
      local outcome = run(steps)
      local duration = os.clock() - started
      expected = expected or outcome
      assert(outcome == expected, 'outcome mismatch')
      results[kind][#results[kind] + 1] = duration
    end
  end
  for _, kind in ipairs({ 'array', 'ring' }) do
    local run = workload(kind, depth)
    run(warmup)
    collectgarbage('collect')
    collectgarbage('stop')
    local before = collectgarbage('count')
    assert(run(steps) == expected)
    local allocated = collectgarbage('count') - before
    collectgarbage('restart')
    collectgarbage('collect')
    local retained = collectgarbage('count') - before
    print(string.format('depth=%d kind=%s median=%.6f p95=%.6f min=%.6f allocated_kib=%.3f retained_delta_kib=%.3f outcome=%d',
      depth, kind, percentile(results[kind], 0.5), percentile(results[kind], 0.95),
      results[kind][1], allocated, retained, expected))
  end
end

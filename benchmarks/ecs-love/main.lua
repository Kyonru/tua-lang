-- Compares two tuaecs.lua runtimes under LOVE's LuaJIT.
--
--   love benchmarks/ecs-love [--jit] <before/tuaecs.lua> [after/tuaecs.lua]
--
-- `after` defaults to the repository's vendor/runtime/tuaecs.lua. LOVE turns
-- the JIT compiler off on some platforms (for example Apple Silicon); `--jit`
-- turns it back on to approximate desktop x64 behavior. Both
-- runtimes are loaded into the same process and measured in alternating
-- samples, so machine noise affects them equally. Each workload also prints a
-- checksum of the final world state; matching checksums show both runtimes
-- computed the same results.

-- Ten-thousand-entity interpreter runs are deliberately bounded so this also
-- finishes on Apple Silicon builds where LuaJIT starts disabled.
local WARMUP_FRAMES = 30
local SAMPLES = 7
local FRAMES_PER_SAMPLE = 20
local GARBAGE_FRAMES = 3
local DT = 1 / 60
local W, H = 720, 480

local function rng(seed)
  local state = seed
  return function()
    state = (state * 1103515245 + 12345) % 2147483648
    return state / 2147483648
  end
end

local function wrap(value, limit)
  if value > limit then
    return value - limit
  elseif value < 0 then
    return value + limit
  end
  return value
end

local function sumOver(world, component, field)
  local sum = 0
  for entity = 1, world._nextEntity do
    local value = world:get(entity, component)
    if value then
      sum = sum + value[field]
    end
  end
  return sum
end

-- Each workload builds a fresh world for one runtime and returns
-- { frame = function(), checksum = function() }.
local workloads = {
  {
    name = "1-component system, 10k",
    setup = function(ecs)
      local world = ecs.world()
      local Body = ecs.component("Body", {})
      local r = rng(1)
      for _ = 1, 10000 do
        world:add(world:create(), Body, { x = r() * W, y = r() * H, dx = r() * 100 - 50, dy = r() * 100 - 50 })
      end
      local system = ecs.system(world, { Body }, function(_, b, dt)
        b.x = wrap(b.x + b.dx * dt, W)
        b.y = wrap(b.y + b.dy * dt, H)
      end)
      return {
        frame = function() system:update(DT) end,
        checksum = function() return sumOver(world, Body, "x") + sumOver(world, Body, "y") end,
      }
    end,
  },
  {
    name = "3-component system, 10k",
    setup = function(ecs)
      local world = ecs.world()
      local Position = ecs.component("Position", {})
      local Velocity = ecs.component("Velocity", {})
      local Tag = ecs.component("Tag", {})
      local tag = {}
      local r = rng(2)
      for _ = 1, 10000 do
        local e = world:create()
        world:add(e, Position, { x = r() * W, y = r() * H })
        world:add(e, Velocity, { x = r() * 100 - 50, y = r() * 100 - 50 })
        world:add(e, Tag, tag)
      end
      local system = ecs.system(world, { Position, Velocity, Tag }, function(_, p, v, _, dt)
        p.x = wrap(p.x + v.x * dt, W)
        p.y = wrap(p.y + v.y * dt, H)
      end)
      return {
        frame = function() system:update(DT) end,
        checksum = function() return sumOver(world, Position, "x") + sumOver(world, Position, "y") end,
      }
    end,
  },
  {
    name = "4-component system, 10k",
    setup = function(ecs)
      local world = ecs.world()
      local Position = ecs.component("Position", {})
      local Velocity = ecs.component("Velocity", {})
      local Tag = ecs.component("Tag", {})
      local Life = ecs.component("Life", {})
      local tag = {}
      local r = rng(3)
      for _ = 1, 10000 do
        local e = world:create()
        world:add(e, Position, { x = r() * W, y = r() * H })
        world:add(e, Velocity, { x = r() * 100 - 50, y = r() * 100 - 50 })
        world:add(e, Tag, tag)
        world:add(e, Life, { t = 0 })
      end
      local system = ecs.system(world, { Position, Velocity, Tag, Life }, function(_, p, v, _, life, dt)
        p.x = wrap(p.x + v.x * dt, W)
        p.y = wrap(p.y + v.y * dt, H)
        life.t = life.t + dt
      end)
      return {
        frame = function() system:update(DT) end,
        checksum = function() return sumOver(world, Position, "x") + sumOver(world, Life, "t") end,
      }
    end,
  },
  {
    name = "2-component query:each, 10k",
    setup = function(ecs)
      local world = ecs.world()
      local Position = ecs.component("Position", {})
      local Sprite = ecs.component("Sprite", {})
      local r = rng(4)
      for _ = 1, 10000 do
        local e = world:create()
        world:add(e, Position, { x = r() * W, y = r() * H })
        world:add(e, Sprite, { radius = 3 })
      end
      local visible = world:query({ Position, Sprite })
      local total = 0
      -- Created once, like a hoisted draw callback.
      local function read(_, p, s)
        total = total + p.x + p.y + s.radius
      end
      return {
        frame = function() visible:each(read) end,
        checksum = function() return total end,
      }
    end,
  },
  {
    name = "bullet-hell churn, 5k",
    setup = function(ecs)
      local world = ecs.world()
      local Bullet = ecs.component("Bullet", {})
      local r = rng(5)
      local free = {}
      local pending = 0
      local drawn = 0

      local function spawn()
        local b = table.remove(free) or {}
        local angle = r() * math.pi * 2
        local speed = 60 + r() * 120
        b.x, b.y = W / 2, H / 2
        b.dx, b.dy = math.cos(angle) * speed, math.sin(angle) * speed
        world:add(world:create(), Bullet, b)
      end

      for _ = 1, 5000 do
        spawn()
      end

      local move = ecs.system(world, { Bullet }, function(e, b, dt)
        b.x = b.x + b.dx * dt
        b.y = b.y + b.dy * dt
        if b.x < -20 or b.x > W + 20 or b.y < -20 or b.y > H + 20 then
          world:remove(e, Bullet)
          world:destroy(e)
          free[#free + 1] = b
          pending = pending + 1
        end
      end)
      local visible = world:query({ Bullet })
      local function draw(_, b)
        drawn = drawn + b.x * 0 + 1
      end

      return {
        frame = function()
          move:update(DT)
          for _ = 1, pending do
            spawn()
          end
          pending = 0
          visible:each(draw)
        end,
        checksum = function()
          return sumOver(world, Bullet, "x") + sumOver(world, Bullet, "y") + drawn + world._nextEntity
        end,
      }
    end,
  },
}

local function median(list)
  local copy = {}
  for index = 1, #list do
    copy[index] = list[index]
  end
  table.sort(copy)
  return copy[math.floor((#copy + 1) / 2)]
end

local function runFrames(state, count)
  for _ = 1, count do
    state.frame()
  end
end

local function measure(runtimes, workload)
  local states = {}
  for index, runtime in ipairs(runtimes) do
    states[index] = workload.setup(runtime.ecs)
    runFrames(states[index], WARMUP_FRAMES)
  end

  local samples = {}
  for index = 1, #runtimes do
    samples[index] = {}
  end

  for sample = 1, SAMPLES do
    -- Alternate which runtime goes first so ordering doesn't favor one.
    local order = sample % 2 == 1 and { 1, 2 } or { 2, 1 }
    for _, index in ipairs(order) do
      if states[index] then
        local start = love.timer.getTime()
        runFrames(states[index], FRAMES_PER_SAMPLE)
        local elapsed = love.timer.getTime() - start
        samples[index][#samples[index] + 1] = elapsed * 1000 / FRAMES_PER_SAMPLE
      end
    end
  end

  local results = {}
  for index = 1, #runtimes do
    local state = states[index]
    collectgarbage("collect")
    collectgarbage("stop")
    local before = collectgarbage("count")
    runFrames(state, GARBAGE_FRAMES)
    local garbageKb = (collectgarbage("count") - before) / GARBAGE_FRAMES
    collectgarbage("restart")
    collectgarbage("collect")

    results[index] = {
      ms = median(samples[index]),
      garbageKb = garbageKb,
      checksum = string.format("%.6f", state.checksum()),
    }
  end
  return results
end

local function loadRuntime(path)
  local chunk, err = loadfile(path)
  if not chunk then
    error("could not load runtime " .. tostring(path) .. ": " .. tostring(err))
  end
  return chunk()
end

local function checkClearDuringIteration(runtime, label)
  local world = runtime.world()
  local Marker = runtime.component("BenchmarkClearMarker", {})
  for _ = 1, 20 do
    world:add(world:create(), Marker, {})
  end

  local visits = 0
  world:query({ Marker }):each(function()
    visits = visits + 1
    world:clear()
  end)

  if visits ~= 1 then
    error(label .. " runtime visited stale values after world:clear()")
  end
end

function love.load(args)
  local paths = {}
  for _, value in ipairs(args) do
    if value == "--jit" then
      if jit then
        jit.on()
        jit.flush()
      end
    else
      paths[#paths + 1] = value
    end
  end

  local defaultAfter = love.filesystem.getSource() .. "/../../vendor/runtime/tuaecs.lua"
  local beforePath = paths[1]
  local afterPath = paths[2] or defaultAfter

  if not beforePath then
    print("usage: love benchmarks/ecs-love [--jit] <before/tuaecs.lua> [after/tuaecs.lua]")
    love.event.quit(1)
    return
  end

  local runtimes = {
    { label = "before", path = beforePath, ecs = loadRuntime(beforePath) },
    { label = "after", path = afterPath, ecs = loadRuntime(afterPath) },
  }
  for _, runtime in ipairs(runtimes) do
    checkClearDuringIteration(runtime.ecs, runtime.label)
  end

  local major, minor, revision = love.getVersion()
  print(string.format("LOVE %d.%d.%d, %s, %s %s, JIT %s",
    major, minor, revision,
    jit and jit.version or _VERSION,
    jit and jit.os or "?", jit and jit.arch or "?",
    jit and (jit.status() and "on" or "off") or "n/a"))
  print("before: " .. beforePath)
  print("after:  " .. afterPath)
  print(string.format("%d samples x %d frames after %d warm-up frames; median ms per frame\n",
    SAMPLES, FRAMES_PER_SAMPLE, WARMUP_FRAMES))

  print(string.format("%-30s %10s %10s %8s %12s %12s %9s",
    "workload", "before ms", "after ms", "speedup", "before KB/f", "after KB/f", "checksum"))

  local allMatch = true
  for _, workload in ipairs(workloads) do
    local results = measure(runtimes, workload)
    local before, after = results[1], results[2]
    local same = before.checksum == after.checksum
    allMatch = allMatch and same
    print(string.format("%-30s %10.3f %10.3f %7.2fx %12.1f %12.1f %9s",
      workload.name, before.ms, after.ms, before.ms / after.ms,
      before.garbageKb, after.garbageKb, same and "match" or "DIFFER"))
  end

  if not allMatch then
    print("\nchecksums differ: the runtimes computed different results")
  end
  love.event.quit(allMatch and 0 or 1)
end

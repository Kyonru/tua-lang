local Combat = require("combat")
local mode = os.getenv("ARENA_MODE") or "cpu"
local count = assert(tonumber(os.getenv("ARENA_ENEMIES")))
local player = { x = 800, y = 600 }
local combat = Combat.new(player)
combat.options.enemyCount = count
local frame = 0
local samples = {}
local outcomes = {}
local timer = love.timer.getTime
local function reset()
  player.x, player.y = 800, 600
  combat.reset()
end
local function step(index)
  -- One-second episodes prevent death from turning the workload into idle updates.
  if (index - 1) % 60 == 0 then
    reset()
  end
  local phase = (index - 1) % 60
  local x = phase < 30 and 1 or -1
  local ticks = combat.state.ticks
  local before = timer()
  combat.update(1 / 60, x, 0, true, 1500, 200)
  local elapsed = (timer() - before) * 1000
  assert(combat.state.health > 0 and combat.state.ticks == ticks + 2, "workload became idle")
  return elapsed
end
local function outcome()
  local state = combat.state
  local values = {
    state.health,
    state.attacks,
    state.hits,
    state.defeated,
    state.shots,
    state.ticks,
    state.skipped,
    #combat.bullets,
    combat.enemies:count(),
    player.x,
    player.y,
  }
  combat.enemies:each(function(id, position, health)
    values[#values + 1] = id
    values[#values + 1] = position.x
    values[#values + 1] = position.y
    values[#values + 1] = health.current
  end)
  for _, bullet in ipairs(combat.bullets) do
    values[#values + 1] = bullet.x
    values[#values + 1] = bullet.y
    values[#values + 1] = bullet.age
  end
  return table.concat(values, ",")
end
function love.load()
  if jit then
    jit.off()
  end
  math.randomseed(71)
  love.math.setRandomSeed(71)
  collectgarbage("restart")
  for index = 1, 120 do
    step(index)
  end
  local major, minor, patch = love.getVersion()
  print(
    string.format(
      "RUNTIME:%d.%d.%d|%s|%s",
      major,
      minor,
      patch,
      jit and jit.version or _VERSION,
      tostring(jit and jit.status() or false)
    )
  )
  if mode == "allocation" then
    reset()
    collectgarbage("collect")
    local before = collectgarbage("count")
    collectgarbage("stop")
    -- Avoid the reset inside step: only simulation allocations belong in this probe.
    for index = 1, 60 do
      combat.update(1 / 60, index <= 30 and 1 or -1, 0, true, 1500, 200)
    end
    assert(combat.state.health > 0 and combat.state.ticks == 120)
    local allocated = collectgarbage("count") - before
    collectgarbage("restart")
    collectgarbage("collect")
    local retained = collectgarbage("count") - before
    print("MEMORY:" .. allocated .. "," .. retained)
    print("OUTCOME:" .. outcome())
    combat.dispose()
    love.event.quit()
  end
end
function love.update()
  if mode == "allocation" then
    return
  end
  -- Multiple simulation updates per rendered frame give the sampler useful work.
  for _ = 1, (mode == "profile" and 120 or 10) do
    frame = frame + 1
    local elapsed = step(frame)
    if mode == "cpu" then
      samples[#samples + 1] = elapsed
    end
    if frame % 60 == 0 then
      outcomes[#outcomes + 1] = outcome()
    end
  end
  if frame >= (mode == "profile" and 6000 or 600) then
    if mode == "cpu" then
      print("TIMES:" .. table.concat(samples, ","))
    end
    print("OUTCOME:" .. table.concat(outcomes, ";"))
    combat.dispose()
    love.event.quit()
  end
end

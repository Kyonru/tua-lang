local Combat = require("combat")
local Components = require("components")
local Collision = require("collision")
local function ids(combat)
  local result = {}
  combat.enemies:each(function(id)
    result[#result + 1] = id
  end)
  table.sort(result)
  return result
end
local function simulate(rate)
  local combat = Combat.new({ x = 800, y = 600 })
  -- Aim between enemies: exhaust the pool without damage.
  for _ = 1, rate * 4 do
    combat.update(1 / rate, 0, 0, true, 1200, 740)
  end
  local result = {
    shots = combat.state.shots,
    hits = combat.state.hits,
    skipped = combat.state.skipped,
    active = #combat.bullets,
    ticks = combat.state.ticks,
  }
  assert(result.skipped > 0 and result.hits == 0 and combat.capacity() == 8)
  for _ = 1, rate * 2 do
    combat.update(1 / rate, 0, 0, false, 1200, 740)
  end
  assert(#combat.bullets == 0)
  combat.dispose()
  return result
end
local baseline = simulate(120)
for _, rate in ipairs({ 30, 60, 144 }) do
  for key, value in pairs(simulate(rate)) do
    assert(value == baseline[key], key .. " at " .. rate)
  end
end
assert(Collision.contact(0, 0, 1000, 0, 500, 0, 25) == 0.475)
assert(Collision.contact(1000, 0, 0, 0, 500, 0, 25) == 0.475)
assert(Collision.contact(500, 0, 500, 0, 500, 0, 25) == 0)
assert(Collision.contact(0, 0, 0, 0, 500, 0, 25) == nil)
assert(Collision.contact(0, 30, 1000, 30, 500, 0, 25) == nil)
local combat = Combat.new({ x = 800, y = 600 })
local live = ids(combat)
local first = live[1]
local second = live[2]
local a, b = combat.readPosition(first), combat.readPosition(second)
a.x, a.y = 830, 572
b.x, b.y = 830, 572
local oldHealth = combat.readHealth(first)
combat.world:add(first, Components.Health, { current = 1, maximum = 1 })
oldHealth.current = 100
combat.update(1 / 120, 0, 0, true, 1030, 572)
assert(combat.state.hits == 1 and combat.state.defeated == 1)
assert(
  combat.readHealth(first) == nil and combat.readHealth(second).current == 3,
  "tie selects lower ID once"
)
assert(#combat.bullets == 0)
for _ = 1, 15 do
  combat.update(1 / 120, 0, 0, true, 1030, 572)
end
assert(
  combat.state.hits == 2 and combat.state.defeated == 1,
  "destroyed enemy cannot absorb another hit"
)
combat.reset()
combat.update(1 / 120, 0, 0, true, 1200, 740)
local retained = combat.bullets[1]
for _ = 1, 150 do
  combat.update(1 / 120, 0, 0, false, 1200, 740)
end
assert(retained.x == 0 and retained.y == 0 and retained.vx == 0 and retained.vy == 0)
assert(retained.age == 0 and not retained.hit)
combat.update(1 / 120, 0, 0, true, 1200, 740)
assert(combat.bullets[1] == retained and not retained.hit)
for _ = 1, 100 do
  first = ids(combat)[1]
  combat.reset()
  assert(combat.readPosition(first) == nil and combat.enemies:count() == 8)
  assert(#combat.bullets == 0 and combat.state.wave == 1 and combat.state.hits == 0)
end
assert(not combat.nextWave())
for _, id in ipairs(ids(combat)) do
  combat.world:destroy(id)
end
assert(combat.nextWave() and combat.state.wave == 2)
combat.update(10, 0, 0, false, 0, 0)
assert(combat.state.ticks == 30)
first = ids(combat)[1]
combat.dispose()
combat.dispose()
combat.reset()
combat.update(1, 0, 0, true, 1030, 572)
assert(combat.readPosition(first) == nil and combat.enemies:count() == 0 and combat.capacity() == 0)
assert(not combat.nextWave())
print("ARENA_COMBAT_OK rate parity, exhaustion, sweep, ties, live health, reuse, reset, disposal")

local Wave = require("wave")
local Effects = require("effects")
local Components = require("components")
local function ids(wave)
  local result = {}
  wave.enemies:each(function(id) result[#result + 1] = id end)
  table.sort(result)
  return result
end
local wave = Wave.new()
assert(wave.enemies:count() == 8 and not wave.spawnWave())
local first = ids(wave)[1]
local oldPosition = wave.readPosition(first)
local oldHealth = wave.readHealth(first)
assert(not wave.damage(first) and oldHealth.current == 1)
assert(wave.damage(first) and wave.state.defeated == 1)
assert(not wave.damage(first) and wave.state.defeated == 1)
assert(wave.readHealth(first) == nil and wave.readPosition(first) == nil)
local x = oldPosition.x
wave.update(0.5)
assert(oldPosition.x == x, "destroyed entity must stop updating")
for _ = 1, 100 do
  local previous = ids(wave)[1]
  wave.reset()
  assert(wave.readPosition(previous) == nil)
  assert(wave.enemies:count() == 8 and wave.state.wave == 1)
  assert(wave.state.defeated == 0 and wave.state.shots == 0)
end
local live = ids(wave)
first = live[1]
oldHealth = wave.readHealth(first)
wave.world:add(first, Components.Health, { current = 4, maximum = 4 })
oldHealth.current = 0
assert(not wave.damage(first) and wave.readHealth(first).current == 3)
wave.world:remove(first, Components.Health)
assert(wave.readHealth(first) == nil and wave.enemies:count() == 7)
wave.world:add(first, Components.Health, { current = 2, maximum = 2 })
assert(wave.enemies:count() == 8)
local a = wave.readPosition(first)
local b = wave.readPosition(live[2])
a.x, a.y = 400, 240
b.x, b.y = 400, 240
wave.shoot(400, 212)
assert(wave.readHealth(first).current == 1, "ties select lower entity ID")
assert(wave.readHealth(live[2]).current == 3)
for _, id in ipairs(live) do
  while wave.readHealth(id) do wave.damage(id) end
end
assert(wave.enemies:count() == 0 and wave.spawnWave())
assert(wave.state.wave == 2 and wave.enemies:count() == 8)
local effects = Effects.new()
assert(effects.capacity() == 0)
assert(effects.spawn(12, 34))
local retained = effects.items[1]
effects.update(0.4)
assert(#effects.items == 0 and effects.activeCount() == 0)
assert(retained.x == 0 and retained.y == 0 and retained.age == 0)
assert(effects.spawn(56, 78) and effects.items[1] == retained)
assert(retained.x == 56 and retained.y == 78 and retained.age == 0)
for _ = 2, 16 do assert(effects.spawn(1, 2)) end
assert(not effects.spawn(1, 2) and effects.capacity() == 16)
effects.clear()
assert(effects.activeCount() == 0 and #effects.items == 0)
effects.dispose()
effects.dispose()
assert(effects.capacity() == 0 and not effects.spawn(1, 2))
first = ids(wave)[1]
wave.dispose()
wave.dispose()
wave.reset()
wave.update(1)
assert(wave.readPosition(first) == nil and wave.enemies:count() == 0)
assert(not wave.spawnWave() and not wave.damage(first))
assert(wave.shoot(1, 2) == nil)
print("WAVE_LIFECYCLE_OK destruction, replacement, reset, ties, pooling, disposal")

local root = assert(arg[1])
-- An optional second argument points at another runtime directory, so the same
-- behavior checks can be run against an older tuaecs.lua for comparison.
local runtimeDir = arg[2] or (root .. "/vendor/runtime")
package.path = runtimeDir .. "/?.lua;" .. package.path

local ecs = require("tuaecs")

local failures = 0

local function check(condition, message)
  if not condition then
    failures = failures + 1
    io.stderr:write("FAIL: " .. message .. "\n")
  end
end

local function sortedKeys(set)
  local keys = {}
  for key in pairs(set) do
    keys[#keys + 1] = key
  end
  table.sort(keys)
  return table.concat(keys, ",")
end

local function components(count)
  local list = {}
  for index = 1, count do
    list[index] = ecs.component("C" .. index, {})
  end
  return list
end

-- Queries of every width visit exactly the matching entities, pass values in
-- component order, and pass no extra arguments.
for width = 1, 5 do
  local world = ecs.world()
  local list = components(width)
  local expected = {}

  for entity = 1, 6 do
    local id = world:create()
    -- Entities 1..4 get every component; 5 and 6 miss the last one.
    local limit = entity <= 4 and width or width - 1
    for index = 1, limit do
      world:add(id, list[index], { owner = id, slot = index })
    end
    if limit == width then
      expected[id] = true
    end
  end

  local visited = {}
  world:query(list):each(function(entity, ...)
    check(select("#", ...) == width, "each passes exactly " .. width .. " values")
    for index = 1, width do
      local value = select(index, ...)
      check(value ~= nil and value.owner == entity and value.slot == index,
        "each value " .. index .. " belongs to entity in component order (width " .. width .. ")")
    end
    visited[entity] = true
  end)
  check(sortedKeys(visited) == sortedKeys(expected), "each visits matching entities (width " .. width .. ")")
  check(world:query(list):count() == 4, "count matches (width " .. width .. ")")

  local systemVisited = {}
  local system = ecs.system(world, list, function(entity, ...)
    check(select("#", ...) == width + 1, "system passes " .. width .. " values plus dt")
    check(select(width + 1, ...) == 0.25, "system passes dt last (width " .. width .. ")")
    for index = 1, width do
      local value = select(index, ...)
      check(value.owner == entity and value.slot == index,
        "system value " .. index .. " in component order (width " .. width .. ")")
    end
    systemVisited[entity] = true
  end)
  system:update(0.25)
  check(sortedKeys(systemVisited) == sortedKeys(expected), "system visits matching entities (width " .. width .. ")")

  system.enabled = false
  local ranWhileDisabled = false
  local disabled = ecs.system(world, list, function() ranWhileDisabled = true end)
  disabled.enabled = false
  disabled:update(0.25)
  check(not ranWhileDisabled, "disabled systems do not run")
end

-- A query whose component was never added visits nothing.
do
  local world = ecs.world()
  local Missing = ecs.component("Missing", {})
  local Present = ecs.component("Present", {})
  world:add(world:create(), Present, {})
  local visits = 0
  world:query({ Present, Missing }):each(function() visits = visits + 1 end)
  check(visits == 0, "query with an absent component visits nothing")
  check(world:query({ Missing }):count() == 0, "count of an absent component is zero")
end

-- Destroying the current entity, or another not-yet-visited entity, during
-- iteration is safe and the destroyed entity is not visited afterwards.
do
  local world = ecs.world()
  local A = ecs.component("A", {})
  local B = ecs.component("B", {})
  local ids = {}
  for index = 1, 50 do
    ids[index] = world:create()
    world:add(ids[index], A, { id = ids[index] })
    world:add(ids[index], B, { id = ids[index] })
  end

  local destroyed = {}
  local visited = {}
  world:query({ A, B }):each(function(entity)
    check(not destroyed[entity], "a destroyed entity is never visited")
    visited[entity] = true
    -- Destroy self and the partner entity (pairs 1-2, 3-4, ...).
    local partner = entity % 2 == 1 and entity + 1 or entity - 1
    world:destroy(entity)
    destroyed[entity] = true
    if not visited[partner] then
      world:destroy(partner)
      destroyed[partner] = true
    end
  end)
  check(world:query({ A }):count() == 0, "every entity was destroyed")
  check(not world:has(ids[1], A), "destroy removes components")
end

-- Removing a component from a not-yet-visited entity excludes it.
do
  local world = ecs.world()
  local A = ecs.component("A", {})
  local B = ecs.component("B", {})
  for _ = 1, 40 do
    local id = world:create()
    world:add(id, A, {})
    world:add(id, B, {})
  end

  local removedFrom = {}
  world:query({ A, B }):each(function(entity)
    check(not removedFrom[entity], "an entity whose component was removed is not visited")
    local other = entity % 40 + 1
    if world:has(other, B) then
      world:remove(other, B)
      removedFrom[other] = true
    end
  end)
end

-- Running a query from inside its own callback is safe.
do
  local world = ecs.world()
  local list = components(5)
  for _ = 1, 4 do
    local id = world:create()
    for index = 1, 5 do
      world:add(id, list[index], { slot = index })
    end
  end
  local query = world:query(list)
  local pairsSeen = 0
  query:each(function(_, a, b, c, d, e)
    query:each(function(_, a2, b2, c2, d2, e2)
      check(a2.slot == 1 and b2.slot == 2 and c2.slot == 3 and d2.slot == 4 and e2.slot == 5,
        "inner iteration values are intact")
      pairsSeen = pairsSeen + 1
    end)
    check(a.slot == 1 and b.slot == 2 and c.slot == 3 and d.slot == 4 and e.slot == 5,
      "outer iteration values survive an inner iteration")
  end)
  check(pairsSeen == 16, "nested iteration visits every pair")
end

-- World basics.
do
  local world = ecs.world()
  local A = ecs.component("A", {})
  local id = world:create()
  local value = { v = 1 }
  check(world:add(id, A, value) == value, "add returns the value")
  check(world:get(id, A) == value and world:has(id, A), "get and has")
  check(world:remove(id, A) == value and not world:has(id, A), "remove returns the value")
  world:add(id, A, value)
  world:clear()
  check(world:query({ A }):count() == 0, "clear empties the world")
  check(not pcall(function() world:add(id, A, value) end), "add to a cleared entity errors")
end

-- Clearing from a callback stops the active iteration instead of visiting
-- values that no longer belong to the world.
do
  local world = ecs.world()
  local A = ecs.component("A", {})
  for _ = 1, 20 do
    world:add(world:create(), A, {})
  end

  local visits = 0
  world:query({ A }):each(function()
    visits = visits + 1
    world:clear()
  end)
  check(visits == 1, "clear stops the active query iteration")
  check(world:query({ A }):count() == 0, "clear leaves no stale query values")
end

-- Wide queries must not allocate per entity.
do
  local world = ecs.world()
  local list = components(5)
  for _ = 1, 2000 do
    local id = world:create()
    for index = 1, 5 do
      world:add(id, list[index], { v = 0 })
    end
  end
  for width = 4, 5 do
    local subset = {}
    for index = 1, width do
      subset[index] = list[index]
    end
    local system = ecs.system(world, subset, function() end)
    system:update(0.1)
    collectgarbage("collect")
    collectgarbage("stop")
    local before = collectgarbage("count")
    system:update(0.1)
    local allocatedKb = collectgarbage("count") - before
    collectgarbage("restart")
    check(allocatedKb < 8, string.format(
      "a %d-component system update over 2000 entities allocates under 8 KB (got %.1f KB)", width, allocatedKb))
  end
end

if failures > 0 then
  io.stderr:write(failures .. " ECS runtime check(s) failed\n")
  os.exit(1)
end
print("ecs runtime checks passed")

-- Tua's built-in entity-component-system runtime.
-- Component data is stored by component, so systems iterate only relevant data.

local ecs = {}
local unpackValues = table.unpack or unpack

local nextComponentId = 0

local function assertComponent(component, level)
  if type(component) ~= "table" or component.__tuaEcsComponent == nil then
    error("expected an ECS component", level or 3)
  end
end

function ecs.component(name, schema)
  if type(name) ~= "string" or name == "" then
    error("ECS component name must be a non-empty string", 2)
  end

  nextComponentId = nextComponentId + 1

  return {
    name = name,
    schema = schema or {},
    __tuaEcsComponent = nextComponentId,
  }
end

local World = {}
World.__index = World

function ecs.world()
  return setmetatable({
    _nextEntity = 0,
    _alive = {},
    _stores = {},
  }, World)
end

function World:_store(component, create)
  assertComponent(component, 4)

  local store = self._stores[component]

  if store == nil and create then
    store = { values = {}, count = 0 }
    self._stores[component] = store
  end

  return store
end

function World:_assertAlive(entity, level)
  if self._alive[entity] ~= true then
    error("unknown or destroyed ECS entity " .. tostring(entity), level or 3)
  end
end

function World:create()
  self._nextEntity = self._nextEntity + 1
  self._alive[self._nextEntity] = true

  return self._nextEntity
end

function World:destroy(entity)
  if self._alive[entity] ~= true then
    return
  end

  self._alive[entity] = nil

  for _, store in pairs(self._stores) do
    if store.values[entity] ~= nil then
      store.values[entity] = nil
      store.count = store.count - 1
    end
  end
end

function World:add(entity, component, value)
  self:_assertAlive(entity, 2)

  local store = self:_store(component, true)

  if store.values[entity] == nil then
    store.count = store.count + 1
  end

  store.values[entity] = value

  return value
end

function World:get(entity, component)
  local store = self:_store(component, false)

  return store and store.values[entity] or nil
end

function World:has(entity, component)
  local store = self:_store(component, false)

  return store ~= nil and store.values[entity] ~= nil
end

function World:remove(entity, component)
  local store = self:_store(component, false)

  if store == nil then
    return nil
  end

  local value = store.values[entity]

  if value ~= nil then
    store.values[entity] = nil
    store.count = store.count - 1
  end

  return value
end

function World:clear()
  -- Empty the existing value tables before detaching them. An active query
  -- holds those tables directly, so this also stops an iteration whose
  -- callback clears the world instead of visiting stale component values.
  for _, store in pairs(self._stores) do
    for entity in pairs(store.values) do
      store.values[entity] = nil
    end
    store.count = 0
  end

  self._alive = {}
  self._stores = {}
end

local Query = {}
Query.__index = Query

-- Returns each component's value table, in query order, plus the smallest one,
-- which drives iteration. Returns nil when a component was never added.
local function queryValues(query)
  local components = query.components
  local world = query.world
  local values = {}
  local driver = nil
  local driverCount = nil

  for index = 1, #components do
    local store = world:_store(components[index], false)

    if store == nil then
      return nil, nil
    end

    values[index] = store.values

    if driverCount == nil or store.count < driverCount then
      driver = store.values
      driverCount = store.count
    end
  end

  return values, driver
end

-- Visits every entity that has all of the query's components. Values are read
-- directly from their component tables, and a nil read means the entity does
-- not match. No liveness check is needed: destroy and clear remove every value,
-- and add requires a live entity.
--
-- Systems pass `hasDt` so `dt` is appended as the last argument. The width
-- branches keep the per-entity path free of closures and table allocations.
local function iterate(query, visit, hasDt, dt)
  local values, driver = queryValues(query)

  if values == nil then
    return
  end

  local count = #values

  if count == 1 then
    if hasDt then
      for entity, a in pairs(driver) do
        visit(entity, a, dt)
      end
    else
      for entity, a in pairs(driver) do
        visit(entity, a)
      end
    end
  elseif count == 2 then
    local va, vb = values[1], values[2]

    for entity in pairs(driver) do
      local a, b = va[entity], vb[entity]

      if a ~= nil and b ~= nil then
        if hasDt then
          visit(entity, a, b, dt)
        else
          visit(entity, a, b)
        end
      end
    end
  elseif count == 3 then
    local va, vb, vc = values[1], values[2], values[3]

    for entity in pairs(driver) do
      local a, b, c = va[entity], vb[entity], vc[entity]

      if a ~= nil and b ~= nil and c ~= nil then
        if hasDt then
          visit(entity, a, b, c, dt)
        else
          visit(entity, a, b, c)
        end
      end
    end
  elseif count == 4 then
    local va, vb, vc, vd = values[1], values[2], values[3], values[4]

    for entity in pairs(driver) do
      local a, b, c, d = va[entity], vb[entity], vc[entity], vd[entity]

      if a ~= nil and b ~= nil and c ~= nil and d ~= nil then
        if hasDt then
          visit(entity, a, b, c, d, dt)
        else
          visit(entity, a, b, c, d)
        end
      end
    end
  else
    -- One argument buffer per iteration, never per entity. unpack copies the
    -- values onto the stack before `visit` runs, so nested iterations and
    -- callbacks can't observe the buffer being reused.
    local args = {}
    local last = count

    if hasDt then
      last = count + 1
      args[last] = dt
    end

    for entity in pairs(driver) do
      local matched = true

      for index = 1, count do
        local value = values[index][entity]

        if value == nil then
          matched = false
          break
        end

        args[index] = value
      end

      if matched then
        visit(entity, unpackValues(args, 1, last))
      end
    end
  end
end

function World:query(components)
  if type(components) ~= "table" or #components == 0 then
    error("ECS query requires at least one component", 2)
  end

  local copied = {}

  for index = 1, #components do
    assertComponent(components[index], 2)
    copied[index] = components[index]
  end

  return setmetatable({ world = self, components = copied }, Query)
end

function Query:each(visit)
  iterate(self, visit, false, nil)
end

function Query:count()
  local count = 0

  self:each(function()
    count = count + 1
  end)

  return count
end

function ecs.system(world, components, update)
  if type(update) ~= "function" then
    error("ECS system update must be a function", 2)
  end

  local system = {
    enabled = true,
    query = world:query(components),
  }

  -- Calls `update` directly, with no wrapper closure per frame or extra
  -- function call per entity.
  function system:update(dt)
    if not self.enabled then
      return
    end

    iterate(self.query, update, true, dt)
  end

  return system
end

return ecs

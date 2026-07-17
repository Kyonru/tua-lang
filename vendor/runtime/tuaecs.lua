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
  self._alive = {}
  self._stores = {}
end

local Query = {}
Query.__index = Query

local function queryStores(query)
  local smallest = nil
  local stores = {}

  for index = 1, #query.components do
    local store = query.world:_store(query.components[index], false)

    if store == nil then
      return nil, nil
    end

    stores[index] = store

    if smallest == nil or store.count < smallest.count then
      smallest = store
    end
  end

  return smallest, stores
end

local function matches(stores, entity)
  for index = 1, #stores do
    if stores[index].values[entity] == nil then
      return false
    end
  end

  return true
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
  local driver, stores = queryStores(self)

  if driver == nil then
    return
  end

  local componentCount = #self.components

  for entity in pairs(driver.values) do
    if self.world._alive[entity] and matches(stores, entity) then
      if componentCount == 1 then
        visit(entity, stores[1].values[entity])
      elseif componentCount == 2 then
        visit(
          entity,
          stores[1].values[entity],
          stores[2].values[entity]
        )
      elseif componentCount == 3 then
        visit(
          entity,
          stores[1].values[entity],
          stores[2].values[entity],
          stores[3].values[entity]
        )
      else
        local values = {}

        for index = 1, componentCount do
          values[index] = stores[index].values[entity]
        end

        visit(entity, unpackValues(values, 1, componentCount))
      end
    end
  end
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

  function system:update(dt)
    if not self.enabled then
      return
    end

    local componentCount = #self.query.components

    if componentCount == 1 then
      self.query:each(function(entity, first)
        update(entity, first, dt)
      end)
    elseif componentCount == 2 then
      self.query:each(function(entity, first, second)
        update(entity, first, second, dt)
      end)
    elseif componentCount == 3 then
      self.query:each(function(entity, first, second, third)
        update(entity, first, second, third, dt)
      end)
    else
      self.query:each(function(entity, ...)
        local values = { ... }
        values[#values + 1] = dt
        update(entity, unpackValues(values, 1, #values))
      end)
    end
  end

  return system
end

return ecs

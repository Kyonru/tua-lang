-- Loaded exclusively by the editor's temporary development launcher.
local directory, port, token, options = ...
options = options or {}
local categories = options.categories or { ecs = true, states = true, signals = true, pools = true, stores = true }
local inspect = assert(loadfile(directory .. "/snapshot.lua"))()
local tick = assert(loadfile(directory .. "/transport.lua"))()(port, token, inspect.snapshot, options.updateInterval)
local originalRequire = require
local wrapped = setmetatable({}, { __mode = "k" })
local function worldData(world)
  local entities, count = {}, 0
  for entity in next, world._alive do
    if count >= 100 then entities["<remaining>"] = "Entity limit (100)"; break end
    count = count + 1
    local components, componentCount = {}, 0
    for component, store in next, world._stores do
      if componentCount >= 32 then components["<remaining>"] = "Component limit (32)"; break end
      componentCount = componentCount + 1
      if store.values[entity] ~= nil then
        components[component.name .. " #" .. component.__tuaEcsComponent] = store.values[entity]
      end
    end
    entities[tostring(entity)] = components
  end
  return { entities = entities, sampledEntities = count }
end
local function stateData(value)
  return { current = value.current, transition = value.currentTransitioningEvent, asyncState = value.asyncState }
end
local function signalData(value)
  local listeners, count = {}, 0
  for name, list in next, value._signals do
    if count >= 100 then listeners["<remaining>"] = "Signal limit (100)"; break end
    count = count + 1; listeners[name] = #list
  end
  return { listeners = listeners }
end
local function poolData(value)
  return { active = value:activeCount(), available = value:availableCount(), capacity = value:capacity() }
end
local function storeData(value)
  local watchers, keys = 0, 0
  for _, listeners in next, value._watchers do
    if keys >= 128 then break end
    keys = keys + 1
    watchers = watchers + #listeners
  end
  return { state = value._state, subscribers = #value._listeners,
    watchers = watchers, watcherKeysSampled = keys, watcherLimitReached = keys >= 128 }
end
local function constructor(original, kind, reader)
  if wrapped[original] then return original end
  local fn = function(...)
    local object = original(...)
    inspect.register(kind, object, reader)
    if kind == "stores" then
      for _, name in ipairs({ "set", "update" }) do
        local method = object[name]
        object[name] = function(self, ...)
          local result = method(self, ...)
          inspect.storeUpdated(self)
          return result
        end
      end
    end
    if kind == "signals" then
      local emit = object.emit
      object.emit = function(self, name, ...)
        inspect.signal(self, name)
        return emit(self, name, ...)
      end
    end
    return object
  end
  wrapped[fn] = true
  return fn
end
local function instrument(name, module)
  if categories.ecs and name == "tuaecs" and type(module) == "table" and type(module.world) == "function" then
    module.world = constructor(module.world, "ecs", worldData)
  elseif categories.states and name == "tuastatemachine" and type(module) == "table" and type(module.create) == "function" then
    module.create = constructor(module.create, "states", stateData)
  elseif categories.signals and name == "tuasignal" and type(module) == "table" and type(module.new) == "function" then
    module.new = constructor(module.new, "signals", signalData)
  elseif categories.stores and name == "tuastore" and type(module) == "function" then
    module = constructor(module, "stores", storeData)
    package.loaded[name] = module
  elseif categories.pools and name == "tuaobjectpool" and type(module) == "function" then
    module = constructor(module, "pools", poolData)
    package.loaded[name] = module
  end
  return module
end
local installed = false
local function installLoop()
  if installed or not love or type(love.run) ~= "function" then return end
  installed = true
  local run = love.run
  love.run = function(...)
    local frame = run(...)
    if type(frame) ~= "function" then return frame end
    return function(...)
      local result = frame(...)
      tick()
      return result
    end
  end
end
_G.require = function(name)
  local module = instrument(name, originalRequire(name))
  if name == "main" then installLoop() end
  return module
end
return inspect

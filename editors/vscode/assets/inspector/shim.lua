-- Loaded exclusively by the editor's temporary development launcher.
local directory, port, token, options = ...
options = options or {}
local categories = options.categories or { ecs = true, states = true, signals = true, pools = true, stores = true, inputs = true, behaviors = true, audio = true }
local inspect = assert(loadfile(directory .. "/snapshot.lua"))()
inspect.configure(options)
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
local function inputData(player)
  local controls, pairs = {}, {}
  local count = 0
  for name, control in next, player._controls do
    if count >= 64 then controls["<remaining>"] = "Control limit (64)"; break end
    count = count + 1
    controls[name] = { value = control.value, raw = control.rawValue, down = control.down,
      sources = table.concat(control.sources, ", ") }
  end
  count = 0
  for name, pair in next, player._pairs do
    if count >= 64 then pairs["<remaining>"] = "Pair limit (64)"; break end
    count = count + 1
    pairs[name] = { x = pair.x, y = pair.y, rawX = pair.rawX, rawY = pair.rawY,
      down = pair.down, controls = table.concat(pair.controls, ", ") }
  end
  local pointer = player.config.pointer and { x = player._pointerX, y = player._pointerY,
    dx = player._pointerDX, dy = player._pointerDY } or nil
  return { device = player:getActiveDevice(), deadzone = player.config.deadzone,
    controls = controls, pairs = pairs, pointer = pointer }
end
local function behaviorData(runner)
  local active, visited, limited = {}, 0, false
  local function visit(node, path)
    if visited >= 256 then limited = true; return end
    visited = visited + 1
    if runner.states[node] then
      if #active < 64 then active[#active + 1] = path .. " · " .. node.kind
      else limited = true end
    end
    if node.children then
      for index, child in ipairs(node.children) do visit(child, path .. "." .. (index - 1)) end
    elseif node.child then visit(node.child, path .. ".0") end
  end
  visit(runner.definition.root, "0")
  return { active = active, steps = runner.steps, maxSteps = runner.maxSteps,
    maxDepth = runner.maxDepth, limited = limited }
end
local function audioData(system)
  local data = system:snapshot()
  local voices = {}
  for _, record in ipairs(system._voices) do
    if record.active then
      voices[#voices + 1] = { cue = record.cueName, bus = record.bus,
        priority = record.priority, volume = record.volume, pitch = record.pitch,
        paused = record.paused, stopping = record.stopping }
      if #voices >= 64 then break end
    end
  end
  data.voices = voices
  data.voicesLimited = data.activeVoices > 64
  return data
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
          inspect.storeUpdated(self, name)
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
    if kind == "pools" then
      local acquire, release = object.acquire, object.release
      object.acquire = function(self, ...)
        local value = acquire(self, ...)
        inspect.poolAcquired(self, self:activeCount())
        return value
      end
      object.release = function(self, ...)
        local released = release(self, ...)
        inspect.poolReleased(self, released)
        return released
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
  elseif categories.inputs and name == "baton" and type(module) == "table" and type(module.new) == "function" then
    module.new = constructor(module.new, "inputs", inputData)
  elseif categories.behaviors and name == "tuabehavior" and type(module) == "table" and type(module.new) == "function" and not wrapped[module.new] then
    local original = module.new
    module.new = function(...)
      local definition = original(...)
      if not wrapped[definition] then
        local instance = definition.instance
        definition.instance = function(self, ...)
          local runner = instance(self, ...)
          inspect.register("behaviors", runner, behaviorData)
          local update = runner.update
          runner.update = function(self, ...)
            local result = update(self, ...)
            inspect.behaviorUpdated(self, result)
            return result
          end
          return runner
        end
        wrapped[definition] = true
      end
      return definition
    end
    wrapped[module.new] = true
  elseif categories.audio and name == "tuaaudio" and type(module) == "function" then
    module = constructor(module, "audio", audioData)
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

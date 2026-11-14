-- Optional named game-boundary timings for a temporary development capture.
local clock = love.timer.getTime
local unpackValues = unpack or table.unpack
local MAX_ENTRIES, MAX_RECENT = 128, 32

local function origin(level)
  local info = debug.getinfo(level, "Sl")
  if not info or not info.source or info.currentline < 1 then return nil end
  local source = info.source:gsub("^@", "")
  return source .. ":" .. info.currentline
end

local function create(categories)
  local records, recent, count, dropped = {}, {}, 0, 0
  local originals = setmetatable({}, { __mode = "k" })
  local restore = {}
  local active = true
  local originalRequire = require
  local installed = {}

  local function record(kind, name, location, elapsed)
    if not active then return end
    name = tostring(name):sub(1, 120)
    local key = kind .. "\0" .. name .. "\0" .. (location or "")
    local item = records[key]
    if not item then
      if count == MAX_ENTRIES then dropped = dropped + 1; return end
      count = count + 1
      item = { kind = kind, name = name, location = location, calls = 0, totalMs = 0, worstMs = 0 }
      records[key] = item
    end
    item.calls = item.calls + 1
    item.totalMs = item.totalMs + elapsed
    item.worstMs = math.max(item.worstMs, elapsed)
    if #recent == MAX_RECENT then table.remove(recent, 1) end
    recent[#recent + 1] = { kind = kind, name = name, location = location, elapsedMs = elapsed }
  end

  local function wrapMethod(object, method, kind, name, location)
    local original = object[method]
    if type(original) ~= "function" then return end
    local replacement = function(self, ...)
      if not active then return original(self, ...) end
      local before = clock()
      local result = original(self, ...)
      record(kind, type(name) == "function" and name(...) or name, location, (clock() - before) * 1000)
      return result
    end
    object[method] = replacement
    local methods = originals[object] or {}
    methods[method] = { original, replacement }
    originals[object] = methods
  end

  local function replace(object, field, replacement)
    local original = object[field]
    object[field] = replacement
    restore[#restore + 1] = function()
      if object[field] == replacement then object[field] = original end
    end
  end

  local function instrument(name, module)
    if installed[name] then return module end
    installed[name] = true
    if name == "tuaecs" and categories.ecs and type(module) == "table" and type(module.system) == "function" then
      local constructor = module.system
      replace(module, "system", function(...)
        local location = origin(3)
        local system = constructor(...)
        wrapMethod(system, "update", "ecs", "system", location)
        return system
      end)
    elseif name == "tuabehavior" and categories.behaviors and type(module) == "table" and type(module.new) == "function" then
      local constructor = module.new
      replace(module, "new", function(...)
        local location = origin(3)
        local definition = constructor(...)
        local instance = definition.instance
        replace(definition, "instance", function(self, ...)
          local runner = instance(self, ...)
          wrapMethod(runner, "update", "behaviors", "tree", location)
          return runner
        end)
        return definition
      end)
    elseif name == "tuasignal" and categories.signals and type(module) == "table" and type(module.new) == "function" then
      local constructor = module.new
      replace(module, "new", function(...)
        local location = origin(3)
        local bus = constructor(...)
        wrapMethod(bus, "emit", "signals", function(event) return event end, location)
        return bus
      end)
    elseif name == "tuaaudio" and categories.audio and type(module) == "function" then
      local constructor = module
      local replacement = function(...)
        local location = origin(3)
        local system = constructor(...)
        wrapMethod(system, "update", "audio", "system update", location)
        return system
      end
      package.loaded[name] = replacement
      restore[#restore + 1] = function()
        if package.loaded[name] == replacement then package.loaded[name] = constructor end
      end
      module = replacement
    end
    return module
  end

  if categories.assets then
    for _, target in ipairs({
      { love.graphics, "newImage", "image" },
      { love.graphics, "newShader", "shader" },
      { love.audio, "newSource", "audio source" },
    }) do
      local owner, method, name = unpackValues(target)
      if owner and type(owner[method]) == "function" then
        local original = owner[method]
        replace(owner, method, function(...)
          local location = origin(3)
          local before = clock()
          local result = original(...)
          record("assets", name, location, (clock() - before) * 1000)
          return result
        end)
      end
    end
  end

  local wrappedRequire = function(name)
    return instrument(name, originalRequire(name))
  end
  _G.require = wrappedRequire

  return {
    snapshot = function()
      local values = {}
      for _, item in pairs(records) do values[#values + 1] = item end
      return values, recent, dropped
    end,
    stop = function()
      active = false
      if _G.require == wrappedRequire then _G.require = originalRequire end
      for object, methods in pairs(originals) do
        for method, pair in pairs(methods) do
          if object[method] == pair[2] then object[method] = pair[1] end
        end
      end
      for _, undo in ipairs(restore) do undo() end
    end,
  }
end

return create

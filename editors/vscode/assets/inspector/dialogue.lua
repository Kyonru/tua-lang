-- Development-only sampling. Never call snapshot(), resolvers or host handlers:
-- snapshots copy presentation and action data that this inspector must omit.
return function(inspect, wrapped)
  local function field(value, key)
    if type(value) == "table" then return rawget(value, key) end
  end

  local function scalar(value)
    local kind = type(value)
    if kind == "string" or kind == "boolean" then return value end
    if kind == "number" and value == value and value ~= math.huge and value ~= -math.huge then
      return value
    end
  end

  local function data(player)
    local document = field(player, "document")
    local current = scalar(field(player, "current"))
    local node = current and field(field(document, "nodes"), current)
    local state = scalar(field(player, "state"))
    local result = {
      state = state,
      nodeId = current,
      documentId = scalar(field(document, "documentId")),
      generation = scalar(field(player, "generation")),
      visit = scalar(field(player, "visit")),
      suspended = field(player, "suspended") == true,
      issueCode = scalar(field(field(player, "issue"), "code")),
    }
    if state == "waiting" then
      local duration = scalar(field(node, "duration"))
      local elapsed = scalar(field(player, "elapsed"))
      if type(duration) == "number" and type(elapsed) == "number" then
        result.wait = {
          duration = duration,
          elapsed = elapsed,
          remaining = math.max(0, duration - elapsed),
        }
      end
    elseif state == "awaiting_action" then
      result.action = {
        id = scalar(field(node, "id")),
        name = scalar(field(node, "name")),
        handleAttached = field(player, "pendingAction") ~= nil,
      }
    end
    return result
  end

  local function playerConstructor(original)
    if type(original) ~= "function" or wrapped[original] then return original end
    local wrapper = function(...)
      local player = original(...)
      inspect.register("dialogues", player, data)
      return player
    end
    wrapped[wrapper] = true
    return wrapper
  end

  return function(module)
    module.new = playerConstructor(module.new)
    for _, name in ipairs({ "context", "actions", "events" }) do
      local original = module[name]
      if type(original) == "function" and not wrapped[original] then
        local wrapper = function(...)
          local definition = original(...)
          definition.new = playerConstructor(definition.new)
          return definition
        end
        wrapped[wrapper] = true
        module[name] = wrapper
      end
    end
    return module
  end
end

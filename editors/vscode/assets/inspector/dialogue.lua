-- Development-only sampling. Never call snapshot(), resolvers or host handlers:
-- snapshots copy presentation and action data that this inspector must omit.
return function(inspect, wrapped, options)
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

  local histories = options and options.dialogueHistory == true
    and setmetatable({}, { __mode = "k" }) or nil
  local historyKeys = {
    "state",
    "nodeId",
    "generation",
    "visit",
    "suspended",
    "issueCode",
    "actionId",
    "actionName",
    "handleAttached",
  }

  local function addHistory(player, result)
    if not histories then return end
    local entry = histories[player]
    if not entry then
      entry = { records = {}, sequence = 0, head = 0 }
      histories[player] = entry
    end
    local record = {
      state = result.state,
      nodeId = result.nodeId,
      generation = result.generation,
      visit = result.visit,
      suspended = result.suspended,
      issueCode = result.issueCode,
      actionId = result.action and result.action.id,
      actionName = result.action and result.action.name,
      handleAttached = result.action and result.action.handleAttached,
    }
    local previous = entry.records[entry.head]
    local changed = previous == nil
    for _, key in ipairs(historyKeys) do
      local value = record[key]
      if type(value) == "string" and #value > 240 then
        record[key] = value:sub(1, 240)
        record.truncated = true
      end
      if previous and record[key] ~= previous[key] then changed = true end
    end
    if changed then
      entry.sequence = math.min(9007199254740991, entry.sequence + 1)
      entry.head = entry.head % 20 + 1
      record.sequence = entry.sequence
      entry.records[entry.head] = record
    end
    result.sampledChanges = entry.sequence
    result.history = {}
    for index = 1, #entry.records do
      local stored = entry.records[(entry.head - index) % 20 + 1]
      local copied = {}
      for key, value in next, stored do copied[key] = value end
      result.history[index] = copied
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
    addHistory(player, result)
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

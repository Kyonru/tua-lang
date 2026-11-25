-- Development-only diagnostics. Never call player snapshots or host handlers.
-- Only the captured built-in action-handle snapshot is read; it returns metadata.
return function(inspect, wrapped, options)
  local handleIds = setmetatable({}, { __mode = "k" })
  local nextHandleId = 0

  local function actionConstructor(original)
    if type(original) ~= "function" or wrapped[original] then return original end
    local wrapper = function(...)
      local handle = original(...)
      local snapshot = handle.snapshot
      nextHandleId = nextHandleId + 1
      local id = nextHandleId
      handleIds[handle] = id
      local counts = { ignoredResolve = 0, ignoredReject = 0, ignoredAfterCancel = 0 }
      -- Lua 5.1 weak keys are not ephemerons. A snapshot closure can reach the
      -- handle through a pending cancellation callback, so keep it weak here.
      local weakSnapshot = setmetatable({ snapshot }, { __mode = "v" })
      local function read(object)
        local readSnapshot = weakSnapshot[1]
        if not readSnapshot then return { unavailable = "Action metadata unavailable" } end
        local current = readSnapshot(object)
        return {
          actionHandle = true,
          handleId = id,
          status = current.status,
          cancellationFailed = current.cancellationError ~= nil,
          ignoredResolve = counts.ignoredResolve,
          ignoredReject = counts.ignoredReject,
          ignoredAfterCancel = counts.ignoredAfterCancel,
        }
      end
      for _, method in ipairs({ "resolve", "reject" }) do
        local settle = handle[method]
        local counter = method == "resolve" and "ignoredResolve" or "ignoredReject"
        handle[method] = function(self, ...)
          local changed = settle(self, ...)
          if changed == false then
            counts[counter] = math.min(9007199254740991, counts[counter] + 1)
            if snapshot(self).status == "cancelled" then
              counts.ignoredAfterCancel = math.min(9007199254740991, counts.ignoredAfterCancel + 1)
            end
          end
          return changed
        end
      end
      inspect.register("dialogues", handle, read, "Dialogue action #" .. id)
      return handle
    end
    wrapped[wrapper] = true
    return wrapper
  end

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
    "handleId",
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
      handleId = result.action and result.action.handleId,
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

  -- Presentation metadata belongs to the resolver. Never resolve again while sampling.
  local function localization(player)
    local shown = field(player, "presentation")
    local locale = field(player, "locale")
    if type(shown) ~= "table" or type(locale) ~= "string" then return nil end
    local templateResolver = field(player, "resolveTemplate")
    local builtin = field(field(field(package, "loaded"), "tuai18n"), "dialogueTemplate")
    local standard = type(builtin) == "function" and templateResolver == builtin
    local result = { requestedLocale = locale, fields = {} }
    local function add(label, key, view)
      if #result.fields == 64 then
        result.truncated = true
        return
      end
      local resolved = field(view, "resolvedLocale")
      local status = "unknown"
      if type(resolved) == "string" then
        status = resolved == locale and "requested" or "locale_fallback"
      elseif type(view) == "table" and standard and type(key) == "string" then
        status = "source_fallback"
      elseif templateResolver == nil and field(player, "resolve") == nil then
        status = "unlocalized"
      end
      result.fields[#result.fields + 1] = {
        field = label,
        textId = scalar(key),
        status = status,
        resolvedLocale = scalar(resolved),
        variant = scalar(field(view, "variant")),
      }
    end
    if field(shown, "kind") == "line" then
      add("line", field(shown, "textId"), field(shown, "presentation"))
    end
    local choices = field(shown, "options")
    for index = 1, 65 do
      local choice = field(choices, index)
      if choice == nil then break end
      add("choice " .. index, field(choice, "id"), field(choice, "presentation"))
      if field(choice, "reasonId") ~= nil then
        add("reason " .. index, field(choice, "reasonId"), field(choice, "reasonPresentation"))
      end
      if result.truncated then break end
    end
    return result
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
        handleId = handleIds[field(player, "pendingAction")],
      }
    end
    result.localization = localization(player)
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
    module.actionHandle = actionConstructor(module.actionHandle)
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

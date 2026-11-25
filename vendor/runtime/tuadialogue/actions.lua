-- Private host-action support. Dispatch and settlement require explicit host calls.
local Actions = {}
local definitions = setmetatable({}, { __mode = "k" })

local function copy(value)
  if type(value) ~= "table" then return value end
  local result = {}
  for key, child in pairs(value) do result[key] = copy(child) end
  return result
end

-- Handles never retain a player. A claimed handle belongs to one action visit.
-- Both sides are weak: Lua 5.1 lacks ephemeron semantics for weak-key values
-- whose cancellation callbacks capture their own handle.
local handles = setmetatable({}, { __mode = "kv" })

function Actions.handle(options)
  if options == nil then options = {} end
  assert(type(options) == "table" and getmetatable(options) == nil, "invalid action handle options")
  for key in pairs(options) do assert(key == "cancel", "unknown action handle option") end
  assert(options.cancel == nil or type(options.cancel) == "function", "cancel must be a function")
  local data = { status = "pending", cancel = options.cancel, claimed = false }
  local methods = {}
  local handle = setmetatable({}, {
    __index = methods,
    __metatable = "dialogue action handle",
  })
  local function settle(status, value)
    if data.status ~= "pending" then return false end
    data.status, data.value, data.cancel = status, value, nil
    return true
  end
  function methods:resolve(result)
    assert(type(result) == "boolean", "action result must be boolean")
    return settle("resolved", result)
  end
  function methods:reject(message)
    assert(type(message) == "string" and #message > 0 and #message <= 4096,
      "action failure needs a message of 1 through 4096 bytes")
    return settle("rejected", message)
  end
  function methods:cancel()
    if data.status ~= "pending" then return false end
    local cancel = data.cancel
    data.status, data.cancel = "cancelled", nil
    -- Invalidate first: reentrant or late completion cannot win cancellation.
    if cancel then
      local ok, message = pcall(cancel)
      if not ok then
        data.cancellationError = type(message) == "string" and message:sub(1, 4096)
          or "Action cancellation failed"
      end
    end
    return true
  end
  function methods:snapshot()
    return { status = data.status, cancellationError = data.cancellationError }
  end
  data.cancelHandle = methods.cancel
  handles[handle] = data
  return handle
end

local function abandon(handle)
  local data = handles[handle]
  if data then data.cancelHandle(handle) end
end

-- Detach before running any user cancellation callback.
function Actions.release(player)
  local handle = player.pendingAction
  player.pendingAction = nil
  if handle then abandon(handle) end
end

local function claim(handle)
  local data = handles[handle]
  if not data or data.claimed then return false end
  data.claimed = true
  return true
end

function Actions.define(specs)
  assert(
    type(specs) == "table" and getmetatable(specs) == nil,
    "action definitions must be a plain record"
  )
  local Shape = require("tuasave_shape")
  local owned, payloads, effects, count = {}, {}, {}, 0
  for name, spec in pairs(specs) do
    assert(
      type(name) == "string" and name:match("^[A-Za-z_][A-Za-z0-9_.-]*$"),
      "invalid action name"
    )
    assert(
      type(spec) == "table" and getmetatable(spec) == nil,
      "action needs a payload shape and effect"
    )
    for key in pairs(spec) do
      assert(key == "payload" or key == "effect", "unknown action definition field")
    end
    assert(
      spec.effect == "presentation" or spec.effect == "consequential",
      "declare an action effect"
    )
    local validate, defaults = Shape.get(spec.payload)
    assert(type(defaults) == "table", "action payload shapes must describe tables")
    owned[name] = { validate = validate, effect = spec.effect }
    payloads[name], effects[name] = copy(defaults), spec.effect
    count = count + 1
  end
  assert(count >= 1 and count <= 128, "declare between 1 and 128 actions")
  local definition = { kind = "actions", payloads = payloads, effects = effects }
  definitions[definition] = owned
  return definition
end

function Actions.valid(definition)
  return definitions[definition] ~= nil
end

function Actions.compile(definition, node, documentId)
  local specs = definitions[definition]
  assert(specs, "document needs a Dialogue.actions definition")
  local spec = specs[node.name]
  assert(spec, "unknown declared dialogue action: " .. node.name)
  local ok, payload = pcall(spec.validate, node.payload)
  if not ok then
    error("dialogue action " .. documentId .. "/" .. node.id .. ": " .. tostring(payload), 0)
  end
  return {
    kind = "action",
    name = node.name,
    id = node.id,
    payload = payload,
    effect = spec.effect,
    success = node.success,
    failure = node.failure,
  }
end

function Actions.handlers(definition, handlers)
  local specs = definitions[definition]
  assert(specs, "document needs a Dialogue.actions definition")
  if handlers == nil then handlers = {} end
  assert(
    type(handlers) == "table" and getmetatable(handlers) == nil,
    "handlers must be a plain record"
  )
  local owned = {}
  for name, handler in pairs(handlers) do
    assert(specs[name], "handler has no action declaration")
    assert(type(handler) == "function", "action handler must be a function")
    owned[name] = handler
  end
  return owned
end

function Actions.request(player)
  local node = player.document.nodes[player.current]
  return {
    status = player.pendingAction and handles[player.pendingAction].status or "ready",
    actionId = node.id,
    name = node.name,
    effect = node.effect,
    source = copy(node.source),
    documentId = player.document.documentId,
    nodeId = player.current,
    generation = player.generation,
    visit = player.visit,
  }
end

local function failure(player, request, code, message)
  local issue = copy(request)
  issue.code, issue.message = code, message
  player.state, player.presentation, player.issue = "faulted", nil, issue
  player.fired = {}
  return false
end

local function transition(player, request, node, result)
  local entered, message = pcall(player.enter, player, result and node.success or node.failure)
  if not entered then
    return failure(
      player,
      request,
      "action_transition_error",
      type(message) == "string" and message or "Action transition failed"
    )
  end
  if player.issue then
    player.issue.actionId, player.issue.name = request.actionId, request.name
    player.issue.documentId, player.issue.effect = request.documentId, request.effect
    player.issue.source = copy(request.source)
  end
  return player.state ~= "faulted"
end

function Actions.run(player)
  if player.executingAction or player.state ~= "awaiting_action" then return false end
  local request = Actions.request(player)
  local node = player.document.nodes[player.current]
  if player.pendingAction then
    local data = handles[player.pendingAction]
    if data.status == "pending" then return false end
    player.pendingAction = nil
    if data.status == "resolved" then return transition(player, request, node, data.value) end
    return failure(player, request,
      data.status == "cancelled" and "action_cancelled" or "action_rejected",
      data.status == "cancelled" and "Action was cancelled" or data.value)
  end
  local handler = player.actionHandlers[node.name]
  if not handler then
    return failure(player, request, "action_handler_missing",
      "No handler registered for action " .. node.name)
  end
  player.executingAction = true
  local ok, result = pcall(handler, copy(node.payload), copy(request))
  player.executingAction = false
  -- A reentrant lifecycle operation wins, including an error after cancellation.
  if player.generation ~= request.generation or player.visit ~= request.visit
    or player.current ~= request.nodeId or player.state ~= "awaiting_action" then
    if ok and type(result) == "table" and claim(result) then abandon(result) end
    return false
  end
  if not ok then
    return failure(player, request, "action_handler_error",
      type(result) == "string" and result or "Action handler failed")
  end
  if player.suspended and type(result) == "boolean" then
    local deferred = Actions.handle()
    deferred:resolve(result)
    result = deferred
  end
  if type(result) == "boolean" then return transition(player, request, node, result) end
  if type(result) == "table" and claim(result) then
    player.pendingAction = result
    return true
  end
  return failure(player, request, "action_result_invalid",
    "Action handler must return a boolean or a fresh Dialogue.actionHandle")
end

return Actions

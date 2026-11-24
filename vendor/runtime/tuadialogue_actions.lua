-- Private synchronous host-action support. No handlers run during traversal.
local Actions = {}
local definitions = setmetatable({}, { __mode = "k" })

local function copy(value)
  if type(value) ~= "table" then return value end
  local result = {}
  for key, child in pairs(value) do result[key] = copy(child) end
  return result
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

function Actions.run(player)
  if player.executingAction or player.state ~= "awaiting_action" then return false end
  local request = Actions.request(player)
  local node = player.document.nodes[player.current]
  local handler = player.actionHandlers[node.name]
  if not handler then
    return failure(
      player,
      request,
      "action_handler_missing",
      "No handler registered for action " .. node.name
    )
  end
  player.executingAction = true
  local ok, result = pcall(handler, copy(node.payload), copy(request))
  player.executingAction = false
  -- A reentrant lifecycle operation wins, including an error after cancellation.
  if player.generation ~= request.generation or player.visit ~= request.visit
    or player.current ~= request.nodeId or player.state ~= "awaiting_action" then
    return false
  end
  if not ok then
    return failure(
      player,
      request,
      "action_handler_error",
      type(result) == "string" and result or "Action handler failed"
    )
  end
  if type(result) ~= "boolean" then
    return failure(
      player,
      request,
      "action_result_invalid",
      "Action handler must return true or false"
    )
  end
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

return Actions

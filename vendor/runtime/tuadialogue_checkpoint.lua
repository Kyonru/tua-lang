-- Bounded, headless checkpoint data. Host storage and migration stay external.
local Checkpoint = {}
local function finite(value)
  return type(value) == "number" and value == value and value >= 0 and value <= 1e12
end
local function record(value, allowed)
  assert(type(value) == "table" and getmetatable(value) == nil, "checkpoint needs plain records")
  for key in pairs(value) do assert(allowed[key], "unknown checkpoint field") end
end
local function text(value, limit)
  return type(value) == "string" and #value > 0 and #value <= limit
    and require("tuasave_json").validUtf8(value)
end
-- Collision-free canonical encoding, not a hash. Length prefixes protect IDs.
local function encode(value, output, budget, seen, depth)
  budget.entries = budget.entries + 1
  assert(budget.entries <= 16384 and depth <= 32, "checkpoint structure limit exceeded")
  local kind = type(value)
  if kind == "table" then
    assert(getmetatable(value) == nil and not seen[value], "invalid checkpoint structure")
    seen[value] = true
    local keys, keyBytes = {}, 0
    for key in pairs(value) do
      assert(
        type(key) == "string" or (type(key) == "number" and finite(key)),
        "invalid checkpoint key"
      )
      if type(key) == "string" then
        keyBytes = keyBytes + #key
        assert(keyBytes <= 262144, "checkpoint key limit exceeded")
      end
      keys[#keys + 1] = key
      assert(#keys <= 16384, "checkpoint structure limit exceeded")
    end
    table.sort(keys, function(a, b)
      if type(a) ~= type(b) then return type(a) == "number" end
      if type(a) == "number" then return a < b end
      -- Lua string ordering may use the process collation locale. Compare bytes.
      for index = 1, math.min(#a, #b) do
        local left, right = a:byte(index), b:byte(index)
        if left ~= right then return left < right end
      end
      return #a < #b
    end)
    output[#output + 1] = "{"
    for _, key in ipairs(keys) do
      encode(key, output, budget, seen, depth + 1)
      encode(value[key], output, budget, seen, depth + 1)
    end
    output[#output + 1] = "}"
    seen[value] = nil
  else
    assert(kind == "string" or kind == "boolean" or kind == "number", "non-JSON checkpoint value")
    if kind == "string" then
      budget.bytes = budget.bytes + #value
      assert(budget.bytes <= 262144, "checkpoint string limit exceeded")
      assert(require("tuasave_json").validUtf8(value), "checkpoint strings must be UTF-8")
    elseif kind == "number" then
      assert(value == value and value ~= math.huge and value ~= -math.huge,
        "invalid checkpoint number")
    end
    assert(budget.bytes <= 262144, "checkpoint string limit exceeded")
    local content = tostring(value)
    if kind == "number" then
      content = value == 0 and "0" or (string.format("%.17g", value):gsub(",", "."))
    end
    output[#output + 1] = kind:sub(1, 1) .. #content .. ":" .. content
  end
end
local function signature(player)
  assert(
    player.checkpointNoEffects and (player.checkpointPlain or player.copyLocals),
    "checkpoints cannot contain action or event registries"
  )
  assert(text(player.document.documentId, 256), "checkpoint needs a documentId")
  assert(text(player.checkpointVersion, 256), "checkpoint needs checkpointVersion")
  local graph = {
    entry = player.document.entry,
    schema = player.document.schemaVersion or 0,
    nodes = {},
  }
  if player.copyLocals then
    graph.bindings = player.bindings
    graph.localDefaults = player.localDefaults
  end
  local work = 0
  local function take()
    work = work + 1
    assert(work <= 8192, "checkpoint graph work limit exceeded")
  end
  for id, node in pairs(player.document.nodes) do
    take()
    assert(node.kind == "line" or node.kind == "choice" or node.kind == "wait" or node.kind == "end"
      or (player.copyLocals and node.kind == "branch"),
      "checkpoint cannot contain actions or plain-dialogue branches")
    local row = {
      kind = node.kind,
      next = node.next,
      fallback = node.fallback,
      duration = node.duration,
      textId = node.textId,
      events = {},
      options = {},
    }
    if player.copyLocals then
      row.condition, row.yes, row.no = node.condition, node.yes, node.no
    end
    for _, event in ipairs(node.events or {}) do
      take()
      row.events[#row.events + 1] = {
        id = event.id,
        name = event.name,
        anchor = event.anchor,
        delay = event.delay,
        payload = event.payload,
      }
    end
    for _, option in ipairs(node.options or {}) do
      take()
      assert(player.copyLocals or (not option.visibleCondition and not option.enabledCondition),
        "checkpoint v1 cannot contain conditional choices")
      local choice = { id = option.id, target = option.target }
      if player.copyLocals then
        choice.visible, choice.enabled = option.visibleCondition, option.enabledCondition
      end
      row.options[#row.options + 1] = choice
    end
    graph.nodes[id] = row
  end
  local output = {}
  encode(graph, output, { entries = 0, bytes = 0 }, {}, 0)
  local result = table.concat(output)
  assert(#result <= 524288, "checkpoint graph limit exceeded")
  return result
end
local fields = {
  version = true,
  documentId = true,
  contentVersion = true,
  graph = true,
  nodeId = true,
  state = true,
  elapsed = true,
  revealedElapsed = true,
  revealedAt = true,
  suspended = true,
  locale = true,
  fired = true,
  locals = true,
}
local states = { line = "awaiting_advance", choice = "awaiting_choice", wait = "waiting" }
function Checkpoint.validate(player, value)
  record(value, fields)
  assert(value.version == (player.copyLocals and 2 or 1), "unsupported checkpoint version")
  local locals
  if player.copyLocals then
    -- Bound raw data before the schema copier or any resolver touches it.
    encode(value.locals, {}, { entries = 0, bytes = 0 }, {}, 0)
    locals = player.copyLocals(value.locals)
  else
    assert(value.locals == nil, "plain checkpoint cannot contain locals")
  end
  local graph = signature(player)
  assert(value.documentId == player.document.documentId, "checkpoint document mismatch")
  assert(value.contentVersion == player.checkpointVersion, "checkpoint content version mismatch")
  assert(value.graph == graph, "checkpoint graph changed; migrate explicitly")
  assert(text(value.nodeId, 1024), "invalid checkpoint position")
  local node = player.document.nodes[value.nodeId]
  assert(
    node and states[node.kind] and value.state == states[node.kind],
    "invalid checkpoint boundary"
  )
  assert(type(value.suspended) == "boolean", "invalid checkpoint suspension")
  assert(value.locale == nil or text(value.locale, 256), "invalid checkpoint locale")
  assert(not (player.resolve or player.resolveTemplate) or value.locale ~= nil,
    "checkpoint locale required by resolver")
  assert(finite(value.elapsed), "invalid checkpoint timer")
  assert((value.revealedAt == nil) == (value.revealedElapsed == nil), "incomplete reveal timers")
  if value.revealedAt ~= nil then
    assert(node.kind == "line" and finite(value.revealedAt) and finite(value.revealedElapsed)
      and value.revealedAt <= value.elapsed and value.revealedElapsed <= value.elapsed,
      "invalid checkpoint reveal timers")
  end
  if node.kind == "wait" then assert(value.elapsed <= node.duration, "invalid wait timer") end
  if node.kind == "choice" then assert(value.elapsed == 0, "invalid choice timer") end
  local fired, count = {}, 0
  assert(
    type(value.fired) == "table" and getmetatable(value.fired) == nil,
    "invalid checkpoint events"
  )
  for key, index in pairs(value.fired) do
    count = count + 1
    assert(count <= 4096 and type(key) == "number" and key % 1 == 0
      and key >= 1 and key <= #value.fired, "checkpoint events must be a bounded array")
    assert(type(index) == "number" and index % 1 == 0 and not fired[index], "invalid fired index")
    local event = (node.events or {})[index]
    assert(event, "missing checkpoint event")
    local elapsed = event.anchor == "shown" and value.elapsed or value.revealedElapsed
    assert(elapsed ~= nil and elapsed >= event.delay, "checkpoint event was not due")
    fired[index] = true
  end
  assert(count == #value.fired, "checkpoint events must be dense")
  return fired, locals
end
function Checkpoint.export(player)
  local node = player.document.nodes[player.current or ""]
  assert(node and states[node.kind] == player.state, "checkpoint requires a line, choice or wait")
  local fired = {}
  for index in pairs(player.fired or {}) do fired[#fired + 1] = index end
  table.sort(fired)
  local value = {
    version = player.copyLocals and 2 or 1,
    documentId = player.document.documentId,
    contentVersion = player.checkpointVersion,
    graph = signature(player),
    nodeId = player.current,
    state = player.state,
    elapsed = player.elapsed,
    revealedElapsed = player.revealedElapsed,
    revealedAt = player.revealedAt,
    suspended = player.suspended == true,
    locale = player.locale,
    fired = fired,
    locals = player.copyLocals and player.locals or nil,
  }
  local _, locals = Checkpoint.validate(player, value)
  value.locals = locals
  return value
end
return Checkpoint

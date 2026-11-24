-- Renderer-independent dialogue playback. No graphics, input or global clock.
local Dialogue = { schemaVersion = 1 }
local Player = {}
Player.__index = Player

local function copy(value)
  if type(value) ~= "table" then return value end
  local result = {}
  for key, child in pairs(value) do result[key] = copy(child) end
  return result
end

local function seconds(value)
  return type(value) == "number" and value >= 0 and value < math.huge
end

local function events(value)
  if value == nil then return {} end
  assert(type(value) == "table" and #value <= 64, "a line supports at most 64 events")
  local count, result = 0, {}
  for key in pairs(value) do
    assert(type(key) == "number" and key % 1 == 0 and key >= 1 and key <= #value,
      "events must be a dense array")
    count = count + 1
  end
  assert(count == #value, "events must be a dense array")
  for index, event in ipairs(value) do
    assert(type(event) == "table" and (event.anchor == "shown" or event.anchor == "revealed"),
      "invalid event anchor")
    assert(seconds(event.delay), "invalid event delay")
    assert(type(event.name) == "string" and event.name ~= "" and type(event.payload) == "string",
      "invalid dialogue event")
    result[index] = {
      anchor = event.anchor,
      delay = event.delay,
      name = event.name,
      payload = event.payload,
    }
  end
  return result
end

local function fields(value, allowed)
  for key in pairs(value) do assert(allowed[key], "unknown dialogue field: " .. tostring(key)) end
end

local function optionalId(value)
  return value == nil or (type(value) == "string" and value ~= "")
end

-- Validate and own the document before any session can execute it.
local function document(value)
  assert(type(value) == "table" and type(value.nodes) == "table", "dialogue needs nodes")
  assert(type(value.entry) == "string", "dialogue needs an entry ID")
  assert(
    value.schemaVersion == nil or value.schemaVersion == 1,
    "unsupported dialogue schemaVersion"
  )
  assert(optionalId(value.documentId), "invalid documentId")
  if value.schemaVersion then
    fields(value, { schemaVersion = true, documentId = true, entry = true, nodes = true })
  end
  local nodes = {}
  for id, node in pairs(value.nodes) do
    assert(type(id) == "string" and id ~= "", "node IDs must be nonempty strings")
    assert(type(node) == "table", "dialogue node must be a record")
    assert(
      node.kind == "line" or node.kind == "choice" or node.kind == "end",
      "unknown dialogue node kind"
    )
    if value.schemaVersion then
      local allowed = { kind = true }
      if node.kind == "line" then
        allowed = {
          kind = true,
          text = true,
          textId = true,
          speaker = true,
          speakerKey = true,
          portrait = true,
          next = true,
          hold = true,
          fade = true,
          events = true,
        }
      elseif node.kind == "choice" then allowed.options = true end
      fields(node, allowed)
    end
    local owned = { kind = node.kind }
    if node.kind == "line" then
      assert(type(node.text) == "string", "line needs text")
      assert(node.speaker == nil or type(node.speaker) == "string", "invalid speaker")
      assert(type(node.next) == "string", "line needs a next node")
      assert(node.portrait == nil or type(node.portrait) == "string", "invalid portrait")
      assert(node.hold == nil or seconds(node.hold), "invalid hold duration")
      assert(node.fade == nil or seconds(node.fade), "invalid fade duration")
      owned.text, owned.speaker, owned.next = node.text, node.speaker, node.next
      owned.portrait, owned.hold, owned.fade = node.portrait, node.hold, node.fade
      assert(optionalId(node.textId) and optionalId(node.speakerKey), "invalid localization ID")
      owned.textId, owned.speakerKey = node.textId, node.speakerKey
      owned.events = events(node.events)
      if value.schemaVersion then
        for _, event in ipairs(node.events or {}) do
          fields(event, { anchor = true, delay = true, name = true, payload = true })
        end
      end
    elseif node.kind == "choice" then
      assert(type(node.options) == "table" and #node.options > 0, "choice needs options")
      owned.options = {}
      local seen = {}
      local count = 0
      for index in pairs(node.options) do
        assert(type(index) == "number" and index % 1 == 0 and index >= 1 and index <= #node.options,
          "choice options must be a dense array")
        count = count + 1
      end
      assert(count == #node.options, "choice options must be a dense array")
      for index, option in ipairs(node.options) do
        assert(
          type(option) == "table" and type(option.id) == "string" and option.id ~= "",
          "choice needs an ID"
        )
        assert(not seen[option.id], "duplicate choice ID")
        assert(type(option.text) == "string" and type(option.target) == "string", "invalid choice")
        if value.schemaVersion then fields(option, { id = true, text = true, target = true }) end
        seen[option.id] = true
        owned.options[index] = { id = option.id, text = option.text, target = option.target }
      end
    end
    nodes[id] = owned
  end
  assert(nodes[value.entry], "missing entry node")
  for _, node in pairs(nodes) do
    if node.next then assert(nodes[node.next], "missing next node") end
    for _, option in ipairs(node.options or {}) do
      assert(nodes[option.target], "missing choice target")
    end
  end
  return {
    entry = value.entry,
    nodes = nodes,
    documentId = value.documentId,
    schemaVersion = value.schemaVersion,
  }
end

local function arguments(values)
  assert(type(values) == "table", "dialogue args must be a record")
  local result = {}
  for key, value in pairs(values) do
    assert(type(key) == "string", "dialogue argument names must be strings")
    assert(type(value) == "string" or type(value) == "boolean"
      or (type(value) == "number" and value == value and math.abs(value) < math.huge),
      "dialogue arguments must be finite scalar values")
    result[key] = value
  end
  return result
end

function Dialogue.new(source, options)
  if options == nil then options = {} end
  assert(type(options) == "table", "invalid dialogue options")
  fields(options, { locale = true, resolve = true, args = true })
  assert(optionalId(options.locale), "invalid dialogue locale")
  assert(options.resolve == nil or type(options.resolve) == "function", "invalid dialogue resolver")
  assert(not options.resolve or options.locale, "dialogue resolver needs an explicit locale")
  return setmetatable({
    document = document(source),
    state = "idle",
    generation = 0,
    visit = 0,
    locale = options.locale,
    resolve = options.resolve,
    args = arguments(options.args == nil and {} or options.args),
    presentationRevision = 0,
    revealComplete = false,
  }, Player)
end

-- Compute presentation before committing a transition; a bad resolver cannot
-- partially advance the visit or overwrite clocks. Snapshot never calls host code.
local function presentation(self, id, locale)
  local node = copy(self.document.nodes[id])
  node.text, node.options = node.text or "", node.options or {}
  if self.resolve then
    local function translate(key, source)
      self.resolving = true
      local ok, value = pcall(self.resolve, key, source, copy(self.args), locale)
      self.resolving = false
      if not ok then error(value, 0) end
      assert(type(value) == "string", "dialogue resolver must return text")
      return value
    end
    if node.textId then node.text = translate(node.textId, node.text) end
    if node.speakerKey then node.speaker = translate(node.speakerKey, node.speaker or "") end
    for _, option in ipairs(node.options) do option.text = translate(option.id, option.text) end
  end
  return node
end

function Player:setLocale(locale)
  assert(not self.resolving, "cannot mutate dialogue while resolving text")
  assert(self.state ~= "disposed", "dialogue is disposed")
  assert(type(locale) == "string" and locale ~= "", "invalid dialogue locale")
  if locale == self.locale then return false end
  local node = self.current and presentation(self, self.current, locale)
  self.locale, self.presentation = locale, node
  self.presentationRevision = self.presentationRevision + 1
  self.revealComplete = self.state == "awaiting_advance"
  if self.revealComplete and self.revealedElapsed == nil then
    self.revealedElapsed, self.revealedAt = 0, self.elapsed
  end
  return true
end

function Player:enter(id)
  assert(not self.resolving, "cannot mutate dialogue while resolving text")
  local nodePresentation = presentation(self, id, self.locale)
  self.presentation, self.revealComplete = nodePresentation, false
  self.presentationRevision = self.presentationRevision + 1
  self.visit = self.visit + 1
  self.elapsed, self.revealedElapsed, self.revealedAt, self.fired = 0, nil, nil, {}
  self.current = id
  local node = self.document.nodes[id]
  if node.kind == "end" then self.state = "completed"
  elseif node.kind == "line" then self.state = "awaiting_advance"
  else self.state = "awaiting_choice" end
end

function Player:start(entry)
  assert(not self.resolving, "cannot mutate dialogue while resolving text")
  assert(self.state ~= "disposed", "dialogue is disposed")
  assert(entry == nil or type(entry) == "string", "entry ID must be a string")
  entry = entry or self.document.entry
  assert(self.document.nodes[entry], "missing entry node")
  self:enter(entry)
  self.generation = self.generation + 1
end

-- The host reports reveal completion; this runtime never reads a wall clock.
-- The first true sample anchors revealed events at elapsed zero, conservatively
-- excluding the update interval in which presentation finished revealing.
function Player:update(dt, revealed)
  assert(not self.resolving, "cannot mutate dialogue while resolving text")
  assert(seconds(dt) and type(revealed) == "boolean", "invalid dialogue update")
  if self.state ~= "awaiting_advance" then return {} end
  local node = self.document.nodes[self.current]
  self.elapsed = math.min(1.7976931348623157e308, self.elapsed + dt)
  if self.revealedElapsed ~= nil then
    self.revealedElapsed = math.min(1.7976931348623157e308, self.revealedElapsed + dt)
  elseif revealed then
    self.revealedElapsed, self.revealedAt = 0, self.elapsed
  end
  local due = {}
  for index, event in ipairs(node.events) do
    local elapsed = event.anchor == "shown" and self.elapsed or self.revealedElapsed
    if not self.fired[index] and elapsed ~= nil and elapsed >= event.delay then
      self.fired[index] = true
      due[#due + 1] = {
        index = index,
        at = event.delay + (event.anchor == "shown" and 0 or self.revealedAt),
      }
    end
  end
  table.sort(due, function(a, b)
    if a.at == b.at then return a.index < b.index end
    return a.at < b.at
  end)
  local result = {}
  for _, item in ipairs(due) do
    local event = node.events[item.index]
    result[#result + 1] = {
      name = event.name,
      payload = event.payload,
      generation = self.generation,
      visit = self.visit,
      nodeId = self.current,
    }
  end
  return result
end

function Player:advance()
  assert(not self.resolving, "cannot mutate dialogue while resolving text")
  if self.state ~= "awaiting_advance" then return false end
  self:enter(self.document.nodes[self.current].next)
  return true
end

function Player:choose(id)
  assert(not self.resolving, "cannot mutate dialogue while resolving text")
  if self.state ~= "awaiting_choice" then return false end
  for _, option in ipairs(self.document.nodes[self.current].options) do
    if option.id == id then
      self:enter(option.target)
      return true
    end
  end
  return false
end

function Player:cancel()
  assert(not self.resolving, "cannot mutate dialogue while resolving text")
  if self.state == "disposed" or self.state == "cancelled" or self.state == "completed" then
    return false
  end
  self.generation = self.generation + 1
  self.state, self.current, self.presentation = "cancelled", nil, nil
  self.fired, self.revealedElapsed, self.revealedAt = nil, nil, nil
  return true
end

function Player:snapshot()
  local presentation = copy(self.presentation)
  return {
    state = self.state,
    generation = self.generation,
    visit = self.visit,
    nodeId = self.current,
    node = presentation,
    locale = self.locale,
    documentId = self.document and self.document.documentId,
    presentationRevision = self.presentationRevision,
    revealComplete = self.revealComplete,
  }
end

function Player:dispose()
  assert(not self.resolving, "cannot mutate dialogue while resolving text")
  if self.state == "disposed" then return end
  self.generation = self.generation + 1
  self.state, self.current, self.document = "disposed", nil, nil
  self.presentation, self.resolve, self.args, self.locale = nil, nil, nil, nil
  self.fired, self.revealedElapsed, self.revealedAt = nil, nil, nil
end

return Dialogue

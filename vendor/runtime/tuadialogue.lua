-- Renderer-independent dialogue playback. No graphics, input or global clock.
local Dialogue = {}
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

-- Validate and own the document before any session can execute it.
local function document(value)
  assert(type(value) == "table" and type(value.nodes) == "table", "dialogue needs nodes")
  assert(type(value.entry) == "string", "dialogue needs an entry ID")
  local nodes = {}
  for id, node in pairs(value.nodes) do
    assert(type(id) == "string" and id ~= "", "node IDs must be nonempty strings")
    assert(type(node) == "table", "dialogue node must be a record")
    assert(
      node.kind == "line" or node.kind == "choice" or node.kind == "end",
      "unknown dialogue node kind"
    )
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
      owned.events = events(node.events)
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
  return { entry = value.entry, nodes = nodes }
end

function Dialogue.new(source)
  return setmetatable(
    { document = document(source), state = "idle", generation = 0, visit = 0 },
    Player
  )
end

function Player:enter(id)
  self.visit = self.visit + 1
  self.elapsed, self.revealedElapsed, self.revealedAt, self.fired = 0, nil, nil, {}
  self.current = id
  local node = self.document.nodes[id]
  if node.kind == "end" then self.state = "completed"
  elseif node.kind == "line" then self.state = "awaiting_advance"
  else self.state = "awaiting_choice" end
end

function Player:start(entry)
  assert(self.state ~= "disposed", "dialogue is disposed")
  assert(entry == nil or type(entry) == "string", "entry ID must be a string")
  entry = entry or self.document.entry
  assert(self.document.nodes[entry], "missing entry node")
  self.generation = self.generation + 1
  self:enter(entry)
end

-- The host reports reveal completion; this runtime never reads a wall clock.
-- The first true sample anchors revealed events at elapsed zero, conservatively
-- excluding the update interval in which presentation finished revealing.
function Player:update(dt, revealed)
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
  if self.state ~= "awaiting_advance" then return false end
  self:enter(self.document.nodes[self.current].next)
  return true
end

function Player:choose(id)
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
  if self.state == "disposed" or self.state == "cancelled" or self.state == "completed" then
    return false
  end
  self.generation = self.generation + 1
  self.state, self.current = "cancelled", nil
  self.fired, self.revealedElapsed, self.revealedAt = nil, nil, nil
  return true
end

function Player:snapshot()
  local node = self.current and self.document and self.document.nodes[self.current]
  local presentation = copy(node)
  if presentation then
    presentation.text = presentation.text or ""
    presentation.options = presentation.options or {}
  end
  return {
    state = self.state,
    generation = self.generation,
    visit = self.visit,
    nodeId = self.current,
    node = presentation,
  }
end

function Player:dispose()
  if self.state == "disposed" then return end
  self.generation = self.generation + 1
  self.state, self.current, self.document = "disposed", nil, nil
  self.fired, self.revealedElapsed, self.revealedAt = nil, nil, nil
end

return Dialogue

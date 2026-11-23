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
      owned.text, owned.speaker, owned.next = node.text, node.speaker, node.next
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
  return setmetatable({ document = document(source), state = "idle", generation = 0 }, Player)
end

function Player:enter(id)
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
    nodeId = self.current,
    node = presentation,
  }
end

function Player:dispose()
  if self.state == "disposed" then return end
  self.generation = self.generation + 1
  self.state, self.current, self.document = "disposed", nil, nil
end

return Dialogue

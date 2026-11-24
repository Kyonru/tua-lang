-- Optional presentation controller. No game flow, rendering, clocks, or audio IO.
local Graphemes = require("tuadialogue_graphemes")
local Reveal = {}
local Player = {}
Player.__index = Player
local nextInstance = 0

local function finite(value)
  return type(value) == "number" and value == value and math.abs(value) < math.huge
end

local function token(value)
  return finite(value) and value >= 0 and value % 1 == 0 and value <= 9007199254740991
end

local function copy(value)
  local result = {}
  for key, child in pairs(value) do
    result[key] = type(child) == "table" and copy(child) or child
  end
  return result
end

function Reveal.new(options)
  if options == nil then options = {} end
  assert(
    type(options) == "table" and getmetatable(options) == nil,
    "reveal options must be a plain record"
  )
  for key in pairs(options) do
    assert(
      key == "rate" or key == "speech" or key == "minCueInterval" or key == "reducedMotion",
      "unknown reveal option"
    )
  end
  local rate = options.rate == nil and 30 or options.rate
  local interval = options.minCueInterval == nil and 0.05 or options.minCueInterval
  assert(
    finite(rate) and rate > 0 and rate <= 1000,
    "reveal rate must be greater than zero and at most 1000"
  )
  assert(finite(interval) and interval >= 0 and interval <= 60, "invalid speech cue interval")
  assert(options.speech == nil or type(options.speech) == "boolean", "invalid speech option")
  assert(
    options.reducedMotion == nil or type(options.reducedMotion) == "boolean",
    "invalid reducedMotion option"
  )
  nextInstance = nextInstance + 1
  assert(token(nextInstance), "reveal instance limit exceeded")
  local player = setmetatable({
    instance = nextInstance,
    sequence = 0,
    rate = rate,
    interval = interval,
    speech = options.speech == true,
    reducedMotion = options.reducedMotion == true,
  }, Player)
  player:clear()
  return player
end

local function invalidate(player)
  player.sequence = player.sequence + 1
  assert(token(player.sequence), "reveal sequence limit exceeded")
end

function Player:clear()
  invalidate(self)
  self.generation, self.visit, self.revision = nil, nil, nil
  self.text, self.units, self.visible = "", {}, 0
  self.credit, self.sinceCue, self.complete = 0, 0, true
end

function Player:present(snapshot)
  assert(type(snapshot) == "table", "reveal needs a dialogue snapshot")
  assert(
    token(snapshot.generation) and token(snapshot.visit) and token(snapshot.presentationRevision),
    "invalid presentation identity"
  )
  assert(type(snapshot.revealComplete) == "boolean", "invalid reveal completion flag")
  local state = snapshot.state
  assert(state == "idle" or state == "awaiting_advance" or state == "awaiting_choice"
    or state == "awaiting_action" or state == "completed" or state == "cancelled"
    or state == "disposed" or state == "faulted", "invalid dialogue snapshot state")
  local active = state == "awaiting_advance"
  local node = snapshot.node
  if not active then
    local changed = self.generation ~= nil
    if changed then self:clear() end
    return changed
  end
  assert(
    type(node) == "table" and node.kind == "line" and type(node.text) == "string",
    "presentation needs a line with text"
  )
  local same = self.generation == snapshot.generation and self.visit == snapshot.visit and self.revision == snapshot.presentationRevision
  if same then
    assert(self.text == node.text, "changed text needs a new presentation identity")
    if snapshot.revealComplete and not self.complete then self:finish() end
    return false
  end
  -- Segment before committing, so malformed UTF-8 does not destroy a valid view.
  local units = Graphemes.segment(node.text)
  invalidate(self)
  self.generation, self.visit, self.revision = snapshot.generation, snapshot.visit, snapshot.presentationRevision
  self.text, self.units, self.visible = node.text, units, 0
  self.credit, self.sinceCue, self.complete = 0, self.interval, #units == 0
  if snapshot.revealComplete or self.reducedMotion then self:finish() end
  return true
end

function Player:snapshot()
  local last = self.units[self.visible]
  return {
    text = self.text:sub(1, last and last.endByte or 0),
    visible = self.visible,
    total = #self.units,
    complete = self.complete,
    cues = {},
    instance = self.instance,
    sequence = self.sequence,
    generation = self.generation,
    visit = self.visit,
    presentationRevision = self.revision,
  }
end

function Player:clusters()
  return copy(self.units)
end

function Player:finish()
  invalidate(self)
  self.visible, self.complete, self.credit = #self.units, true, 0
  return self:snapshot()
end

function Player:update(dt)
  assert(finite(dt) and dt >= 0, "reveal dt must be finite and nonnegative")
  if self.complete then return self:snapshot() end
  local before = self.visible
  -- Bound credit before multiplying; huge finite dt cannot overflow.
  local remaining = #self.units - self.visible
  if dt >= (remaining - self.credit) / self.rate then
    self.visible, self.credit = #self.units, 0
  else
    self.credit = math.min(remaining, self.credit + dt * self.rate)
    local count = math.floor(self.credit)
    self.credit = self.credit - count
    self.visible = math.min(#self.units, self.visible + count)
  end
  self.complete = self.visible == #self.units
  self.sinceCue = math.min(60, self.sinceCue + math.min(dt, 60))
  local result = self:snapshot()
  if self.speech and self.sinceCue >= self.interval then
    -- Coalesce to the last eligible newly revealed cluster. Never retain a queue.
    for index = self.visible, before + 1, -1 do
      local unit = self.units[index]
      if unit.speakable then
        result.cues[1] = {
          instance = self.instance,
          sequence = self.sequence,
          generation = self.generation,
          visit = self.visit,
          presentationRevision = self.revision,
          index = index,
          text = unit.text,
          startByte = unit.startByte,
          endByte = unit.endByte,
        }
        self.sinceCue = 0
        break
      end
    end
  end
  return result
end

return Reveal

-- Renderer-independent dialogue playback. No graphics, input or global clock.
local Dialogue = { schemaVersion = 10 }
local Rich
local function rich()
  if not Rich then Rich = require("tuadialogue_rich") end
  return Rich
end
local Conditions
local function conditions()
  if not Conditions then Conditions = require("tuadialogue_conditions") end
  return Conditions
end
local eventDefinitions = setmetatable({}, { __mode = "k" })
local Actions
local function actions()
  if not Actions then Actions = require("tuadialogue_actions") end
  return Actions
end
function Dialogue.actionHandle(options)
  return actions().handle(options)
end

function Dialogue.speechProfile(profile)
  return require("tuadialogue_media").speech(profile)
end

function Dialogue.reveal(options)
  return require("tuadialogue_reveal").new(options)
end

local function history() return require("tuadialogue_history") end

local Player = {}
Player.__index = Player
local refresh

local function copy(value)
  if type(value) ~= "table" then return value end
  local result = {}
  for key, child in pairs(value) do result[key] = copy(child) end
  return result
end

local function seconds(value)
  return type(value) == "number" and value >= 0 and value < math.huge
end

local function eventSource(value)
  if value == nil then return nil end
  assert(type(value) == "table" and getmetatable(value) == nil, "invalid event source")
  for key in pairs(value) do
    assert(key == "line" or key == "column", "unknown event source field")
  end
  for _, key in ipairs({ "line", "column" }) do
    local number = value[key]
    assert(type(number) == "number" and number >= 1 and number < math.huge and number % 1 == 0,
      "event source positions must be positive integers")
  end
  return { line = value.line, column = value.column }
end

local function events(value, schemaVersion, documentId, seen, validators)
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
    assert(type(event.name) == "string" and event.name ~= "", "invalid dialogue event name")
    local payload
    if event.kind ~= nil then
      assert(
        (schemaVersion == 5 or schemaVersion == 6 or schemaVersion == 7 or schemaVersion == 8 or schemaVersion == 9 or schemaVersion == 10) and event.kind == event.name,
        "invalid typed event kind"
      )
      assert(event.id ~= nil, "typed events require an explicit ID")
      local validate = validators and validators[event.name]
      assert(validate, "unknown typed dialogue event: " .. event.name)
      local ok, value = pcall(validate, event.payload)
      if not ok then
        error("dialogue event " .. event.name .. " (" .. tostring(documentId) .. "/"
          .. tostring(event.id) .. "): " .. tostring(value), 0)
      end
      payload = value
    else
      assert(type(event.payload) == "string", "legacy event payload must be a string")
      payload = event.payload
    end
    if event.id ~= nil or event.source ~= nil then
      assert(
        schemaVersion == 4 or schemaVersion == 5 or schemaVersion == 6 or schemaVersion == 7 or schemaVersion == 8 or schemaVersion == 9 or schemaVersion == 10,
        "event identity requires schema version 4 or later"
      )
      assert(
        type(documentId) == "string" and documentId ~= "",
        "event identity requires documentId"
      )
      assert(type(event.id) == "string" and event.id ~= "", "event ID must be a nonempty string")
      assert(not seen[event.id], "duplicate event ID: " .. event.id)
      seen[event.id] = true
    end
    result[index] = {
      kind = event.kind,
      id = event.id,
      source = eventSource(event.source),
      anchor = event.anchor,
      delay = event.delay,
      name = event.name,
      payload = payload,
    }
  end
  return result
end

-- Schema field sets are immutable and private; validation does not allocate a
-- fresh lookup table or an error string for every valid field/session.
local documentFields = {
  schemaVersion = true,
  documentId = true,
  entry = true,
  nodes = true,
  context = true,
  bindings = true,
  events = true,
  actions = true,
}
local nodeFields = {
  wait = { kind = true, duration = true, next = true, source = true },
  action = {
    kind = true,
    id = true,
    name = true,
    payload = true,
    source = true,
    success = true,
    failure = true,
  },
  line = {
    voice = true,
    speech = true,
    kind = true,
    text = true,
    textId = true,
    richText = true,
    speaker = true,
    speakerKey = true,
    portrait = true,
    expression = true,
    expressions = true,
    next = true,
    hold = true,
    fade = true,
    events = true,
  },
  choice = { kind = true, options = true, fallback = true },
  branch = { kind = true, condition = true, yes = true, no = true },
  ["end"] = { kind = true },
}
local eventFields = {
  kind = true,
  anchor = true,
  delay = true,
  name = true,
  payload = true,
  id = true,
  source = true,
}
local optionFields = {
  richText = true,
  richReason = true,
  id = true,
  text = true,
  target = true,
  visible = true,
  enabled = true,
  reason = true,
  reasonId = true,
}
local playerOptions = {
  history = true,
  checkpointVersion = true,
  locale = true,
  resolve = true,
  resolveTemplate = true,
  args = true,
  transitionBudget = true,
  handlers = true,
}

local function fields(value, allowed)
  for key in pairs(value) do
    if not allowed[key] then error("unknown dialogue field: " .. tostring(key)) end
  end
end

local function optionalId(value)
  return value == nil or (type(value) == "string" and value ~= "")
end

local function compiledRich(template, fallback, version, timing)
  if template == nil then return nil end
  assert(
    version == 8 or version == 9 or version == 10,
    "rich text requires schema version 8 or later"
  )
  local compiled = rich().parse(template, timing)
  assert(compiled.plain == fallback, "rich text fallback must match the stripped template")
  return compiled
end

-- Validate and own the document before any session can execute it.
local function document(value)
  assert(type(value) == "table" and type(value.nodes) == "table", "dialogue needs nodes")
  assert(type(value.entry) == "string", "dialogue needs an entry ID")
  assert(
    value.schemaVersion == nil or value.schemaVersion == 1 or value.schemaVersion == 2 or value.schemaVersion == 3 or value.schemaVersion == 4 or value.schemaVersion == 5 or value.schemaVersion == 6 or value.schemaVersion == 7 or value.schemaVersion == 8 or value.schemaVersion == 9 or value.schemaVersion == 10,
    "unsupported dialogue schemaVersion"
  )
  assert(optionalId(value.documentId), "invalid documentId")
  if value.schemaVersion then
    fields(value, documentFields)
  end
  local validators
  if value.events ~= nil then
    assert(
      value.schemaVersion == 5 or value.schemaVersion == 6 or value.schemaVersion == 7 or value.schemaVersion == 8 or value.schemaVersion == 9 or value.schemaVersion == 10,
      "event contracts require schema version 5 or later"
    )
    validators = eventDefinitions[value.events]
    assert(validators, "document needs a Dialogue.events definition")
  end
  if value.actions ~= nil then
    assert(
      value.schemaVersion == 6 or value.schemaVersion == 7 or value.schemaVersion == 8 or value.schemaVersion == 9 or value.schemaVersion == 10,
      "action contracts require schema version 6 or later"
    )
    assert(actions().valid(value.actions), "document needs a Dialogue.actions definition")
  end
  local nodes, eventIds = {}, {}
  for id, node in pairs(value.nodes) do
    assert(type(id) == "string" and id ~= "", "node IDs must be nonempty strings")
    assert(type(node) == "table", "dialogue node must be a record")
    assert(
      node.kind == "line" or node.kind == "choice" or node.kind == "end" or node.kind == "branch" or node.kind == "action" or node.kind == "wait",
      "unknown dialogue node kind"
    )
    if value.schemaVersion then fields(node, nodeFields[node.kind]) end
    local owned = { kind = node.kind }
    if node.kind == "line" then
      assert(type(node.text) == "string", "line needs text")
      if node.voice ~= nil or node.speech ~= nil then
        assert(value.schemaVersion == 10, "voice and speech metadata require schema version 10")
        local Media = require("tuadialogue_media")
        if node.voice ~= nil then owned.voice = Media.voice(node.voice) end
        if node.speech ~= nil then
          if node.speech == "off" then owned.speech = "off"
          else owned.speech = Media.speech(node.speech) end
        end
      end
      assert(node.speaker == nil or type(node.speaker) == "string", "invalid speaker")
      assert(type(node.next) == "string", "line needs a next node")
      assert(node.portrait == nil or type(node.portrait) == "string", "invalid portrait")
      assert(node.hold == nil or seconds(node.hold), "invalid hold duration")
      assert(node.fade == nil or seconds(node.fade), "invalid fade duration")
      owned._rich = compiledRich(node.richText, node.text, value.schemaVersion, true)
      owned.text, owned.speaker, owned.next = node.text, node.speaker, node.next
      owned.portrait, owned.hold, owned.fade = node.portrait, node.hold, node.fade
      if node.expression ~= nil or node.expressions ~= nil then
        assert((value.schemaVersion or 0) >= 7, "expressions require schema version 7 or later")
        assert(
          type(node.expression) == "string" and node.expression ~= "",
          "expression needs a name"
        )
        assert(
          type(node.expressions) == "table" and getmetatable(node.expressions) == nil,
          "expressions need a plain record"
        )
        local expression = node.expressions[node.expression]
        assert(
          type(expression) == "table" and getmetatable(expression) == nil,
          "unknown character expression: " .. node.expression
        )
        fields(expression, { portrait = true, animation = true })
        assert(
          type(expression.portrait) == "string" and expression.portrait ~= "",
          "expression needs a portrait asset ID"
        )
        assert(
          expression.animation == nil or type(expression.animation) == "string" and expression.animation ~= "",
          "invalid portrait animation"
        )
        owned.expression, owned.portrait = node.expression, expression.portrait
        owned.portraitAnimation = expression.animation
      end
      assert(optionalId(node.textId) and optionalId(node.speakerKey), "invalid localization ID")
      owned.textId, owned.speakerKey = node.textId, node.speakerKey
      owned.events = events(
        node.events,
        value.schemaVersion,
        value.documentId,
        eventIds,
        validators
      )
      if value.schemaVersion then
        for _, event in ipairs(node.events or {}) do
          fields(event, eventFields)
        end
      end
    elseif node.kind == "wait" then
      assert(
        value.schemaVersion == 9 or value.schemaVersion == 10,
        "waits require schema version 9 or later"
      )
      assert(seconds(node.duration) and node.duration <= 86400, "invalid wait duration")
      assert(type(node.next) == "string", "wait needs a next node")
      owned.duration, owned.next, owned.source = node.duration, node.next, eventSource(node.source)
    elseif node.kind == "action" then
      assert(
        value.schemaVersion == 6 or value.schemaVersion == 7 or value.schemaVersion == 8 or value.schemaVersion == 9 or value.schemaVersion == 10,
        "actions require schema version 6 or later"
      )
      assert(
        type(value.documentId) == "string" and value.documentId ~= "",
        "actions require documentId"
      )
      assert(type(node.id) == "string" and node.id ~= "", "action needs a stable ID")
      assert(type(node.name) == "string" and node.name ~= "", "action needs a name")
      assert(
        type(node.success) == "string" and type(node.failure) == "string",
        "action needs both outcome targets"
      )
      assert(not eventIds[node.id], "duplicate event/action ID: " .. node.id)
      eventIds[node.id] = true
      owned = actions().compile(value.actions, node, value.documentId)
      owned.source = eventSource(node.source)
    elseif node.kind == "branch" then
      assert((value.schemaVersion or 0) >= 3, "branches require schema version 3 or later")
      assert(
        optionalId(node.yes) and node.yes and optionalId(node.no) and node.no,
        "branch needs both targets"
      )
      owned.condition, owned.yes, owned.no = conditions().compile(node.condition), node.yes, node.no
    elseif node.kind == "choice" then
      assert(type(node.options) == "table" and #node.options > 0, "choice needs options")
      assert(optionalId(node.fallback), "invalid choice fallback")
      if node.fallback then
        assert((value.schemaVersion or 0) >= 3, "fallback requires schema version 3 or later")
      end
      owned.options, owned.fallback = {}, node.fallback
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
        if value.schemaVersion then fields(option, optionFields) end
        seen[option.id] = true
        owned.options[index] = { id = option.id, text = option.text, target = option.target }
        local target = owned.options[index]
        target._rich = compiledRich(option.richText, option.text, value.schemaVersion, false)
        target._richReason = compiledRich(
          option.richReason,
          option.reason,
          value.schemaVersion,
          false
        )
        if option.visible ~= nil
          or option.enabled ~= nil
          or option.reason ~= nil
          or option.reasonId ~= nil then
          assert(
            (value.schemaVersion or 0) >= 3,
            "choice conditions/reasons require schema version 3 or later"
          )
          assert(
            optionalId(option.reasonId) and (option.reason == nil or type(option.reason) == "string"),
            "invalid disabled reason"
          )
          assert(
            (option.reason == nil) == (option.reasonId == nil),
            "disabled reason needs a text ID"
          )
          if option.visible ~= nil then
            target.visibleCondition = conditions().compile(option.visible)
          end
          if option.enabled ~= nil then
            target.enabledCondition = conditions().compile(option.enabled)
          end
          if option.visible ~= nil or option.enabled ~= nil then
            assert(node.fallback, "conditional choices need a fallback")
          end
          target.reason, target.reasonId = option.reason, option.reasonId
        end
      end
    end
    nodes[id] = owned
  end
  assert(nodes[value.entry], "missing entry node")
  for _, node in pairs(nodes) do
    if node.next then assert(nodes[node.next], "missing next node") end
    if node.kind == "action" then
      assert(nodes[node.success] and nodes[node.failure], "missing action outcome target")
    end
    if node.fallback then assert(nodes[node.fallback], "missing fallback node") end
    if node.kind == "branch" then
      assert(nodes[node.yes] and nodes[node.no], "missing branch target")
    end
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

local function newPlayer(source, options)
  if options == nil then options = {} end
  assert(type(options) == "table", "invalid dialogue options")
  fields(options, playerOptions)
  assert(optionalId(options.locale), "invalid dialogue locale")
  assert(options.checkpointVersion == nil or (type(options.checkpointVersion) == "string"
    and #options.checkpointVersion > 0 and #options.checkpointVersion <= 256), "invalid checkpointVersion")
  assert(options.resolve == nil or type(options.resolve) == "function", "invalid dialogue resolver")
  assert(not options.resolve or options.locale, "dialogue resolver needs an explicit locale")
  assert(
    options.resolveTemplate == nil or type(options.resolveTemplate) == "function",
    "invalid template resolver"
  )
  assert(
    not options.resolveTemplate or options.locale,
    "template resolver needs an explicit locale"
  )
  local budget = options.transitionBudget
  if budget == nil then budget = 256 end
  assert(
    type(budget) == "number" and budget % 1 == 0 and budget >= 1 and budget <= 4096,
    "transitionBudget must be an integer from 1 to 4096"
  )
  local handlers
  if source.actions ~= nil then
    handlers = actions().handlers(source.actions, options.handlers)
  else
    assert(options.handlers == nil, "handlers require an action registry")
  end
  return setmetatable({
    transcript = options.history ~= nil and history().new(options.history) or nil,
    checkpointVersion = options.checkpointVersion,
    checkpointPlain = source.context == nil and source.actions == nil and source.events == nil,
    actionHandlers = handlers,
    transitionBudget = budget,
    document = document(source),
    state = "idle",
    generation = 0,
    visit = 0,
    locale = options.locale,
    resolve = options.resolve,
    resolveTemplate = options.resolveTemplate,
    args = arguments(options.args == nil and {} or options.args),
    presentationRevision = 0,
    revealComplete = false,
  }, Player)
end

function Dialogue.new(source, options)
  assert(
    source.context == nil and source.bindings == nil and source.events == nil and source.actions == nil,
    "use the context, event or action definition's new function"
  )
  return newPlayer(source, options)
end

-- Bound data is copied by a schema validator before it becomes session state.
-- No user callbacks run while following a field path.
local function boundArguments(self, facts, locals)
  local result = copy(self.args)
  for _, binding in ipairs(self.bindings or {}) do
    local value = binding.root == "context" and facts or locals
    for _, key in ipairs(binding.keys) do
      if value == nil then break end
      assert(type(value) == "table" and getmetatable(value) == nil, "binding needs a plain record")
      value = rawget(value, key)
    end
    if value == nil then value = binding.fallback end
    assert(value ~= nil, "missing dialogue binding: " .. binding.name)
    result[binding.name] = value
  end
  return arguments(result)
end

local function available(option, facts, locals)
  local visible = not option.visibleCondition or conditions().test(option.visibleCondition, facts, locals)
  local enabled = visible and (not option.enabledCondition or conditions().test(option.enabledCondition, facts, locals))
  return visible, enabled
end

-- Resolve automatic nodes in authored order with a fixed per-operation budget.
local function destination(self, id, facts, locals)
  local steps = 0
  while true do
    local node = self.document.nodes[id]
    local nextId
    if node.kind == "branch" then
      nextId = conditions().test(node.condition, facts, locals) and node.yes or node.no
    elseif node.kind == "choice" and node.fallback then
      local any = false
      for _, option in ipairs(node.options) do
        local _, enabled = available(option, facts, locals)
        if enabled then
          any = true
          break
        end
      end
      if not any then nextId = node.fallback end
    end
    if not nextId then return id end
    if steps >= self.transitionBudget then
      return id, {
        code = "transition_budget_exceeded",
        message = "Automatic dialogue transition budget exceeded",
        nodeId = id,
      }
    end
    steps, id = steps + 1, nextId
  end
end

-- Compute presentation before committing a transition; a bad resolver cannot
-- partially advance the visit or overwrite clocks. Snapshot never calls host code.
local function presentation(self, id, locale, facts, locals)
  if self.document.nodes[id].kind == "action" or self.document.nodes[id].kind == "wait" then
    return nil
  end
  -- The document is already owned. Without localization, retain only an internal
  -- reference; snapshot still copies it before exposing anything to the caller.
  if not self.resolve and not self.bindings and (self.document.schemaVersion or 0) < 3 then
    return self.document.nodes[id]
  end
  local args = boundArguments(self, facts or self.facts, locals or self.locals)
  local node = copy(self.document.nodes[id])
  node.text, node.options = node.text or "", node.options or {}
  if node.voice then node.voice.locale = locale or node.voice.sourceLocale end
  if node.kind == "choice" and (self.document.schemaVersion or 0) >= 3 then
    local options = {}
    for _, option in ipairs(node.options) do
      local visible, enabled = available(option, facts or self.facts, locals or self.locals)
      if visible then
        option.visibleCondition, option.enabledCondition, option.enabled = nil, nil, enabled
        if enabled then option.reason, option.reasonId, option._richReason = nil, nil, nil end
        options[#options + 1] = option
      end
    end
    node.options = options
  end
  local function invoke(callback, key, source)
    self.resolving = true
    local ok, value = pcall(callback, key, source, copy(args), locale)
    self.resolving = false
    if not ok then error(value, 0) end
    return value
  end
  local function resolveField(compiled, text, key)
    if compiled and (self.resolveTemplate or not self.resolve or not key) then
      local selected
      if self.resolveTemplate and key then
        selected = invoke(self.resolveTemplate, key, compiled.source)
        assert(type(selected) == "table", "template resolver must return a template record")
      end
      local view = rich().resolve(compiled, args, selected)
      return view.text, view
    end
    if self.resolve and key then
      local value = invoke(self.resolve, key, text)
      assert(type(value) == "string", "dialogue resolver must return text")
      return value
    end
    if self.bindings then return require("tuadialogue_text").format(text, args) end
    return text
  end
  node.text, node.presentation = resolveField(node._rich, node.text, node.textId)
  node._rich = nil
  if self.resolve and node.speakerKey then
    node.speaker = invoke(self.resolve, node.speakerKey, node.speaker or "")
    assert(type(node.speaker) == "string", "dialogue resolver must return text")
  end
  for _, option in ipairs(node.options) do
    option.text, option.presentation = resolveField(option._rich, option.text, option.id)
    if option.reason then
      option.reason, option.reasonPresentation = resolveField(
        option._richReason,
        option.reason,
        option.reasonId
      )
    end
    option._rich, option._richReason = nil, nil
  end
  return node
end

function Player:setLocale(locale)
  assert(not self.resolving, "cannot mutate dialogue while resolving text")
  assert(self.state ~= "disposed", "dialogue is disposed")
  assert(type(locale) == "string" and locale ~= "", "invalid dialogue locale")
  if locale == self.locale then return false end
  local node = self.state ~= "faulted" and self.current and presentation(
    self,
    self.current,
    locale
  ) or nil
  self.locale, self.presentation = locale, node
  self.presentationRevision = self.presentationRevision + 1
  self.revealComplete = self.state == "awaiting_advance"
  if self.revealComplete and self.revealedElapsed == nil then
    self.revealedElapsed, self.revealedAt = 0, self.elapsed
  end
  return true
end

function Player:enter(id, locals, facts, resolved, issue, selectedReply)
  assert(not self.resolving, "cannot mutate dialogue while resolving text")
  if not resolved then
    id, issue = destination(self, id, facts or self.facts, locals or self.locals)
  end
  local nodePresentation = not issue and presentation(self, id, self.locale, facts, locals) or nil
  self.issue = issue
  self.restored = false
  self.presentation, self.revealComplete = nodePresentation, false
  self.presentationRevision = self.presentationRevision + 1
  self.visit = self.visit + 1
  self.elapsed, self.revealedElapsed, self.revealedAt, self.fired = 0, nil, nil, {}
  self.current = id
  self.deferredChoiceRefresh = nil
  local node = self.document.nodes[id]
  if issue then self.state = "faulted"
  elseif node.kind == "end" then self.state = "completed"
  elseif node.kind == "wait" then self.state = "waiting"
  elseif node.kind == "action" then self.state = "awaiting_action"
  elseif node.kind == "line" then self.state = "awaiting_advance"
  else self.state = "awaiting_choice" end
  if self.transcript and not issue then
    if selectedReply then history().push(self.transcript, selectedReply) end
    if node.kind == "line" then
      history().push(self.transcript, history().capture(self, nodePresentation))
    end
  end
end

function Player:start(entry)
  assert(not self.resolving, "cannot mutate dialogue while resolving text")
  assert(self.state ~= "disposed", "dialogue is disposed")
  assert(entry == nil or type(entry) == "string", "entry ID must be a string")
  entry = entry or self.document.entry
  assert(self.document.nodes[entry], "missing entry node")
  local locals = self.localDefaults and self.copyLocals(self.localDefaults)
  self:enter(entry, locals)
  if locals then self.locals = locals end
  self.generation = self.generation + 1
  self.suspended = false
  if Actions then Actions.release(self) end
end

-- Suspension preserves execution identity and queued host results.
function Player:suspend()
  assert(not self.resolving, "cannot mutate dialogue while resolving text")
  if self.suspended or (self.state ~= "awaiting_advance" and self.state ~= "awaiting_choice"
    and self.state ~= "awaiting_action" and self.state ~= "waiting") then return false end
  self.suspended = true
  return true
end

function Player:resume()
  assert(not self.resolving, "cannot mutate dialogue while resolving text")
  if not self.suspended then return false end
  self.suspended = false
  if self.deferredChoiceRefresh then
    local ok, message = pcall(refresh, self, self.facts, self.locals)
    if not ok then
      self.suspended = true
      error(message, 0)
    end
  end
  return true
end

-- The host reports reveal completion; this runtime never reads a wall clock.
-- The first true sample anchors revealed events at elapsed zero, conservatively
-- excluding the update interval in which presentation finished revealing.
function Player:update(dt, revealed)
  assert(not self.resolving, "cannot mutate dialogue while resolving text")
  assert(seconds(dt) and type(revealed) == "boolean", "invalid dialogue update")
  if self.suspended then return {} end
  if self.state == "waiting" then
    local node = self.document.nodes[self.current]
    self.elapsed = math.min(node.duration, self.elapsed + dt)
    if self.elapsed >= node.duration then
      local id = self.current
      local ok, message = pcall(self.enter, self, node.next)
      if not ok then
        self.state, self.presentation = "faulted", nil
        self.issue = {
          code = "wait_transition_error",
          message = type(message) == "string" and message or "Wait transition failed",
          nodeId = id,
          source = copy(node.source),
        }
      end
    end
    -- Discard excess time; never recursively consume another wait or line event.
    return {}
  end
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
      kind = event.kind,
      eventId = event.id,
      documentId = event.id and self.document.documentId or nil,
      source = copy(event.source),
      name = event.name,
      payload = copy(event.payload),
      generation = self.generation,
      visit = self.visit,
      nodeId = self.current,
    }
  end
  return result
end

function Player:runAction()
  assert(not self.resolving, "cannot mutate dialogue while resolving text")
  if self.suspended or self.state ~= "awaiting_action" then return false end
  return actions().run(self)
end

function Player:advance()
  assert(not self.resolving, "cannot mutate dialogue while resolving text")
  if self.suspended or self.state ~= "awaiting_advance" then return false end
  self:enter(self.document.nodes[self.current].next)
  return true
end

function Player:choose(id)
  assert(not self.resolving, "cannot mutate dialogue while resolving text")
  if self.suspended or self.state ~= "awaiting_choice" then return false end
  for _, option in ipairs(self.document.nodes[self.current].options) do
    if option.id == id then
      local _, enabled = available(option, self.facts, self.locals)
      if not enabled then return false end
      local selected
      if self.transcript then
        for _, shown in ipairs(self.presentation.options) do
          if shown.id == id then selected = history().capture(self, self.presentation, shown) end
        end
      end
      self:enter(option.target, nil, nil, nil, nil, selected)
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
  self.suspended = false
  self.deferredChoiceRefresh = nil
  self.restored = false
  self.state, self.current, self.presentation, self.issue = "cancelled", nil, nil, nil
  self.fired, self.revealedElapsed, self.revealedAt = nil, nil, nil
  if Actions then Actions.release(self) end
  return true
end

-- Transcript browsing never advances playback or dispatches events.
function Player:history()
  return history().snapshot(self.transcript)
end
function Player:clearHistory()
  assert(not self.resolving, "cannot mutate dialogue while resolving text")
  if self.transcript then history().clear(self.transcript) end
end
function Player:redactHistory(id)
  assert(not self.resolving, "cannot mutate dialogue while resolving text")
  assert(type(id) == "number" and id >= 1 and id < math.huge and id % 1 == 0,
    "history entry ID must be a positive integer")
  return self.transcript ~= nil and history().redact(self.transcript, id)
end

-- Restore only into a fresh session. Validation and presentation precede mutation.
function Player:checkpoint()
  assert(not self.resolving and self.state ~= "disposed", "cannot checkpoint this player")
  return require("tuadialogue_checkpoint").export(self)
end

function Player:restore(value)
  assert(not self.resolving and self.state == "idle", "restore needs a fresh idle player")
  local fired = require("tuadialogue_checkpoint").validate(self, value)
  value = copy(value)
  local node = presentation(self, value.nodeId, value.locale)
  self.current, self.state, self.presentation = value.nodeId, value.state, node
  self.restored = true
  self.locale, self.suspended = value.locale, value.suspended
  self.elapsed, self.fired = value.elapsed, fired
  self.revealedElapsed, self.revealedAt = value.revealedElapsed, value.revealedAt
  self.revealComplete = value.state == "awaiting_advance"
  if self.revealComplete and self.revealedElapsed == nil then
    self.revealedElapsed, self.revealedAt = 0, self.elapsed
  end
  self.generation, self.visit, self.presentationRevision = 1, 1, 1
end

function Player:snapshot()
  local presentation = copy(self.presentation)
  if presentation then
    presentation.text = presentation.text or ""
    presentation.options = presentation.options or {}
  end
  return {
    state = self.state,
    restored = self.restored == true,
    suspended = self.suspended == true,
    wait = self.state == "waiting" and {
      duration = self.document.nodes[self.current].duration,
      elapsed = self.elapsed,
      remaining = math.max(0, self.document.nodes[self.current].duration - self.elapsed),
      source = copy(self.document.nodes[self.current].source),
    } or nil,
    action = self.state == "awaiting_action" and actions().request(self) or nil,
    issue = copy(self.issue),
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
  self.restored = false
  self.generation = self.generation + 1
  self.suspended = false
  self.deferredChoiceRefresh = nil
  self.state, self.current, self.document, self.issue = "disposed", nil, nil, nil
  self.presentation, self.resolve, self.args, self.locale = nil, nil, nil, nil
  self.transcript = nil
  self.actionHandlers, self.resolveTemplate = nil, nil
  self.facts, self.locals, self.localDefaults = nil, nil, nil
  self.bindings, self.copyFacts, self.copyLocals = nil, nil, nil
  self.fired, self.revealedElapsed, self.revealedAt = nil, nil, nil
  if Actions then Actions.release(self) end
end

-- Context changes may take an empty-choice fallback; suspension defers it.
refresh = function(self, facts, locals)
  boundArguments(self, facts, locals)
  if self.state == "faulted" then
    self.facts, self.locals = facts, locals
    return
  end
  if self.state == "awaiting_choice" and not self.suspended then
    local id, issue = destination(self, self.current, facts, locals)
    if issue or id ~= self.current then
      self:enter(id, locals, facts, true, issue)
      self.facts, self.locals = facts, locals
      return
    end
  end
  local node = self.current and presentation(self, self.current, self.locale, facts, locals)
  self.facts, self.locals, self.presentation = facts, locals, node
  self.deferredChoiceRefresh = self.suspended and self.state == "awaiting_choice"
  self.presentationRevision = self.presentationRevision + 1
  self.revealComplete = self.state == "awaiting_advance"
  if self.revealComplete and self.revealedElapsed == nil then
    self.revealedElapsed, self.revealedAt = 0, self.elapsed
  end
end

local function contextReady(self)
  assert(not self.resolving, "cannot mutate dialogue while resolving text")
  assert(self.state ~= "disposed", "dialogue is disposed")
  assert(self.copyFacts, "dialogue has no typed context")
end

function Player:getContext()
  contextReady(self)
  return copy(self.facts)
end

function Player:getLocals()
  contextReady(self)
  return copy(self.locals)
end

function Player:updateContext(value)
  contextReady(self)
  refresh(self, self.copyFacts(value), self.locals)
end

function Player:setLocals(value)
  contextReady(self)
  refresh(self, self.facts, self.copyLocals(value))
end

function Dialogue.actions(specs)
  local definition = actions().define(specs)
  function definition.new(source, options)
    assert(source.actions == definition, "document uses another action definition")
    assert(source.context == nil and source.bindings == nil and source.events == nil,
      "use a context definition to combine action and event registries")
    return newPlayer(source, options)
  end
  return definition
end

function Dialogue.events(shapes)
  assert(
    type(shapes) == "table" and getmetatable(shapes) == nil,
    "event schemas must be a plain record"
  )
  local Shape = require("tuasave_shape")
  local validators, payloads, count = {}, {}, 0
  for name, shape in pairs(shapes) do
    assert(
      type(name) == "string" and name:match("^[A-Za-z_][A-Za-z0-9_.-]*$"),
      "invalid event name"
    )
    local validate, defaults = Shape.get(shape)
    assert(type(defaults) == "table", "event payload shapes must describe tables")
    validators[name], payloads[name] = validate, copy(defaults)
    count = count + 1
  end
  assert(count >= 1 and count <= 128, "declare between 1 and 128 events")
  local readers = {}
  for name, validate in pairs(validators) do
    readers[name] = function(event)
      assert(type(event) == "table", "expected a dialogue notification")
      if event.kind ~= name or event.name ~= name then return nil end
      return validate(event.payload)
    end
  end
  local definition = { kind = "events", payloads = payloads, read = readers }
  eventDefinitions[definition] = validators
  function definition.new(source, options)
    assert(source.events == definition, "document uses another event definition")
    assert(source.actions == nil, "use a context definition to combine registries")
    assert(
      source.context == nil and source.bindings == nil,
      "use the context definition's new function"
    )
    return newPlayer(source, options)
  end
  return definition
end

function Dialogue.context(schemas)
  assert(type(schemas) == "table", "context needs fact and local shapes")
  fields(schemas, { facts = true, locals = true, events = true, actions = true })
  local Shape = require("tuasave_shape")
  local copyFacts, initialFacts = Shape.get(schemas.facts)
  local copyLocals, initialLocals = Shape.get(schemas.locals)
  local registeredActions = schemas.actions
  assert(
    registeredActions == nil or actions().valid(registeredActions),
    "invalid context action definition"
  )
  local registeredEvents = schemas.events
  assert(
    registeredEvents == nil or eventDefinitions[registeredEvents],
    "invalid context event definition"
  )
  local definition = { kind = "context", facts = copy(initialFacts), locals = copy(initialLocals) }
  function definition.new(source, value, options)
    assert(source.context == definition, "document uses another context definition")
    assert(source.actions == registeredActions, "document uses another action definition")
    assert(source.events == registeredEvents, "document uses another event definition")
    assert(
      source.schemaVersion == 2 or source.schemaVersion == 3 or source.schemaVersion == 4 or source.schemaVersion == 5 or source.schemaVersion == 6 or source.schemaVersion == 7 or source.schemaVersion == 8 or source.schemaVersion == 9 or source.schemaVersion == 10,
      "typed context requires dialogue schema version 2 through 10"
    )
    local player = newPlayer(source, options)
    player.facts, player.locals = copyFacts(value), copyLocals(initialLocals)
    player.copyFacts, player.copyLocals = copyFacts, copyLocals
    player.localDefaults, player.bindings = copyLocals(initialLocals), {}
    local seen = {}
    assert(
      type(source.bindings) == "table" and #source.bindings <= 256,
      "invalid dialogue bindings"
    )
    local count = 0
    for index in pairs(source.bindings) do
      assert(type(index) == "number" and index >= 1 and index <= #source.bindings and index % 1 == 0,
        "bindings must be a dense array")
      count = count + 1
    end
    assert(count == #source.bindings, "bindings must be a dense array")
    for _, binding in ipairs(source.bindings) do
      assert(type(binding) == "table", "invalid binding")
      fields(binding, { name = true, path = true, fallback = true })
      assert(
        type(binding.name) == "string" and binding.name:match("^[A-Za-z_][A-Za-z0-9_]*$"),
        "invalid binding name"
      )
      assert(
        not seen[binding.name] and player.args[binding.name] == nil,
        "duplicate dialogue argument"
      )
      seen[binding.name] = true
      assert(type(binding.path) == "string", "invalid binding path")
      local root, tail = binding.path:match("^(%a+)%.(.+)$")
      assert(root == "context" or root == "locals", "invalid binding root")
      local keys = {}
      for key in tail:gmatch("[^.]+") do
        assert(key:match("^[A-Za-z_][A-Za-z0-9_]*$"), "invalid binding field")
        keys[#keys + 1] = key
      end
      assert(#keys > 0 and #keys < 32 and table.concat(keys, ".") == tail, "invalid binding path")
      if binding.fallback ~= nil then arguments({ fallback = binding.fallback }) end
      player.bindings[#player.bindings + 1] = {
        name = binding.name,
        root = root,
        keys = keys,
        fallback = binding.fallback,
      }
    end
    boundArguments(player, player.facts, player.locals)
    return player
  end
  return definition
end

return Dialogue

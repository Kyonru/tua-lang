do
  local Dialogue = require("tuadialogue")
  local Save = require("tuasave")
  local function clone(value)
    if type(value) ~= "table" then return value end
    local result = {}
    for key, child in pairs(value) do result[key] = clone(child) end
    return result
  end
  local actions = Dialogue.actions({
    purchase = {
      payload = Save.shape(Save.record({ item = Save.string("shell") })),
      effect = "consequential",
    },
  })
  local events = Dialogue.events({ notice = Save.shape(Save.record({ flag = Save.boolean(false) })) })
  local context = Dialogue.context({
    facts = Save.shape(Save.record({ coins = Save.number(0) })),
    locals = Save.shape(Save.record({ visits = Save.number(0) })),
    actions = actions,
    events = events,
  })
  local source = {
    schemaVersion = 9,
    documentId = "effects.checkpoint",
    context = context,
    bindings = {},
    actions = actions,
    events = events,
    entry = "buy",
    nodes = {
      buy = {
        kind = "action",
        name = "purchase",
        id = "buy.shell",
        payload = { item = "shell" },
        success = "line",
        failure = "declined",
      },
      line = {
        kind = "line",
        text = "Purchased",
        next = "choice",
        events = {
          {
            id = "shown",
            name = "notice",
            kind = "notice",
            anchor = "shown",
            delay = 0,
            payload = { flag = false },
          },
          {
            id = "revealed",
            name = "notice",
            kind = "notice",
            anchor = "revealed",
            delay = 1,
            payload = { flag = true },
          },
        },
      },
      choice = { kind = "choice", options = { { id = "wait", text = "Wait", target = "wait" } } },
      wait = { kind = "wait", duration = 2, next = "line" },
      declined = { kind = "line", text = "Declined", next = "done" },
      done = { kind = "end" },
    },
  }
  local calls = 0
  local function fresh(policy, handler, document)
    return context.new(document or source, { coins = 20 }, {
      checkpointVersion = "effects-v1",
      checkpointEffects = policy,
      history = { capacity = 4 },
      handlers = {
        purchase = handler or function()
          calls = calls + 1
          return true
        end,
      },
    })
  end
  for _, policy in ipairs({ "reject", "settled" }) do
    local player = fresh(policy)
    player:start()
    assert(not pcall(function() player:checkpoint() end)) -- Ready actions are not boundaries.
    player:dispose()
  end
  local legacy = fresh()
  legacy:start("line")
  assert(not pcall(function() legacy:checkpoint() end))
  legacy:dispose()

  local original = fresh("settled")
  original:start()
  assert(original:runAction() and calls == 1)
  original:setLocals({ visits = 3 })
  assert(#original:update(0.25, true) == 1)
  local saved = original:checkpoint()
  assert(saved.version == 3 and saved.actionState == "settled" and saved.locals.visits == 3)
  local restored = fresh("settled")
  restored:restore(saved)
  assert(calls == 1 and restored:snapshot().restored)
  assert(#restored:history().entries == 0)
  assert(restored:getContext().coins == 20 and restored:getLocals().visits == 3)
  assert(#restored:update(0, true) == 0)
  local due = restored:update(1, true)
  assert(#due == 1 and due[1].eventId == "revealed")
  assert(events.read.notice(due[1]).flag == true)
  restored:advance()
  local choosing = fresh("settled")
  choosing:restore(restored:checkpoint())
  assert(choosing:choose("wait"))
  choosing:update(0.5, false)
  choosing:suspend()
  local waiting = fresh("settled")
  waiting:restore(choosing:checkpoint())
  waiting:update(10, false)
  assert(waiting:snapshot().state == "waiting")
  waiting:resume()
  waiting:update(1.5, false)
  assert(waiting:snapshot().nodeId == "line" and calls == 1)

  for _, change in ipairs({
    function(value) value.actionState = nil end,
    function(value) value.actionState = "pending" end,
    function(value) value.version = 2 end,
    function(value) value.nodeId = "buy" end,
    function(value) value.fired = { 2 } end,
    function(value) value.locals.visits = "wrong" end,
    function(value) value.handle = {} end,
  }) do
    local broken = clone(saved)
    change(broken)
    local player = fresh("settled")
    assert(not pcall(function() player:restore(broken) end))
    assert(player:snapshot().state == "idle" and calls == 1)
    player:dispose()
  end
  for _, change in ipairs({
    function(value) value.nodes.buy.id = "changed" end,
    function(value) value.nodes.buy.payload.item = "other" end,
    function(value) value.nodes.buy.name = "other" end,
    function(value) value.nodes.buy.success = "declined" end,
    function(value) value.nodes.buy.failure = "line" end,
    function(value) value.nodes.line.events[1].payload.flag = true end,
  }) do
    -- Preserve private registry identities while copying authored data.
    local changed = clone(source)
    changed.context, changed.actions, changed.events = context, actions, events
    change(changed)
    local ok, player = pcall(fresh, "settled", nil, changed)
    if ok then
      assert(not pcall(function() player:restore(saved) end))
      assert(player:snapshot().state == "idle")
      player:dispose()
    end
    assert(calls == 1)
  end
  local handle = Dialogue.actionHandle()
  local pending = fresh("settled", function() return handle end)
  pending:start()
  pending:runAction()
  assert(not pcall(function() pending:checkpoint() end))
  handle:resolve(false)
  assert(not pcall(function() pending:checkpoint() end)) -- Settlement still needs consumption.
  assert(pending:runAction())
  local declined = fresh("settled")
  declined:restore(pending:checkpoint())
  assert(declined:snapshot().nodeId == "declined" and calls == 1)

  -- Registry-only factories do not acquire a synthetic locals payload.
  local actionSource = clone(source)
  actionSource.context, actionSource.bindings, actionSource.events = nil, nil, nil
  actionSource.actions = actions
  actionSource.nodes.line.events = {}
  local actionOptions = {
    checkpointVersion = "actions-v1",
    checkpointEffects = "settled",
    handlers = { purchase = function() return false end },
  }
  local actionOnly = actions.new(actionSource, actionOptions)
  actionOnly:start()
  actionOnly:runAction()
  local actionSaved = actionOnly:checkpoint()
  assert(actionSaved.version == 3 and actionSaved.locals == nil)
  local actionRestored = actions.new(actionSource, actionOptions)
  actionRestored:restore(actionSaved)
  assert(actionRestored:snapshot().nodeId == "declined")
  local changedEffects = Dialogue.actions({
    purchase = {
      payload = Save.shape(Save.record({ item = Save.string("shell") })),
      effect = "presentation",
    },
  })
  actionSource.actions = changedEffects
  local different = changedEffects.new(actionSource, actionOptions)
  assert(not pcall(function() different:restore(actionSaved) end))
  assert(different:snapshot().state == "idle")

  local eventSource = clone(source)
  eventSource.context, eventSource.bindings, eventSource.actions = nil, nil, nil
  eventSource.events, eventSource.entry, eventSource.nodes.buy = events, "line", nil
  local eventOptions = { checkpointVersion = "events-v1", checkpointEffects = "settled" }
  local eventOnly = events.new(eventSource, eventOptions)
  eventOnly:start()
  assert(#eventOnly:update(0, false) == 1)
  local eventSaved = eventOnly:checkpoint()
  assert(eventSaved.version == 3 and eventSaved.locals == nil)
  local eventRestored = events.new(eventSource, eventOptions)
  eventRestored:restore(eventSaved)
  assert(#eventRestored:update(0, false) == 0)
  for _, player in ipairs({ actionOnly, actionRestored, different, eventOnly, eventRestored }) do
    player:dispose()
  end

  local reentrant
  reentrant = fresh("settled", function()
    reentrant:start("line")
    assert(not pcall(function() reentrant:checkpoint() end))
    return true
  end)
  reentrant:start()
  reentrant:runAction()
  assert(reentrant:checkpoint().version == 3)
  for _, player in ipairs({
    original,
    restored,
    choosing,
    waiting,
    pending,
    declined,
    reentrant,
  }) do
    player:dispose()
  end
  print("settled registry checkpoints passed")
end

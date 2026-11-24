local actions = require("actions")
local scene = require("scene")
local calls = 0
local function create(handler)
  return actions.new(scene, { handlers = { purchase = handler } })
end
local player = create(function(payload, request)
  calls = calls + 1
  assert(payload.item == "shell")
  assert(request.actionId == "shop.purchase" and request.effect == "consequential")
  assert(request.source.line == 5)
  payload.item = "mutated"
  return true
end)
player:start()
assert(player:snapshot().action.name == "purchase")
player:update(10, true)
player:snapshot()
assert(not player:advance())
assert(calls == 0)
assert(player:runAction())
assert(calls == 1 and player:snapshot().nodeId == "done")
assert(not player:runAction())
player:start()
assert(player:runAction() and calls == 2)
local rejected = create(function() return false end)
rejected:start()
assert(rejected:runAction() and rejected:snapshot().nodeId == "declined")
for _, case in ipairs({
  { code = "action_handler_missing" },
  { code = "action_handler_error", handler = function() error("failure") end },
  { code = "action_result_invalid", handler = function() return "yes" end },
}) do
  local faulty = create(case.handler)
  faulty:start()
  assert(not faulty:runAction())
  assert(faulty:snapshot().issue.code == case.code)
  assert(faulty:snapshot().issue.actionId == "shop.purchase")
  assert(not faulty:runAction())
  faulty:dispose()
end
for _, operation in ipairs({ "cancel", "start", "dispose" }) do
  local reentrant
  reentrant = create(function()
    assert(not reentrant:runAction())
    reentrant[operation](reentrant)
    error("stale failure")
  end)
  reentrant:start()
  assert(not reentrant:runAction())
  assert(reentrant:snapshot().state ~= "faulted")
  reentrant:dispose()
end
local handlers = { purchase = function() return true end }
local owned = actions.new(scene, { handlers = handlers })
handlers.purchase = function() error("mutated handler") end
owned:start()
assert(owned:runAction())
assert(not pcall(actions.new, scene, { handlers = { unknown = function() return true end } }))
assert(not pcall(actions.new, scene, { handlers = { purchase = true } }))
scene.nodes.buy.payload.item = 42
assert(not pcall(actions.new, scene))
scene.nodes.buy.payload.item = "shell"
player:dispose()
rejected:dispose()
owned:dispose()
print("registered action lifecycle passed")

local context = require("context")
local bound = require("bound")
local contextual
contextual = context.new(bound, { allowed = false }, {
  handlers = {
    purchase = function()
      contextual:updateContext({ allowed = true })
      return true
    end,
  },
})
contextual:start()
assert(contextual:runAction())
assert(contextual:snapshot().nodeId == "yes")
assert(contextual:update(0, false)[1].payload.value == 1)
assert(contextual:update(0, true)[1].payload.value == 2)
assert(#contextual:update(0, true) == 0)
contextual:dispose()
local transition = actions.new(scene, {
  locale = "en",
  handlers = { purchase = function() return true end },
  resolve = function() error("translation failed") end,
})
transition:start()
assert(not transition:runAction())
assert(transition:snapshot().issue.code == "action_transition_error")
assert(not transition:runAction())
transition:dispose()
-- Repeated names and a self-loop still require one explicit dispatch per visit.
local originalTarget = scene.nodes.buy.success
scene.nodes.buy.success = "buy"
local chained = create(function() return true end)
chained:start()
local firstVisit = chained:snapshot().action.visit
assert(chained:runAction())
assert(chained:snapshot().action.visit == firstVisit + 1)
assert(chained:snapshot().action.actionId == "shop.purchase")
chained:dispose()
scene.nodes.buy.success = originalTarget

local Dialogue = require("tuadialogue")
local Save = require("tuasave")
local shape = Save.shape(Save.record({ item = Save.string("original") }))
assert(not pcall(Dialogue.actions, { purchase = { payload = shape } }))
assert(not pcall(Dialogue.actions, { purchase = { payload = shape, effect = "unknown" } }))
assert(not pcall(Dialogue.actions, {}))
assert(not pcall(Dialogue.new, scene))
local captured = create(function(payload) return payload.item == "shell" end)
scene.nodes.buy.payload.item = "changed after construction"
captured:start()
assert(captured:runAction() and captured:snapshot().nodeId == "done")
scene.nodes.buy.payload.item = "shell"
captured:dispose()
local originalActions = bound.actions
bound.actions = { kind = "actions", payloads = actions.payloads }
assert(not pcall(context.new, bound, { allowed = true }))
bound.actions = originalActions
-- The same action name can occur at independent, stable authored IDs.
scene.nodes.second = {
  kind = "action",
  name = "purchase",
  id = "shop.second",
  payload = { item = "shell" },
  success = "done",
  failure = "declined",
}
scene.nodes.buy.success = "second"
local repeated = create(function() return true end)
repeated:start()
assert(repeated:runAction())
assert(repeated:snapshot().action.actionId == "shop.second")
assert(repeated:runAction() and repeated:snapshot().nodeId == "done")
repeated:dispose()
scene.nodes.second.id = "shop.purchase"
assert(not pcall(actions.new, scene))
scene.nodes.second = nil
scene.nodes.buy.success = originalTarget
-- Consequential dispatch cannot retry after a bounded navigation fault.
bound.nodes.check.yes = "check"
local exhausted = context.new(bound, { allowed = true }, {
  transitionBudget = 2,
  handlers = { purchase = function() return true end },
})
exhausted:start()
assert(not exhausted:runAction())
assert(exhausted:snapshot().issue.code == "transition_budget_exceeded")
assert(exhausted:snapshot().issue.actionId == "bound.purchase")
assert(not exhausted:runAction())
exhausted:dispose()
bound.nodes.check.yes = "yes"

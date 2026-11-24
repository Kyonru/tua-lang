local Dialogue = require("tuadialogue")
local Conditions = require("tuadialogue_conditions")
local context = require("context")
local scene = require("scene")
local function fails(fn, expected)
  local ok, message = pcall(fn)
  assert(not ok and (not expected or tostring(message):find(expected, 1, true)), tostring(message))
end
local function test(code, facts)
  return Conditions.test(Conditions.compile(code), facts or {}, {})
end
assert(test({
  { op = "or" },
  { op = "value", value = true },
  { op = "path", path = "context.bad.field" },
}, { bad = 3 }))
assert(not test({
  { op = "and" },
  { op = "value", value = false },
  { op = "path", path = "context.bad.field" },
}, { bad = 3 }))
assert(test({ { op = "not" }, { op = "nil" } }))
assert(not test({
  { op = ">" },
  { op = "path", path = "context.missing.value" },
  { op = "value", value = 0 },
}))
assert(test({ { op = "==" }, { op = "path", path = "context.missing" }, { op = "nil" } }))
assert(test({
  { op = "==" },
  { op = "path", path = "context.flag" },
  { op = "value", value = false },
}, { flag = false }))
fails(function() test({ { op = "value", value = 2 } }) end, "boolean")
fails(function() Conditions.compile({ { op = "call", path = "context.method" } }) end)
fails(function() Conditions.compile({ { op = "value", value = function() end } }) end)
fails(function() Conditions.compile({ { op = "value", value = 0 / 0 } }) end)
fails(function() Conditions.compile({ { op = "and" }, { op = "value", value = true } }) end)
fails(function() Conditions.compile({ { op = "value", value = true }, { op = "nil" } }) end)
fails(function() Conditions.compile({ { op = "path", path = "context..value" } }) end)
local deep = {}
for _ = 1, 32 do deep[#deep + 1] = { op = "not" } end
deep[#deep + 1] = { op = "value", value = true }
fails(function() Conditions.compile(deep) end, "nesting")
local wide = {}
for _ = 1, 257 do wide[#wide + 1] = { op = "nil" } end
fails(function() Conditions.compile(wide) end, "256")

local player = context.new(scene, { coins = 25, unlocked = true }, { transitionBudget = 2 })
local other = context.new(scene, { coins = 0, unlocked = false })
player:start()
other:start()
assert(other:snapshot().nodeId == "locked")
assert(#player:update(0, false) == 1)
player:updateContext({ coins = 25, unlocked = true, bonus = 1 })
assert(#player:update(0, false) == 0)
assert(player:advance())
local before = player:snapshot()
assert(#before.node.options == 3)
assert(before.node.options[1].id == "shop.buy" and before.node.options[1].enabled)
assert(before.node.options[2].id == "shop.bonus")
-- An old UI snapshot never authorizes selection after facts change.
player:updateContext({ coins = 0, unlocked = true })
local refreshed = player:snapshot()
assert(refreshed.visit == before.visit and refreshed.generation == before.generation)
assert(refreshed.presentationRevision > before.presentationRevision)
assert(#refreshed.node.options == 2)
assert(not refreshed.node.options[1].enabled)
assert(refreshed.node.options[1].reason == "Requires 20 coins; you have 0.")
assert(not player:choose("shop.buy") and not player:choose("shop.bonus"))
assert(other:snapshot().nodeId == "locked")
player:updateContext({ coins = 0, unlocked = false })
assert(player:snapshot().nodeId == "locked")
assert(player:snapshot().visit == before.visit + 1)
assert(player:snapshot().generation == before.generation)
assert(not player:choose("shop.leave"))
player:start("loop")
assert(player:snapshot().state == "faulted")
assert(player:snapshot().node == nil)
assert(player:snapshot().issue.nodeId == "loop")
player:updateContext({ coins = 4, unlocked = true })
assert(player:snapshot().state == "faulted")
assert(not player:advance() and not player:choose("shop.buy"))
assert(#player:update(1, true) == 0)
assert(player:cancel())
player:start("gate")
assert(player:snapshot().nodeId == "shop" and player:snapshot().issue == nil)
player:dispose()
assert(player:snapshot().state == "disposed" and player:snapshot().issue == nil)
for _, budget in ipairs({ 0, -1, 1.5, 4097, false, math.huge }) do
  fails(function()
    context.new(scene, { coins = 0, unlocked = true }, { transitionBudget = budget })
  end)
end

local reject = false
local localized = context.new(scene, { coins = 0, unlocked = true }, {
  locale = "en",
  resolve = function(key, source, args, locale)
    if reject then error("deliberate translation failure") end
    if key == "shop.buy.reason" and locale == "fr" then return "Il faut 20 pièces." end
    return require("tuadialogue_text").format(source, args)
  end,
})
localized:start()
localized:advance()
assert(localized:setLocale("fr"))
assert(localized:snapshot().node.options[1].reason == "Il faut 20 pièces.")
local accepted = localized:snapshot()
reject = true
fails(function()
  localized:updateContext({ coins = 0, unlocked = false })
end, "translation failure")
assert(localized:getContext().unlocked == true)
assert(localized:snapshot().nodeId == accepted.nodeId)
assert(localized:snapshot().visit == accepted.visit)
assert(localized:snapshot().presentationRevision == accepted.presentationRevision)
reject = false
assert(localized:choose("shop.leave"))
localized:dispose()
other:dispose()

-- Programmatic documents receive the same loop, malformed data and target guards.
local raw = {
  schemaVersion = 3,
  entry = "cycle",
  nodes = {
    cycle = {
      kind = "branch",
      condition = { { op = "value", value = true } },
      yes = "cycle",
      no = "end",
    },
    ["end"] = { kind = "end" },
  },
}
local bounded = Dialogue.new(raw, { transitionBudget = 1 })
bounded:start()
assert(bounded:snapshot().state == "faulted")
bounded:start("end")
assert(bounded:snapshot().state == "completed")
raw.nodes.cycle.no = "unknown"
fails(function() Dialogue.new(raw) end, "branch target")
print("dialogue conditions lifecycle passed")

local visits = Conditions.compile({
  { op = "==" },
  { op = "path", path = "locals.visits" },
  { op = "value", value = 2 },
})
assert(Conditions.test(visits, {}, { visits = 2 }))
assert(not Conditions.test(visits, {}, { visits = 1 }))
local reordered = Conditions.compile({
  { op = "or" },
  { op = "and" },
  { op = "value", value = true },
  { op = "value", value = false },
  { op = "value", value = true },
})
assert(Conditions.test(reordered, {}, {}))

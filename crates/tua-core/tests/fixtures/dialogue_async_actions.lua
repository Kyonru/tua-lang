local Dialogue = require("tuadialogue")
local actions = require("actions")
local scene = require("scene")

local function create(handler)
  return actions.new(scene, { handlers = { purchase = handler } })
end

-- Completion is queued in a handle; only explicit dispatch advances the player.
local handle = Dialogue.actionHandle()
local calls = 0
local player = create(function(_, request)
  calls = calls + 1
  assert(request.status == "ready")
  return handle
end)
player:start()
assert(player:runAction())
assert(player:snapshot().action.status == "pending")
assert(not player:runAction() and calls == 1)
assert(handle:resolve(false))
assert(not handle:resolve(true) and not handle:reject("late"))
player:update(100, true)
assert(player:snapshot().action.status == "resolved")
assert(player:snapshot().nodeId == "buy")
assert(player:runAction())
assert(player:snapshot().nodeId == "declined")
player:dispose()

-- An already-settled handle still yields until the next runAction call.
local immediate = Dialogue.actionHandle()
assert(immediate:resolve(true))
player = create(function() return immediate end)
player:start()
assert(player:runAction() and player:snapshot().state == "awaiting_action")
assert(player:runAction() and player:snapshot().nodeId == "done")
player:dispose()

for _, operation in ipairs({ "start", "cancel", "dispose" }) do
  local cancelled = 0
  local pending
  pending = Dialogue.actionHandle({
    cancel = function()
      cancelled = cancelled + 1
      assert(not pending:resolve(true))
    end,
  })
  local current = create(function() return pending end)
  current:start()
  assert(current:runAction())
  current[operation](current)
  assert(cancelled == 1)
  assert(pending:snapshot().status == "cancelled")
  assert(not pending:reject("late failure"))
  current:dispose()
  assert(cancelled == 1)
end

-- Handles returned after reentrant replacement are cancelled, never attached.
for _, operation in ipairs({ "start", "cancel", "dispose" }) do
  local pending = Dialogue.actionHandle()
  local current
  current = create(function()
    current[operation](current)
    return pending
  end)
  current:start()
  assert(not current:runAction())
  assert(pending:snapshot().status == "cancelled")
  current:dispose()
end

-- External rejection faults once with original action/source identity.
local rejected = Dialogue.actionHandle()
player = create(function() return rejected end)
player:start()
assert(player:runAction())
assert(rejected:reject("host timeout"))
assert(not player:runAction())
local issue = player:snapshot().issue
assert(issue.code == "action_rejected" and issue.message == "host timeout")
assert(issue.actionId == "shop.purchase" and issue.source.line == 5)
assert(not player:runAction())
player:dispose()

-- Cancellation is distinct from a false domain outcome and cleanup cannot throw.
local cancelled = Dialogue.actionHandle({ cancel = function() error("cleanup failed") end })
player = create(function() return cancelled end)
player:start()
assert(player:runAction())
assert(cancelled:cancel() and not cancelled:cancel())
assert(cancelled:snapshot().cancellationError:find("cleanup failed", 1, true))
assert(not player:runAction())
assert(player:snapshot().issue.code == "action_cancelled")
player:dispose()

-- A handle cannot be shared by two players or reused on another visit.
local shared = Dialogue.actionHandle()
local first = create(function() return shared end)
local second = create(function() return shared end)
first:start()
second:start()
assert(first:runAction())
assert(not second:runAction())
assert(second:snapshot().issue.code == "action_result_invalid")
assert(shared:snapshot().status == "pending")
second:dispose()
assert(shared:resolve(true))
assert(first:runAction())
first:start()
assert(not first:runAction())
assert(first:snapshot().issue.code == "action_result_invalid")
first:dispose()

-- Concurrent handles remain independent; settlement does not invoke another action.
local left = Dialogue.actionHandle()
local right = Dialogue.actionHandle()
first = create(function() return left end)
second = create(function() return right end)
first:start()
second:start()
assert(first:runAction() and second:runAction())
assert(right:resolve(true))
assert(not first:runAction())
assert(second:runAction() and second:snapshot().nodeId == "done")
first:cancel()
assert(left:snapshot().status == "cancelled")
first:dispose()
second:dispose()

-- Snapshot mutation cannot settle a handle, and malformed values fail explicitly.
local owned = Dialogue.actionHandle()
owned:snapshot().status = "resolved"
assert(owned:snapshot().status == "pending")
assert(not pcall(owned.resolve, owned, "yes"))
assert(not pcall(owned.reject, owned, ""))
assert(not pcall(owned.reject, owned, string.rep("x", 4097)))
assert(not pcall(Dialogue.actionHandle, false))
assert(not pcall(Dialogue.actionHandle, { unknown = true }))
assert(not pcall(Dialogue.actionHandle, { cancel = false }))
player = create(function() return { resolve = function() end } end)
player:start()
assert(not player:runAction())
assert(player:snapshot().issue.code == "action_result_invalid")
player:dispose()

-- A retained host handle does not keep its former player alive.
local weak = setmetatable({}, { __mode = "v" })
local retained = Dialogue.actionHandle()
do
  local current = create(function() return retained end)
  current:start()
  assert(current:runAction())
  current:dispose()
  weak[1] = current
end
collectgarbage("collect")
assert(weak[1] == nil)
assert(retained:snapshot().status == "cancelled")
print("asynchronous dialogue action lifecycle passed")

-- Lifecycle cleanup may restart a player; the replacement remains authoritative.
local restarting
local restartHandle = Dialogue.actionHandle({
  cancel = function() restarting:start() end,
})
restarting = create(function() return restartHandle end)
restarting:start()
assert(restarting:runAction())
local generation = restarting:snapshot().generation
assert(restarting:cancel())
assert(restarting:snapshot().generation == generation + 2)
assert(restarting:snapshot().state == "awaiting_action")
assert(restarting:snapshot().action.status == "ready")
assert(not restartHandle:resolve(true))
restarting:dispose()

-- Failed presentation after settlement faults once without redispatching work.
local settled = Dialogue.actionHandle()
local transition = actions.new(scene, {
  locale = "en",
  handlers = { purchase = function() return settled end },
  resolve = function() error("translation failed") end,
})
transition:start()
assert(transition:runAction())
assert(settled:resolve(true))
assert(not transition:runAction())
assert(transition:snapshot().issue.code == "action_transition_error")
assert(not transition:runAction())
transition:dispose()

-- Lua 5.1 weak registries must not root a self-capturing cancellation closure.
do
  local pending
  pending = Dialogue.actionHandle({ cancel = function() pending:resolve(true) end })
  local current = create(function() return pending end)
  current:start()
  assert(current:runAction())
  weak[1], weak[2] = current, pending
end
collectgarbage("collect")
collectgarbage("collect")
assert(weak[1] == nil and weak[2] == nil)

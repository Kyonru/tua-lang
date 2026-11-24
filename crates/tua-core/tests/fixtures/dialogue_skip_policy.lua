local Dialogue = require("tuadialogue")
local Policy = require("skip-policy")
local Actions = require("forward")
local Scene = require("scene")
local function make(cleanup, track)
  local policy = Policy.new()
  local handle
  local player = Actions.new(Scene, {
    handlers = {
      purchase = function(_, request)
        if track == false then handle = Dialogue.actionHandle({ cancel = cleanup })
        else handle = policy.begin(request, cleanup) end
        return handle
      end,
    },
  })
  policy.bind(player)
  player:start()
  player:runAction()
  return policy, player, handle
end
local cleanups = 0
local policy, player, handle = make(function() cleanups = cleanups + 1 end)
local otherPolicy, other, otherHandle = make(nil)
assert(policy.skipConversation())
assert(cleanups == 1 and player:snapshot().state == "cancelled")
assert(other:snapshot().state == "awaiting_action")
assert(not otherPolicy.skipConversation())
assert(not handle:resolve(true))
assert(otherHandle:resolve(false))
local accepted, reason = otherPolicy.skipConversation()
assert(not accepted and reason == "consume_action_result")
assert(other:runAction())
assert(other:snapshot().nodeId == "declined")
assert(otherPolicy.skipConversation())
player:dispose()
other:dispose()

local foreign, unowned, unownedHandle = make(function() end, false)
accepted, reason = foreign.skipConversation()
assert(not accepted and reason == "unowned_action")
unowned:dispose()
assert(not unownedHandle:resolve(true))

local broken, brokenPlayer, brokenHandle = make(function() error("cleanup failed") end)
assert(broken.skipConversation())
assert(brokenPlayer:snapshot().state == "cancelled")
assert(brokenHandle:snapshot().cancellationError:find("cleanup failed", 1, true))
assert(not brokenHandle:resolve(true))
brokenPlayer:dispose()

local reentrantPlayer
local reentrant, boundPlayer, old = make(function() reentrantPlayer:start() end)
reentrantPlayer = boundPlayer
assert(reentrant.skipConversation())
assert(reentrantPlayer:snapshot().state == "awaiting_action")
assert(not old:resolve(true))
accepted, reason = reentrant.skipConversation()
assert(not accepted and reason == "action_not_started")
reentrantPlayer:dispose()

local waitingPolicy = Policy.new()
local waiting = Dialogue.new(require("wait"))
waitingPolicy.bind(waiting)
waiting:start()
assert(not waiting:advance(), "advance never bypasses waits")
assert(waitingPolicy.skipConversation())
assert(waiting:snapshot().state == "cancelled")
waiting:dispose()
assert(not pcall(waitingPolicy.bind, other), "policy cannot bind another player")
print("dialogue skip lifecycle passed")

-- A new pending visit cannot reuse the previous visit's cancellation capability.
local epochs = Policy.new()
local runs, stops = 0, 0
local latest
local restarted = Actions.new(Scene, {
  handlers = {
    purchase = function(_, request)
      runs = runs + 1
      if runs == 1 then
        latest = epochs.begin(request, function() stops = stops + 1 end)
      else
        latest = Dialogue.actionHandle()
      end
      return latest
    end,
  },
})
epochs.bind(restarted)
restarted:start()
restarted:runAction()
local previous = latest
restarted:start()
assert(stops == 1 and not previous:resolve(true))
restarted:runAction()
accepted, reason = epochs.skipConversation()
assert(not accepted and reason == "unowned_action")
assert(latest:snapshot().status == "pending")
restarted:dispose()
assert(not latest:resolve(true))
print("skip capabilities do not survive restart")

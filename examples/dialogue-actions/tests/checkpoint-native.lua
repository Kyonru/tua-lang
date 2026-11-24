local Save = require("tuasave")
local actions = require("actions")
local scene = require("scene")
local checkpointSchema = Save.record({
  version = Save.number(3),
  actionState = Save.enum({ "settled" }, "settled"),
  documentId = Save.string(""),
  contentVersion = Save.string(""),
  graph = Save.string(""),
  nodeId = Save.string(""),
  state = Save.enum({ "awaiting_advance", "awaiting_choice", "waiting" }, "awaiting_advance"),
  elapsed = Save.number(0),
  revealedElapsed = Save.optional(Save.number()),
  revealedAt = Save.optional(Save.number()),
  suspended = Save.boolean(false),
  locale = Save.optional(Save.string()),
  fired = Save.array(Save.number(), {}),
})
function love.load()
  local ok, issue = pcall(function()
    if os.getenv("TUA_CHECKPOINT_JIT") == "off" then jit.off() else jit.on() end
    print(_VERSION, jit.version, "JIT", jit.status())
    local phase = os.getenv("TUA_CHECKPOINT_PHASE")
    if phase == "fixtures" then
      assert(love.filesystem.load("v1.lua"))()
      assert(love.filesystem.load("v2.lua"))()
      assert(love.filesystem.load("v3.lua"))()
      return
    end
    local calls = 0
    local player = actions.new(scene, {
      checkpointVersion = "native-settled-v1",
      checkpointEffects = "settled",
      handlers = {
        purchase = function()
          calls = calls + 1
          return true
        end,
      },
    })
    local disk = Save.contract({
      slot = "settled_checkpoint_test",
      version = 1,
      schema = Save.record({ rewardCount = Save.number(0), checkpoint = checkpointSchema }),
    })
    if phase == "write" then
      local absent, loadIssue = disk:load()
      assert(absent == nil and loadIssue == nil)
      player:start()
      player:runAction()
      assert(calls == 1)
      assert(disk:save({ rewardCount = calls, checkpoint = player:checkpoint() }))
    else
      assert(phase == "read")
      local saved, loadIssue = disk:load()
      assert(saved and loadIssue == nil and saved.rewardCount == 1)
      player:restore(saved.checkpoint)
      assert(calls == 0 and not player:runAction())
      assert(player:snapshot().restored)
      assert(love.filesystem.remove("__tua_saves/settled_checkpoint_test.a"))
      assert(love.filesystem.remove("__tua_saves"))
    end
    player:dispose()
    print("settled checkpoint native phase passed: " .. phase)
  end)
  if not ok then print(tostring(issue)) end
  love.event.quit(ok and 0 or 1)
end

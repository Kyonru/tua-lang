function love.load()
  if os.getenv("TUA_DURABLE_JIT") == "off" then jit.off() else jit.on() end
  print("RUNTIME", love.getVersion(), jit.version, jit.status())
  local ok, message = pcall(function()
    local phase = os.getenv("TUA_DURABLE_PHASE")
    if phase == "fixtures" then
      require("recovery")
      return
    end
    local journal = require("journal").new()
    local flow = require("session").new()
    if phase == "prepare" then
      journal.initialize()
      local active = flow.begin()
      assert(active.key == "demo:1")
      assert(journal.load().coins == 10 and journal.load().shells == 0)
    elseif phase == "commit" then
      local active = assert(flow.recover())
      assert(journal.load().coins == 10)
      assert(active.player:runAction())
      assert(journal.load().coins == 5 and journal.load().shells == 1)
      -- Exit with a durable receipt but without acknowledgement.
    elseif phase == "replay" then
      local active = assert(flow.recover())
      assert(active.player:runAction())
      assert(journal.load().coins == 5 and journal.load().shells == 1)
      active.acknowledge()
    elseif phase == "verify" then
      assert(flow.recover() == nil)
      assert(journal.load().coins == 5 and journal.load().shells == 1)
      local nextPurchase = flow.begin()
      assert(nextPurchase.key == "demo:2")
      assert(nextPurchase.player:runAction())
      nextPurchase.acknowledge()
      assert(journal.load().coins == 0 and journal.load().shells == 2)
      -- Only this run's unique test identity is touched.
      assert(love.filesystem.remove("__tua_saves/dialogue_durable_demo.a"))
      assert(love.filesystem.remove("__tua_saves/dialogue_durable_demo.b"))
      assert(love.filesystem.remove("__tua_saves"))
    else error("unknown native test phase") end
    print("DURABLE_PHASE_OK", phase)
  end)
  if not ok then print(message) end
  love.event.quit(ok and 0 or 1)
end

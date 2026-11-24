function love.load()
  local ok, issue = pcall(function()
    if os.getenv("TUA_CHECKPOINT_JIT") == "off" then jit.off() else jit.on() end
    print(_VERSION, jit.version, "JIT", jit.status())
    assert(require("forward_demo"))
    local session = require("saved_session").new(20)
    local phase = os.getenv("TUA_CHECKPOINT_PHASE")
    if phase == "line" then
      assert(not session.load())
      session.player:start()
      session.player:setLocals({ visits = 3 })
      session.save()
    elseif phase == "choice" then
      assert(session.load())
      assert(session.player:getLocals().visits == 3)
      assert(session.player:snapshot().restored)
      session.player:advance()
      session.save()
    elseif phase == "wait" then
      assert(session.load())
      assert(session.player:snapshot().state == "awaiting_choice")
      session.player:start("tide")
      session.player:update(0.5, false)
      session.player:suspend()
      session.save()
    else
      assert(phase == "finish" and session.load())
      session.player:update(10, false)
      assert(session.player:snapshot().state == "waiting")
      session.player:resume()
      session.player:update(1.5, false)
      assert(session.player:snapshot().nodeId == "done")
      for _, slot in ipairs({ "a", "b" }) do
        assert(love.filesystem.remove("__tua_saves/dialogue_checkpoint_demo." .. slot))
      end
      assert(love.filesystem.remove("__tua_saves"))
    end
    session.player:dispose()
    print("native checkpoint phase passed: " .. phase)
  end)
  if not ok then print(tostring(issue)) end
  love.event.quit(ok and 0 or 1)
end

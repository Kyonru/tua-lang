-- Copied to an isolated build by verify.py; production generated files stay intact.
function love.load()
  if os.getenv("TUA_AUDIO_JIT") == "off" then jit.off() else jit.on() end
  print("RUNTIME", love.getVersion(), jit.version, jit.status())
  require("controller_test")
  require("backend_test")
  local Backend = require("backend")
  local first, second = Backend.new(), Backend.new()
  assert(first.chatter("guide", 1, 0.1))
  assert(first.voice("greeting", "en"))
  assert(second.voice("greeting", "en"))
  assert(first.speaking() and second.speaking())
  first.pause()
  assert(not first.speaking() and second.speaking())
  first.resume()
  assert(first.speaking())
  first.volume(0, 0)
  first.stop()
  assert(not first.speaking() and second.speaking())
  first.dispose()
  second.dispose()
  assert(love.audio.getActiveSourceCount() == 0)
  if os.getenv("TUA_AUDIO_RENDER") ~= "1" then
    print("NATIVE_AUDIO_OK")
    love.event.quit()
    return
  end
  require("showcase")
  local update, quit = love.update, love.quit
  local frame = 0
  function love.update(dt)
    frame = frame + 1
    if frame == 20 or frame == 30 then love.keypressed("p") end
    if frame == 40 or frame == 45 or frame == 50 or frame == 55 then love.keypressed("space") end
    if frame == 60 then love.keypressed("m") end
    if frame == 70 then love.keypressed("l") end
    if frame == 80 then love.keypressed("r") end
    update(dt)
    if frame == 100 then
      quit()
      assert(love.audio.getActiveSourceCount() == 0)
      print("RENDERED_AUDIO_OK")
      love.event.quit()
    end
  end
end

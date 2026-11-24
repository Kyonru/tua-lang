local Dialogue = require("tuadialogue")
local document = require("scene")
local benchmark = require("runtime_benchmark")
local players, views = {}, {}
local updateTimes, drawTimes = {}, {}
local frame = 0
local output = assert(os.getenv("TUA_DIALOGUE_BASELINE_OUT"))
function love.load()
  if jit then
    if os.getenv("TUA_DIALOGUE_BASELINE_JIT") == "on" then jit.on() else jit.off() end
  end
  print("RUNTIME\t" .. _VERSION .. "\t" .. tostring(jit and jit.version) .. "\t" .. tostring(jit and jit.status()))
  local major, minor, patch = love.getVersion()
  print("LOVE\t" .. major .. "." .. minor .. "." .. patch)
  dofile("ownership.lua")
  dofile("compatibility.lua")
  benchmark.run()
  for index = 1, 16 do
    players[index] = Dialogue.new(document)
    players[index]:start()
  end
end
function love.update()
  local began = os.clock()
  frame = frame + 1
  for index, player in ipairs(players) do
    player:update(1 / 60, true)
    views[index] = player:snapshot()
    assert(views[index].node.speaker == "Guide")
  end
  if frame > 60 then updateTimes[#updateTimes + 1] = (os.clock() - began) * 1000 end
end
function love.draw()
  local began = os.clock()
  love.graphics.clear(0.08, 0.12, 0.16)
  for index, view in ipairs(views) do
    local x = 20 + ((index - 1) % 4) * 235
    local y = 20 + math.floor((index - 1) / 4) * 145
    love.graphics.setColor(0.18, 0.25, 0.32)
    love.graphics.rectangle("fill", x, y, 220, 120, 8, 8)
    love.graphics.setColor(1, 1, 1)
    love.graphics.print(view.node.speaker, x + 12, y + 12)
    love.graphics.printf(view.node.text, x + 12, y + 42, 196)
    love.graphics.print("Independent player " .. index, x + 12, y + 90)
  end
  if frame > 60 then drawTimes[#drawTimes + 1] = (os.clock() - began) * 1000 end
  if frame == 240 then
    benchmark.report("rendered_16_players_update", "cpu_ms_per_callback", updateTimes)
    benchmark.report("rendered_16_panels_draw", "cpu_ms_per_callback", drawTimes)
    love.graphics.captureScreenshot(function(data)
      local encoded = data:encode("png")
      local file = assert(io.open(output .. "/render.png", "wb"))
      file:write(encoded:getString())
      file:close()
      encoded:release()
      data:release()
      for _, player in ipairs(players) do player:dispose() end
      print("RENDER_OK\t240")
      love.event.quit()
    end)
  end
end

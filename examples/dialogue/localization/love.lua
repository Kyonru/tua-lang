-- Native correctness host. Copy into an isolated emitted build as main.lua.
local Dialogue = require("tuadialogue")
local I18n = require("tuai18n")
local scene = require("scene")
local players, seen, views = {}, {}, {}
local frame = 0
local output = assert(os.getenv("TUA_DIALOGUE_NATIVE_OUT"))
function love.load()
  if os.getenv("TUA_DIALOGUE_NATIVE_JIT") == "on" then jit.on() else jit.off() end
  print("RUNTIME", _VERSION, jit.version, jit.status(), love.getVersion())
  dofile("typed.lua")
  dofile("oracle.lua")
  for index, locale in ipairs({ "en", "fr-CA" }) do
    players[index] = Dialogue.new(
      scene,
      { locale = locale, resolve = I18n.dialogue, args = { name = "Zoé", count = index } }
    )
    players[index]:start()
    seen[index] = {}
  end
end
function love.update()
  frame = frame + 1
  for index, player in ipairs(players) do
    if frame == 60 and index == 2 then player:setLocale("en") end
    if frame == 90 and index == 2 then player:setLocale("fr") end
    if frame == 120 then player:advance() end
    if frame == 150 then assert(player:choose("harbor.leave")) end
    if frame == 180 then
      player:cancel()
      player:start()
    end
    for _, event in ipairs(player:update(1 / 60, frame % 60 >= 20)) do
      local identity = event.generation .. ":" .. event.visit .. ":" .. event.name
      assert(not seen[index][identity], "event replayed during locale refresh")
      seen[index][identity] = true
    end
    views[index] = player:snapshot()
    assert(views[index].node)
  end
  assert(I18n.getLocale() == "en")
end
function love.draw()
  love.graphics.clear(0.06, 0.09, 0.13)
  for index, view in ipairs(views) do
    local y = 36 + (index - 1) * 190
    love.graphics.setColor(0.14, 0.2, 0.27)
    love.graphics.rectangle("fill", 30, y, 740, 160, 10, 10)
    love.graphics.setColor(1, 1, 1)
    love.graphics.print("Player " .. index .. " · " .. view.locale, 50, y + 18)
    love.graphics.print(view.node.speaker or "Narrator", 50, y + 46)
    love.graphics.printf(view.node.text, 50, y + 76, 700)
    for offset, option in ipairs(view.node.options) do
      love.graphics.print(option.id .. " · " .. option.text, 50, y + 60 + offset * 24)
    end
  end
  if frame == 240 then
    love.graphics.captureScreenshot(function(data)
      local encoded = data:encode("png")
      local file = assert(io.open(output, "wb"))
      file:write(encoded:getString())
      file:close()
      encoded:release()
      data:release()
      for _, player in ipairs(players) do player:dispose() end
      print("LOCALIZATION_NATIVE_OK", frame)
      love.event.quit()
    end)
  end
end

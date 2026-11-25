local host
local function firstEnemy()
  local selected
  host.wave.enemies:each(function(id)
    if not selected or id < selected then selected = id end
  end)
  return selected
end
local function shootFirst()
  local id = firstEnemy()
  if not id then return end
  local position = host.wave.readPosition(id)
  love.mousepressed(position.x, position.y - 28, 1)
end
local function setup(context)
  host = require("__tua_replay_game")
  assert(host.wave.enemies:count() == 8 and not host.options.effects)
end
local function beforeFrame(context)
  local frame = context.frame
  if frame == 15 then
    shootFirst()
    assert(host.wave.readHealth(firstEnemy()).current == 1)
  end
  if frame == 30 then
    love.keypressed("e")
    shootFirst()
    assert(host.wave.enemies:count() == 7 and host.effects.activeCount() == 1)
  end
  if frame == 50 then assert(host.effects.activeCount() == 0) end
  if frame >= 60 and frame <= 90 then shootFirst() end
  if frame == 95 then assert(host.wave.enemies:count() == 0) end
  if frame == 110 then
    love.keypressed("space")
    love.keypressed("space")
    assert(host.wave.enemies:count() == 8 and host.wave.state.wave == 2)
  end
  if frame == 130 then
    shootFirst()
    shootFirst()
    assert(host.effects.activeCount() == 1)
    love.keypressed("r")
    assert(host.effects.activeCount() == 0 and host.wave.state.wave == 1)
  end
end
local function verify(context)
  assert(host.wave.enemies:count() == 8 and host.wave.state.defeated == 0)
  print("WAVE_UI_OK damage, defeat, ring, clear, next wave, reset")
end
return { setup = setup, beforeFrame = beforeFrame, verify = verify }

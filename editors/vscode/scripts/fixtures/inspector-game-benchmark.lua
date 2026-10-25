-- Appended only to a temporary, freshly built copy of the zombie game.
local range, gun, shared = require('game.range'), require('player.gun'), require('core.state')
local warmup = tonumber(os.getenv('TUA_GAME_WARMUP_FRAMES')) or 180
local measuredFrames = tonumber(os.getenv('TUA_GAME_FRAMES')) or 600
local frame = 0
love.mouse.getPosition = function() return 480 + math.sin(frame / 60) * 160, 270 end
love.mouse.isDown = function() return false end
love.keyboard.isDown = function() return false end
function love.run()
  if love.load then love.load({}) end
  local previousStart, measurementStart, previousMeasured
  local work = {}
  return function()
    local now = love.timer.getTime()
    if previousMeasured then work[#work + 1] = (now - previousStart) * 1000 end
    if #work >= measuredFrames then
      local elapsed = now - measurementStart
      local encoded = {}; for i, value in ipairs(work) do encoded[i] = string.format('%.6f', value) end
      print(string.format('GAME_RESULT {"elapsed":%.6f,"fps":%.6f,"frames":%d,"workMs":[%s]}', elapsed, #work / elapsed, #work, table.concat(encoded, ',')))
      return 0
    end
    if previousStart then love.timer.sleep(math.max(0, 1 / 60 - (love.timer.getTime() - previousStart))) end
    previousStart = love.timer.getTime()
    frame = frame + 1
    previousMeasured = frame > warmup
    if previousMeasured and not measurementStart then measurementStart = previousStart end
    love.event.pump()
    for name in love.event.poll() do if name == 'quit' then return 1 end end
    if love.update then love.update(1 / 60) end
    if frame % 12 == 0 then
      local player = shared.player:get()
      local x, y = gun.getMuzzlePosition()
      range.shoot(player.pointing.x, player.pointing.y, x, y)
    end
    if love.graphics.isActive() then
      love.graphics.origin()
      love.graphics.clear(love.graphics.getBackgroundColor())
      if love.draw then love.draw() end
      love.graphics.present()
    end
  end
end

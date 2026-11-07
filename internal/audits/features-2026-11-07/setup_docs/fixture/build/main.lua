local x = 80
local speed = 120

function love.update(dt)
  x = x + (speed * dt)
end

function love.draw()
  love.graphics.circle("fill", x, 120, 24)
end

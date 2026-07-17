local inspect = require("inspect")

local background = {}

function background.load(r, g, b, a)
  background.list = {}
  love.graphics.setBackgroundColor(r, g, b, a)
end

function background.update(dt)
  for i = 1, #background.list do
    local parallax = background.list[i]
    parallax.x = parallax.x - parallax.speed
  end
end

function background.draw()
  for i = 1, #background.list do
    local parallax = background.list[i]
    background.draw_background(parallax)
  end
end

function background.draw_background(parallax)
  if parallax.x < -screen_width then
    parallax.x = screen_width - 1
  end
  love.graphics.draw(
    parallax.sprite,
    parallax.x,
    parallax.y,
    0,
    screen_width / parallax.width,
    parallax.background_height / parallax.height
  )

  if inspect.inspecting then
    love.graphics.setColor(parallax.inspectingColor)
    love.graphics.setLineWidth(12)
    love.graphics.rectangle(
      "line",
      parallax.x,
      parallax.y,
      screen_width,
      parallax.background_height
    )
    love.graphics.setColor(1, 1, 1, 1)
  end
end

function background.create_infinite_parallax(sprite, background_height, speed)
  local parallax_1 = {
    sprite = sprite,
    width = sprite:getWidth(),
    height = sprite:getHeight(),
    background_height = background_height,
    x = 0,
    y = screen_height - background_height,
    inspectingColor = { 1, 0, 0, 0.5 },
    speed = speed,
  }
  local parallax_2 = {
    sprite = sprite,
    width = sprite:getWidth(),
    height = sprite:getHeight(),
    background_height = background_height,
    x = screen_width,
    y = screen_height - background_height,
    inspectingColor = { 0, 1, 0, 0.5 },
    speed = speed,
  }

  background.list[#background.list + 1] = parallax_1
  background.list[#background.list + 1] = parallax_2

  return parallax_1, parallax_2
end

return background

local inspect = {
  inspecting = false,
}

local canvas

function inspect.load()
  canvas = love.graphics.newCanvas(_WIDTH * 2, _HEIGHT)
  canvas:setFilter("nearest", "nearest")
  inspect.inspecting = false
end

function inspect.update(dt)
end

function inspect.draw(fn)
  love.graphics.setCanvas(canvas)
  love.graphics.push("all")
  love.graphics.clear()
  if inspect.inspecting then
    love.graphics.translate(_WIDTH / 4, 0)
    love.graphics.scale(0.50, 0.50)
  end
  fn()
  love.graphics.pop()
  love.graphics.setCanvas()

  local x, y = 0, 0

  love.graphics.draw(canvas, x, y)
end

function inspect.keypressed(key)
  if key == "i" then
    inspect.inspecting = not inspect.inspecting
  end
end

return inspect

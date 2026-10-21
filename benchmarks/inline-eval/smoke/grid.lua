-- Inline-completion smoke fixture (plain Lua).

local Grid = {}
Grid.__index = Grid

function Grid.new(width, height)
  local self = setmetatable({}, Grid)
  self.width = width
  self.height = height
  self.cells = {}
  for y = 1, height do
    self.cells[y] = {}
    for x = 1, width do
      self.cells[y][x] = 0
    end
  end
  return self
end

function Grid:get(x, y)
  if x < 1 or x > self.width or y < 1 or y > self.height then
    return nil
  end
  return self.cells[y][x]
end

function Grid:set(x, y, value)
  if x < 1 or x > self.width or y < 1 or y > self.height then
    return
  end
  self.cells[y][x] = value
end

function Grid:count(value)
  local total = 0
  for y = 1, self.height do
    for x = 1, self.width do
      if self.cells[y][x] == value then
        total = total + 1
      end
    end
  end
  return total
end

return Grid

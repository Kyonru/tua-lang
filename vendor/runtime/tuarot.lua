--
-- tuarot: optional, deterministic game-generation helpers for Tua and LÖVE.
-- This is an independent implementation inspired by the feature areas in
-- rotLove. It has no third-party runtime dependency and is copied into a build
-- only when a source file references the `Rot` module.
-- Copyright (c) 2026 Kyonru. MIT License.
--

---@alias RotMapAlgorithm "arena" | "binaryTree" | "sidewinder"
---@class RotPoint
---@field x number
---@field y number

---@class RotGrid
---@field width number
---@field height number
---@field get fun(self: RotGrid, x: number, y: number): boolean
---@field set fun(self: RotGrid, x: number, y: number, walkable: boolean)

---@class RotColorGrid
---@field width number
---@field height number
---@field get fun(self: RotColorGrid, x: number, y: number): RotColorValue

---@class RotColorValue
---@field r number red, from 0 to 1
---@field g number green, from 0 to 1
---@field b number blue, from 0 to 1
---@field a number alpha, from 0 to 1

---@class RotRng
---@field next fun(self: RotRng): number deterministic value in [0, 1)
---@field int fun(self: RotRng, min: number, max: number): number inclusive range
---@field float fun(self: RotRng, min: number, max: number): number
---@field choice fun(self: RotRng, values: any[]): any
---@field shuffle fun(self: RotRng, values: any[]): any[] shuffled copy

---@class RotNoise
---@field sample fun(self: RotNoise, x: number, y: number): number smooth value in [-1, 1]

---@class RotRngModule
---@field new fun(seed: number): RotRng

---@class RotColorModule
---@field parse fun(value: string): RotColorValue
---@field mix fun(a: RotColorValue, b: RotColorValue, amount: number): RotColorValue
---@field add fun(a: RotColorValue, b: RotColorValue): RotColorValue
---@field multiply fun(a: RotColorValue, b: RotColorValue): RotColorValue

---@class RotDiceModule
---@field roll fun(expression: string, rng: RotRng): number, number[]

---@class RotMapModule
---@field arena fun(width: number, height: number): RotGrid
---@field generate fun(width: number, height: number, algorithm: RotMapAlgorithm, rng: RotRng): RotGrid

---@class RotPathModule
---@field astar fun(grid: RotGrid, startX: number, startY: number, goalX: number, goalY: number, passable?: fun(cell: boolean, x: number, y: number): boolean): RotPoint[]|nil
---@field dijkstra fun(grid: RotGrid, startX: number, startY: number, goalX: number, goalY: number, passable?: fun(cell: boolean, x: number, y: number): boolean): RotPoint[]|nil

---@class RotFovModule
---@field compute fun(width: number, height: number, originX: number, originY: number, radius: number, opaque: fun(x: number, y: number): boolean): RotGrid

---@class RotNoiseModule
---@field new fun(seed: number): RotNoise

---@class RotStringGeneratorModule
---@field generate fun(grammar: any, start: string, rng: RotRng, maxDepth?: number): string

---@class RotLightingModule
---@field compute fun(width: number, height: number, lights: any[], opaque?: fun(x: number, y: number): boolean): RotColorGrid

---@class Rot
---@field Rng RotRngModule seeded random generators
---@field Color RotColorModule color parsing and interpolation
---@field Dice RotDiceModule roguelike dice expressions
---@field Map RotMapModule grid and maze generation
---@field Path RotPathModule A-star and Dijkstra grid search
---@field FOV RotFovModule grid visibility
---@field Noise RotNoiseModule deterministic 2D noise
---@field StringGenerator RotStringGeneratorModule recursive grammar expansion
---@field Lighting RotLightingModule radial light accumulation

local Rot = {}

local function finite(value)
  return type(value) == "number" and value == value and value ~= math.huge and value ~= -math.huge
end

local function integer(value, name)
  assert(finite(value) and value == math.floor(value), name .. " must be an integer")
  return value
end

local function clamp(value, low, high)
  return math.max(low, math.min(high, value))
end

---@type RotRngModule
local Rng = {}
Rng.__index = Rng

---Create a repeatable random stream without changing LOVE's global RNG.
---@param seed number
---@return RotRng
function Rng.new(seed)
  seed = integer(seed, "seed") % 2147483647
  if seed <= 0 then seed = seed + 2147483646 end
  return setmetatable({ state = seed }, Rng)
end

---@return number
function Rng:next()
  self.state = (self.state * 16807) % 2147483647
  return (self.state - 1) / 2147483646
end

---@param min number
---@param max number
---@return number
function Rng:int(min, max)
  min, max = integer(min, "min"), integer(max, "max")
  assert(max >= min and max - min <= 2147483646, "integer range exceeds the seeded RNG's supported range")
  return min + math.floor(self:next() * (max - min + 1))
end

---@param min number
---@param max number
---@return number
function Rng:float(min, max)
  assert(finite(min) and finite(max) and max >= min, "invalid float range")
  return min + self:next() * (max - min)
end

---@param values any[]
---@return any
function Rng:choice(values)
  assert(#values > 0, "cannot choose from an empty list")
  return values[self:int(1, #values)]
end

---@param values any[]
---@return any[]
function Rng:shuffle(values)
  local result = {}
  for i, value in ipairs(values) do result[i] = value end
  for i = #result, 2, -1 do
    local j = self:int(1, i)
    result[i], result[j] = result[j], result[i]
  end
  return result
end

Rot.Rng = Rng

---@type RotColorModule
local Color = {}
local namedColors = {
  black = { 0, 0, 0 }, white = { 1, 1, 1 }, red = { 1, 0, 0 }, green = { 0, 0.5, 0 },
  blue = { 0, 0, 1 }, yellow = { 1, 1, 0 }, cyan = { 0, 1, 1 }, magenta = { 1, 0, 1 },
  orange = { 1, 0.647, 0 }, purple = { 0.5, 0, 0.5 }, gray = { 0.5, 0.5, 0.5 }, grey = { 0.5, 0.5, 0.5 },
  transparent = { 0, 0, 0, 0 },
}

---Parse #RGB, #RRGGBB, #RRGGBBAA, or one of the documented basic color names.
---@param value string
---@return RotColorValue
function Color.parse(value)
  assert(type(value) == "string", "color must be a string")
  local hex = value:match("^#([%da-fA-F]+)$")
  if hex then
    if #hex == 3 then
      local r, g, b = hex:match("^(%x)(%x)(%x)$")
      hex = r .. r .. g .. g .. b .. b
    end
    assert(#hex == 6 or #hex == 8, "hex color must have 3, 6, or 8 digits")
    local r, g, b, a = hex:match("^(%x%x)(%x%x)(%x%x)(%x?%x?)$")
    return { r = tonumber(r, 16) / 255, g = tonumber(g, 16) / 255, b = tonumber(b, 16) / 255, a = a ~= "" and tonumber(a, 16) / 255 or 1 }
  end
  local rgb = namedColors[value:lower()]
  assert(rgb, "unknown color name: " .. value)
  return { r = rgb[1], g = rgb[2], b = rgb[3], a = rgb[4] or 1 }
end

---@param a RotColorValue
---@param b RotColorValue
---@param amount number
---@return RotColorValue
function Color.mix(a, b, amount)
  assert(finite(amount), "amount must be finite")
  amount = clamp(amount, 0, 1)
  return { r = a.r + (b.r - a.r) * amount, g = a.g + (b.g - a.g) * amount, b = a.b + (b.b - a.b) * amount, a = a.a + (b.a - a.a) * amount }
end

---@param a RotColorValue
---@param b RotColorValue
---@return RotColorValue
function Color.add(a, b)
  return { r = clamp(a.r + b.r, 0, 1), g = clamp(a.g + b.g, 0, 1), b = clamp(a.b + b.b, 0, 1), a = clamp(a.a + b.a, 0, 1) }
end

---@param a RotColorValue
---@param b RotColorValue
---@return RotColorValue
function Color.multiply(a, b)
  return { r = a.r * b.r, g = a.g * b.g, b = a.b * b.b, a = a.a * b.a }
end

Rot.Color = Color

---@type RotDiceModule
local Dice = {}

---Roll `NdM`, `dM`, and optional signed constant expressions with a local RNG.
---@param expression string
---@param rng RotRng
---@return number total
---@return number[] rolls
function Dice.roll(expression, rng)
  assert(type(expression) == "string", "dice expression must be a string")
  assert(type(rng) == "table" and type(rng.next) == "function", "dice roll requires a seeded RNG")
  local count, sides, sign, bonus = expression:lower():match("^%s*(%d*)d(%d+)%s*([+-]?)%s*(%d*)%s*$")
  assert(sides, "invalid dice expression")
  count, sides = tonumber(count == "" and "1" or count), tonumber(sides)
  bonus = tonumber(bonus == "" and "0" or bonus) * (sign == "-" and -1 or 1)
  assert((sign == "" or expression:match("[+-]%s*%d+%s*$")) and count >= 1 and count <= 1000 and sides >= 1 and sides <= 1000000000, "dice expression is invalid or exceeds supported limits")
  local rolls, total = {}, bonus
  for i = 1, count do rolls[i] = rng:int(1, sides); total = total + rolls[i] end
  return total, rolls
end

Rot.Dice = Dice

---@type RotMapModule
local Map = {}

local Grid = {}
Grid.__index = Grid

---@param x number
---@param y number
---@return boolean
function Grid:get(x, y)
  x, y = integer(x, "x"), integer(y, "y")
  if x < 1 or y < 1 or x > self.width or y > self.height then return false end
  return self.cells[(y - 1) * self.width + x] == true
end

---@param x number
---@param y number
---@param walkable boolean
function Grid:set(x, y, walkable)
  x, y = integer(x, "x"), integer(y, "y")
  assert(x >= 1 and y >= 1 and x <= self.width and y <= self.height and type(walkable) == "boolean", "invalid grid coordinate or value")
  self.cells[(y - 1) * self.width + x] = walkable
end

local function makeGrid(width, height, initial)
  width, height = integer(width, "width"), integer(height, "height")
  assert(width >= 1 and height >= 1 and width * height <= 250000, "grid dimensions exceed supported limits")
  local grid = { width = width, height = height, cells = {} }
  for i = 1, width * height do grid.cells[i] = initial end
  return setmetatable(grid, Grid)
end

---Create a fully-walled rectangular arena.
---@param width number
---@param height number
---@return RotGrid grid with true cells walkable
function Map.arena(width, height)
  return makeGrid(width, height, false)
end

---Generate an arena, binary-tree maze, or sidewinder maze.
---@param width number
---@param height number
---@param algorithm RotMapAlgorithm
---@param rng RotRng
---@return RotGrid grid with true cells walkable
function Map.generate(width, height, algorithm, rng)
  width, height = integer(width, "width"), integer(height, "height")
  assert(width >= 3 and height >= 3 and width * height <= 250000, "maze dimensions exceed supported limits")
  assert(algorithm == "arena" or algorithm == "binaryTree" or algorithm == "sidewinder", "unsupported map algorithm")
  assert(type(rng) == "table" and type(rng.next) == "function", "map generation requires a seeded RNG")
  local grid = makeGrid(width, height, false)
  if algorithm == "arena" then return grid end
  local maxX, maxY = width - 1, height - 1
  for y = 2, maxY, 2 do
    for x = 2, maxX, 2 do grid:set(x, y, true) end
  end
  if algorithm == "binaryTree" then
    for y = 2, maxY, 2 do
      for x = 2, maxX, 2 do
        local choices = {}
        if y > 2 then choices[#choices + 1] = { x, y - 1 } end
        if x > 2 then choices[#choices + 1] = { x - 1, y } end
        if #choices > 0 then local chosen = rng:choice(choices); grid:set(chosen[1], chosen[2], true) end
      end
    end
  elseif algorithm == "sidewinder" then
    for y = 2, maxY, 2 do
      local run = {}
      for x = 2, maxX, 2 do
        run[#run + 1] = x
        local atEast = x == maxX
        local atNorth = y == 2
        if atEast or (not atNorth and rng:int(0, 1) == 0) then
          local chosen = run[rng:int(1, #run)]
          if not atNorth then grid:set(chosen, y - 1, true) end
          run = {}
        else
          grid:set(x + 1, y, true)
        end
      end
    end
  end
  return grid
end

Rot.Map = Map

---@type RotPathModule
local Path = {}
local dirs = { { 1, 0 }, { 0, 1 }, { -1, 0 }, { 0, -1 } }

local function heapPush(heap, item)
  local index = #heap + 1
  while index > 1 do
    local parent = math.floor(index / 2)
    if heap[parent].priority <= item.priority then break end
    heap[index], index = heap[parent], parent
  end
  heap[index] = item
end

local function heapPop(heap)
  local first, last = heap[1], table.remove(heap)
  if #heap == 0 then return first end
  local index = 1
  while index * 2 <= #heap do
    local child = index * 2
    if child + 1 <= #heap and heap[child + 1].priority < heap[child].priority then child = child + 1 end
    if heap[child].priority >= last.priority then break end
    heap[index], index = heap[child], child
  end
  heap[index] = last
  return first
end

local function search(grid, startX, startY, goalX, goalY, passable, heuristic)
  local height, width = grid.height, grid.width
  assert(width > 0 and height * width <= 250000, "grid must not be empty or exceed supported limits")
  startX, startY = integer(startX, "startX"), integer(startY, "startY")
  goalX, goalY = integer(goalX, "goalX"), integer(goalY, "goalY")
  assert(type(passable) == "function", "passable callback must be a function")
  local function valid(x, y) return x >= 1 and y >= 1 and x <= width and y <= height and passable(grid:get(x, y), x, y) end
  if not valid(startX, startY) or not valid(goalX, goalY) then return nil end
  local start = (startY - 1) * width + startX
  local goal = (goalY - 1) * width + goalX
  local dist, previous, open, closed = { [start] = 0 }, {}, { { id = start, cost = 0, priority = heuristic(startX, startY) } }, {}
  while #open > 0 do
    local item = heapPop(open)
    local current = item.id
    if not closed[current] and item.cost == dist[current] then
      if current == goal then
      local result, id = {}, goal
      while id do local x = (id - 1) % width + 1; local y = math.floor((id - 1) / width) + 1; result[#result + 1] = { x = x, y = y }; if id == start then break end; id = previous[id] end
      for i = 1, math.floor(#result / 2) do result[i], result[#result - i + 1] = result[#result - i + 1], result[i] end
      return result, dist[goal]
      end
      closed[current] = true
      local x, y = (current - 1) % width + 1, math.floor((current - 1) / width) + 1
      for _, dir in ipairs(dirs) do
        local nx, ny = x + dir[1], y + dir[2]
        if valid(nx, ny) then
          local id, cost = (ny - 1) * width + nx, dist[current] + 1
          if not closed[id] and (dist[id] == nil or cost < dist[id]) then
            dist[id], previous[id] = cost, current
            heapPush(open, { id = id, cost = cost, priority = cost + heuristic(nx, ny) })
          end
        end
      end
    end
  end
  return nil
end

---Find a shortest four-way route through true grid cells using A-star.
---@param grid RotGrid
---@param startX number
---@param startY number
---@param goalX number
---@param goalY number
---@param passable? fun(cell: boolean, x: number, y: number): boolean
---@return RotPoint[]|nil path
function Path.astar(grid, startX, startY, goalX, goalY, passable)
  assert(type(grid) == "table" and type(grid.get) == "function", "grid must be a RotGrid")
  passable = passable or function(cell) return cell == true end
  return search(grid, startX, startY, goalX, goalY, passable, function(x, y) return math.abs(goalX - x) + math.abs(goalY - y) end)
end

---Find a shortest four-way route with Dijkstra's algorithm.
---@param grid RotGrid
---@param startX number
---@param startY number
---@param goalX number
---@param goalY number
---@param passable? fun(cell: boolean, x: number, y: number): boolean
---@return RotPoint[]|nil path
function Path.dijkstra(grid, startX, startY, goalX, goalY, passable)
  assert(type(grid) == "table" and type(grid.get) == "function", "grid must be a RotGrid")
  passable = passable or function(cell) return cell == true end
  return search(grid, startX, startY, goalX, goalY, passable, function() return 0 end)
end

Rot.Path = Path

---@type RotFovModule
local FOV = {}

local function lineVisible(x0, y0, x1, y1, opaque)
  local dx, dy = math.abs(x1 - x0), math.abs(y1 - y0)
  local sx, sy = x0 < x1 and 1 or -1, y0 < y1 and 1 or -1
  local err = dx - dy
  while x0 ~= x1 or y0 ~= y1 do
    local twice = 2 * err
    if twice > -dy then err = err - dy; x0 = x0 + sx end
    if twice < dx then err = err + dx; y0 = y0 + sy end
    if x0 == x1 and y0 == y1 then return true end
    if opaque(x0, y0) then return false end
  end
  return true
end

---Return visible cells using bounded line-of-sight ray casting.
---@param width number
---@param height number
---@param originX number
---@param originY number
---@param radius number
---@param opaque fun(x: number, y: number): boolean
---@return RotGrid cells indexed by x and y
function FOV.compute(width, height, originX, originY, radius, opaque)
  width, height, radius = integer(width, "width"), integer(height, "height"), integer(radius, "radius")
  assert(width > 0 and height > 0 and width * height <= 250000 and radius >= 0 and radius <= 64, "FOV dimensions exceed supported limits")
  assert(type(opaque) == "function", "opaque callback is required")
  originX, originY = integer(originX, "originX"), integer(originY, "originY")
  local visible = makeGrid(width, height, false)
  if originX < 1 or originY < 1 or originX > width or originY > height then return visible end
  for y = math.max(1, originY - radius), math.min(height, originY + radius) do
    for x = math.max(1, originX - radius), math.min(width, originX + radius) do
      local dx, dy = x - originX, y - originY
      if dx * dx + dy * dy <= radius * radius and lineVisible(originX, originY, x, y, opaque) then visible:set(x, y, true) end
    end
  end
  return visible
end

Rot.FOV = FOV

---@type RotNoiseModule
local Noise = {}
Noise.__index = Noise
local gradients = { { 1, 0 }, { -1, 0 }, { 0, 1 }, { 0, -1 }, { 0.707, 0.707 }, { -0.707, 0.707 }, { 0.707, -0.707 }, { -0.707, -0.707 } }

---Create deterministic 2D simplex noise with a seeded permutation.
---@param seed number
---@return RotNoise
function Noise.new(seed)
  local rng, permutation = Rng.new(seed), {}
  for i = 0, 255 do permutation[i] = i end
  for i = 255, 1, -1 do local j = rng:int(0, i); permutation[i], permutation[j] = permutation[j], permutation[i] end
  for i = 0, 255 do permutation[i + 256] = permutation[i] end
  return setmetatable({ permutation = permutation }, Noise)
end

---@param x number
---@param y number
---@return number
function Noise:sample(x, y)
  assert(finite(x) and finite(y), "noise coordinates must be finite")
  local f2, g2 = 0.3660254037844386, 0.21132486540518713
  local skew = (x + y) * f2
  local i, j = math.floor(x + skew), math.floor(y + skew)
  local unskew = (i + j) * g2
  local x0, y0 = x - (i - unskew), y - (j - unskew)
  local i1, j1 = x0 > y0 and 1 or 0, x0 > y0 and 0 or 1
  local x1, y1 = x0 - i1 + g2, y0 - j1 + g2
  local x2, y2 = x0 - 1 + 2 * g2, y0 - 1 + 2 * g2
  local p = self.permutation
  local ii, jj = i % 256, j % 256
  local function corner(dx, dy, offset)
    local t = 0.5 - dx * dx - dy * dy
    if t < 0 then return 0 end
    local gradient = gradients[p[(ii + offset[1] + p[(jj + offset[2]) % 256]) % 256] % 8 + 1]
    t = t * t
    return t * t * (gradient[1] * dx + gradient[2] * dy)
  end
  return clamp(70 * (corner(x0, y0, { 0, 0 }) + corner(x1, y1, { i1, j1 }) + corner(x2, y2, { 1, 1 })), -1, 1)
end

Rot.Noise = Noise

---@type RotStringGeneratorModule
local StringGenerator = {}

---Expand `{rule}` references from a grammar table with a recursion bound.
---@param grammar any
---@param start string
---@param rng RotRng
---@param maxDepth? number
---@return string
function StringGenerator.generate(grammar, start, rng, maxDepth)
  assert(type(grammar) == "table" and type(start) == "string", "grammar and start rule are required")
  assert(type(rng) == "table" and type(rng.next) == "function", "grammar generation requires a seeded RNG")
  maxDepth = maxDepth or 32
  maxDepth = integer(maxDepth, "maxDepth")
  assert(maxDepth >= 0 and maxDepth <= 256, "maxDepth must be between 0 and 256")
  local function expand(rule, depth)
    assert(depth <= maxDepth, "grammar recursion limit exceeded")
    local options = grammar[rule]
    assert(options, "undefined grammar rule: " .. rule)
    local selected = type(options) == "string" and options or rng:choice(options)
    assert(type(selected) == "string", "grammar alternatives must be strings")
    return (selected:gsub("{([%w_]+)}", function(child) return expand(child, depth + 1) end))
  end
  return expand(start, 0)
end

Rot.StringGenerator = StringGenerator

---@type RotLightingModule
local Lighting = {}

local ColorGrid = {}
ColorGrid.__index = ColorGrid

---@param x number
---@param y number
---@return RotColorValue
function ColorGrid:get(x, y)
  x, y = integer(x, "x"), integer(y, "y")
  if x < 1 or y < 1 or x > self.width or y > self.height then return { r = 0, g = 0, b = 0, a = 1 } end
  return self.cells[(y - 1) * self.width + x]
end

local function makeColorGrid(width, height)
  local grid = { width = width, height = height, cells = {} }
  for i = 1, width * height do grid.cells[i] = { r = 0, g = 0, b = 0, a = 1 } end
  return setmetatable(grid, ColorGrid)
end

---Accumulate radial point lights into a flat color grid.
---@param width number
---@param height number
---@param lights any[] entries have x, y, radius, and color
---@param opaque? fun(x: number, y: number): boolean blocks light between cells
---@return RotColorGrid
function Lighting.compute(width, height, lights, opaque)
  width, height = integer(width, "width"), integer(height, "height")
  assert(width > 0 and height > 0 and width * height <= 65536, "lighting dimensions exceed supported limits")
  assert(type(lights) == "table" and #lights <= 256 and (opaque == nil or type(opaque) == "function"), "lights or optional opacity callback are invalid")
  local result = makeColorGrid(width, height)
  for _, light in ipairs(lights) do
    assert(finite(light.x) and finite(light.y) and finite(light.radius) and light.radius >= 0, "light needs finite x, y, and non-negative radius")
    assert(light.radius <= 64, "light radius exceeds the supported limit")
    if opaque then assert(light.radius <= 32 and #lights <= 16, "occlusion is limited to 16 lights with radius at most 32") end
    local color = type(light.color) == "string" and Color.parse(light.color) or light.color
    assert(type(color) == "table" and finite(color.r) and finite(color.g) and finite(color.b) and finite(color.a), "light color must have finite r, g, b, and a channels")
    local minX, maxX = math.max(1, math.floor(light.x - light.radius)), math.min(width, math.ceil(light.x + light.radius))
    local minY, maxY = math.max(1, math.floor(light.y - light.radius)), math.min(height, math.ceil(light.y + light.radius))
    for y = minY, maxY do
      for x = minX, maxX do
        local distance = math.sqrt((x - light.x) ^ 2 + (y - light.y) ^ 2)
        if distance <= light.radius and (not opaque or lineVisible(math.floor(light.x), math.floor(light.y), x, y, opaque)) then
          local amount = light.radius == 0 and (distance == 0 and 1 or 0) or (1 - distance / light.radius) * color.a
          local cell = result:get(x, y)
          cell.r = clamp(cell.r + color.r * amount, 0, 1)
          cell.g = clamp(cell.g + color.g * amount, 0, 1)
          cell.b = clamp(cell.b + color.b * amount, 0, 1)
        end
      end
    end
  end
  return result
end

Rot.Lighting = Lighting

return Rot

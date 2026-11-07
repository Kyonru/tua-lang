---@alias Point { x: number, y: number }

--- Make a point.
---@param x number
---@param y number
---@return Point
local function point(x, y) return {x=x,y=y} end
---@type Point
local p = point(2,3)
print(p.x+p.y)
---@class MainExports
---@field point fun(x: number, y: number): Point
return {point=point}

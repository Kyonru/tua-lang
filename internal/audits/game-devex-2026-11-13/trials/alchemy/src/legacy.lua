---@param value number
---@param lower number
---@param upper number
---@return number
local function clamp(value, lower, upper)
  return math.max(lower, math.min(upper, value))
end

return { clamp = clamp }

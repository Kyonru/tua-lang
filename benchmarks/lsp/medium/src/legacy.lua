---Benchmark: Lua interop provider with LuaDoc annotations.
---@param value number
---@return number
local function clamp(value)
  if value < 0 then
    return 0
  end

  return value
end

return {
  clamp = clamp,
}

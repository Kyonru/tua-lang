---@alias EnemyType 'friendly' | 'mad'

---@class Enemy
---@field hp number hit points
---@field name string display name
---@field type EnemyType
local Enemy = {}

-- Feature: LuaDoc overloads provide multiple call signatures to Tua.
---@overload fun(path: string): Image
---@overload fun(path: string, mode: "stream"): Source
local function loadAsset(path, mode)
  return nil
end

-- Feature: LuaHelper `identifier` is treated as string in type positions,
-- and `---@vararg` types the extra `...` arguments.
---@param message string
---@vararg identifier
local function logLabels(message, ...)
  print(message, ...)
end

return {
  Enemy = Enemy,
  loadAsset = loadAsset,
  logLabels = logLabels,
}

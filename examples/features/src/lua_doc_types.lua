---@alias EnemyType 'friendly' | 'mad'

---@class Enemy
---@field hp number hit points
---@field name string display name
---@field type EnemyType
local Enemy = {}

return {
  Enemy = Enemy,
}

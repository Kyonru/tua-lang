---@meta feel

---@class FeelCue
---@field duration number?
---@field strength number?

---@class Feel
local feel = {}

---@param name string
---@param cue FeelCue
function feel.define(name, cue) end

---@param name string
---@return boolean
function feel.play(name) end

---@param dt number
function feel.update(dt) end

---@param name string
---@return number
function feel.amount(name) end

return feel


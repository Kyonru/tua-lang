---@alias Label string
---@generic T
---@param value any
---@return any
local function id(value) return value end
---@deprecated Use id
---@nodiscard
---@param value string
---@return string
local function old(value) return value end
old("hi")
---@diagnostic disable: all
---@type number
local wrong = "bad"
---@param value Label
local function label(value) return value end

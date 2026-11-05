---@meta hump.timer
-- Declaration-only subset of HUMP Timer's handle-based API.
-- Keep the actual library under your runtime source root; never execute this file.
-- Tween/script and third-party extensions are intentionally outside this subset.

---@class HumpTimerHandle

---@class HumpTimer
---@field update fun(self: HumpTimer, dt: number)
---@field after fun(self: HumpTimer, delay: number, action: fun()): HumpTimerHandle
---@field every fun(self: HumpTimer, delay: number, action: fun(): boolean|nil, count?: number): HumpTimerHandle
---@field during fun(self: HumpTimer, delay: number, action: fun(dt: number, remaining: number), after?: fun()): HumpTimerHandle
---@field cancel fun(self: HumpTimer, handle: HumpTimerHandle)
---@field clear fun(self: HumpTimer)

local Timer = {}

---Create an independent timer. Call update once per game update.
---@return HumpTimer
function Timer.new() end

return Timer

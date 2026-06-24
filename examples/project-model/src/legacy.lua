-- Lua interop feature: Tua does not rewrite .lua files; build copies this file
-- into dist/legacy.lua so the emitted main.lua can require it.
local M = {}

function M.label(name)
  return "player:" .. name
end

return M

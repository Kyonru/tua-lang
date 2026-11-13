-- Plain Lua interop for a UI label in the porting workflow.
local M = {}
function M.decorate(label, number)
  return label .. " danger " .. tostring(number)
end
return M

local M = {}

function M.wrap(value, limit)
  if value < 0 then return value + limit end
  if value > limit then return value - limit end
  return value
end

return M

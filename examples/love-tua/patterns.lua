-- This module intentionally stays ordinary Lua. Tua copies it unchanged and
-- lets the typed game call it as a gradual Lua dependency.
local patterns = {}

function patterns.ring(count, speed)
  local shots = {}

  for index = 1, count do
    local angle = (index - 1) * math.pi * 2 / count
    shots[index] = {
      dx = math.cos(angle) * speed,
      dy = math.sin(angle) * speed,
    }
  end

  return shots
end

return patterns


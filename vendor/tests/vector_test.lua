local vector = require('brinevector')
local result = vector(1e-200, 2e-200) / vector(1e-200, 1e-200)
assert(result.x == 1 and result.y == 2)
for _, denominator in ipairs({vector(0,1), vector(1,0)}) do
  assert(not pcall(function() return vector(1,1) / denominator end))
end

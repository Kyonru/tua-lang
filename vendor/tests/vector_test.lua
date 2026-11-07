local vector = require('brinevector')
local result = vector(1e-200, 2e-200) / vector(1e-200, 1e-200)
assert(result.x == 1 and result.y == 2)
for _, denominator in ipairs({vector(0,1), vector(1,0)}) do
  assert(not pcall(function() return vector(1,1) / denominator end))
end
for _, magnitude in ipairs({1e-300, 1e300}) do
  local normalized = vector(magnitude, magnitude).normalized
  assert(math.abs(normalized.x - 1/math.sqrt(2)) < 1e-15)
  assert(math.abs(normalized.length - 1) < 1e-15)
  assert(vector(magnitude, 0).length == magnitude)
end
assert(vector(0,0).normalized.length == 0)

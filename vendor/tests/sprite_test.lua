local anim8 = require('anim8')
for _, durations in ipairs({0, -1, math.huge, 0/0, {0, 0}, {1, -1}, {1, math.huge}, {1, 0/0}}) do
  assert(not pcall(anim8.newAnimation, {true, true}, durations), 'invalid durations accepted')
end
local valid = anim8.newAnimation({true, true}, {0.1, 0.2})
valid:update(0.2)
assert(valid.position == 2)

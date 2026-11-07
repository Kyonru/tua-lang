local anim8 = require('anim8')
for _, durations in ipairs({0, -1, math.huge, 0/0, {0, 0}, {1, -1}, {1, math.huge}, {1, 0/0}}) do
  assert(not pcall(anim8.newAnimation, {true, true}, durations), 'invalid durations accepted')
end
local valid = anim8.newAnimation({true, true}, {0.1, 0.2})
valid:update(0.2)
assert(valid.position == 2)
local sprite = require('tuasprite')
for _, method in ipairs({'pauseAtStart', 'pauseAtEnd', 'pause'}) do
  local player = sprite.new({animations = {idle = anim8.newAnimation({true,true}, .5, method)},
    animationOrder = {'idle'}, looping = {idle = false}})
  player:update(2.2)
  assert(not player.playing)
  assert(player.animation.position == (method == 'pauseAtStart' and 1 or 2))
  assert(player.animation.timer == (method == 'pauseAtStart' and 0 or 1))
end

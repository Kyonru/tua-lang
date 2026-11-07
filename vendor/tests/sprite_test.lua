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
for _, speed in ipairs({1, -1}) do
  local calls, markers = {}, 0
  local player = sprite.new({animations = {idle = anim8.newAnimation({true,true}, .5,
    function(animation, loops) calls[#calls+1] = loops; animation:pause() end)},
    animationOrder = {'idle'}, looping = {}, markers = {idle = {{frame=1,name='step'}}}})
  player.speed = speed
  player:on('step', function() markers = markers + 1 end)
  player:update(2.2)
  assert(#calls == 1 and calls[1] == (speed > 0 and 2 or -3))
  assert(not player.playing and markers == 2)
end

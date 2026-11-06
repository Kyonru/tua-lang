local world = require('world')
world.reset()
for i=1,120 do world.update(1/60,0) end
assert(world.player.grounded and world.player.y==400,'initial landing')
world.jump()
assert(world.player.vy==-420,'jump impulse')
for i=1,120 do world.update(1/60,0) end
assert(world.player.grounded,'jump returns to ground')
world.reset()
for i=1,240 do world.update(1/60,1) end
assert(world.state.lost and world.state.deaths==1,'fall loses')
world.reset()
assert(not world.state.lost and world.state.deaths==1,'restart preserves deaths')
-- Each roof transition is physically achievable at the advertised movement speed.
for i=1,3 do
 local platform=world.platforms[i]
 world.player.x=platform.x+platform.w-30
 world.player.y=platform.y-world.player.h
 world.player.grounded=true
 world.player.vy=0
 world.jump()
 for frame=1,40 do world.update(1/60,1) end
 assert(world.player.grounded,'roof transition '..i)
 assert(not world.state.lost,'survive roof transition '..i)
end
world.player.x=720
world.player.y=220
world.update(1/60,0)
assert(world.state.won,'goal wins')
world.reset()
local started=os.clock()
for i=1,100000 do world.update(1/60,0) end
print('PASS platformer landing/jump/fall/restart/three roof transitions/win; 100000 updates seconds='..(os.clock()-started))

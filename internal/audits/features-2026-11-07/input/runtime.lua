local base = assert(arg[1])
local held, mx, my, reads = {}, 10, 20, 0
love={keyboard={isDown=function(k)return held[k] or false end,isScancodeDown=function(k)return held[k] or false end},mouse={isDown=function()return false end,getPosition=function()reads=reads+1;return mx,my end}}
package.path=base..'/positive/build/?.lua;'..package.path
local c=dofile(base..'/positive/build/main.lua')
assert(reads==1)
local dx,dy=c:pointerDelta();assert(dx==0 and dy==0)
held.d=true;c:update();local x,y=c:get('move');assert(x==1 and y==0 and c:pressed('right'))
c:update();assert(c:down('right') and not c:pressed('right'))
held.d=false;c:update();assert(c:released('right'))
mx,my=15,18;c:update();dx,dy=c:pointerDelta();assert(dx==5 and dy==-2 and reads==5)
print('positive controls/pairs/press/release/pointer PASS')
local bad=dofile(base..'/negative_deadzone/build/main.lua');bad:update();print('negative_deadzone active device',bad:getActiveDevice())
local hex=dofile(base..'/negative_hex/build/main.lua');held.a=true;hex:update();print('hex deadzone key held',hex:down('go'),'value',hex:get('go'))
local ok,err=pcall(dofile,base..'/empty_unknown/build/main.lua');print('empty unknown accepted build then runtime',ok,tostring(err))
local baton=require('baton')
local axis=.3
local j={getGamepadAxis=function()return axis end}
local radial=baton.new({controls={l={'axis:leftx-'},r={'axis:leftx+'},u={'axis:lefty-'},d={'axis:lefty+'}},pairs={move={'l','r','u','d'}},deadzone=.25,joystick=j})
radial:update();x,y=radial:get('move');assert(math.abs(x-.3)<.001 and math.abs(y-.3)<.001)
axis=.1;radial:update();x,y=radial:get('move');assert(x==0 and y==0)
print('radial joystick and deadzone PASS')

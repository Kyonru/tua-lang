package.path = "internal/audits/features-2026-11-07/camera/happy/build/?.lua;" .. package.path
dofile("vendor/tests/camera_test.lua")
print("existing runtime suite PASS (LOVE double, Lua 5.4)")
local C = require("tuacamera")
local function near(a,b) assert(math.abs(a-b)<1e-7) end
for _,style in ipairs({"lockon","platformer","topdown","topdowntight","screen"}) do
 local c=C.new({style=style,zoom=2,viewport={x=400,y=0,w=400,h=600}})
 c:follow(1000,500); c:update(0.1)
 local sx,sy=c:toScreen(333,111); local wx,wy=c:toWorld(sx,sy)
 near(wx,333);near(wy,111)
 print("style/zoom/split viewport roundtrip PASS",style,c.x,c.y)
end
local a=C.new({smoothing=8}); local b=C.new({smoothing=8})
a:follow(100,50); b:follow(100,50); a:update(0.2);b:update(0.1);b:update(0.1);near(a.x,b.x)
print("smoothing timestep equivalence PASS")
local bounded=C.new({rotation=math.pi/4,bounds={x=0,y=0,w=2000,h=2000}})
bounded:lookAt(-100,-100)
print("rotated bounded center",bounded.x,bounded.y,"visible bounds",bounded:visibleBounds())
local pixel=C.new({pixelPerfect=true,viewport={x=0,y=0,w=401,h=301}})
local first
love.graphics.translate=function(x,y) if not first then first={x,y} end end
pixel:attach();pixel:detach()
print("pixel perfect drawn center",first[1],first[2],"toScreen center",pixel:toScreen(0,0))
local shake=C.new({shakeDecay=0})
shake:shake(1);shake:update(0.1)
local bx,by,bw,bh=shake:visibleBounds()
local wx,wy=shake:toWorld(0,0)
print("shake viewport top-left",wx,wy,"reported visibleBounds",bx,by,bw,bh,"offset",shake.shakeX,shake.shakeY)
local platform=C.new({style="platformer"})
print("platformer deadzone x y w h",platform:deadzoneRect())

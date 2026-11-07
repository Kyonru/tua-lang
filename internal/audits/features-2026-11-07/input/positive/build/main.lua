local __tua_input = require("baton").new
local c = __tua_input({controls={left={"key:a"},right={"key:d"},up={"key:w"},down={"key:s"},fire={"mouse:1"}},pairs={move={"left","right","up","down"}},pointer=true,deadzone=0.25})
local x,y=c:get("move")
local nx = x
local ny = y
local fire = c:get("fire")
local mx,my=c:pointer()
return c

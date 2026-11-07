local ecs = require("tuaecs")
local P = ecs.component("Position", {x="number"})
local world = ecs.world()
local system = ecs.system(world,{P},function(entity,position,dt)
 local n = position.x
 local bad = position.x
end)

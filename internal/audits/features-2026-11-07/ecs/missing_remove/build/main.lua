local ecs = require("tuaecs")
local P = ecs.component("Position", {x="number"})
local world = ecs.world()
local e = world:create()
local position = world:remove(e,P)
local x = position.x
print(x)

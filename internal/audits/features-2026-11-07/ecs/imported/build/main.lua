local ecs = require("tuaecs")
local domain = require("components")
local world = ecs.world()
local e = world:create()
world:add(e,domain.Position,{x=4})
local position = world:get(e,domain.Position)
local n = position.x
assert(n == 4)
print("imported passed")

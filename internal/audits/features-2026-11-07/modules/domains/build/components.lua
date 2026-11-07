local ecs = require("tuaecs")
local Position=ecs.component("Position",{x="number"})
return {Position=Position}

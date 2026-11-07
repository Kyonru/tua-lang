local ecs = require("tuaecs")
local G = require("game")
local game = G.game
local state = game:get()
local direct = require("game").game
local ds = direct:get()
local N = require("normal")
local n = N.inc(3)
local C = require("components")
local world = ecs.world()
local e = world:create()
world:add(e,C.Position,{x=4})
local position = world:get(e,C.Position)
local x = position.x
print(state.score,ds.score,n,x)

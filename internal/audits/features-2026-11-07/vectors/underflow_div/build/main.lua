local __tua_vec2 = require("brinevector")
local p = __tua_vec2(1e-200,1e-200)
local q = __tua_vec2(1e-200,1e-200)
local result = p / q
print(result.x,result.y)

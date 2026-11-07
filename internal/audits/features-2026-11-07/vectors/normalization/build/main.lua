local __tua_vec2 = require("brinevector")
local b = __tua_vec2(1e200,0)
local t = __tua_vec2(1e-200,0)
local big = b.normalized
local tiny = t.normalized
print("big",big.x,big.y,big.length)
print("tiny",tiny.x,tiny.y,tiny.length)

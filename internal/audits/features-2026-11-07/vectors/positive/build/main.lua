local __tua_vec2 = require("brinevector")
math.clamp = math.clamp or function(value, minimum, maximum) return math.max(minimum, math.min(maximum, value)) end
math.inverseLerp = math.inverseLerp or function(from, to, value) return (value - from) / (to - from) end
math.lerp = math.lerp or function(from, to, amount) return from + (to - from) * amount end
math.remap = math.remap or function(value, inputMinimum, inputMaximum, outputMinimum, outputMaximum) return outputMinimum + (value - inputMinimum) / (inputMaximum - inputMinimum) * (outputMaximum - outputMinimum) end
math.rgb = math.rgb or function(red, green, blue) return { red / 255, green / 255, blue / 255 } end
math.round = math.round or function(value, places) local factor = 10 ^ (places or 0); local scaled = value * factor; local rounded = scaled >= 0 and math.floor(scaled + 0.5) or math.ceil(scaled - 0.5); return rounded / factor end
math.sign = math.sign or function(value) return value < 0 and -1 or value > 0 and 1 or 0 end
math.smoothstep = math.smoothstep or function(edge0, edge1, value) local amount = math.max(0, math.min(1, (value - edge0) / (edge1 - edge0))); return amount * amount * (3 - 2 * amount) end
math.tau = math.tau or math.pi * 2
love.math.rgba = love.math.rgba or function(red, green, blue, alpha) return { red / 255, green / 255, blue / 255, alpha / 255 } end
local p = __tua_vec2(3,4)
local v = __tua_vec2(2,-1)
local dt = 0.5
local moved = __tua_vec2(p.x + v.x * dt, p.y + v.y * dt)
local scaled = __tua_vec2(p.x * 2, p.y * 2)
local divided = __tua_vec2(p.x / 2, p.y / 2)
local neg = __tua_vec2(-p.x, -p.y)
local product = __tua_vec2(p.x * v.x, p.y * v.y)
local x,y = p:split()
assert(moved.x == 4 and moved.y == 3.5)
assert(scaled.x == 6 and divided.y == 2 and neg.x == -3)
assert(product.x == 6 and product.y == -4)
assert(x == 3 and y == 4 and p:dot(v) == 2)
assert(p.length == 5 and p.length2 == 25)
local zero = __tua_vec2(0,0)
assert(zero.normalized == zero)
assert(p:trim(2).length == 2)
assert(p:hadamard(v) == product)
assert(p:clamp(__tua_vec2(0,0),__tua_vec2(2,3)) == __tua_vec2(2,3))
assert(p.floor == p and p.ceil == p and p.copy == p)
p.length = 10
assert(p.length == 10)
p.angle = 0
assert(p.x == 10 and p.y == 0)
assert(p:angled(0) == p and p:rotated(0) == p)
assert(math.round(-1.5) == -2 and math.round(150,-2) == 200)
assert(math.round(10.946,2) == 10.95)
assert(math.clamp(20,0,10) == 10 and math.lerp(0,10,1.5) == 15)
assert(math.inverseLerp(0,10,5) == 0.5 and math.remap(5,0,10,0,100) == 50)
assert(math.sign(-10) == -1 and math.sign(0) == 0)
assert(math.smoothstep(0,1,0.5) == 0.5 and math.tau == 2*math.pi)
assert(math.rgb(255,0,128)[1] == 1 and love.math.rgba(255,0,0,128)[4] == 128/255)
print("positive passed",type(p))

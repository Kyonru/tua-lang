math.clamp = math.clamp or function(value, minimum, maximum) return math.max(minimum, math.min(maximum, value)) end
math.inverseLerp = math.inverseLerp or function(from, to, value) return (value - from) / (to - from) end
math.round = math.round or function(value, places) local factor = 10 ^ (places or 0); local scaled = value * factor; local rounded = scaled >= 0 and math.floor(scaled + 0.5) or math.ceil(scaled - 0.5); return rounded / factor end
math.smoothstep = math.smoothstep or function(edge0, edge1, value) local amount = math.max(0, math.min(1, (value - edge0) / (edge1 - edge0))); return amount * amount * (3 - 2 * amount) end
print("equal endpoints",math.inverseLerp(1,1,1),math.smoothstep(1,1,1))
print("reverse clamp",math.clamp(5,10,0))
print("round extreme",math.round(1,400))

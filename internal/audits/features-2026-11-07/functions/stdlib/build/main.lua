local a = { 1, 2 }
table.insert(a, 3)
local s = string.format("%s:%d", "ok", #a)
local n = tonumber("12")
print(s, n, math.floor(2.9), select("#", 1, 2, 3))

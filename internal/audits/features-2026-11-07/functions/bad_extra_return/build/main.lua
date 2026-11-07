local function takes(n) return n end
local function f() return 1, takes("bad") end
print(f())

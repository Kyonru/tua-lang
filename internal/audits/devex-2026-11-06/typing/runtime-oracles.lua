-- Pure Lua semantic oracles for value-list behavior. No project code is loaded.
local function pair() return 1, "a" end
local function consume(a, b) assert(type(a)=="number"); assert(type(b)=="string"); return b .. a end
print("expanded final call", consume(pair()))
local a, b = (pair())
assert(a==1 and b==nil)
print("parenthesized second slot", type(b))
local x, y = 1
assert(x==1 and y==nil)
print("scalar second slot", type(y))

local function factor(n)
 if n <= 1 then return 1 end
 return n * factor(n - 1)
end
local function makeCounter(start)
  if start == nil then
    start = 2
  end
 local count = start
 return function(step)
   if step == nil then
     step = 1
   end
  count = count + step
  return count
 end
end
local function pair()
 return 3, "ok"
end
local function forward()
 return pair()
end
local function spread(...)
 return ...
end
local function label(v)
 if type(v) == "string" then return v end
 return tostring(v)
end
local mode = "fast"
local c = makeCounter()
local a, b = forward()
local x, y = spread(4, 5)
local z = 2
z = z * (3 + 4)
print(factor(5), c(), c(2), a, b, x, y, z, label("yes"), mode)

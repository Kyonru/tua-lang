local function f(x)
 if x == nil then return 7 end
 return x
end
print(f(), f(nil), f(0))

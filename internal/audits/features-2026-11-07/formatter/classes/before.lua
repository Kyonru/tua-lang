local Box = {}
Box.__index = Box
Box.__type_id = 1
Box.__is = {
  [1] = true,
}

local function Box_init(self)
  self.value = 4
end

local function Box_get(self)

  return self.value
end
Box.get = Box_get

local function Box_new()
  return setmetatable({
    value = 4,
  }, Box)
end

Box.new = Box_new

local function add(x,y)
  if y == nil then
    y = 3
  end
return x+y
end
local b=Box.new()
print(Box_get(b),add(2))

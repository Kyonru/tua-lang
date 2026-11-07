local Base = {}
Base.__index = Base
Base.__type_id = 1
Base.__is = {
  [1] = true,
}

local function Base_init(self, value)
  self.value = value 
  self.value = 3
end

local function Base_new(value)
  return setmetatable({
    value = value,
    value = 3,
  }, Base)
end

Base.new = Base_new

local Child = {}
Child.__index = Child
Child.__type_id = 2
Child.__is = {
  [1] = true,
  [2] = true,
}

local function Child_init(self, value)
  Base_init(self, value) 
  self.value = 5
end

setmetatable(Child, { __index = Base })

local function Child_new(value)
  return setmetatable({
    value = value,
    value = 3,
    value = 5,
  }, Child)
end

Child.new = Child_new

assert(Base.new(1).value == 3)
assert(Child.new(1).value == 5)
print("defaults ok")

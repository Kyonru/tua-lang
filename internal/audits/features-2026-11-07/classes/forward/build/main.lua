local Base = {}

local Child = {}
Child.__index = Child
Child.__type_id = 1
Child.__is = {
  [2] = true,
  [1] = true,
}

local function Child_init(self, value)
  Base.__init(self, value) 
end

local function Child_read(self)
  return Base.read(self) + 1 
end
Child.read = Child_read

setmetatable(Child, { __index = Base })

local function Child_new(value)
  return setmetatable({
    value = value,
  }, Child)
end

Child.new = Child_new

Base.__index = Base
Base.__type_id = 2
Base.__is = {
  [2] = true,
}

local function Base_init(self, value)
  self.value = value 
end
Base.__init = Base_init

local function Base_read(self)
  return self.value 
end
Base.read = Base_read

local function Base_new(value)
  return setmetatable({
    value = value,
  }, Base)
end

Base.new = Base_new

assert(Child.new(2):read() == 3)
print("forward ok")

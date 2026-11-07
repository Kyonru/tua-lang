local Actor = {}
Actor.__index = Actor
Actor.__type_id = 1
Actor.__is = {
  [1] = true,
}

local function Actor_init(self)
  self.value = 0
end

local function Actor_update(self, dt)
  self.value = self.value + dt; return self.value 
end
Actor.update = Actor_update

local function Actor_new()
  return setmetatable({
    value = 0,
  }, Actor)
end

Actor.new = Actor_new

local Fast = {}
Fast.__index = Fast
Fast.__type_id = 2
Fast.__is = {
  [1] = true,
  [2] = true,
}

local function Fast_init(self)
  Actor_init(self)
end

local function Fast_update(self, dt)
  self.value = self.value + (dt * 2); return self.value 
end
Fast.update = Fast_update

setmetatable(Fast, { __index = Actor })

local function Fast_new()
  return setmetatable({
    value = 0,
  }, Fast)
end

Fast.new = Fast_new
return { Fast = Fast }

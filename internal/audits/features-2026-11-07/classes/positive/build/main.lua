local Entity = {}
Entity.__index = Entity
Entity.__type_id = 1
Entity.__is = {
  [1] = true,
}

local function Entity_init(self, x)
  self.x = x 
end

local function Entity_update(self, dt)
  self.x = self.x + dt 
end
Entity.update = Entity_update

local function Entity_read(self)
  return self.x 
end
Entity.read = Entity_read

local function Entity_same(self)
  return self 
end
Entity.same = Entity_same

local function Entity_new(x)
  return setmetatable({
    x = x,
  }, Entity)
end

Entity.new = Entity_new

local Particle = {}
Particle.__index = Particle
Particle.__type_id = 2
Particle.__is = {
  [1] = true,
  [2] = true,
}

local function Particle_init(self, x)

  Entity_init(self, x)
  self.trail = {}

  self.speed = 2
end

local function Particle_update(self, dt)

  Entity_update(self, dt)
  self.x = self.x + (self.speed * dt)
  table.insert(self.trail, self.x)

end
Particle.update = Particle_update

setmetatable(Particle, { __index = Entity })

local function Particle_new(x)
  local self = setmetatable({}, Particle)
  Particle_init(self, x)
  return self
end

Particle.new = Particle_new

local p = Particle.new(10)
local entities = {p}
entities[1]:update(2)
assert(Entity_read(p) == 16)
assert(Entity_same(p) == p)
assert(p.trail[1] == 16)
print("positive ok")

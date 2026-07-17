local stars = love.graphics.newImage("assets/star.png")
stars:setFilter("linear", "linear")

local ring = love.graphics.newImage("assets/ring.png")
ring:setFilter("linear", "linear")

local block = love.graphics.newImage("assets/block.png")
block:setFilter("linear", "nearest")

local circle = love.graphics.newImage("assets/circle.png")
circle:setFilter("linear", "nearest")

local heart = love.graphics.newImage("assets/heart.png")
heart:setFilter("linear", "nearest")

local particles = {}

function particles.create_stars()
  local ps = love.graphics.newParticleSystem(stars, 1000)
  ps:setColors(
    115 / 255,
    95 / 255,
    50 / 255,
    1
  )
  ps:setDirection(math.pi)
  ps:setEmissionArea("borderrectangle", 0, screen_height, 0, false)
  ps:setEmissionRate(1)
  ps:setInsertMode("top")
  ps:setLinearDamping(0.003, 0.004)
  ps:setParticleLifetime(300, 360)
  ps:setSizes(0.025, 0.2)
  ps:setSizeVariation(0.75)
  ps:setRelativeRotation(true)
  ps:setRotation(0, 0.1)
  ps:setSpeed(5, 10)

  local dt = 0.1
  for time = 0, 500, dt do
    ps:update(dt)
  end

  particles.list[#particles.list + 1] = {
    x = screen_width + 64,
    y = 0,
    vfx = ps,
  }
end

function particles.create_trail()
  local ps = love.graphics.newParticleSystem(ring, 30)
  ps:setColors(1, 0, 0, 0.18, 1, 0.65, 0.21875, 1, 1, 1, 1, 1, 0.5, 0.5, 0.5, 1, 0, 0, 0, 1)
  ps:setDirection(1.57)
  ps:setEmissionRate(20)
  ps:setEmitterLifetime(-1)
  ps:setLinearAcceleration(40, 0, -50, 0)
  ps:setLinearDamping(0.0002, 0)
  ps:setOffset(32, 8)
  ps:setParticleLifetime(0.2, 0.5)
  ps:setSizes(0.2, 0.6, 0)
  ps:setSpeed(61.75, 86.25)
  ps:setSpread(0.32)
  ps:setTangentialAcceleration(0.11, 0.11)
  return ps
end

function particles.init_block_explosion()
  local block_particle = love.graphics.newParticleSystem(block, 40)
  block_particle:setColors(1, 1, 1, 1)
  block_particle:setEmitterLifetime(-1)
  block_particle:setLinearDamping(0.4, 2.75)
  block_particle:setOffset(32, 32)
  block_particle:setParticleLifetime(0.15, 0.9)
  block_particle:setSizes(0.1, 0.1)
  block_particle:setSizeVariation(0.6)
  block_particle:setSpeed(35, 185)
  block_particle:setSpin(-0.15, 0.5)
  block_particle:setSpinVariation(0.5)
  block_particle:setSpread(6.5)

  local wave = love.graphics.newParticleSystem(circle, 5)
  wave:setColors(1, 1, 1, 0.6, 1, 1, 1, 0.1, 1, 1, 1, 0)
  wave:setEmitterLifetime(-1)
  wave:setOffset(50, 50)
  wave:setParticleLifetime(0.25)
  wave:setSizes(0, 5)

  particles.block_explosion = {
    wave,
    block_particle,
  }
end

function particles.init_heart_explosion()
  local wave = love.graphics.newParticleSystem(heart, 10)
  wave:setColors(
    217 / 255,
    95 / 255,
    89 / 255,
    0.1,
    198 / 255,
    60 / 255,
    81 / 255,
    0.4,
    140 / 255,
    48 / 255,
    97 / 255,
    0.1,
    82 / 255,
    34 / 255,
    88 / 255,
    0
  )
  wave:setEmitterLifetime(-1)
  wave:setOffset(64 / 2, 64 / 2)
  wave:setParticleLifetime(0.25)
  wave:setSizes(0, 2)

  particles.heart_explosion = {
    wave,
  }
end

function particles.create_block_explosion(x, y, size)
  local wave = particles.block_explosion[1]
  local block_particle = particles.block_explosion[2]

  wave:setPosition(x, y)
  block_particle:setPosition(x, y)

  block_particle:setSizes(size, size / 1.5, 0)

  wave:emit(1)
  block_particle:emit(40)
end

function particles.create_heart_explosion(x, y)
  local wave = particles.heart_explosion[1]
  wave:setPosition(x, y)
  wave:emit(1)
end

function particles.load()
  particles.list = {}
  particles.create_stars()
  particles.init_block_explosion()
  particles.init_heart_explosion()
end

function particles.update(dt)
  for i = 1, #particles.list do
    local ps = particles.list[i].vfx
    ps:update(dt)
  end

  for i = 1, #particles.block_explosion do
    local ps = particles.block_explosion[i]
    ps:update(dt)
  end

  for i = 1, #particles.heart_explosion do
    local ps = particles.heart_explosion[i]
    ps:update(dt)
  end
end

function particles.draw()
  for i = 1, #particles.list do
    local ps = particles.list[i].vfx
    love.graphics.draw(ps, particles.list[i].x, particles.list[i].y)
  end
end

function particles.draw_block_explosion()
  for i = 1, #particles.block_explosion do
    local ps = particles.block_explosion[i]
    love.graphics.draw(ps, 0, 0)
  end
end

function particles.draw_heart_explosion()
  for i = 1, #particles.heart_explosion do
    local ps = particles.heart_explosion[i]
    love.graphics.draw(ps, 0, 0)
  end
end

return particles

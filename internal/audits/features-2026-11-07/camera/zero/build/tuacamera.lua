--
-- tuacamera: the runtime behind Tua's built-in `Camera` module.
--
-- It covers the common cases so a game can get moving quickly: following a
-- target through a deadzone, smoothing, look-ahead, world bounds, zoom,
-- rotation, screen shake, split-screen viewports, and coordinate conversion.
-- Tua types the `Camera` global from these annotations and copies this file
-- into the build when a project uses it. Games that need behavior these
-- options don't express can provide their own tuacamera.lua, or write their
-- own camera.
--
-- Copyright (c) 2026 Kyonru. MIT License, the same license as Tua.
--

---@alias CameraStyle
---| "lockon"       # keep the target exactly centered
---| "platformer"   # wide horizontal, short vertical deadzone
---| "topdown"      # square deadzone, a quarter of the shorter side
---| "topdowntight" # square deadzone, an eighth of the shorter side
---| "screen"       # move one whole screen at a time, like room-based games

---@class CameraRect
---@field x number
---@field y number
---@field w number
---@field h number

---@class CameraOptions
---@field x? number world position the camera starts centered on
---@field y? number
---@field zoom? number scale factor; 2 draws the world twice as large
---@field rotation? number radians
---@field style? CameraStyle deadzone preset, "lockon" by default
---@field smoothing? number how fast the camera catches up, per second; 0 snaps
---@field lead? number seconds of target movement to look ahead
---@field bounds? CameraRect world area the view must stay inside
---@field viewport? CameraRect screen area to draw into; the whole window by default
---@field pixelPerfect? boolean round the view to whole screen pixels
---@field shakeAmount? number largest shake offset, in screen pixels
---@field shakeDecay? number how much shake trauma fades per second

---@class Camera
---@field x number world position at the center of the view
---@field y number
---@field zoom number
---@field rotation number
---@field smoothing number
---@field lead number
---@field pixelPerfect boolean
---@field follow fun(self: Camera, x: number, y: number) set the point the camera tracks
---@field update fun(self: Camera, dt: number) move toward the target and fade shake
---@field lookAt fun(self: Camera, x: number, y: number) center on a point immediately
---@field move fun(self: Camera, dx: number, dy: number)
---@field setStyle fun(self: Camera, style: CameraStyle)
---@field setDeadzone fun(self: Camera, x: number, y: number, w: number, h: number) custom deadzone in screen pixels, relative to the view's top left
---@field setBounds fun(self: Camera, x: number, y: number, w: number, h: number)
---@field clearBounds fun(self: Camera)
---@field setViewport fun(self: Camera, x: number, y: number, w: number, h: number)
---@field clearViewport fun(self: Camera)
---@field shake fun(self: Camera, trauma: number) add trauma from 0 to 1; shake grows with its square
---@field attach fun(self: Camera) start drawing in world coordinates
---@field detach fun(self: Camera) return to screen coordinates
---@field draw fun(self: Camera, draw: fun()) attach, call `draw`, and detach
---@field toWorld fun(self: Camera, sx: number, sy: number): number, number
---@field toScreen fun(self: Camera, wx: number, wy: number): number, number
---@field mousePosition fun(self: Camera): number, number mouse position in world coordinates
---@field visibleBounds fun(self: Camera): number, number, number, number world x, y, width, and height currently in view

local Camera = {}
Camera.__index = Camera

local unpack = unpack or table.unpack

-- Deadzone presets as fractions of the viewport: width, height, and how far
-- the zone sits above center. "screen" is handled separately.
local STYLES = {
  lockon = { 0, 0, 0 },
  platformer = { 1 / 8, 1 / 3, 1 / 12 },
  topdown = "square:4",
  topdowntight = "square:8",
  screen = "screen",
}

local function clamp(value, low, high)
  if value < low then
    return low
  end
  if value > high then
    return high
  end
  return value
end

local function round(value)
  return math.floor(value + 0.5)
end

local function noise(seed, t)
  if love and love.math and love.math.noise then
    return love.math.noise(seed, t) * 2 - 1
  end
  return math.sin(t * 12.9898 + seed * 78.233)
end

function Camera:viewportRect()
  if self.viewport then
    local v = self.viewport
    return v.x, v.y, v.w, v.h
  end
  local w, h = 800, 600
  if love and love.graphics and love.graphics.getDimensions then
    w, h = love.graphics.getDimensions()
  end
  return 0, 0, w, h
end

-- The deadzone in screen pixels, relative to the viewport's top left.
function Camera:deadzoneRect()
  local _, _, w, h = self:viewportRect()
  if self.deadzone then
    local d = self.deadzone
    return d.x, d.y, d.w, d.h
  end
  local preset = STYLES[self.style]
  if type(preset) == "string" and preset:sub(1, 7) == "square:" then
    local size = math.min(w, h) / tonumber(preset:sub(8))
    return (w - size) / 2, (h - size) / 2, size, size
  end
  if type(preset) ~= "table" then
    return w / 2, h / 2, 0, 0
  end
  local dw, dh = w * preset[1], h * preset[2]
  return (w - dw) / 2, (h - dh) / 2 - h * preset[3], dw, dh
end

function Camera:follow(x, y)
  if self.targetX and self.lastUpdateDt and self.lastUpdateDt > 0 then
    self.velocityX = (x - self.targetX) / self.lastUpdateDt
    self.velocityY = (y - self.targetY) / self.lastUpdateDt
  end
  self.targetX, self.targetY = x, y
end

function Camera:desiredPosition()
  local tx, ty = self.targetX, self.targetY
  if not tx then
    return self.x, self.y
  end
  tx = tx + (self.velocityX or 0) * self.lead
  ty = ty + (self.velocityY or 0) * self.lead

  local _, _, w, h = self:viewportRect()
  if self.style == "screen" and not self.deadzone then
    local viewW, viewH = w / self.zoom, h / self.zoom
    local originX = self.bounds and self.bounds.x or 0
    local originY = self.bounds and self.bounds.y or 0
    local column = math.floor((tx - originX) / viewW)
    local row = math.floor((ty - originY) / viewH)
    return originX + (column + 0.5) * viewW, originY + (row + 0.5) * viewH
  end

  -- Keep the target inside the deadzone, converted to world units around the
  -- camera's current center. Rotation is ignored for the deadzone test.
  local dx, dy, dw, dh = self:deadzoneRect()
  local left = self.x + (dx - w / 2) / self.zoom
  local top = self.y + (dy - h / 2) / self.zoom
  local right = left + dw / self.zoom
  local bottom = top + dh / self.zoom
  local x, y = self.x, self.y
  if tx < left then
    x = x + tx - left
  elseif tx > right then
    x = x + tx - right
  end
  if ty < top then
    y = y + ty - top
  elseif ty > bottom then
    y = y + ty - bottom
  end
  return x, y
end

function Camera:clampToBounds()
  local b = self.bounds
  if not b then
    return
  end
  local _, _, w, h = self:viewportRect()
  local halfW, halfH = w / 2 / self.zoom, h / 2 / self.zoom
  if b.w <= halfW * 2 then
    self.x = b.x + b.w / 2
  else
    self.x = clamp(self.x, b.x + halfW, b.x + b.w - halfW)
  end
  if b.h <= halfH * 2 then
    self.y = b.y + b.h / 2
  else
    self.y = clamp(self.y, b.y + halfH, b.y + b.h - halfH)
  end
end

function Camera:update(dt)
  self.lastUpdateDt = dt
  local x, y = self:desiredPosition()
  if self.smoothing > 0 then
    local blend = 1 - math.exp(-self.smoothing * dt)
    self.x = self.x + (x - self.x) * blend
    self.y = self.y + (y - self.y) * blend
  else
    self.x, self.y = x, y
  end
  self:clampToBounds()

  self.time = self.time + dt
  self.trauma = math.max(0, self.trauma - self.shakeDecay * dt)
  local strength = self.shakeAmount * self.trauma * self.trauma
  self.shakeX = strength * noise(1, self.time * 25)
  self.shakeY = strength * noise(2, self.time * 25)
end

function Camera:lookAt(x, y)
  self.x, self.y = x, y
  self.targetX, self.targetY = nil, nil
  self.velocityX, self.velocityY = 0, 0
  self:clampToBounds()
end

function Camera:move(dx, dy)
  self.x, self.y = self.x + dx, self.y + dy
  self:clampToBounds()
end

function Camera:setStyle(style)
  assert(STYLES[style], "unknown camera style '" .. tostring(style) .. "'")
  self.style = style
  self.deadzone = nil
end

function Camera:setDeadzone(x, y, w, h)
  self.deadzone = { x = x, y = y, w = w, h = h }
end

function Camera:setBounds(x, y, w, h)
  self.bounds = { x = x, y = y, w = w, h = h }
  self:clampToBounds()
end

function Camera:clearBounds()
  self.bounds = nil
end

function Camera:setViewport(x, y, w, h)
  self.viewport = { x = x, y = y, w = w, h = h }
end

function Camera:clearViewport()
  self.viewport = nil
end

function Camera:shake(trauma)
  self.trauma = clamp(self.trauma + trauma, 0, 1)
end

-- The world point drawn at the viewport's center, including shake and
-- pixel rounding.
function Camera:viewCenter()
  local x = self.x + self.shakeX / self.zoom
  local y = self.y + self.shakeY / self.zoom
  if self.pixelPerfect then
    x = round(x * self.zoom) / self.zoom
    y = round(y * self.zoom) / self.zoom
  end
  return x, y
end

function Camera:attach()
  local vx, vy, w, h = self:viewportRect()
  local x, y = self:viewCenter()
  love.graphics.push()
  if self.viewport then
    self.previousScissor = { love.graphics.getScissor() }
    love.graphics.intersectScissor(vx, vy, w, h)
  end
  local cx, cy = vx + w / 2, vy + h / 2
  if self.pixelPerfect then
    cx, cy = round(cx), round(cy)
  end
  love.graphics.translate(cx, cy)
  love.graphics.scale(self.zoom)
  love.graphics.rotate(-self.rotation)
  love.graphics.translate(-x, -y)
end

function Camera:detach()
  love.graphics.pop()
  if self.previousScissor then
    love.graphics.setScissor(unpack(self.previousScissor))
    self.previousScissor = nil
  end
end

function Camera:draw(draw)
  self:attach()
  local ok, err = pcall(draw)
  self:detach()
  if not ok then
    error(err, 0)
  end
end

function Camera:toWorld(sx, sy)
  local vx, vy, w, h = self:viewportRect()
  local x, y = self:viewCenter()
  local dx, dy = (sx - vx - w / 2) / self.zoom, (sy - vy - h / 2) / self.zoom
  local cos, sin = math.cos(self.rotation), math.sin(self.rotation)
  return x + dx * cos - dy * sin, y + dx * sin + dy * cos
end

function Camera:toScreen(wx, wy)
  local vx, vy, w, h = self:viewportRect()
  local x, y = self:viewCenter()
  local dx, dy = wx - x, wy - y
  local cos, sin = math.cos(-self.rotation), math.sin(-self.rotation)
  return vx + w / 2 + (dx * cos - dy * sin) * self.zoom,
    vy + h / 2 + (dx * sin + dy * cos) * self.zoom
end

function Camera:mousePosition()
  return self:toWorld(love.mouse.getPosition())
end

function Camera:visibleBounds()
  local _, _, w, h = self:viewportRect()
  local halfW, halfH = w / 2 / self.zoom, h / 2 / self.zoom
  -- A rotated view covers a larger axis-aligned area.
  local cos, sin = math.abs(math.cos(self.rotation)), math.abs(math.sin(self.rotation))
  local extentW = halfW * cos + halfH * sin
  local extentH = halfW * sin + halfH * cos
  return self.x - extentW, self.y - extentH, extentW * 2, extentH * 2
end

---Create a camera. Every option is optional.
---@param options? CameraOptions
---@return Camera
local function new(options)
  options = options or {}
  local camera = setmetatable({
    x = options.x or 0,
    y = options.y or 0,
    zoom = options.zoom or 1,
    rotation = options.rotation or 0,
    style = options.style or "lockon",
    smoothing = options.smoothing or 0,
    lead = options.lead or 0,
    bounds = options.bounds,
    viewport = options.viewport,
    pixelPerfect = options.pixelPerfect or false,
    shakeAmount = options.shakeAmount or 12,
    shakeDecay = options.shakeDecay or 1.5,
    trauma = 0,
    time = 0,
    shakeX = 0,
    shakeY = 0,
  }, Camera)
  assert(STYLES[camera.style], "unknown camera style '" .. tostring(camera.style) .. "'")
  camera:clampToBounds()
  return camera
end

return {
  new = new,
}

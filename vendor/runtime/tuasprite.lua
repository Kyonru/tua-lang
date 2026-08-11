-- Tua's replaceable sprite-player runtime.
-- Projects may provide their own tuasprite.lua as long as it exposes new(sheet, initial).

local SpritePlayer = {}
SpritePlayer.__index = SpritePlayer

local function clamp(value, minimum, maximum)
  return math.max(minimum, math.min(maximum, value))
end

local function animationFor(self, name)
  local animation = self.animations[name]
  if animation == nil then
    error(("unknown sprite animation %q"):format(tostring(name)), 3)
  end
  return animation
end

local function invokeLoop(animation, loops)
  local callback = animation.onLoop
  local fn = type(callback) == "function" and callback or animation[callback]
  if fn then fn(animation, loops) end
end

local function syncAnimation(self)
  if self.animation == nil then return end
  self.animation.status = self.playing and "playing" or "paused"
end

local function markerIndex(sheet)
  local index = {}
  for animationName, markers in pairs(sheet.markers or {}) do
    local frames = {}
    for _, marker in ipairs(markers) do
      local names = frames[marker.frame]
      if names == nil then
        names = {}
        frames[marker.frame] = names
      end
      names[#names + 1] = marker.name
    end
    index[animationName] = frames
  end
  return index
end

function SpritePlayer:_emitFrame(frame)
  local frames = self.markerFrames[self.name]
  local names = frames and frames[frame]
  if names == nil then return end

  for _, marker in ipairs(names) do
    local callbacks = self.listeners[marker]
    if callbacks then
      for _, callback in ipairs(callbacks) do
        callback(self, marker, self.name, frame)
      end
    end
  end
end

function SpritePlayer:_select(name, restart)
  local animation = animationFor(self, name)
  if self.name ~= name or restart then
    self.name = name
    self.animation = animation
    self.animation:gotoFrame(1)
  end
  syncAnimation(self)
end

function SpritePlayer:play(name, restart)
  self.playing = true
  self:_select(name, restart == true)
  syncAnimation(self)
end

function SpritePlayer:stop()
  self.playing = false
  syncAnimation(self)
end

function SpritePlayer:resume()
  if self.animation ~= nil then
    self.playing = true
    syncAnimation(self)
  end
end

function SpritePlayer:gotoAndPlay(name, frame)
  self.playing = true
  self:_select(name, true)
  self:seekFrame(frame or 1, false)
  syncAnimation(self)
end

function SpritePlayer:gotoAndStop(name, frame)
  self.playing = false
  self:_select(name, true)
  self:seekFrame(frame or 1, false)
  syncAnimation(self)
end

function SpritePlayer:seekFrame(frame, emitMarkers)
  if self.animation == nil then return end
  frame = clamp(math.floor(frame), 1, #self.animation.frames)
  self.animation:gotoFrame(frame)
  if emitMarkers then self:_emitFrame(frame) end
  syncAnimation(self)
end

function SpritePlayer:seek(progress, emitMarkers)
  if self.animation == nil then return end
  progress = clamp(progress, 0, 1)
  local frame = math.floor(progress * #self.animation.frames) + 1
  self:seekFrame(math.min(frame, #self.animation.frames), emitMarkers)
end

function SpritePlayer:update(dt)
  for _, name in ipairs(self.sheet.childOrder or {}) do
    self.children[name].player:update(dt)
  end
  local animation = self.animation
  if not self.playing or animation == nil or dt == 0 or self.speed == 0 then return end

  local direction = self.speed < 0 and -1 or 1
  local remaining = math.abs(dt * self.speed)
  local looping = self.sheet.looping[self.name] ~= false

  while remaining > 0 and self.playing do
    local position = animation.position
    local frameStart = animation.intervals[position]
    local elapsed = animation.timer - frameStart

    if direction > 0 then
      local untilBoundary = animation.durations[position] - elapsed
      if remaining < untilBoundary then
        animation.timer = animation.timer + remaining
        remaining = 0
      else
        remaining = remaining - untilBoundary
        if position < #animation.frames then
          animation.position = position + 1
          animation.timer = animation.intervals[animation.position]
          self:_emitFrame(animation.position)
        elseif looping then
          animation.position = 1
          animation.timer = 0
          invokeLoop(animation, 1)
          self:_emitFrame(1)
          if animation.status == "paused" then self.playing = false end
        else
          animation.position = #animation.frames
          animation.timer = animation.totalDuration
          self.playing = false
        end
      end
    else
      if remaining <= elapsed and elapsed > 0 then
        animation.timer = animation.timer - remaining
        remaining = 0
      elseif position > 1 then
        remaining = remaining - math.max(0, elapsed)
        animation.position = position - 1
        animation.timer = animation.intervals[animation.position + 1]
        self:_emitFrame(animation.position)
      elseif looping then
        remaining = remaining - math.max(0, elapsed)
        animation.position = #animation.frames
        animation.timer = animation.totalDuration
        invokeLoop(animation, -1)
        self:_emitFrame(animation.position)
        if animation.status == "paused" then self.playing = false end
      else
        animation.position = 1
        animation.timer = 0
        self.playing = false
      end
    end
  end

  syncAnimation(self)
end

function SpritePlayer:draw(x, y, rotation, scaleX, scaleY, originX, originY, shearX, shearY)
  if self.animation == nil then return end
  local origin = self.sheet.origins[self.name] or self.sheet.origin or { x = 0, y = 0 }
  local animationMetadata = self.sheet.frameMetadata and self.sheet.frameMetadata[self.name]
  local frameMetadata = animationMetadata and animationMetadata[self.animation.position]
  if frameMetadata then
    if originX == nil then originX = frameMetadata.originX end
    if originY == nil then originY = frameMetadata.originY end
    originX = originX - frameMetadata.trimX
    originY = originY - frameMetadata.trimY
  else
    if originX == nil then originX = origin.x end
    if originY == nil then originY = origin.y end
  end
  scaleX = scaleX or 1
  scaleY = scaleY or scaleX
  love.graphics.push()
  love.graphics.translate(x or 0, y or 0)
  love.graphics.rotate(rotation or 0)
  love.graphics.scale(scaleX, scaleY)
  love.graphics.shear(shearX or 0, shearY or 0)
  self.animation:draw(self.sheet.image, 0, 0, 0, 1, 1, originX, originY)
  for _, name in ipairs(self.sheet.childOrder or {}) do
    local child = self.children[name]
    child.player:draw(
      child.x,
      child.y,
      child.rotation,
      child.scaleX,
      child.scaleY
    )
  end
  love.graphics.pop()
end

function SpritePlayer:on(marker, callback)
  assert(type(callback) == "function", "sprite marker callback must be a function")
  local callbacks = self.listeners[marker]
  if callbacks == nil then
    callbacks = {}
    self.listeners[marker] = callbacks
  end
  callbacks[#callbacks + 1] = callback
end

function SpritePlayer:off(marker)
  self.listeners[marker] = nil
end

function SpritePlayer:child(name)
  local child = self.children[name]
  if child == nil then
    error(("unknown nested sprite clip %q"):format(tostring(name)), 2)
  end
  return child.player
end

local function new(sheet, initial)
  assert(type(sheet) == "table", "sprite player requires a sprite sheet")
  local animations = {}
  for name, animation in pairs(sheet.animations or {}) do
    animations[name] = animation:clone()
  end

  local self = setmetatable({
    sheet = sheet,
    animations = animations,
    animation = nil,
    name = nil,
    playing = false,
    speed = 1,
    listeners = {},
    markerFrames = markerIndex(sheet),
    children = {},
  }, SpritePlayer)

  for _, name in ipairs(sheet.childOrder or {}) do
    local child = sheet.children[name]
    self.children[name] = {
      player = new(child.sheet, child.initial),
      x = child.x or 0,
      y = child.y or 0,
      rotation = child.rotation or 0,
      scaleX = child.scaleX or 1,
      scaleY = child.scaleY or child.scaleX or 1,
    }
  end

  initial = initial or (sheet.animationOrder and sheet.animationOrder[1])
  if initial ~= nil then self:play(initial, true) end
  return self
end

return {
  new = new,
}

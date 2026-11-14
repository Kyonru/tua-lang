-- Bounded LOVE input capture and approximate replay, loaded only by temporary launchers.
local supported = {
  keypressed = true, keyreleased = true, textinput = true,
  mousemoved = true, mousepressed = true, mousereleased = true, wheelmoved = true,
}
local MAX_EVENTS = 4096
local unpackValues = unpack or table.unpack

local function seedRandom(seed)
  math.randomseed(seed)
  if love.math and love.math.setRandomSeed then love.math.setRandomSeed(seed) end
end

local function safeArguments(...)
  local values = {}
  for index = 1, select("#", ...) do
    if index > 8 then return nil end
    local value = select(index, ...)
    if type(value) == "string" then
      if #value > 128 then return nil end
    elseif type(value) == "number" then
      if value ~= value or value == math.huge or value == -math.huge then return nil end
    elseif type(value) ~= "boolean" and value ~= nil then
      return nil
    end
    values[index] = value
  end
  return values
end

local function record(seed, currentFrame)
  seedRandom(seed)
  local events, dropped = {}, 0
  local originalPoll = love.event.poll
  local wrappedPoll = function(...)
    local iterator = originalPoll(...)
    return function()
      local name, a, b, c, d, e, f, g, h = iterator()
      if supported[name] then
        local args = safeArguments(a, b, c, d, e, f, g, h)
        if args and #events < MAX_EVENTS then
          events[#events + 1] = { frame = currentFrame() + 1, name = name, args = args }
        else dropped = dropped + 1 end
      end
      return name, a, b, c, d, e, f, g, h
    end
  end
  love.event.poll = wrappedPoll
  return {
    snapshot = function() return { seed = seed, events = events, dropped = dropped } end,
    stop = function() if love.event.poll == wrappedPoll then love.event.poll = originalPoll end end,
  }
end

local function replay(data)
  seedRandom(data.seed)
  local originalPoll = love.event.poll
  local nextEvent, frameIndex = 1, 0
  local wrappedPoll = function(...)
    local iterator = originalPoll(...)
    return function()
      local item = data.events[nextEvent]
      if item and item.frame <= frameIndex then
        nextEvent = nextEvent + 1
        return item.name, unpackValues(item.args)
      end
      while true do
        local name, a, b, c, d, e, f, g, h = iterator()
        if name == nil or name == "quit" or not supported[name] then
          return name, a, b, c, d, e, f, g, h
        end
      end
    end
  end
  love.event.poll = wrappedPoll
  return {
    install = function()
      local originalRun = love.run
      love.run = function(...)
        local frame = originalRun(...)
        if type(frame) ~= "function" then return frame end
        return function(...)
          frameIndex = frameIndex + 1
          return frame(...)
        end
      end
    end,
  }
end

return { record = record, replay = replay }

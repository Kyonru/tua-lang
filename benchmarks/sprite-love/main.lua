-- Compares sprite construction, nested updates, and real LOVE leaf draws.
--
-- love benchmarks/sprite-love [--jit]
--   <before/anim8.lua> <before/tuasprite.lua>
--   <after/anim8.lua> <after/tuasprite.lua>

local SAMPLES = 7
local CONSTRUCTION_PLAYERS = 2000
local ACTIVE_PLAYERS = 2000
local UPDATE_FRAMES = 100
local DRAW_FRAMES = 30

local function loadRuntime(path)
  local chunk, err = loadfile(path)
  if not chunk then error("could not load runtime " .. tostring(path) .. ": " .. tostring(err)) end
  return chunk()
end

local function median(values)
  local copy = {}
  for index = 1, #values do copy[index] = values[index] end
  table.sort(copy)
  return copy[math.floor((#copy + 1) / 2)]
end

local function measurePair(beforeFn, afterFn)
  local samples = { {}, {} }
  local callbacks = { beforeFn, afterFn }
  for sample = 1, SAMPLES do
    local order = sample % 2 == 1 and { 1, 2 } or { 2, 1 }
    for _, index in ipairs(order) do
      local start = love.timer.getTime()
      callbacks[index]()
      samples[index][#samples[index] + 1] = (love.timer.getTime() - start) * 1000
    end
  end
  return median(samples[1]), median(samples[2])
end

local function makeSheet(runtime, image, frames, preindexed, child)
  local animations = {}
  local animationOrder = {}
  local origins = {}
  local looping = {}
  local markers = {}
  local markerFrames = {}
  for animationIndex = 1, 4 do
    local name = "animation" .. animationIndex
    animations[name] = runtime.anim8.newAnimation(frames, 1 / (8 + animationIndex))
    animationOrder[animationIndex] = name
    origins[name] = { x = 8, y = 8 }
    looping[name] = true
    markers[name] = { { frame = 2, name = "step" } }
    markerFrames[name] = { [2] = { "step" } }
  end
  local children = {}
  local childOrder = {}
  if child then
    children.child = { sheet = child, initial = "animation1", x = 2, y = 3 }
    childOrder[1] = "child"
  end
  return {
    image = image,
    animations = animations,
    animationOrder = animationOrder,
    origin = { x = 8, y = 8 },
    origins = origins,
    looping = looping,
    markers = markers,
    __tuaMarkerFrames = preindexed and markerFrames or nil,
    frameMetadata = {},
    children = children,
    childOrder = childOrder,
  }
end

local function players(runtime, sheet, count)
  local result = {}
  for index = 1, count do result[index] = runtime.sprite.new(sheet, "animation1") end
  return result
end

local function measureConstruction(runtimes, sheets)
  local samples = { {}, {} }
  for sample = 1, SAMPLES do
    local order = sample % 2 == 1 and { 1, 2 } or { 2, 1 }
    for _, index in ipairs(order) do
      collectgarbage("collect")
      local start = love.timer.getTime()
      local retained = players(runtimes[index], sheets[index], CONSTRUCTION_PLAYERS)
      samples[index][#samples[index] + 1] = (love.timer.getTime() - start) * 1000
      if retained[CONSTRUCTION_PLAYERS].name ~= "animation1" then error("invalid player") end
      retained = nil
    end
  end
  collectgarbage("collect")
  return median(samples[1]), median(samples[2])
end

local function updates(list)
  return function()
    for _ = 1, UPDATE_FRAMES do
      for index = 1, #list do list[index]:update(1 / 60) end
    end
  end
end

local function draws(list, canvas)
  return function()
    love.graphics.setCanvas(canvas)
    for _ = 1, DRAW_FRAMES do
      love.graphics.clear(0, 0, 0, 0)
      for index = 1, #list do
        list[index]:draw(index % 64, math.floor(index / 64) % 64, index * 0.001)
      end
    end
    love.graphics.setCanvas()
  end
end

local function batchedDraws(list, canvas, image)
  local batch = love.graphics.newSpriteBatch(image, #list, "stream")
  return function()
    love.graphics.setCanvas(canvas)
    for _ = 1, DRAW_FRAMES do
      batch:clear()
      for index = 1, #list do
        list[index]:addToBatch(batch, index % 64, math.floor(index / 64) % 64, index * 0.001)
      end
      love.graphics.clear(0, 0, 0, 0)
      love.graphics.draw(batch)
    end
    love.graphics.setCanvas()
  end
end

local function constructionGarbage(runtime, sheet)
  collectgarbage("collect")
  collectgarbage("stop")
  local before = collectgarbage("count")
  local retained = players(runtime, sheet, CONSTRUCTION_PLAYERS)
  local garbage = (collectgarbage("count") - before) / CONSTRUCTION_PLAYERS
  collectgarbage("restart")
  if retained[1].name ~= "animation1" then error("invalid player") end
  return garbage
end

function love.load(args)
  local paths = {}
  for _, value in ipairs(args) do
    if value == "--jit" then
      if jit then jit.on(); jit.flush() end
    else
      paths[#paths + 1] = value
    end
  end
  if #paths ~= 4 then
    print("usage: love benchmarks/sprite-love [--jit] <before anim8> <before tuasprite> <after anim8> <after tuasprite>")
    love.event.quit(2)
    return
  end

  local runtimes = {
    { label = "before", anim8 = loadRuntime(paths[1]), sprite = loadRuntime(paths[2]) },
    { label = "after", anim8 = loadRuntime(paths[3]), sprite = loadRuntime(paths[4]) },
  }
  local data = love.image.newImageData(16, 16)
  data:mapPixel(function() return 1, 1, 1, 1 end)
  local image = love.graphics.newImage(data)
  local frames = {
    love.graphics.newQuad(0, 0, 8, 8, 16, 16),
    love.graphics.newQuad(8, 0, 8, 8, 16, 16),
    love.graphics.newQuad(0, 8, 8, 8, 16, 16),
    love.graphics.newQuad(8, 8, 8, 8, 16, 16),
  }
  local canvas = love.graphics.newCanvas(64, 64)

  local leafSheets = {
    makeSheet(runtimes[1], image, frames, false),
    makeSheet(runtimes[2], image, frames, true),
  }
  local childSheets = {
    makeSheet(runtimes[1], image, frames, false),
    makeSheet(runtimes[2], image, frames, true),
  }
  local parentSheets = {
    makeSheet(runtimes[1], image, frames, false, childSheets[1]),
    makeSheet(runtimes[2], image, frames, true, childSheets[2]),
  }
  local leafPlayers = {
    players(runtimes[1], leafSheets[1], ACTIVE_PLAYERS),
    players(runtimes[2], leafSheets[2], ACTIVE_PLAYERS),
  }
  local parentPlayers = {
    players(runtimes[1], parentSheets[1], ACTIVE_PLAYERS),
    players(runtimes[2], parentSheets[2], ACTIVE_PLAYERS),
  }

  local major, minor, revision = love.getVersion()
  print(string.format("LOVE %d.%d.%d, %s, JIT %s", major, minor, revision,
    jit and jit.version or _VERSION, jit and (jit.status() and "on" or "off") or "n/a"))
  print(string.format("%d alternating samples; %d players", SAMPLES, ACTIVE_PLAYERS))
  print(string.format("%-24s %12s %12s %9s", "workload", "before ms", "after ms", "speedup"))

  local before, after = measureConstruction(runtimes, leafSheets)
  print(string.format("%-24s %12.3f %12.3f %8.2fx", "construct 2k", before, after, before / after))
  before, after = measurePair(updates(parentPlayers[1]), updates(parentPlayers[2]))
  print(string.format("%-24s %12.3f %12.3f %8.2fx", "nested update x100", before, after, before / after))
  before, after = measurePair(draws(leafPlayers[1], canvas), draws(leafPlayers[2], canvas))
  print(string.format("%-24s %12.3f %12.3f %8.2fx", "leaf draw x30", before, after, before / after))

  local direct = draws(leafPlayers[2], canvas)
  local batched = batchedDraws(leafPlayers[2], canvas, image)
  before, after = measurePair(direct, batched)
  print(string.format("%-24s %12.3f %12.3f %8.2fx", "direct vs batch x30", before, after, before / after))

  print(string.format("construction KB/player: before %.3f, after %.3f",
    constructionGarbage(runtimes[1], leafSheets[1]),
    constructionGarbage(runtimes[2], leafSheets[2])))
  love.event.quit(0)
end

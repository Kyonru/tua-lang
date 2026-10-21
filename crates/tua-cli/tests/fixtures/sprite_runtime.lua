local root = assert(arg[1])
local sprite = assert(loadfile(root .. "/vendor/runtime/tuasprite.lua"))()

local graphicsCalls = {}
love = {
  graphics = {
    push = function() graphicsCalls[#graphicsCalls + 1] = { "push" } end,
    translate = function(...) graphicsCalls[#graphicsCalls + 1] = { "translate", ... } end,
    rotate = function(...) graphicsCalls[#graphicsCalls + 1] = { "rotate", ... } end,
    scale = function(...) graphicsCalls[#graphicsCalls + 1] = { "scale", ... } end,
    shear = function(...) graphicsCalls[#graphicsCalls + 1] = { "shear", ... } end,
    pop = function() graphicsCalls[#graphicsCalls + 1] = { "pop" } end,
  },
}

local function animation(name, drawCalls)
  local value = {
    name = name,
    frames = { true },
    position = 1,
    intervals = { 0 },
    durations = { 1 },
    totalDuration = 1,
    timer = 0,
    status = "paused",
  }

  function value:clone() return animation(self.name, drawCalls) end
  function value:gotoFrame(frame) self.position = frame end
  function value:draw(...)
    if drawCalls then drawCalls[#drawCalls + 1] = { self.name, ... } end
  end
  function value:getFrameInfo(...)
    return self.frames[self.position], ...
  end
  return value
end

local function sheet(name, drawCalls)
  return {
    image = name .. "-image",
    animations = { idle = animation(name, drawCalls) },
    animationOrder = { "idle" },
    origin = { x = 0, y = 0 },
    origins = { idle = { x = 0, y = 0 } },
    looping = {},
    markers = {},
    __tuaMarkerFrames = {},
    childOrder = {},
    children = {},
  }
end

-- Generated sheets precompute marker lookup once, and every player reuses it.
do
  local marked = sheet("marked")
  marked.__tuaMarkerFrames = { idle = { [1] = { "step" } } }
  local first = sprite.new(marked)
  local second = sprite.new(marked)
  assert(first.markerFrames == marked.__tuaMarkerFrames)
  assert(second.markerFrames == marked.__tuaMarkerFrames)

  local calls = 0
  first:on("step", function() calls = calls + 1 end)
  first:_emitFrame(1)
  assert(calls == 1)
end

-- A generated-shape leaf delegates its complete transform to anim8 and does
-- not touch the graphics stack.
do
  local drawCalls = {}
  local leaf = sheet("leaf", drawCalls)
  local player = sprite.new(leaf)
  player:draw(10, 20, 0.3, 2, 3, nil, nil, 0.4, 0.5)

  assert(#graphicsCalls == 0, "leaf draw must not make graphics-stack calls")
  local call = assert(drawCalls[1])
  assert(call[1] == "leaf" and call[2] == "leaf-image")
  assert(call[3] == 10 and call[4] == 20 and call[5] == 0.3)
  assert(call[6] == 2 and call[7] == 3)
  assert(call[8] == 0 and call[9] == 0)
  assert(call[10] == 0.4 and call[11] == 0.5)
end

-- SpriteBatch insertion uses the same resolved frame transform as direct draw
-- and rejects display trees that cannot be represented by one batch entry.
do
  local leaf = sheet("batch")
  local player = sprite.new(leaf)
  local added
  local batch = {
    add = function(_, ...)
      added = { ... }
      return 17
    end,
  }
  assert(player:addToBatch(batch, 10, 20, 0.3, 2, 3, 4, 5, 0.6, 0.7) == 17)
  assert(added[1] == true and added[2] == 10 and added[3] == 20)
  assert(added[4] == 0.3 and added[5] == 2 and added[6] == 3)
  assert(added[7] == 4 and added[8] == 5 and added[9] == 0.6 and added[10] == 0.7)

  local parent = sheet("batch-parent")
  parent.childOrder = { "child" }
  parent.children.child = { sheet = sheet("batch-child") }
  local ok, message = pcall(function() sprite.new(parent):addToBatch(batch, 0, 0) end)
  assert(not ok and tostring(message):find("nested children", 1, true))
end

-- Trimmed-frame origins are preserved on the direct path.
do
  local drawCalls = {}
  local leaf = sheet("trimmed", drawCalls)
  leaf.frameMetadata = {
    idle = { { originX = 8, originY = 9, trimX = 2, trimY = 3 } },
  }
  local player = sprite.new(leaf)
  player:draw()
  local call = assert(drawCalls[1])
  assert(call[8] == 6 and call[9] == 6)
end

-- The generated-sheet clone path rebuilds independent public arrays and
-- playback state in one pass; sprite optimizations must not couple players.
do
  local anim8 = assert(loadfile(root .. "/vendor/runtime/anim8.lua"))()
  local source = anim8.newAnimation({ "one", "two" }, { 0.1, 0.2 })
  source.timer = 0.15
  source.position = 2
  source.status = "paused"
  source:flipH()
  local clone = source:_cloneTuaPlayer()

  assert(clone ~= source)
  assert(clone.frames ~= source.frames and clone.durations ~= source.durations)
  assert(clone.intervals ~= source.intervals)
  assert(clone.frames[1] == "one" and clone.durations[2] == 0.2)
  assert(clone.totalDuration == source.totalDuration and clone.onLoop == source.onLoop)
  assert(clone.timer == 0 and clone.position == 1 and clone.status == "playing")
  assert(clone.flippedH and not clone.flippedV)
  clone.frames[1] = "changed"
  clone.durations[1] = 9
  clone.intervals[1] = 9
  assert(source.frames[1] == "one" and source.durations[1] == 0.1 and source.intervals[1] == 0)

  -- Numeric prototype edits are reflected exactly as they are by public clone.
  source.durations[1] = 0.4
  local edited = source:_cloneTuaPlayer()
  assert(edited.durations[1] == 0.4 and edited.intervals[2] == 0.4)
  assert(math.abs(edited.totalDuration - 0.6) < 1e-12)
end

-- A parent retains the graphics stack, while its leaf child uses the direct
-- path inside that inherited transform.
do
  graphicsCalls = {}
  local drawCalls = {}
  local parent = sheet("parent", drawCalls)
  local child = sheet("child", drawCalls)
  parent.childOrder = { "hat" }
  parent.children.hat = {
    sheet = child,
    x = 4,
    y = 5,
    rotation = 0.6,
    scaleX = 2,
    scaleY = 3,
  }

  sprite.new(parent):draw(10, 20, 0.3, 2, 3, 7, 8, 0.4, 0.5)
  assert(#graphicsCalls == 6)
  assert(graphicsCalls[1][1] == "push")
  assert(graphicsCalls[2][1] == "translate" and graphicsCalls[2][2] == 10 and graphicsCalls[2][3] == 20)
  assert(graphicsCalls[3][1] == "rotate" and graphicsCalls[3][2] == 0.3)
  assert(graphicsCalls[4][1] == "scale" and graphicsCalls[4][2] == 2 and graphicsCalls[4][3] == 3)
  assert(graphicsCalls[5][1] == "shear" and graphicsCalls[5][2] == 0.4 and graphicsCalls[5][3] == 0.5)
  assert(graphicsCalls[6][1] == "pop")
  assert(drawCalls[1][1] == "parent" and drawCalls[1][3] == 0 and drawCalls[1][4] == 0)
  assert(drawCalls[2][1] == "child" and drawCalls[2][3] == 4 and drawCalls[2][4] == 5)
  assert(drawCalls[2][5] == 0.6 and drawCalls[2][6] == 2 and drawCalls[2][7] == 3)
end

-- Stable leaf updates and draws stay allocation-free after warm-up.
do
  local leaf = sheet("quiet")
  local player = sprite.new(leaf)
  player:draw()
  player:update(0)

  collectgarbage("collect")
  collectgarbage("stop")
  local before = collectgarbage("count")
  for _ = 1, 10000 do
    player:update(0)
    player:draw()
  end
  local allocatedKb = collectgarbage("count") - before
  collectgarbage("restart")

  assert(allocatedKb < 32, string.format(
    "10,000 idle sprite updates and draws should allocate under 32 KB (got %.1f KB)", allocatedKb))
end

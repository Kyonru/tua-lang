-- A minimal LOVE stub: an 800x600 window and a recorder for graphics calls.
local calls = {}
local scissor = nil
_G.love = {
  graphics = {
    getDimensions = function() return 800, 600 end,
    push = function() calls[#calls + 1] = "push" end,
    pop = function() calls[#calls + 1] = "pop" end,
    translate = function(x, y) calls[#calls + 1] = ("translate %g %g"):format(x, y) end,
    scale = function(s) calls[#calls + 1] = ("scale %g"):format(s) end,
    rotate = function(r) calls[#calls + 1] = ("rotate %g"):format(r) end,
    getScissor = function()
      if scissor then return scissor[1], scissor[2], scissor[3], scissor[4] end
    end,
    setScissor = function(...) scissor = select("#", ...) > 0 and { ... } or nil end,
    intersectScissor = function(...) scissor = { ... } end,
  },
  mouse = { getPosition = function() return 400, 300 end },
}

local camera = require("tuacamera")

local function near(actual, expected, label)
  assert(math.abs(actual - expected) < 1e-6, ("%s: expected %s, got %s"):format(label or "value", expected, actual))
end

-- Lock-on without smoothing centers the target on update.
local cam = camera.new()
cam:follow(120, 80)
cam:update(1 / 60)
near(cam.x, 120); near(cam.y, 80)

-- Smoothing approaches the target exponentially and never overshoots.
local smooth = camera.new({ smoothing = 10 })
smooth:follow(100, 0)
smooth:update(0.1)
near(smooth.x, 100 * (1 - math.exp(-1)), "smoothing step")
for _ = 1, 200 do smooth:update(1 / 60) end
near(smooth.x, 100, "smoothing settles")

-- The platformer deadzone ignores small moves and follows large ones.
local platform = camera.new({ style = "platformer" })
platform:follow(40, 0) -- the zone is 100 px wide at zoom 1, so 40 stays inside
platform:update(1 / 60)
near(platform.x, 0, "inside deadzone")
platform:follow(250, 0)
platform:update(1 / 60)
near(platform.x, 250 - 50, "pushed by deadzone edge")

-- Bounds keep the view inside the world, and center a world smaller than it.
local bounded = camera.new({ bounds = { x = 0, y = 0, w = 2000, h = 400 } })
bounded:lookAt(-500, 1000)
near(bounded.x, 400, "left bound"); near(bounded.y, 200, "short world is centered")
bounded:lookAt(5000, 0)
near(bounded.x, 1600, "right bound")

-- Zoom shrinks the visible world area.
local zoomed = camera.new({ x = 100, y = 50, zoom = 2 })
local vx, vy, vw, vh = zoomed:visibleBounds()
near(vx, -100); near(vy, -100); near(vw, 400); near(vh, 300)

-- toWorld and toScreen invert each other with zoom, rotation, and a viewport.
local tilted = camera.new({ x = 30, y = -20, zoom = 1.5, rotation = 0.3, viewport = { x = 100, y = 50, w = 400, h = 300 } })
local wx, wy = tilted:toWorld(123, 456)
local sx, sy = tilted:toScreen(wx, wy)
near(sx, 123, "round trip x"); near(sy, 456, "round trip y")
local cx, cy = tilted:toWorld(300, 200)
near(cx, 30, "viewport center maps to camera x"); near(cy, -20, "viewport center maps to camera y")
local mx, my = camera.new({ x = 7, y = 9 }):mousePosition()
near(mx, 7); near(my, 9)

-- Screen style moves a whole view at a time.
local rooms = camera.new({ style = "screen" })
rooms:follow(1000, 100)
rooms:update(1 / 60)
near(rooms.x, 1200, "second room column"); near(rooms.y, 300, "first room row")

-- Look-ahead leads the target by its velocity.
local leading = camera.new({ lead = 0.5 })
leading:follow(0, 0); leading:update(0.1)
leading:follow(10, 0); leading:update(0.1)
near(leading.x, 10 + 100 * 0.5, "lead")

-- Shake grows with trauma squared and fades back to rest.
local shaky = camera.new()
shaky:shake(0.8)
shaky:update(0.016)
assert(shaky.trauma > 0 and shaky.trauma < 0.8)
for _ = 1, 120 do shaky:update(1 / 60) end
assert(shaky.trauma == 0 and shaky.shakeX == 0 and shaky.shakeY == 0, "shake settles")

-- attach applies one transform and detach restores it and the scissor.
local view = camera.new({ x = 10, y = 20, zoom = 2, viewport = { x = 0, y = 0, w = 400, h = 300 } })
scissor = { 1, 2, 3, 4 }
calls = {}
view:draw(function() calls[#calls + 1] = "draw" end)
local recorded = table.concat(calls, ","):gsub("rotate %-0,", "rotate 0,")
assert(recorded == "push,translate 200 150,scale 2,rotate 0,translate -10 -20,draw,pop", recorded)
assert(scissor and scissor[1] == 1 and scissor[4] == 4, "outer scissor restored")

-- A failing draw callback still detaches before the error propagates.
calls = {}
assert(not pcall(function() view:draw(function() error("boom") end) end))
assert(calls[#calls] == "pop", "detached after error")

assert(not pcall(camera.new, { style = "orbit" }))

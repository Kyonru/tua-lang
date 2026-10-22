local tween = require("tuatween")

local function near(actual, expected)
  assert(math.abs(actual - expected) < 1e-9, ("expected %s, got %s"):format(expected, actual))
end

-- Values move linearly, completion callbacks fire, and `after` chains.
local box = { x = 0, y = 10 }
local log = {}
local first = tween.to(box, 1, { x = 100 }):ease("linear"):oncomplete(function()
  log[#log + 1] = "x"
end)
first:after(0.5, { y = 0 }):ease("linear"):oncomplete(function()
  log[#log + 1] = "y"
end)
tween.update(0.5)
near(box.x, 50)
tween.update(0.5)
assert(box.x == 100 and log[1] == "x")
tween.update(0.25)
near(box.y, 5)
tween.update(0.25)
assert(box.y == 0 and log[2] == "y")

-- Groups update independently, delays wait, and stop leaves the value.
local group = tween.group()
local fade = { a = 1 }
local fading = group:to(fade, 1, { a = 0 }):delay(0.5)
group:update(0.5)
assert(fade.a == 1)
group:update(0.5)
assert(fade.a > 0 and fade.a < 1)
local stopped_at = fade.a
fading:stop()
group:update(1)
assert(fade.a == stopped_at)
tween.update(1)
assert(fade.a == stopped_at, "the default group must not touch another group's tweens")

-- remove() stops a default-group tween before it starts.
local value = { v = 0 }
tween.remove(tween.to(value, 1, { v = 1 }))
tween.update(1)
assert(value.v == 0)

-- Every documented easing exists, and a tween lands exactly on its target
-- even where the curve itself stops just short of 1 (expo).
for _, family in ipairs({ "quad", "cubic", "quart", "quint", "expo", "sine", "circ", "back", "elastic" }) do
  for _, suffix in ipairs({ "in", "out", "inout" }) do
    local name = family .. suffix
    assert(type(tween.easing[name](0.5)) == "number", name)
    local target = { v = 0 }
    tween.to(target, 1, { v = 10 }):ease(name)
    tween.update(1)
    assert(target.v == 10, name)
  end
end
near(tween.easing.quadin(0.5), 0.25)
assert(not pcall(function()
  tween.to({ x = 0 }, 1, { x = 1 }):ease("bounce")
end))

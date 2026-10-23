local mouseX, mouseY = 10, 20
local positionReads = 0

love = {
  keyboard = {
    isDown = function()
      return false
    end,
    isScancodeDown = function()
      return false
    end,
  },
  mouse = {
    getPosition = function()
      positionReads = positionReads + 1
      return mouseX, mouseY
    end,
    isDown = function()
      return false
    end,
  },
}

local baton = require("baton")

local disabled = baton.new({ controls = {} })
disabled:update()
assert(positionReads == 0, "disabled pointer tracking must not poll the mouse")
local ok, err = pcall(function()
  disabled:pointer()
end)
assert(not ok and tostring(err):find("Pointer tracking is not enabled", 1, true))

local joystick = {
  getAxis = function()
    return 0
  end,
  getGamepadAxis = function(_, axis)
    return axis == "rightx" and 0.75 or 0
  end,
}

local controls = baton.new({
  controls = {
    aimRight = { "axis:rightx+" },
  },
  joystick = joystick,
  pointer = true,
})

assert(positionReads == 1, "pointer initialization should poll exactly once")
local x, y = controls:pointer()
local dx, dy = controls:pointerDelta()
assert(x == 10 and y == 20)
assert(dx == 0 and dy == 0)

controls:update()
assert(positionReads == 2, "each enabled update should poll exactly once")
assert(controls:getActiveDevice() == "joy")

mouseX, mouseY = 14, 17
controls:update()
x, y = controls:pointer()
dx, dy = controls:pointerDelta()
assert(positionReads == 3)
assert(x == 14 and y == 17)
assert(dx == 4 and dy == -3)
assert(controls:getActiveDevice() == "kbm")

return true

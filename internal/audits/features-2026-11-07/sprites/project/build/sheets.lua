-- Sprite helper: Sheet lowers to anim8-backed Lua.
local anim8 = require("anim8")
local tuasprite = require("tuasprite")
local SheetImage = love.graphics.newImage("assets/player.png")
local SheetGrid = anim8.newGrid(32, 32, SheetImage:getWidth(), SheetImage:getHeight())
local Sheet = {
  image = SheetImage,
  grid = SheetGrid,
  frame = { width = 32, height = 32, left = 0, top = 0, border = 0 },
  origin = { x = 16, y = 32 },
  animations = {
    run = anim8.newAnimation(SheetGrid("1-4", 1), 1 / 4),
    stopStart = anim8.newAnimation(SheetGrid("1-4", 1), 1 / 4, "pauseAtStart"),
    stopEnd = anim8.newAnimation(SheetGrid("1-4", 1), 1 / 4, "pauseAtEnd"),
    loopFn = anim8.newAnimation(SheetGrid("1-4", 1), 1 / 4, function(animation, loops)
      print("LOOP_CALLBACK", loops)
      if loops > 1 then animation:pause() end
    end),
    single = anim8.newAnimation(SheetGrid(1, 1), 1 / 1),
    multi = anim8.newAnimation(SheetGrid("1-2", 1, 2, 2), { 0.1, 0.2, 0.3 }),
  },
  animationOrder = {
    "run",
    "stopStart",
    "stopEnd",
    "loopFn",
    "single",
    "multi",
  },
  origins = {
    run = { x = 16, y = 32 },
    stopStart = { x = 16, y = 32 },
    stopEnd = { x = 16, y = 32 },
    loopFn = { x = 16, y = 32 },
    single = { x = 16, y = 32 },
    multi = { x = 16, y = 32 },
  },
  markers = {
    run = {
      { frame = 1, name = "start" },
      { frame = 2, name = "step" },
    },
    stopStart = {
    },
    stopEnd = {
    },
    loopFn = {
    },
    single = {
    },
    multi = {
    },
  },
  __tuaMarkerFrames = {
    run = {
      [1] = { "start", },
      [2] = { "step", },
    },
  },
  frameMetadata = {
    run = {
    },
    stopStart = {
    },
    stopEnd = {
    },
    loopFn = {
    },
    single = {
    },
    multi = {
    },
  },
  childOrder = {
  },
  children = {
  },
  looping = {
    run = true,
    stopStart = false,
    stopEnd = false,
    loopFn = true,
    single = true,
    multi = true,
  },
  player = function(self, initial)
    return tuasprite.new(self, initial)
  end,
}
-- Sprite helper: Parent lowers to anim8-backed Lua.
local anim8 = require("anim8")
local tuasprite = require("tuasprite")
local ParentImage = love.graphics.newImage("assets/player.png")
local ParentGrid = anim8.newGrid(32, 32, ParentImage:getWidth(), ParentImage:getHeight())
local Parent = {
  image = ParentImage,
  grid = ParentGrid,
  frame = { width = 32, height = 32, left = 0, top = 0, border = 0 },
  origin = { x = 0, y = 0 },
  animations = {
    idle = anim8.newAnimation(ParentGrid(1, 1), 1 / 1),
  },
  animationOrder = {
    "idle",
  },
  origins = {
    idle = { x = 0, y = 0 },
  },
  markers = {
    idle = {
    },
  },
  __tuaMarkerFrames = {
  },
  frameMetadata = {
    idle = {
    },
  },
  childOrder = {
    "hat",
  },
  children = {
    hat = { sheet = Sheet, initial = "run", x = 4, y = -8, rotation = 0, scaleX = 1, scaleY = 1 },
  },
  looping = {
    idle = true,
  },
  player = function(self, initial)
    return tuasprite.new(self, initial)
  end,
}
return { Sheet = Sheet, Parent = Parent }

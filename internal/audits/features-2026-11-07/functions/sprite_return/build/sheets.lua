-- Sprite helper: S lowers to anim8-backed Lua.
local anim8 = require("anim8")
local tuasprite = require("tuasprite")
local SImage = love.graphics.newImage("assets/player.png")
local SGrid = anim8.newGrid(32, 32, SImage:getWidth(), SImage:getHeight())
local S = {
  image = SImage,
  grid = SGrid,
  frame = { width = 32, height = 32, left = 0, top = 0, border = 0 },
  origin = { x = 0, y = 0 },
  animations = {
    idle = anim8.newAnimation(SGrid("1-2", 1), 1 / 4),
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
  },
  children = {
  },
  looping = {
    idle = true,
  },
  player = function(self, initial)
    return tuasprite.new(self, initial)
  end,
}
return S:player("absent")

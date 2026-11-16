local Log = require("tualog")
local original_love, original_print = love, print
local prints, text_objects, draw_calls = {}, {}, {}
_G.print = function(line) prints[#prints + 1] = line end
_G.love = { graphics = {
  newText = function(_, value)
    local object = { value = value, sets = 0, releases = 0 }
    function object:set(next_value) self.value = next_value; self.sets = self.sets + 1 end
    function object:getWidth() return #self.value * 8 end
    function object:getHeight() return 16 end
    function object:release() self.releases = self.releases + 1 end
    text_objects[#text_objects + 1] = object
    return object
  end,
  getFont = function() return {} end,
  push = function(mode) assert(mode == "all") end,
  origin = function() draw_calls[#draw_calls + 1] = "origin" end,
  setColor = function() end,
  rectangle = function(_, x, y, width, height)
    draw_calls[#draw_calls + 1] = { x, y, width, height }
  end,
  draw = function(object, x, y)
    assert(object == text_objects[#text_objects])
    draw_calls[#draw_calls + 1] = { x, y }
  end,
  pop = function() end,
} }

local ok, err = pcall(function()
  Log.configure({ console = false, screen = { lines = 2, maxBytes = 64 } })
  Log.draw()
  assert(#text_objects == 0 and #draw_calls == 0, "empty overlay must not draw")

  Log.info("first")
  Log.draw(12, 20)
  assert(#prints == 0 and #text_objects == 1, "screen-only logging must not print")
  assert(text_objects[1].value:find("first", 1, true))
  assert(draw_calls[1] == "origin", "overlay must use screen coordinates")
  assert(draw_calls[3][1] == 12 and draw_calls[3][2] == 20)
  Log.draw(12, 20)
  assert(text_objects[1].sets == 0, "unchanged overlay must reuse text layout")

  Log.info("second")
  Log.info("third")
  Log.draw()
  assert(text_objects[1].sets == 1)
  assert(not text_objects[1].value:find("first", 1, true))
  assert(text_objects[1].value:find("second", 1, true)
    and text_objects[1].value:find("third", 1, true), "ring must keep newest messages in order")

  Log.info(string.rep("é", 40))
  Log.draw()
  local last = text_objects[1].value:match("[^\n]+$")
  assert(#last <= 64 and last:sub(-3) == "...", "visible line must be bounded")
  assert(last:sub(-5, -4) == "é", "truncation must preserve UTF-8 boundaries")

  local skipped = 0
  Log.info(string.rep("x", 10000), setmetatable({}, {
    __tostring = function() skipped = skipped + 1; return "unused" end,
  }))
  Log.draw()
  assert(skipped == 0, "screen-only logging must skip arguments beyond its visible limit")
  assert(#text_objects[1].value:match("[^\n]+$") <= 64)

  assert(not pcall(Log.configure, { console = false, screen = { lines = 0 } }))
  Log.draw()
  assert(text_objects[1].releases == 0, "invalid reconfiguration must retain the overlay")
  Log.clearScreen()
  local calls = #draw_calls
  Log.draw()
  assert(#draw_calls == calls, "cleared overlay must not draw")
  Log.configure({ console = false })
  assert(text_objects[1].releases == 1, "disabling screen output must release cached Text")
  local conversions = 0
  Log.info(setmetatable({}, { __tostring = function() conversions = conversions + 1 end }))
  assert(conversions == 0, "disabled destinations must skip message formatting")
end)

Log.configure({ console = true })
_G.love, _G.print = original_love, original_print
assert(ok, tostring(err))

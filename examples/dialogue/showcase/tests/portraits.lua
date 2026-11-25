local loaded = 0
local released = 0
local drawn = {}
local function noop() end
love = {
  graphics = {
    newImage = function(path)
      loaded = loaded + 1
      return {
        setFilter = noop,
        getWidth = function() return 512 end,
        getHeight = function() return 512 end,
        release = function() released = released + 1 end,
        path = path,
      }
    end,
    push = noop,
    pop = noop,
    translate = noop,
    scale = noop,
    setColor = noop,
    rectangle = noop,
    circle = noop,
    draw = function(image) drawn[#drawn + 1] = image.path end,
  },
}
local portraits = require("portraits").new()
assert(loaded == 8)
portraits.draw("mara", "", 0, 0, 120)
portraits.draw("mara.worried", "worried", 0, 0, 120)
assert(drawn[1] ~= drawn[2], "resolved expression IDs select different images")
for index = 1, 100 do portraits.draw("ivo", "", 0, 0, 120) end
assert(loaded == 8, "drawing reuses loaded images")
local beforeUnknown = #drawn
portraits.draw("missing", "", 0, 0, 120)
assert(#drawn == beforeUnknown, "unknown portraits leave the panel readable")
portraits.dispose()
portraits.dispose()
assert(released == loaded, "owned image resources release exactly once")
portraits.draw("mara", "", 0, 0, 120)
assert(#drawn == beforeUnknown)
print("SHOWCASE_PORTRAITS_OK bundled expressions, reuse, disposal")

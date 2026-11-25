local Controller = require("controller")
local function noop() end
local chatter = 0
local function backend()
  return {
    stop = noop,
    pause = noop,
    resume = noop,
    dispose = noop,
    volume = noop,
    voice = function() return false end,
    speaking = function() return false end,
    chatter = function()
      chatter = chatter + 1
      return true
    end,
    wordStart = function() return true end,
  }
end
local host = Controller.new(backend(), backend(), "rich")
host.player():start("@line:rich.timing")
host.update(0.5)
assert(host.view().visible == 4, "scoped speed is eight graphemes per second")
local beforeFinish = chatter
host.advance()
assert(host.view().complete)
assert(chatter == beforeFinish, "manual finish skips timing without catch-up chatter")
host.settings.reducedMotion = true
host.player():start("@line:rich.aside")
host.update(0)
assert(host.view().complete, "reduced motion skips the authored pause")
host.player():start("@line:rich.nested")
host.update(0)
local node = host.player():snapshot().node
local nested = false
for _, run in ipairs(node.presentation.runs) do
  nested = nested or (run.emphasis and #run.styles == 2)
end
assert(nested, "highlight, emphasis and underline share a nested run")
host.player():updateContext({ name = "[style=warning]literal[/style]", permit = true })
node = host.player():snapshot().node
assert(node.text:find("[style=warning]literal[/style]", 1, true))
for _, run in ipairs(node.presentation.runs) do
  for _, style in ipairs(run.styles) do assert(style ~= "warning", "bindings remain literal") end
end
host.locale()
assert(host.player():snapshot().node.text:find("Souviens", 1, true))
assert(host.player():snapshot().node.presentation.resolvedLocale == "fr")
host.dispose()
-- Rendering checks use the emitted adapter with a recording graphics backend.
local prints = {}
local lines = 0
local boxes = 0
love = {
  graphics = {
    setColor = noop,
    setLineWidth = noop,
    print = function(text, x, y) prints[#prints + 1] = { x, y } end,
    line = function() lines = lines + 1 end,
    rectangle = function() boxes = boxes + 1 end,
  },
}
local Rich = require("rich_styles")
local font = { getWidth = function() return 12 end, getHeight = function() return 20 end }
local palette = require("theme").palette("panel")
local effects = Rich.resolve({
  {
    firstCluster = 1,
    lastCluster = 1,
    emphasis = true,
    styles = { "wave", "shake", "underline", "strike", "highlight" },
  },
}, 1)
Rich.draw("é", font, effects, palette, 20, 40, 1, 0, true)
local stationary = prints[1]
assert(lines == 2 and boxes == 1, "static decorations remain visible")
prints = {}
Rich.draw("é", font, effects, palette, 20, 40, 1, 2, true)
assert(prints[1][1] == stationary[1] and prints[1][2] == stationary[2], "reduced motion is static")
prints = {}
Rich.draw("é", font, effects, palette, 20, 40, 1, 2, false)
assert(prints[1][1] ~= stationary[1] or prints[1][2] ~= stationary[2])
local plain = Rich.resolve(
  { { firstCluster = 1, lastCluster = 1, emphasis = false, styles = { "unknown_style" } } },
  1
)
assert(not plain.emphasis and not plain.wave and not plain.highlight)
print("SHOWCASE_RICH_OK timing, nesting, localization, literal bindings and reduced motion")

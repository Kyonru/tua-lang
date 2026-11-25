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
local Layout = require("staging_layout")
local mara = Layout.resolve("cast.mara.name", "staging.mara.left")
local ivo = Layout.resolve("cast.ivo.name", "staging.ivo.right")
local center = Layout.resolve("cast.mara.name", "staging.mara.center")
assert(mara.portrait == "left" and ivo.portrait == "right")
assert(ivo.y == 0.7, "scene override")
assert(center.anchor == "screen" and center.y == 0 and center.portrait == "left")
assert(Layout.resolve("cast.beacon.name").portrait == "none")
center.x = 0
assert(Layout.resolve("cast.mara.name", "staging.mara.center").x == 0.5)
for _, size in ipairs({ { 640, 480 }, { 1120, 720 } }) do
  local left = Layout.geometry(mara, size[1], size[2], false)
  local right = Layout.geometry(ivo, size[1], size[2], false)
  local middle = Layout.geometry(
    Layout.resolve("cast.mara.name", "staging.mara.center"),
    size[1],
    size[2],
    false
  )
  assert(left.x < right.x and middle.y < left.y)
  assert(left.y + left.height < left.portraitY)
  assert(right.x + right.width <= size[1] - 32)
  assert(middle.tailX == nil and left.tailX ~= nil)
end
local host = Controller.new(backend(), backend(), "staging")
host.settings.reducedMotion = true
host.update(0)
assert(host.player():snapshot().node.speakerKey == "cast.mara.name")
assert(host.modal(), "staging reserves priority over announcements")
assert(host.save())
host.locale()
assert(host.player():snapshot().node.speakerKey == "cast.mara.name")
assert(host.player():snapshot().node.presentation.resolvedLocale == "fr")
host.settings.entry = "rich"
host.reset()
assert(host.player():snapshot().node.textId == "rich.palette")
host.restore()
assert(host.settings.entry == "staging" and host.settings.locale == "fr")
assert(host.player():snapshot().node.presentation.resolvedLocale == "fr")
for _, option in ipairs({ 1, 2 }) do
  host.reset()
  local steps = 0
  while host.player():snapshot().state ~= "completed" do
    steps = steps + 1
    assert(steps < 30, "staging must terminate")
    if host.player():snapshot().state == "awaiting_choice" then host.settings.selected = option end
    host.advance()
  end
  assert(not host.save(), "terminal checkpoint is unavailable")
end
host.settings.entry = "start"
host.reset()
assert(host.player():snapshot().state == "awaiting_advance")
host.dispose()
print("SHOWCASE_STAGING_OK precedence, bounds, locale, document restore, both branches")

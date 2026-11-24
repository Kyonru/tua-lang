-- Execute in a built fixture directory; no LÖVE or source compiler is required.
local Dialogue = require("tuadialogue")
local source = {
  entry = "line",
  nodes = {
    line = {
      kind = "line",
      text = "Original",
      speaker = "Guide",
      next = "choice",
      events = { { anchor = "shown", delay = 0, name = "ping", payload = "original" } },
    },
    choice = { kind = "choice", options = { { id = "again", text = "Again", target = "line" } } },
  },
}
local a, b = Dialogue.new(source), Dialogue.new(source)
source.nodes.line.text = "mutated"
source.nodes.line.events[1].payload = "mutated"
source.nodes.choice.options[1].target = "missing"
a:start()
b:start()
local snapshot = a:snapshot()
snapshot.node.text = "snapshot mutation"
snapshot.node.events[1].payload = "snapshot mutation"
assert(a:snapshot().node.text == "Original" and b:snapshot().node.text == "Original")
assert(a:update(0, false)[1].payload == "original")
assert(#a:update(0, false) == 0)
assert(b:update(0, false)[1].payload == "original")
local before = a:snapshot().visit
assert(a:advance() and a:choose("again"))
assert(a:snapshot().visit > before and #a:update(0, false) == 1)
a:cancel()
assert(#a:update(100, true) == 0 and b:snapshot().state == "awaiting_advance")
a:start()
a:dispose()
assert(not pcall(function() a:start() end))
assert(#a:update(100, true) == 0)
b:dispose()
print("dialogue ownership passed")

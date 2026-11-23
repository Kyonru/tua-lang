local source = document.nodes.start
assert(source.portrait == "guide.png" and source.hold == 2 and source.fade == 0.5)
local player = Dialogue.new(document)
source.events[1].name = "mutated"
player:start()
local snapshot = player:snapshot()
snapshot.node.events[1].delay = 999
assert(#player:update(0.1, false) == 0)
local first = player:update(0.1, false)
assert(#first == 1 and first[1].name == "pulse" and first[1].payload == "hello\nworld")
assert(first[1].visit == player:snapshot().visit)
assert(#player:update(10, false) == 0, "revealed clock must wait for the host")
assert(#player:update(0, true) == 0)
local revealed = player:update(0.25, true)
assert(#revealed == 2 and revealed[1].name == "ready" and revealed[2].name == "tie")
assert(#player:update(100, true) == 0)
local visit = player:snapshot().visit
player:start()
assert(player:snapshot().visit > visit)
assert(#player:update(0.2, false) == 1)
player:advance()
assert(#player:update(100, true) == 0)
player:start()
player:cancel()
assert(#player:update(100, true) == 0)
player:start()
player:dispose()
assert(#player:update(100, true) == 0)
for _, dt in ipairs({ -1, math.huge, 0 / 0, "1" }) do
  assert(not pcall(function() player:update(dt, true) end))
end
local independent = Dialogue.new(document)
independent:start()
assert(#independent:update(0.2, false) == 1)
for _, field in ipairs({ "hold", "fade" }) do
  source[field] = -1
  assert(not pcall(Dialogue.new, document))
  source[field] = 0
end
source.portrait = 42
assert(not pcall(Dialogue.new, document))
source.portrait = nil
source.events[1].delay = math.huge
assert(not pcall(Dialogue.new, document))
-- A large update orders mixed anchors by scheduled time, with stable ties.
source.events = {
  { anchor = "shown", delay = 1, name = "later", payload = "" },
  { anchor = "revealed", delay = 0.5, name = "earlier", payload = "" },
  { anchor = "shown", delay = 0.5, name = "tie", payload = "" },
  { anchor = "shown", delay = 0, name = "now", payload = "" },
}
local ordered = Dialogue.new(document)
ordered:start()
assert(ordered:update(0, true)[1].name == "now")
local batch = ordered:update(2, true)
assert(#batch == 3 and batch[1].name == "earlier" and batch[2].name == "tie" and batch[3].name == "later")
assert(#ordered:update(0, true) == 0)
source.events[2] = nil
assert(not pcall(Dialogue.new, document), "sparse event arrays must fail")
print("timed events passed")

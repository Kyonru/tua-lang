local player = Dialogue.new(document)
player:start()
local first = player:update(0, false)
assert(#first == 2)
assert(first[1].name == first[2].name)
assert(first[1].eventId == "first" and first[2].eventId == "second")
assert(first[1].documentId == "arrival" and first[1].source.line == 6)
local generation, visit = first[1].generation, first[1].visit
first[1].source.line = 999
first[1].eventId = "changed"
document.nodes.start.events[1].id = "mutated"
document.nodes.start.events[1].source.line = 888
local snapshot = player:snapshot()
snapshot.node.events[1].source.line = 777
player:setLocale("fr")
-- Locale refresh finishes reveal under the existing presentation contract.
local legacy = player:update(0, false)
assert(#legacy == 1 and legacy[1].eventId == nil and legacy[1].source == nil)
assert(legacy[1].documentId == nil, "legacy fired record stays unchanged")
assert(#player:update(0, true) == 0, "refresh must not replay events")
player:start()
local again = player:update(0, false)
assert(again[1].eventId == "first" and again[1].source.line == 6)
assert(again[1].generation > generation and again[1].visit > visit)
player:cancel()
assert(#player:update(100, true) == 0)
player:start()
player:advance()
assert(#player:update(100, true) == 0)
player:start()
player:dispose()
assert(#player:update(100, true) == 0)
local edited = Dialogue.new(revised)
edited:start()
local batch = edited:update(0, false)
assert(batch[1].eventId == "second" and batch[2].eventId == "first")
assert(batch[1].payload == "two" and batch[2].payload == "one")
assert(batch[2].documentId == "arrival" and batch[2].source.line == 8)
assert(batch[1].name == "pulse" and batch[2].name == "pulse")
local event = revised.nodes.start.events[1]
for _, version in ipairs({ 1, 2, 3, 7 }) do
  revised.schemaVersion = version
  assert(not pcall(Dialogue.new, revised), "identity requires supported schema 4")
end
revised.schemaVersion = nil
assert(not pcall(Dialogue.new, revised))
revised.schemaVersion = 4
revised.documentId = nil
assert(not pcall(Dialogue.new, revised))
revised.documentId = "arrival"
for _, bad in ipairs({ false, 1, "" }) do
  event.id = bad
  assert(not pcall(Dialogue.new, revised))
end
event.id = "first"
assert(not pcall(Dialogue.new, revised), "duplicate IDs across a document must fail")
event.id = "second"
for _, bad in ipairs({ 0, -1, 0.5, math.huge, 0 / 0, "1", false }) do
  event.source.line = bad
  assert(not pcall(Dialogue.new, revised), "invalid source position")
end
event.source.line = 7
event.source.extra = true
assert(not pcall(Dialogue.new, revised))
event.source.extra = nil
event.source = setmetatable({}, { __index = { line = 1, column = 1 } })
assert(not pcall(Dialogue.new, revised))
event.source = nil
assert(Dialogue.new(revised), "programmatic identities do not require source positions")
revised.nodes.other = {
  kind = "line",
  text = "Other",
  next = "@end",
  events = { { id = "second", anchor = "shown", delay = 0, name = "other", payload = "" } },
}
assert(not pcall(Dialogue.new, revised), "duplicate IDs across nodes must fail")
print("event identity passed")

local Dialogue = require("tuadialogue")
local definitions = require("events")
local scene = require("scene")
local player = definitions.new(scene)
local independent = definitions.new(scene)
scene.nodes.start.events[1].payload.item.id = "mutated input"
definitions.payloads.reward.item.id = "mutated defaults"
player:start()
local batch = player:update(0, false)
assert(batch[1].payload.item.id == "shell")
assert(batch[1].payload.bonus == false)
local reward = definitions.read.reward(batch[1])
assert(reward and reward.coins == 10)
reward.item.id = "mutated reader copy"
assert(batch[1].payload.item.id == "shell")
assert(definitions.read.portrait(batch[1]) == nil)
assert(definitions.read.reward(batch[3]) == nil)
assert(batch[2].payload.bonus == nil)
assert(batch[3].kind == nil and batch[3].payload == "legacy")
batch[1].payload.item.id = "mutated delivery"
assert(batch[2].payload.item.id == "pearl")
local view = player:snapshot()
view.node.events[1].payload.item.id = "mutated snapshot"
player:start()
assert(player:update(0, false)[1].payload.item.id == "shell")
independent:start()
assert(independent:update(0, false)[1].payload.item.id == "shell")
assert(#player:update(0, true) == 0)
player:cancel()
assert(#player:update(1, true) == 0)
player:start()
player:dispose()
assert(#player:update(1, true) == 0)
assert(not pcall(Dialogue.new, scene), "use the typed constructor")
local event = scene.nodes.start.events[1]
event.payload.coins = "bad"
local ok, message = pcall(definitions.new, scene)
assert(not ok and message:find("rewards.example/rewards.first", 1, true))
event.payload.coins = 10
event.payload.extra = true
assert(not pcall(definitions.new, scene))
event.payload.extra = nil
event.name, event.kind = "unknown", "unknown"
assert(not pcall(definitions.new, scene))
event.name, event.kind = "reward", "reward"
event.id = nil
assert(not pcall(definitions.new, scene))
event.id = "rewards.first"
event.payload.item = setmetatable({}, { __index = { id = "fake" } })
assert(not pcall(definitions.new, scene))
event.payload.item = { id = "restored" }
local fake = { kind = "events", payloads = definitions.payloads }
scene.events = fake
assert(not pcall(definitions.new, scene))
scene.events = definitions
local context = require("context")
local bound = require("bound")
bound.events = fake
assert(not pcall(context.new, bound, { name = "Captain" }))
bound.events = definitions
local contextual = context.new(bound, { name = "Captain" })
contextual:start()
assert(contextual:update(0, false)[1].payload.coins == 5)
contextual:updateContext({ name = "Other" })
assert(#contextual:update(0, false) == 0)
contextual:dispose()
independent:dispose()
print("typed event lifecycle passed")

local Save = require("tuasave")
local shape = Save.shape(Save.record({ value = Save.number(7) }))
shape.copy = function() error("public copy must not run") end
shape.defaults.value = 99
local contract = Dialogue.events({ sample = shape })
assert(contract.payloads.sample.value == 7)
assert(not pcall(Dialogue.events, { fake = { defaults = {}, copy = function() return {} end } }))
assert(not pcall(Dialogue.events, {}))
local fakePayload = { kind = "sample", name = "sample", payload = { value = "wrong" } }
assert(not pcall(contract.read.sample, fakePayload))
local sample = { kind = "sample", name = "sample", payload = { value = 8 } }
assert(contract.read.sample(sample).value == 8)

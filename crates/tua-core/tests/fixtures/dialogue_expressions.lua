local Dialogue = require("tuadialogue")
local scene = require("scene")
local cast = require("characters")
local first = Dialogue.new(scene)
local second = Dialogue.new(scene)
cast.Guide.expressions.happy.portrait = "changed.png"
first:start()
second:start("talking")
assert(first:snapshot().node.portrait == "portraits/guide-happy.png")
assert(second:snapshot().node.portraitAnimation == "talk")
local view = first:snapshot()
view.node.portrait = "mutated.png"
view.node.expression = "mutated"
assert(first:snapshot().node.expression == "happy")
assert(first:snapshot().node.portrait == "portraits/guide-happy.png")
first:setLocale("fr")
assert(first:snapshot().node.expression == "happy")
first:advance()
assert(first:snapshot().node.expression == nil)
assert(first:snapshot().node.portraitAnimation == nil)
assert(first:snapshot().node.portrait == "portraits/guide.png")
first:cancel()
first:start("worried")
assert(first:snapshot().node.expression == "worried")
assert(second:snapshot().node.expression == "talking")
assert(first:snapshot().node.expressions == nil)
first:dispose()
second:dispose()
cast.Guide.expressions.happy.portrait = "portraits/guide-happy.png"
scene.nodes.start.expression = "unknown"
assert(not pcall(Dialogue.new, scene))
scene.nodes.start.expression = "happy"
for _, bad in ipairs({
  false,
  42,
  {},
  { portrait = 7 },
  { portrait = "" },
  { portrait = "ok", animation = false },
  { portrait = "ok", extra = true },
}) do
  cast.Guide.expressions.happy = bad
  assert(not pcall(Dialogue.new, scene))
end
cast.Guide.expressions.happy = { portrait = "portraits/guide-happy.png" }
scene.schemaVersion = 6
assert(not pcall(Dialogue.new, scene))
scene.schemaVersion = 7
print("expression lifecycle passed")

-- New portraits compose with existing actions, context, and both event clocks.
local Save = require("tuasave")
local actions = Dialogue.actions({
  confirm = {
    payload = Save.shape(Save.record({ ok = Save.boolean(false) })),
    effect = "presentation",
  },
})
local events = Dialogue.events({ notice = Save.shape(Save.record({ value = Save.number(0) })) })
local context = Dialogue.context({
  facts = Save.shape(Save.record({ ready = Save.boolean(true) })),
  locals = Save.shape(Save.record({})),
  actions = actions,
  events = events,
})
local mixed = {
  schemaVersion = 7,
  documentId = "mixed",
  context = context,
  bindings = {},
  actions = actions,
  events = events,
  entry = "action",
  nodes = {
    action = {
      kind = "action",
      name = "confirm",
      id = "mixed.confirm",
      payload = { ok = true },
      success = "line",
      failure = "done",
    },
    line = {
      kind = "line",
      text = "Hello",
      next = "done",
      expression = "happy",
      expressions = { happy = { portrait = "happy.png" } },
      events = {
        {
          kind = "notice",
          name = "notice",
          id = "mixed.shown",
          anchor = "shown",
          delay = 0,
          payload = { value = 1 },
        },
        {
          kind = "notice",
          name = "notice",
          id = "mixed.revealed",
          anchor = "revealed",
          delay = 0,
          payload = { value = 2 },
        },
      },
    },
    done = { kind = "end" },
  },
}
local player = context.new(mixed, { ready = true }, {
  handlers = {
    confirm = function(payload)
      return payload.ok
    end,
  },
})
player:start()
assert(player:runAction())
assert(player:snapshot().node.expression == "happy")
assert(player:update(0, false)[1].payload.value == 1)
assert(player:update(0, true)[1].payload.value == 2)
player:dispose()

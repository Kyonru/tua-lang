local Dialogue = require("tuadialogue")
local doc = {
  schemaVersion = 9,
  entry = "wait",
  nodes = {
    wait = { kind = "wait", duration = 1, next = "line" },
    line = {
      kind = "line",
      text = "Hello",
      textId = "line",
      richText = "[em]Hello[/em]",
      next = "end",
      events = {
        { anchor = "shown", delay = 0.5, name = "ping", payload = "" },
        { anchor = "revealed", delay = 0.5, name = "revealed", payload = "" },
      },
    },
    ["end"] = { kind = "end" },
  },
}
local p = Dialogue.new(doc)
p:start()
assert(p:snapshot().wait.remaining == 1)
assert(not p:advance() and not p:choose("anything"))
p:update(0.4, false)
assert(p:suspend() and not p:suspend())
local identity = p:snapshot()
p:update(1000, true)
assert(p:snapshot().wait.elapsed == 0.4)
assert(p:snapshot().visit == identity.visit)
assert(p:resume() and not p:resume())
assert(#p:update(1000, false) == 0)
assert(p:snapshot().nodeId == "line" and p:snapshot().wait == nil)
assert(#p:update(0.25, false) == 0)
assert(p:suspend())
assert(not p:advance())
assert(#p:update(10, true) == 0)
assert(p:resume())
assert(#p:update(0.25, false) == 1)
assert(#p:update(0, true) == 0)
assert(p:update(0.5, true)[1].name == "revealed")

-- Reveal keeps fractional progress and invalidates old speech cues across suspension.
local reveal = Dialogue.reveal({ rate = 2, speech = true })
reveal:present(p:snapshot())
assert(reveal:update(0.25).visible == 0)
local sequence = reveal:snapshot().sequence
p:suspend()
reveal:present(p:snapshot())
assert(reveal:snapshot().sequence > sequence)
assert(reveal:update(1000).visible == 0)
p:resume()
reveal:present(p:snapshot())
assert(reveal:update(0.25).visible == 1)
p:cancel()
assert(not p:snapshot().suspended)
p:start()
assert(not p:snapshot().suspended and p:snapshot().wait.elapsed == 0)
p:dispose()
assert(not p:resume() and not p:suspend())

-- Zero-duration self-loops consume exactly one visit per explicit update.
doc.nodes.wait.duration, doc.nodes.wait.next = 0, "wait"
p = Dialogue.new(doc)
p:start()
local visit = p:snapshot().visit
p:update(1e300, false)
assert(p:snapshot().visit == visit + 1)
p:update(0, false)
assert(p:snapshot().visit == visit + 2)
p:dispose()
for _, duration in ipairs({ -1, 86401, math.huge, 0 / 0 }) do
  doc.nodes.wait.duration = duration
  assert(not pcall(Dialogue.new, doc))
end
doc.nodes.wait.duration = 1
doc.schemaVersion = 8
assert(not pcall(Dialogue.new, doc))
doc.schemaVersion = 9
doc.nodes.wait.next = "missing"
assert(not pcall(Dialogue.new, doc))

-- Resolved external work stays queued while suspended and is consumed explicitly.
local actions = require("actions")
local scene = require("scene")
local handle = Dialogue.actionHandle()
p = actions.new(scene, { handlers = { purchase = function() return handle end } })
p:start()
assert(p:runAction())
p:suspend()
assert(handle:resolve(true))
assert(not p:runAction())
assert(p:snapshot().action.status == "resolved")
p:resume()
assert(p:snapshot().state == "awaiting_action")
assert(p:runAction() and p:snapshot().nodeId == "done")
p:dispose()

-- Suspending within a synchronous handler queues its result without repeating work.
local current
local calls = 0
current = actions.new(scene, {
  handlers = {
    purchase = function()
      calls = calls + 1
      current:suspend()
      return false
    end,
  },
})
current:start()
assert(current:runAction())
assert(current:snapshot().suspended and current:snapshot().state == "awaiting_action")
assert(not current:runAction() and calls == 1)
current:resume()
assert(current:runAction() and current:snapshot().nodeId == "declined")
current:dispose()

-- Wait transition errors fault once; following updates never retry the resolver.
doc.nodes.wait.next = "line"
local resolutions = 0
p = Dialogue.new(doc, {
  locale = "en",
  resolve = function()
    resolutions = resolutions + 1
    error("failed translation")
  end,
})
p:start()
p:update(1, false)
assert(p:snapshot().state == "faulted")
assert(p:snapshot().issue.code == "wait_transition_error")
p:update(100, false)
assert(resolutions == 1)
p:dispose()
print("dialogue wait and suspension lifecycle passed")

-- Suspended cancellation invalidates external work and restart clears suspension.
local pending = Dialogue.actionHandle()
p = actions.new(scene, { handlers = { purchase = function() return pending end } })
p:start()
assert(p:runAction())
p:suspend()
p:start()
assert(not p:snapshot().suspended)
assert(pending:snapshot().status == "cancelled")
assert(not pending:resolve(true))
p:dispose()

-- Choice navigation is blocked without replacing its snapshot or visit.
local choices = Dialogue.new({
  entry = "choice",
  nodes = {
    choice = { kind = "choice", options = { { id = "go", text = "Go", target = "end" } } },
    ["end"] = { kind = "end" },
  },
})
choices:start()
choices:suspend()
assert(not choices:choose("go"))
assert(choices:snapshot().state == "awaiting_choice")
choices:resume()
assert(choices:choose("go"))
choices:dispose()

-- Context refresh cannot take an empty-choice fallback until explicit resume.
local Save = require("tuasave")
local schema = Dialogue.context({
  facts = Save.shape(Save.record({ allowed = Save.boolean(true) })),
  locals = Save.shape(Save.record({ count = Save.number(0) })),
})
local failResume = true
local conditional = schema.new({
  schemaVersion = 9,
  context = schema,
  bindings = {},
  entry = "choice",
  nodes = {
    choice = {
      kind = "choice",
      fallback = "fallback",
      options = {
        {
          id = "go",
          text = "Go",
          target = "end",
          visible = { { op = "path", path = "context.allowed" } },
        },
      },
    },
    fallback = { kind = "line", text = "Fallback", textId = "fallback", next = "end" },
    ["end"] = { kind = "end" },
  },
}, { allowed = true }, {
  locale = "en",
  resolve = function(key, text)
    if key == "fallback" and failResume then error("retry resume") end
    return text
  end,
})
conditional:start()
conditional:suspend()
local before = conditional:snapshot().visit
conditional:updateContext({ allowed = false })
assert(conditional:snapshot().nodeId == "choice")
assert(conditional:snapshot().visit == before)
assert(#conditional:snapshot().node.options == 0)
assert(not pcall(conditional.resume, conditional))
assert(conditional:snapshot().suspended and conditional:snapshot().visit == before)
failResume = false
assert(conditional:resume())
assert(conditional:snapshot().nodeId == "fallback")
assert(not conditional:snapshot().suspended)
conditional:dispose()

local Dialogue = require("tuadialogue")
local Rich = require("tuadialogue.rich")
local function fails(callback)
  assert(not pcall(callback), "expected rejection")
end
local function document(template, text)
  return {
    schemaVersion = 8,
    documentId = "rich.test",
    entry = "s",
    nodes = {
      s = { kind = "line", text = text, richText = template, textId = "hello", next = "e" },
      e = { kind = "end" },
    },
  }
end
local template = "[pause=0.5][em]Hi %{name}[/em][speed=2]![/speed][pause=0.5]"
local source = document(template, "Hi %{name}!")
local p = Dialogue.new(source, { args = { name = "A" } })
source.nodes.s.richText = "broken"
p:start()
local snapshot = p:snapshot()
assert(snapshot.node.text == "Hi A!")
assert(not snapshot.node._rich)
local first = Dialogue.reveal({ rate = 4, speech = true })
local second = Dialogue.reveal({ rate = 4 })
first:present(snapshot)
second:present(snapshot)
snapshot.node.presentation.runs[1].rate = 999
fails(function() first:present(snapshot) end)
assert(first:update(0.5).visible == 0)
assert(first:update(1).visible == 4)
assert(first:update(0.5).visible == 5 and not first:snapshot().complete)
assert(first:update(0.5).complete)
assert(second:update(2.5).complete)
assert(first:snapshot().text == second:snapshot().text)
local finish = Dialogue.reveal({ rate = 4, speech = true })
finish:present(p:snapshot())
assert(finish:finish().complete and #finish:update(100).cues == 0)
local reduced = Dialogue.reveal({ reducedMotion = true, speech = true })
reduced:present(p:snapshot())
assert(reduced:snapshot().complete and #reduced:update(10).cues == 0)
local bad = p:snapshot()
bad.presentationRevision = bad.presentationRevision + 1
bad.node.presentation.pauses[1].seconds = -1
fails(function() first:present(bad) end)
assert(first:snapshot().complete and first:snapshot().text == "Hi A!")
local mode = "valid"
local translated = Dialogue.new(document(template, "Hi %{name}!"), {
  locale = "en",
  args = { name = "[pause=60]" },
  resolveTemplate = function()
    if mode == "invalid" then return { template = "[em]broken" } end
    return { template = "[style=warning]%{name}[/style]", locale = "fr" }
  end,
})
translated:start()
local before = translated:snapshot()
assert(before.node.text == "[pause=60]" and #before.node.presentation.pauses == 0)
assert(before.node.presentation.runs[1].styles[1] == "warning")
mode = "invalid"
fails(function() translated:setLocale("ja") end)
local after = translated:snapshot()
assert(after.presentationRevision == before.presentationRevision and after.locale == "en")
assert(after.node.text == before.node.text)
mode = "valid"
assert(translated:setLocale("fr"))
assert(translated:snapshot().revealComplete)
local legacy = Dialogue.new(document(template, "Hi %{name}!"), {
  locale = "en",
  resolve = function() return "[pause=60]literal" end,
})
legacy:start()
assert(legacy:snapshot().node.text == "[pause=60]literal")
assert(not legacy:snapshot().node.presentation)
fails(function() Dialogue.new(document("[em]x[/em]", "wrong")) end)
fails(function() Rich.resolve(Rich.parse("e[em]́[/em]", true), {}) end)
fails(function() Rich.resolve(Rich.parse("[em]%{name}[/em]", true), { name = "" }) end)
fails(function() Rich.resolve(Rich.parse("%{name}", true), { name = string.rep("a", 65537) }) end)
fails(function() Rich.resolve(Rich.parse("%{name}", true), { name = string.char(255) }) end)
local sharedCluster = Rich.resolve(Rich.parse("e%{accent}", true), { accent = "́" })
assert(#sharedCluster.runs == 1 and sharedCluster.runs[1].lastCluster == 1)
local escaped = Rich.resolve(Rich.parse("\\[em\\] %%{name}", true), {})
assert(escaped.text == "[em] %{name}")
local nested = Rich.resolve(Rich.parse("[speed=4]a[speed=2]b[/speed]c[/speed]", true), {})
assert(nested.runs[1].rate == 4 and nested.runs[2].rate == 2 and nested.runs[3].rate == 4)
for _, source in ipairs({
  "[unknown]x",
  "[em][style=a]x[/em][/style]",
  "[pause=61]x",
  "[speed=0]x[/speed]",
  "[em][/em]",
}) do
  fails(function() Rich.parse(source, true) end)
end
fails(function() Rich.parse("[pause=0]x", false) end)
first:clear()
assert(first:snapshot().total == 0)
p:cancel()
second:present(p:snapshot())
assert(second:snapshot().total == 0)
p:dispose()
translated:dispose()
legacy:dispose()
print("rich dialogue lifecycle passed")

-- Schema 8 retains event IDs, typed events, context bindings and expressions.
local eventSource = document("[pause=0.5]x[pause=0.5]", "x")
eventSource.nodes.s.events = {
  { id = "revealed", anchor = "revealed", delay = 0, name = "ready", payload = "yes" },
}
eventSource.nodes.s.expression = "happy"
eventSource.nodes.s.expressions = { happy = { portrait = "happy.png" } }
local eventPlayer = Dialogue.new(eventSource)
eventPlayer:start()
assert(eventPlayer:snapshot().node.portrait == "happy.png")
local eventReveal = Dialogue.reveal({ rate = 2 })
eventReveal:present(eventPlayer:snapshot())
local eventView = eventReveal:update(1)
assert(eventView.visible == 1 and not eventView.complete)
assert(#eventPlayer:update(1, eventView.complete) == 0)
assert(#eventPlayer:update(0.5, eventReveal:update(0.5).complete) == 1)
assert(#eventPlayer:update(0, true) == 0)
local Events = Dialogue.events({ ready = require("tuasave").shape(require("tuasave").record({})) })
local typedSource = document("[em]x[/em]", "x")
typedSource.events = Events
typedSource.nodes.s.events = { { kind = "ready", id = "typed", name = "ready", anchor = "shown", delay = 0, payload = {} } }
local typed = Events.new(typedSource)
typed:start()
assert(#typed:update(0, false) == 1)
local choices = {
  schemaVersion = 8,
  entry = "c",
  nodes = {
    c = {
      kind = "choice",
      options = { { id = "choice", text = "Go", richText = "[em]Go[/em]", target = "e" } },
    },
    e = { kind = "end" },
  },
}
local choicePlayer = Dialogue.new(choices)
choicePlayer:start()
assert(choicePlayer:snapshot().node.options[1].presentation.runs[1].emphasis)
local reentrant
reentrant = Dialogue.new(document("[em]x[/em]", "x"), {
  locale = "en",
  resolveTemplate = function()
    reentrant:dispose()
    return { template = "x" }
  end,
})
fails(function() reentrant:start() end)
assert(reentrant:snapshot().state == "idle" and reentrant:snapshot().visit == 0)
eventPlayer:setLocale("en")
local tiny = Dialogue.reveal({ rate = 1e-320 })
tiny:present(eventPlayer:snapshot()) -- Already-completed snapshots skip timing silently.
assert(tiny:snapshot().complete)
eventPlayer:dispose()
typed:dispose()
choicePlayer:dispose()
reentrant:dispose()

-- Extreme finite updates cannot overflow into completing an unreachable schedule.
local slowPlayer = Dialogue.new(document("[speed=0.00000000000000000001]a[/speed]", "a"))
slowPlayer:start()
local slowReveal = Dialogue.reveal()
slowReveal:present(slowPlayer:snapshot())
assert(slowReveal:update(1e100).complete)
local tinyPlayer = Dialogue.new(document("a", "a"))
tinyPlayer:start()
local tinyReveal = Dialogue.reveal({ rate = 1e-320 })
tinyReveal:present(tinyPlayer:snapshot())
assert(not tinyReveal:update(1e308).complete)
assert(not tinyReveal:update(1e308).complete)
assert(tinyReveal:finish().complete)
slowPlayer:dispose()
tinyPlayer:dispose()

local Save = require("tuasave")
local context = Dialogue.context({
  facts = Save.shape(Save.record({ name = Save.string("A") })),
  locals = Save.shape(Save.record({})),
})
local boundSource = document("[em]%{name}[/em]", "%{name}")
boundSource.context = context
boundSource.bindings = { { name = "name", path = "context.name" } }
local bound = context.new(boundSource, { name = "Ada" })
bound:start()
local boundBefore = bound:snapshot()
fails(function() bound:updateContext({ name = "" }) end)
assert(bound:getContext().name == "Ada")
assert(bound:snapshot().presentationRevision == boundBefore.presentationRevision)
bound:updateContext({ name = "Zoé" })
assert(bound:snapshot().node.text == "Zoé" and bound:snapshot().revealComplete)
bound:dispose()

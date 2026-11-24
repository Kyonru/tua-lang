local Dialogue = require("tuadialogue")
local Save = require("tuasave")
local function scene(text, options)
  local player = Dialogue.new({
    entry = "line",
    nodes = {
      line = {
        kind = "line",
        text = text,
        textId = "line",
        next = "done",
        events = { { anchor = "revealed", delay = 0, name = "ready", payload = "" } },
      },
      done = { kind = "end" },
    },
  }, options)
  player:start()
  return player
end
local player = scene("Á👩🏽‍🚀🇯🇵 नमस्ते 中文!")
local reveal = Dialogue.reveal({ rate = 4, speech = true, minCueInterval = 0 })
assert(reveal:present(player:snapshot()))
local units = reveal:clusters()
assert(units[1].text == "Á" and units[2].text == "👩🏽‍🚀" and units[3].text == "🇯🇵")
local step = reveal:update(0.25)
assert(step.visible == 1 and step.text == "Á" and #step.cues == 1)
assert(step.cues[1].sequence == step.sequence)
assert(step.cues[1].instance == step.instance)
assert(#reveal:update(0.25).cues == 0, "emoji is not speech")
assert(#reveal:update(0.25).cues == 0, "flags are not speech")
assert(#reveal:update(0.25).cues == 0, "space is silent")
units[1].text = "mutated"
assert(reveal:clusters()[1].text == "Á")
local before = reveal:snapshot()
local finished = reveal:finish()
assert(finished.complete and #finished.cues == 0 and finished.sequence ~= before.sequence)
assert(#reveal:update(100).cues == 0)
assert(player:update(0, finished.complete)[1].name == "ready")
assert(#player:update(0, true) == 0)
local sequence = reveal:snapshot().sequence
assert(not reveal:present(player:snapshot()))
assert(reveal:snapshot().sequence == sequence)
player:cancel()
assert(reveal:present(player:snapshot()))
assert(reveal:snapshot().text == "" and #reveal:clusters() == 0)
player:start()
assert(reveal:present(player:snapshot()))
assert(reveal:snapshot().visible == 0)
local independent = Dialogue.reveal({ rate = 4 })
independent:present(player:snapshot())
reveal:update(0.25)
assert(independent:snapshot().visible == 0)
assert(independent:snapshot().instance ~= reveal:snapshot().instance)
local catchup = reveal:update(1e300)
assert(catchup.complete and #catchup.cues == 1)
assert(#reveal:update(0).cues == 0)
local silent = Dialogue.reveal({ reducedMotion = true, speech = true })
silent:present(player:snapshot())
assert(silent:snapshot().complete and #silent:update(1).cues == 0)
local punctuation = scene(" ,.!—\n")
local chatter = Dialogue.reveal({ speech = true })
chatter:present(punctuation:snapshot())
assert(#chatter:update(100).cues == 0)
local limited = Dialogue.reveal({ rate = 100, speech = true, minCueInterval = 1 })
local letters = scene("abcdef")
limited:present(letters:snapshot())
assert(#limited:update(0.01).cues == 1)
assert(#limited:update(0.01).cues == 0)
assert(#limited:update(2).cues == 1)
assert(#limited:update(2).cues == 0)
-- Locale refresh is already fully revealed by the dialogue player: no cue burst.
local translated = scene("Hello", {
  locale = "en",
  resolve = function(_, _, _, locale)
    return locale == "en" and "Hello" or "こんにちは"
  end,
})
local localReveal = Dialogue.reveal({ speech = true })
localReveal:present(translated:snapshot())
localReveal:update(0.1)
translated:setLocale("ja")
assert(localReveal:present(translated:snapshot()))
assert(localReveal:snapshot().text == "こんにちは")
assert(#localReveal:update(1).cues == 0)
-- Substituted punctuation/directive-like text stays ordinary displayed text.
local context = Dialogue.context({
  facts = Save.shape(Save.record({ name = Save.string("") })),
  locals = Save.shape(Save.record({})),
})
local bound = context.new({
  schemaVersion = 2,
  context = context,
  bindings = { { name = "name", path = "context.name" } },
  entry = "line",
  nodes = {
    line = { kind = "line", text = "Hello %{name}", next = "done" },
    done = { kind = "end" },
  },
}, { name = "[pause] @action fake" })
bound:start()
local substitution = Dialogue.reveal()
substitution:present(bound:snapshot())
assert(substitution:finish().text == "Hello [pause] @action fake")
local malformed = player:snapshot()
malformed.presentationRevision = malformed.presentationRevision + 1
malformed.node.text = string.char(255)
local valid = reveal:snapshot()
assert(not pcall(reveal.present, reveal, malformed))
assert(reveal:snapshot().text == valid.text)
assert(not pcall(reveal.update, reveal, -1))
assert(not pcall(reveal.update, reveal, 0 / 0))
for _, bad in ipairs({
  false,
  { rate = 0 },
  { rate = false },
  { rate = math.huge },
  { rate = 1001 },
  { minCueInterval = -1 },
  { minCueInterval = false },
  { speech = 1 },
  { reducedMotion = 1 },
  { unknown = true },
}) do
  assert(not pcall(Dialogue.reveal, bad))
end
player:dispose()
reveal:present(player:snapshot())
assert(reveal:snapshot().complete and #reveal:clusters() == 0)
print("reveal lifecycle passed")

-- Saturation must finish even when division/multiplication round below an integer.
local rounded = Dialogue.reveal({ rate = 7 })
local longLine = scene(string.rep("a", 61))
rounded:present(longLine:snapshot())
assert(rounded:update(1e300).complete)
-- Two different frame partitions reveal the same logical prefix.
local partitioned = Dialogue.reveal({ rate = 8 })
local combined = Dialogue.reveal({ rate = 8 })
partitioned:present(longLine:snapshot())
combined:present(longLine:snapshot())
partitioned:update(0.125)
partitioned:update(0.25)
assert(partitioned:snapshot().text == combined:update(0.375).text)

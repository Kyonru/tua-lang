local Dialogue = require("tuadialogue")
local profile = {
  sampleSet = "test",
  pitchMin = 0.5,
  pitchMax = 2,
  variation = 0.1,
  gain = 0.5,
  minCueInterval = 0.1,
  cadence = "character",
}
local owned = Dialogue.speechProfile(profile)
profile.gain = 0
assert(owned.gain == 0.5)
for _, edit in ipairs({
  { "pitchMin", 0 },
  { "pitchMax", 5 },
  { "pitchMax", 0.1 },
  { "gain", 2 },
  { "variation", -1 },
  { "minCueInterval", math.huge },
  { "cadence", "letters" },
  { "sampleSet", "" },
  { "unknown", true },
}) do
  local bad = {}
  for key, value in pairs(owned) do bad[key] = value end
  bad[edit[1]] = edit[2]
  assert(not pcall(Dialogue.speechProfile, bad))
end
local doc = {
  schemaVersion = 10,
  entry = "s",
  nodes = {
    s = {
      kind = "line",
      text = "Hello",
      next = "silent",
      speech = owned,
      voice = { cue = "hello", sourceLocale = "en", fallback = "source" },
    },
    silent = { kind = "line", text = "Quiet", next = "plain", speech = "off" },
    plain = { kind = "line", text = "Plain", next = "end" },
    ["end"] = { kind = "end" },
  },
}
local a = Dialogue.new(doc)
local b = Dialogue.new(doc, { locale = "fr" })
owned.gain = 0.1
a:start()
b:start()
assert(a:snapshot().node.voice.locale == "en")
local view = b:snapshot()
assert(view.node.voice.locale == "fr")
assert(view.node.voice.sourceLocale == "en" and not view.node.voice.mixSpeech)
assert(view.node.speech.gain == 0.5)
view.node.voice.cue = "mutated"
view.node.speech.gain = 0
assert(b:snapshot().node.voice.cue == "hello")
assert(b:snapshot().node.speech.gain == 0.5)
b:suspend()
b:setLocale("de")
assert(b:snapshot().suspended and b:snapshot().node.voice.locale == "de")
assert(a:snapshot().node.voice.locale == "en")
b:resume()
b:advance()
assert(b:snapshot().node.speech == "off" and b:snapshot().node.voice == nil)
b:advance()
assert(b:snapshot().node.speech == nil)
a:dispose()
b:dispose()
for _, version in ipairs({ 8, 9 }) do
  doc.schemaVersion = version
  assert(not pcall(Dialogue.new, doc))
end
doc.schemaVersion = 10
for _, voice in ipairs({
  { cue = "hello", sourceLocale = "en", fallback = "other" },
  { cue = "hello", fallback = "silent" },
  { cue = "hello", sourceLocale = "en", fallback = "silent", mixSpeech = "yes" },
}) do
  doc.nodes.s.voice = voice
  assert(not pcall(Dialogue.new, doc))
end
print("voice and speech ownership passed")

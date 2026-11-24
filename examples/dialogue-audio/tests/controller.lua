local Dialogue = require("tuadialogue")
local Adapter = require("adapter")
local function fake()
  local state = { cues = {}, voices = {}, playing = false, pauses = 0, resumes = 0, releases = 0 }
  local backend = {
    stop = function() state.playing = false end,
    pause = function() state.pauses = state.pauses + 1 end,
    resume = function() state.resumes = state.resumes + 1 end,
    dispose = function() state.releases = state.releases + 1 end,
    voice = function(cue, locale)
      state.voices[#state.voices + 1] = locale
      state.playing = cue == "hello" and locale == "en"
      return state.playing
    end,
    speaking = function() return state.playing end,
    chatter = function(name, pitch, gain)
      state.cues[#state.cues + 1] = { name = name, pitch = pitch, gain = gain }
      return true
    end,
    volume = function() end,
    wordStart = function(text, index)
      return index == 1 or text:sub(index - 1, index - 1):match("[%s%p]") ~= nil
    end,
  }
  return backend, state
end
local function player(cadence, voice, text)
  return Dialogue.new({
    schemaVersion = 10,
    documentId = "audio.test",
    entry = "s",
    nodes = {
      s = {
        kind = "line",
        text = text or "Hello world, é 👩‍🚀! More words follow.",
        next = "done",
        speech = Dialogue.speechProfile({
          sampleSet = "guide",
          pitchMin = 0.8,
          pitchMax = 1.2,
          variation = 0.8,
          gain = 0.5,
          minCueInterval = 0.1,
          cadence = cadence or "character",
        }),
        voice = voice,
      },
      done = { kind = "end" },
    },
  }, { locale = "fr", checkpointVersion = "v1" })
end
local function run()
  local backend, state = fake()
  local audio = Adapter.new(backend, 7)
  local p = player()
  p:start()
  audio.update(p:snapshot(), 0.04)
  assert(#state.cues == 1)
  audio.update(p:snapshot(), 0.04)
  assert(#state.cues == 1, "rate limiting")
  p:suspend()
  audio.update(p:snapshot(), 20)
  assert(#state.cues == 1 and state.pauses == 1)
  p:resume()
  audio.update(p:snapshot(), 0.1)
  assert(state.resumes == 1)
  audio.update(p:snapshot(), 100)
  assert(#state.cues <= 3, "long frames coalesce")
  local count = #state.cues
  audio.update(p:snapshot(), 0)
  assert(#state.cues == count, "no replay")
  p:start()
  audio.update(p:snapshot(), 0)
  audio.finish()
  audio.update(p:snapshot(), 1)
  assert(#state.cues == count, "finish is silent")
  p:start()
  audio.volume(0, 1)
  audio.update(p:snapshot(), 0.5)
  assert(#state.cues == count)
  audio.volume(1, 1)
  audio.update(p:snapshot(), 0)
  assert(#state.cues == count, "unmute has no catch-up")
  p:setLocale("en")
  audio.update(p:snapshot(), 0)
  assert(#state.cues == count, "locale refresh is silent")
  p:cancel()
  audio.update(p:snapshot(), 1)
  audio.dispose()
  audio.dispose()
  assert(state.releases == 1)
  assert(not pcall(audio.update, p:snapshot(), 0))
  return state.cues
end
local a, b = run(), run()
assert(#a == #b)
for index, cue in ipairs(a) do
  assert(cue.pitch == b[index].pitch, "private RNG reproducibility")
end
local backend, state = fake()
local audio = Adapter.new(backend, 8)
local p = player("word", { cue = "hello", sourceLocale = "en", fallback = "source" })
p:start()
audio.update(p:snapshot(), 0.5)
assert(state.voices[1] == "fr" and state.voices[2] == "en")
assert(#state.cues == 0, "recording suppresses chatter")
state.playing = false
audio.update(p:snapshot(), 0.5)
assert(#state.cues <= 1)
p:start()
audio.update(p:snapshot(), 0)
audio.finish()
local count = #state.cues
state.playing = false
audio.update(p:snapshot(), 1)
assert(#state.cues == count, "word cadence finish is silent")
audio.dispose()
for _, text in ipairs({ "   ...!?", "👩‍🚀" }) do
  local host, observed = fake()
  local controller = Adapter.new(host, 9)
  local q = player("character", nil, text)
  q:start()
  controller.update(q:snapshot(), 10)
  assert(#observed.cues == 0, "punctuation and emoji remain silent")
  controller.dispose()
end
print("dialogue audio controller passed")

-- Recording completion, explicit mixing and unavailable recordings.
for _, scenario in ipairs({
  { cue = "missing", fallback = "silent", mixSpeech = false, expected = 1 },
  { cue = "hello", fallback = "source", mixSpeech = true, expected = 1 },
  { cue = "hello", fallback = "source", mixSpeech = false, expected = 0 },
}) do
  local host, observed = fake()
  local controller = Adapter.new(host, 12)
  local q = player("character", {
    cue = scenario.cue,
    sourceLocale = "en",
    fallback = scenario.fallback,
    mixSpeech = scenario.mixSpeech,
  })
  q:start()
  controller.update(q:snapshot(), 0.1)
  assert(#observed.cues == scenario.expected)
  local wasPlaying = observed.playing
  controller.finish()
  assert(observed.playing == wasPlaying, "finish does not stop recording")
  q:advance()
  controller.update(q:snapshot(), 0)
  assert(not observed.playing, "advance stops recording")
  controller.dispose()
end

local random = math.random
math.random = function() error("gameplay RNG must not be used") end
run()
math.random = random
print("mixing, navigation and gameplay RNG isolation passed")

local wordHost, wordState = fake()
local wordAudio = Adapter.new(wordHost, 21)
local wordPlayer = player("word", nil, "Hello world")
wordPlayer:start()
wordAudio.update(wordPlayer:snapshot(), 0.032)
assert(#wordState.cues == 1, "first word starts a cue")
wordAudio.update(wordPlayer:snapshot(), 0.032)
assert(#wordState.cues == 1, "characters inside a word stay silent")
wordAudio.update(wordPlayer:snapshot(), 0.16)
assert(#wordState.cues == 2, "next word starts a cue")
wordAudio.update(wordPlayer:snapshot(), 10)
assert(#wordState.cues == 2, "word tail creates no extra cue")
wordAudio.dispose()
print("word cadence passed")

local restoredHost, restoredState = fake()
local restoredAudio = Adapter.new(restoredHost, 30)
local restoredPlayer = player(
  "character",
  { cue = "hello", sourceLocale = "en", fallback = "source" }
)
restoredPlayer:start()
local savedAudio = restoredPlayer:checkpoint()
local resumedPlayer = player(
  "character",
  { cue = "hello", sourceLocale = "en", fallback = "source" }
)
resumedPlayer:restore(savedAudio)
restoredAudio.update(resumedPlayer:snapshot(), 1)
assert(#restoredState.cues == 0 and #restoredState.voices == 0, "restore is silent")
restoredAudio.dispose()
restoredPlayer:dispose()
resumedPlayer:dispose()
print("restored presentation is silent")

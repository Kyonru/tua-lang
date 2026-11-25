-- Owned host-audio descriptions. No audio assets, playback, clocks or RNG.
local Media = {}

local function record(value, allowed)
  assert(type(value) == "table" and getmetatable(value) == nil, "media needs a plain record")
  for key in pairs(value) do assert(allowed[key], "unknown media field: " .. tostring(key)) end
end

local function id(value, limit)
  return type(value) == "string" and #value > 0 and #value <= limit
    and not value:find("[%z\1-\31\127]")
end

local function range(value, low, high)
  return type(value) == "number" and value >= low and value <= high
end

function Media.speech(value)
  record(value, {
    sampleSet = true,
    pitchMin = true,
    pitchMax = true,
    variation = true,
    gain = true,
    minCueInterval = true,
    cadence = true,
  })
  assert(id(value.sampleSet, 256), "invalid speech sampleSet")
  assert(range(value.pitchMin, 0.25, 4) and range(value.pitchMax, value.pitchMin, 4),
    "speech pitch range must be ordered within 0.25..4")
  assert(range(value.variation, 0, 1) and range(value.gain, 0, 1), "invalid speech variation/gain")
  assert(range(value.minCueInterval, 0, 60), "invalid speech cue interval")
  assert(value.cadence == "character" or value.cadence == "word", "invalid speech cadence")
  return {
    sampleSet = value.sampleSet,
    pitchMin = value.pitchMin,
    pitchMax = value.pitchMax,
    variation = value.variation,
    gain = value.gain,
    minCueInterval = value.minCueInterval,
    cadence = value.cadence,
  }
end

function Media.voice(value)
  record(value, { cue = true, sourceLocale = true, fallback = true, mixSpeech = true })
  assert(id(value.cue, 256) and id(value.sourceLocale, 64), "invalid voice cue/source locale")
  assert(value.fallback == "silent" or value.fallback == "source", "invalid voice fallback")
  assert(value.mixSpeech == nil or type(value.mixSpeech) == "boolean", "invalid voice mixing flag")
  return {
    cue = value.cue,
    sourceLocale = value.sourceLocale,
    fallback = value.fallback,
    mixSpeech = value.mixSpeech == true,
  }
end

return Media

-- Tua procedural cue renderer for LÖVE 11.x.
-- Copyright (c) 2026 Kyonru. MIT license.
-- Rendering is explicit and bounded; keep returned Sources and reuse them.

---@alias AudioDesignWaveform "sine" | "triangle" | "square" | "saw" | "noise"
---@class AudioDesignEnvelope
---@field attack? number seconds
---@field decay? number seconds
---@field sustain? number level from 0 to 1
---@field release? number seconds
---@class AudioDesignFilter
---@field type "lowpass"
---@field frequency number cutoff in hertz
---@class AudioDesignOptions
---@field waveform AudioDesignWaveform
---@field duration number seconds, greater than 0 and at most 2
---@field frequency? number fundamental in hertz; ignored for noise
---@field gain? number level from 0 to 1, default 0.5
---@field seed? number integer seed for repeatable noise, default 1
---@field sampleRate? number samples per second, 22050 to 48000, default 44100
---@field envelope? AudioDesignEnvelope
---@field filter? AudioDesignFilter
---@class AudioDesignModule
---@field render fun(options: AudioDesignOptions): Source render once to a reusable static LOVE Source

---@type AudioDesignModule
local AudioDesign = {}
local tau = math.pi * 2
local floor, sin, exp, min = math.floor, math.sin, math.exp, math.min

local function finite(value)
  return type(value) == "number" and value == value and value ~= math.huge and value ~= -math.huge
end

local function bounded(value, name, low, high)
  assert(finite(value) and value >= low and value <= high, name .. " must be between " .. low .. " and " .. high)
  return value
end

local function envelopeAt(time, duration, envelope)
  local attack = envelope.attack or 0
  local decay = envelope.decay or 0
  local sustain = envelope.sustain == nil and 1 or envelope.sustain
  local release = envelope.release or 0
  if attack > 0 and time < attack then return time / attack end
  if decay > 0 and time < attack + decay then
    return 1 + (sustain - 1) * ((time - attack) / decay)
  end
  if release > 0 and time >= duration - release then
    return sustain * min(1, (duration - time) / release)
  end
  return sustain
end

local function oscillator(waveform, frequency, time, harmonics, state)
  if waveform == "noise" then
    state = (1664525 * state + 1013904223) % 4294967296
    return state / 2147483648 - 1, state
  end
  local phase = tau * frequency * time
  if waveform == "sine" then return sin(phase), state end
  local sample = 0
  if waveform == "saw" then
    for harmonic = 1, harmonics do sample = sample + sin(phase * harmonic) / harmonic end
    return sample * (-2 / math.pi), state
  end
  for harmonic = 1, harmonics, 2 do
    if waveform == "square" then sample = sample + sin(phase * harmonic) / harmonic
    else
      local sign = harmonic % 4 == 1 and 1 or -1
      sample = sample + sign * sin(phase * harmonic) / (harmonic * harmonic)
    end
  end
  return sample * (waveform == "square" and 4 / math.pi or 8 / (math.pi * math.pi)), state
end

---@param options AudioDesignOptions
---@return Source
function AudioDesign.render(options)
  assert(type(options) == "table", "AudioDesign.render expects an options table")
  local waveform = options.waveform
  assert(waveform == "sine" or waveform == "triangle" or waveform == "square" or waveform == "saw" or waveform == "noise", "unsupported AudioDesign waveform")
  local duration = bounded(options.duration, "duration", 0.001, 2)
  local rate = bounded(options.sampleRate or 44100, "sampleRate", 22050, 48000)
  assert(rate == floor(rate), "sampleRate must be an integer")
  local gain = bounded(options.gain == nil and 0.5 or options.gain, "gain", 0, 1)
  local frequency = waveform == "noise" and 440 or bounded(options.frequency or 440, "frequency", 20, rate / 2 - 1)
  local seed = bounded(options.seed or 1, "seed", 0, 4294967295)
  assert(seed == floor(seed), "seed must be an integer")
  local envelope = options.envelope or {}
  assert(type(envelope) == "table", "envelope must be a table")
  local attack = bounded(envelope.attack or 0, "attack", 0, duration)
  local decay = bounded(envelope.decay or 0, "decay", 0, duration)
  bounded(envelope.sustain == nil and 1 or envelope.sustain, "sustain", 0, 1)
  local release = bounded(envelope.release or 0, "release", 0, duration)
  assert(attack + decay + release <= duration, "envelope stages exceed duration")
  local filter = options.filter
  local alpha
  if filter then
    assert(type(filter) == "table" and filter.type == "lowpass", "only a lowpass render filter is supported")
    local cutoff = bounded(filter.frequency, "filter.frequency", 20, rate / 2 - 1)
    alpha = 1 - exp(-tau * cutoff / rate)
  end
  local count = floor(rate * duration)
  assert(count <= 96000, "render exceeds sample budget")
  local data = love.sound.newSoundData(count, rate, 16, 1)
  local harmonics = min(24, floor((rate / 2) / frequency))
  local filtered = 0
  for index = 0, count - 1 do
    local time = index / rate
    local sample
    sample, seed = oscillator(waveform, frequency, time, harmonics, seed)
    if alpha then
      filtered = filtered + alpha * (sample - filtered)
      sample = filtered
    end
    sample = sample * envelopeAt(time, duration, envelope) * gain
    data:setSample(index, math.max(-1, math.min(1, sample)))
  end
  return love.audio.newSource(data, "static")
end

return AudioDesign

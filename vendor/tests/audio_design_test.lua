local captures = {}
love = {
  sound = { newSoundData = function(count, rate, bits, channels)
    assert(count > 0 and count <= 96000 and bits == 16 and channels == 1)
    local data = { count = count, rate = rate, samples = {} }
    function data:setSample(index, value)
      assert(index >= 0 and index < count and value >= -1 and value <= 1)
      self.samples[index + 1] = value
    end
    captures[#captures + 1] = data
    return data
  end },
  audio = { newSource = function(data, kind)
    assert(kind == "static")
    return { data = data, kind = kind }
  end },
}

local Design = require("tua.audio.design")
local options = {
  waveform = "noise", duration = 0.02, sampleRate = 22050, seed = 7,
  envelope = { attack = 0.002, decay = 0.004, sustain = 0.4, release = 0.004 },
  filter = { type = "lowpass", frequency = 1400 },
}
local first = Design.render(options)
local second = Design.render(options)
assert(first.data.count == 441 and first.data.rate == 22050)
assert(first.data.samples[1] == 0, "attack begins silently")
for i = 1, first.data.count do
  assert(first.data.samples[i] == second.data.samples[i], "seeded render is deterministic")
end
local different = Design.render({ waveform = "noise", duration = 0.02, sampleRate = 22050, seed = 8, frequency = 30000 })
assert(first.data.samples[20] ~= different.data.samples[20], "seed changes the sound")
for _, waveform in ipairs({ "sine", "triangle", "square", "saw" }) do
  local source = Design.render({ waveform = waveform, duration = 0.01, frequency = 440 })
  assert(source.kind == "static" and #source.data.samples == source.data.count)
end
for _, invalid in ipairs({
  { waveform = "pulse", duration = 0.1 },
  { waveform = "sine", duration = 3 },
  { waveform = "sine", duration = 0.1, frequency = 30000 },
  { waveform = "noise", duration = 0.1, envelope = { attack = 0.08, release = 0.08 } },
  { waveform = "noise", duration = 0.1, filter = { type = "highpass", frequency = 1000 } },
}) do
  assert(not pcall(Design.render, invalid), "invalid render options must fail")
end
assert(#captures == 7, "validation must reject before allocating SoundData")

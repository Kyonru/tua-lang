-- Exercise source ownership without an audio device, using the emitted backend.
local sources = {}
local previousLove = love
love = { audio = {} }
function love.audio.newSource(path)
  local source = { path = path, playing = false, released = false, plays = 0 }
  function source:play()
    assert(not self.released)
    self.playing = true
    self.plays = self.plays + 1
  end
  function source:pause() self.playing = false end
  function source:stop() self.playing = false end
  function source:isPlaying() return self.playing end
  function source:setVolume(value) self.volume = value end
  function source:setPitch(value) self.pitch = value end
  function source:release()
    assert(not self.released)
    self.released = true
  end
  sources[#sources + 1] = source
  return source
end
local Backend = require("backend")
local first = Backend.new()
local second = Backend.new()
assert(#sources == 10, "five preallocated sources per conversation")
assert(first.chatter("guide", 1, 0.3))
assert(first.chatter("guide", 1, 0.3))
assert(not first.chatter("guide", 1, 0.3), "bounded polyphony drops overflow")
assert(not first.chatter("missing", 1, 1))
assert(not first.voice("greeting", "fr"))
assert(first.voice("greeting", "en"))
assert(second.voice("greeting", "en"))
first.pause()
assert(not first.speaking() and second.speaking(), "independent source ownership")
first.volume(0, 0.5)
first.resume()
assert(first.speaking())
assert(not sources[1].playing and not sources[2].playing, "muted chatter cannot resume")
assert(sources[5].volume == 0.5)
first.stop()
assert(second.speaking())
first.volume(1, 1)
assert(first.chatter("guide", 1.1, 0.2))
assert(#sources == 10, "sources are reused")
first.dispose()
second.dispose()
for _, source in ipairs(sources) do assert(source.released and not source.playing) end
love = previousLove
print("dialogue audio backend passed")

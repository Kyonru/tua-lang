local root = assert(arg[1])
package.path = root .. "/vendor/runtime/?.lua;" .. package.path

local Source = {}
Source.__index = Source
function Source:clone() return setmetatable({ playing = false, volume = 0, position = 0 }, Source) end
function Source:play() self.playing = true return true end
function Source:stop() self.playing = false end
function Source:pause() self.playing = false end
function Source:isPlaying() return self.playing end
function Source:setVolume(value) self.volume = value end
function Source:setPitch(value) self.pitch = value end
function Source:setLooping(value) self.looping = value end
function Source:tell() return self.position end
function Source:seek(value) self.position = value end

love = {
  audio = {
    isEffectsSupported = function() return false end,
    setPosition = function() end,
    setVelocity = function() end,
    setOrientation = function() end,
  },
}

local audioSystem = require("tuaaudio")
local source = setmetatable({}, Source)
local audio = audioSystem({
  maxVoices = 2,
  buses = { master = { volume = 0.5 }, sfx = { parent = "master", volume = 0.5 } },
  cues = {
    low = { source = source, bus = "sfx", priority = 1 },
    high = { source = source, bus = "sfx", priority = 10 },
  },
})

local first, firstResult = audio:play("low")
assert(first and firstResult == "started")
local second = assert(audio:play("low"))
local high, highResult = audio:play("high")
assert(high and highResult == "stolen")
assert(first:status() == "released", "stolen handles must become stale")
local high2, highResult2 = audio:play("high")
assert(high2 and highResult2 == "stolen")
local rejected, rejectedResult = audio:play("low")
assert(rejected == nil and rejectedResult == "rejected", "lower priority must not steal")

audio:update(0.1)
assert(math.abs(high._record.source.volume - 0.25) < 0.0001, "ancestor bus gains should multiply")
audio:setBusVolume("sfx", 1, { duration = 1, curve = "linear" })
audio:update(0.5)
assert(math.abs(audio:snapshot().buses.sfx.gain - 0.375) < 0.0001, "bus fades should update effective gain")

local snapshot = audio:snapshot()
assert(snapshot.activeVoices == 2)
assert(snapshot.started == 4 and snapshot.stolen == 2 and snapshot.rejected == 1)
assert(high2:stop(0))
audio:update(0)
assert(audio:snapshot().pooledVoices >= 1)

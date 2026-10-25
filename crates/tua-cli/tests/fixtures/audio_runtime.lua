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

-- Hash iteration must not decide which equal-priority voice is oldest.
-- Force newest-first candidates, then exercise repeated pooled-record reuse
-- without advancing the audio clock.
do
  local ordered = audioSystem({ maxVoices = 2, cues = { tone = { source = source } } })
  local ranks = {}
  local oldest = assert(ordered:play("tone"))
  ranks[oldest._record] = 1
  local newest = assert(ordered:play("tone"))
  ranks[newest._record] = 2
  local sort = table.sort
  table.sort = function(list, compare)
    sort(list, function(a, b) return ranks[a] > ranks[b] end)
    return sort(list, compare)
  end
  for order = 3, 34 do
    local nextVoice, result = ordered:play("tone")
    assert(nextVoice and result == "stolen")
    assert(oldest:status() == "released", "same-tick stealing must release the oldest handle")
    assert(newest:status() ~= "released", "same-tick stealing must preserve the newer handle")
    ranks[nextVoice._record] = order
    oldest, newest = newest, nextVoice
  end
  table.sort = sort
end

-- Idle updates reuse their bus cache, do not build active-voice lists, and do
-- not wake control subscribers until the public control state changes.
do
  local tracked = audioSystem({
    buses = { master = { volume = 1 } },
    cues = { tone = { source = source, bus = "master" } },
  })
  local notifications = 0
  tracked:subscribe(function() notifications = notifications + 1 end)
  tracked:update(0)

  collectgarbage("collect")
  collectgarbage("stop")
  local before = collectgarbage("count")
  for _ = 1, 500 do tracked:update(1 / 60) end
  local allocatedKb = collectgarbage("count") - before
  collectgarbage("restart")

  assert(notifications == 0, "idle updates must not notify control subscribers")
  assert(allocatedKb < 32, string.format(
    "500 idle audio updates should allocate under 32 KB (got %.1f KB)", allocatedKb))

  local voice = assert(tracked:play("tone"))
  tracked:update(0)
  assert(notifications == 1 and tracked:snapshot().activeVoices == 1,
    "starting a voice must publish the new active count once")
  tracked:update(0)
  assert(notifications == 1, "unchanged active voices must stay silent")
  assert(voice:stop(0))
  tracked:update(0)
  assert(notifications == 2 and tracked:snapshot().activeVoices == 0,
    "stopping a voice must publish the new active count once")
end

-- Cyclic bus graphs keep the previous fail-closed gain of zero without
-- allocating a cycle-detection table for every voice.
do
  local cyclic = audioSystem({
    buses = {
      master = { parent = "loop", volume = 1 },
      loop = { parent = "master", volume = 1 },
    },
  })
  assert(cyclic:snapshot().buses.master.gain == 0)
  assert(cyclic:snapshot().buses.loop.gain == 0)
end

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

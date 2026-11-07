local root = assert(arg[1])
package.path = (arg[2] or (root .. "/vendor/runtime")) .. "/?.lua;" .. package.path

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


local a = audioSystem({ maxVoices=2, cues={ low={source=source, priority=1}, high={source=source, priority=10,maxVoices=1} } })
local low=a:play("low")
local high=a:play("high")
local v,r=a:play("high")
print("overlap limits",r,"high count",a:_activeCount("high"),"old high",high:status(),"low",low:status())
local b = audioSystem({buses={master={maxVoices=1},sfx={parent="master"}},cues={tone={source=source,bus="sfx"}}})
b:play("tone")
local _,r2=b:play("tone")
print("ancestor limit",r2,"active",b:snapshot().activeVoices)
local c=audioSystem({cues={tone={source=source,volume=0.5}}})
local cv=c:play("tone",{volume=0.5})
c:update(0)
print("cue voice gain",cv._record.source.volume)
local m=audioSystem({music={one={bpm=60,layers={main={source=source}}},two={bpm=120,layers={main={source=source}}}}})
local beats,bars=0,0
m:on("beat",function() beats=beats+1 end)
m:on("bar",function() bars=bars+1 end)
m:setMusicState("one")
m:update(0)
print("bar schedule",m:setMusicState("two",{quantize="bar"}))
m._musicRuntime.layers.main._record.source.position=5.1
m:update(5.1)
print("crossed bar",m:snapshot().music.state,"beats",beats,"bars",bars)
local n=audioSystem({music={one={bpm=60,layers={main={source=source}}}}})
local events=0
n:on("beat",function() events=events+1 end)
n:setMusicState("one")
n._musicRuntime.layers.main._record.source.position=3.9
n:update(3.9)
local before=events
n._musicRuntime.layers.main._record.source.position=0.1
n:update(0.2)
n._musicRuntime.layers.main._record.source.position=1.1
n:update(1)
print("loop beats",before,events)
local q=audioSystem({music={one={loopRegion={start=1,finish=2},layers={main={source=source}}}}})
q:setMusicState("one")
q._musicRuntime.layers.main._record.source.position=2.1
q:update(2.1)
print("untimed loop region",q._musicRuntime.layers.main._record.source.position)
local snap=c:snapshot()
snap.analysis.fake="corrupted"
print("detached analysis",c:snapshot().analysis.fake)

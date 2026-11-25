local Soundscape = require("soundscape")
local calls = {
  scenes = 0,
  shot = 0,
  hit = 0,
  hurt = 0,
  defeat = 0,
  stops = 0,
  updates = 0,
  disposed = 0,
}
local backend = {
  update = function() calls.updates = calls.updates + 1 end,
  scene = function(active)
    calls.scenes = calls.scenes + 1
    calls.active = active
  end,
  pauseMusic = function(value) calls.paused = value end,
  stopEffects = function() calls.stops = calls.stops + 1 end,
  shot = function() calls.shot = calls.shot + 1 end,
  hit = function() calls.hit = calls.hit + 1 end,
  hurt = function() calls.hurt = calls.hurt + 1 end,
  defeat = function() calls.defeat = calls.defeat + 1 end,
  volumes = function(music, effects, muted)
    calls.music, calls.effects, calls.muted = music, effects, muted
  end,
  dispose = function() calls.disposed = calls.disposed + 1 end,
}
local sound = Soundscape.new(backend)
assert(calls.music == 0.35 and calls.effects == 0.6 and not calls.muted)
sound.update(0.1, "title", true, 0, 0, 0, 0)
assert(calls.scenes == 1 and not calls.active)
for _ = 1, 100 do sound.update(0.1, "title", true, 0, 0, 0, 0) end
assert(calls.scenes == 1, "updates must not restart music")
sound.update(0.1, "playing", true, 1, 0, 0, 0)
assert(calls.scenes == 2 and calls.active)
sound.update(0.1, "playing", true, 1, 8, 4, 2)
assert(calls.shot == 1 and calls.hit == 1 and calls.hurt == 1, "coalesce tick events")
sound.update(0.1, "playing", true, 1, 8, 4, 2)
assert(calls.shot == 1)
sound.update(0.1, "paused", true, 1, 8, 4, 2)
assert(calls.paused and calls.scenes == 2)
sound.update(0.1, "paused", false, 1, 8, 4, 2)
sound.update(0.1, "playing", true, 1, 8, 4, 2)
assert(not calls.paused and calls.scenes == 2 and calls.shot == 1)
sound.adjust(true, -10)
sound.adjust(false, 10)
assert(calls.music == 0 and calls.effects == 1)
sound.mute()
assert(calls.muted)
sound.update(0.1, "gameover", true, 1, 8, 4, 10)
sound.update(0.1, "gameover", true, 1, 8, 4, 10)
assert(calls.defeat == 1 and not calls.active and calls.scenes == 3)
sound.update(0.1, "playing", true, 2, 0, 0, 0)
assert(calls.scenes == 4 and calls.shot == 1 and calls.muted)
sound.update(0.1, "playing", false, 2, 1, 0, 0)
assert(calls.paused and calls.shot == 1)
sound.update(0.1, "playing", true, 2, 1, 0, 0)
assert(not calls.paused and calls.shot == 1)
sound.dispose()
sound.dispose()
local updates = calls.updates
sound.update(1, "playing", true, 3, 9, 9, 9)
sound.adjust(true, 1)
sound.mute()
assert(calls.disposed == 1 and calls.updates == updates and calls.music == 0)
print("ARENA_AUDIO_POLICY_OK scenes, cue coalescing, focus/pause, volume, mute, reset, disposal")

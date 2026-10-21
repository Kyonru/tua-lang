-- Tua audio runtime for LÖVE 11.x.
-- Copyright (c) 2026 Kyonru. MIT license.

local signal = require("tuasignal")
local store = require("tuastore")

local Audio = {}
Audio.__index = Audio
local Voice = {}
Voice.__index = Voice

local nextSystemId = 0

local function copy(value)
  local result = {}
  if value then
    for key, item in pairs(value) do result[key] = item end
  end
  return result
end

local function serialize(value)
  local kind = type(value)
  if kind == "number" or kind == "boolean" then return tostring(value) end
  if kind == "string" then return string.format("%q", value) end
  if kind ~= "table" then return "nil" end
  local parts = { "{" }
  for key, item in pairs(value) do
    parts[#parts + 1] = "[" .. serialize(key) .. "]=" .. serialize(item) .. ","
  end
  parts[#parts + 1] = "}"
  return table.concat(parts)
end

local function count(values)
  local result = 0
  for _ in pairs(values) do result = result + 1 end
  return result
end

local function clamp(value, low, high)
  return math.max(low, math.min(high, value))
end

local function fadeSpec(value)
  if value == nil then return 0, "linear" end
  if type(value) == "number" then return math.max(0, value), "linear" end
  return math.max(0, value.duration or 0), value.curve or "linear"
end

local function fadeProgress(curve, value)
  value = clamp(value, 0, 1)
  if curve == "smoothstep" then return value * value * (3 - 2 * value) end
  if curve == "exponential" then
    if value == 0 then return 0 end
    return (math.pow(2, 10 * (value - 1)) - 0.001) / 0.999
  end
  return value
end

local function beginFade(owner, target, spec, after)
  local duration, curve = fadeSpec(spec)
  if duration <= 0 then
    owner._gain = target
    owner._fade = nil
    if after then after() end
    return
  end
  owner._fade = {
    from = owner._gain or 1,
    target = target,
    duration = duration,
    elapsed = 0,
    curve = curve,
    after = after,
  }
end

local function updateFade(owner, dt)
  local fade = owner._fade
  if not fade then return false end
  fade.elapsed = math.min(fade.duration, fade.elapsed + dt)
  local progress = fadeProgress(fade.curve, fade.elapsed / fade.duration)
  owner._gain = fade.from + (fade.target - fade.from) * progress
  if fade.elapsed >= fade.duration then
    owner._fade = nil
    local after = fade.after
    if after then after() end
    return true
  end
  return false
end

local function sourceCall(source, method, ...)
  if source and source[method] then return source[method](source, ...) end
end

local function sourcePlaying(source)
  return source and source.isPlaying and source:isPlaying() or false
end

local function configureSpatial(source, options)
  if not source or not options then return end
  local position = options.position
  if position and source.setPosition then source:setPosition(position[1] or position.x or 0, position[2] or position.y or 0, position[3] or position.z or 0) end
  local velocity = options.velocity
  if velocity and source.setVelocity then source:setVelocity(velocity[1] or velocity.x or 0, velocity[2] or velocity.y or 0, velocity[3] or velocity.z or 0) end
  local direction = options.direction
  if direction and source.setDirection then source:setDirection(direction[1] or direction.x or 0, direction[2] or direction.y or 0, direction[3] or direction.z or 0) end
  if options.relative ~= nil and source.setRelative then source:setRelative(options.relative) end
  if options.attenuation and source.setAttenuationDistances then source:setAttenuationDistances(options.attenuation[1], options.attenuation[2]) end
  if options.cone and source.setCone then source:setCone(options.cone[1], options.cone[2], options.cone[3]) end
end

local function validVoice(self)
  return self._active and self._generation == self._record.generation
end

function Voice:pause(fade)
  if not validVoice(self) then return false end
  local record = self._record
  beginFade(record, 0, fade, function() sourceCall(record.source, "pause") record.paused = true end)
  return true
end

function Voice:resume(fade)
  if not validVoice(self) then return false end
  local record = self._record
  record.paused = false
  sourceCall(record.source, "play")
  record._gain = 0
  beginFade(record, record.volume, fade)
  return true
end

function Voice:stop(fade)
  if not validVoice(self) then return false end
  self._record.system:_stopRecord(self._record, fade, "stopped")
  return true
end

function Voice:fadeTo(volume, fade)
  if not validVoice(self) then return false end
  self._record.volume = clamp(volume, 0, 1)
  beginFade(self._record, self._record.volume, fade)
  return true
end

function Voice:setVolume(volume)
  if not validVoice(self) then return false end
  self._record.volume = clamp(volume, 0, 1)
  self._record._gain = self._record.volume
  self._record._fade = nil
  return true
end

function Voice:setPitch(pitch)
  if not validVoice(self) then return false end
  self._record.pitch = pitch
  sourceCall(self._record.source, "setPitch", pitch)
  return true
end

function Voice:setPosition(x, y, z)
  if not validVoice(self) then return false end
  sourceCall(self._record.source, "setPosition", x, y, z)
  return true
end

function Voice:isPlaying()
  return validVoice(self) and sourcePlaying(self._record.source)
end

function Voice:status()
  if not validVoice(self) then return "released" end
  if self._record.stopping then return "stopping" end
  if self._record.paused then return "paused" end
  return sourcePlaying(self._record.source) and "playing" or "ended"
end

function Audio:_emit(name, payload)
  self._events:emit(name, payload)
end

function Audio:on(name, listener)
  local token = self._events:on(name, listener)
  return function() self._events:off(name, token) end
end

function Audio:once(name, listener)
  local token = self._events:once(name, listener)
  return function() self._events:off(name, token) end
end

function Audio:off(token)
  if type(token) == "function" then token() return true end
  return false
end

function Audio:subscribe(listener)
  return self._control:subscribe(listener)
end

local function computeBusGain(self, name)
  if not name then return 1 end
  local bus = self._buses[name]
  if not bus then return 1 end
  local state = self._busGainState[name]
  if state == 2 then return self._busGains[name] end
  if state == 1 then return 0 end
  self._busGainState[name] = 1
  local gain = bus.muted and 0 or (bus._gain or bus.volume or 1)
  gain = gain * computeBusGain(self, bus.parent)
  self._busGains[name] = gain
  self._busGainState[name] = 2
  return gain
end

function Audio:_refreshBusGains()
  for name in pairs(self._busGainState) do self._busGainState[name] = nil end
  for name in pairs(self._buses) do computeBusGain(self, name) end
  self._busGainsDirty = false
end

function Audio:_busGain(name)
  if self._busGainsDirty then self:_refreshBusGains() end
  if not name or not self._buses[name] then return 1 end
  return self._busGains[name]
end

function Audio:_busContains(candidate, ancestor)
  while candidate do
    if candidate == ancestor then return true end
    local bus = self._buses[candidate]
    candidate = bus and bus.parent or nil
  end
  return false
end

function Audio:_activeFor(cueName, busName)
  local result = {}
  for _, record in ipairs(self._voices) do
    if record.active and (not cueName or record.cueName == cueName) and (not busName or self:_busContains(record.bus, busName)) then
      result[#result + 1] = record
    end
  end
  return result
end

function Audio:_activeCount(cueName, busName)
  local result = 0
  for _, record in ipairs(self._voices) do
    if record.active and (not cueName or record.cueName == cueName) and (not busName or self:_busContains(record.bus, busName)) then
      result = result + 1
    end
  end
  return result
end

function Audio:_limitVictims(cueName, cue, bus)
  local candidates = {}
  local constrained = false
  local function append(records)
    for _, record in ipairs(records) do candidates[record] = true end
  end
  if self.maxVoices and self._activeVoiceCount >= self.maxVoices then constrained = true append(self:_activeFor()) end
  if cue.maxVoices and self:_activeCount(cueName) >= cue.maxVoices then constrained = true append(self:_activeFor(cueName)) end
  if bus and bus.maxVoices and self:_activeCount(nil, cue.bus) >= bus.maxVoices then constrained = true append(self:_activeFor(nil, cue.bus)) end
  if not constrained then return nil end
  local list = {}
  for record in pairs(candidates) do list[#list + 1] = record end
  table.sort(list, function(a, b)
    if a.priority == b.priority then return a.startedAt < b.startedAt end
    return a.priority < b.priority
  end)
  return list[1]
end

function Audio:_overflow(cue, bus)
  if cue.overflow then return cue.overflow end
  local current = bus
  while current do
    if current.overflow then return current.overflow end
    current = self._buses[current.parent]
  end
  return self.overflow
end

function Audio:_record()
  local record = table.remove(self._pool)
  if not record then record = { generation = 0 } end
  record.generation = record.generation + 1
  record.active = true
  self._activeVoiceCount = self._activeVoiceCount + 1
  if not record.registered then
    record.registered = true
    self._voices[#self._voices + 1] = record
  end
  return record
end

function Audio:_release(record, reason)
  if not record.active then return end
  record.active = false
  self._activeVoiceCount = self._activeVoiceCount - 1
  record.stopping = false
  sourceCall(record.source, "stop")
  local handle = record.handle
  if handle then handle._active = false end
  self:_emit("voiceEnded", { cue = record.cueName, voice = handle, reason = reason })
  record.handle = nil
  record.source = nil
  record._fade = nil
  self._pool[#self._pool + 1] = record
end

function Audio:_stopRecord(record, fade, reason)
  record.stopping = true
  beginFade(record, 0, fade, function() self:_release(record, reason) end)
end

function Audio:_applyEffects(record, cue, options)
  if not record.source or not record.source.setEffect then return end
  local selected = options.effect or cue.effect
  local bus = self._buses[record.bus]
  while not selected and bus do
    selected = bus.effect
    bus = self._buses[bus.parent]
  end
  if not selected then return end
  local effect = self._effects[selected]
  if not effect then return end
  local runtimeName = self._effectNames[selected]
  if love.audio.isEffectsSupported and not love.audio.isEffectsSupported() then
    self:_emit("effectUnsupported", { effect = selected, reason = "platform" })
    return
  end
  local settings = copy(effect.settings or effect)
  settings.type = effect.type or settings.type
  local ok = love.audio.setEffect(runtimeName, settings)
  if ok then record.source:setEffect(runtimeName, true) else self:_emit("effectUnsupported", { effect = selected, reason = "rejected" }) end
end

function Audio:play(cueName, options)
  options = options or {}
  local cue = self._cues[cueName]
  if not cue or not cue.source then
    self._stats.rejected = self._stats.rejected + 1
    self:_emit("voiceRejected", { cue = cueName, reason = "unknown-cue" })
    return nil, "rejected"
  end
  local bus = self._buses[cue.bus]
  local priority = options.priority or cue.priority or 0
  local victim = self:_limitVictims(cueName, cue, bus)
  local result = "started"
  if victim then
    if self:_overflow(cue, bus) ~= "steal" or victim.priority > priority then
      self._stats.rejected = self._stats.rejected + 1
      self:_emit("voiceRejected", { cue = cueName, priority = priority, reason = "voice-limit" })
      return nil, "rejected"
    end
    self._stats.stolen = self._stats.stolen + 1
    self:_emit("voiceStolen", { cue = cueName, stolenCue = victim.cueName, priority = priority, stolenPriority = victim.priority })
    self:_release(victim, "stolen")
    result = "stolen"
  end
  local record = self:_record()
  local source = cue.source.clone and cue.source:clone() or cue.source
  record.system, record.source = self, source
  record.cueName, record.bus = cueName, cue.bus or "master"
  record.priority, record.startedAt = priority, self._clock
  record.volume = clamp(options.volume or cue.volume or 1, 0, 1)
  record._gain = 0
  record.pitch = options.pitch or cue.pitch or 1
  record.paused, record.stopping = false, false
  sourceCall(source, "setPitch", record.pitch)
  sourceCall(source, "setLooping", options.looping == nil and cue.looping or options.looping)
  configureSpatial(source, options)
  self:_applyEffects(record, cue, options)
  local handle = setmetatable({ _record = record, _generation = record.generation, _active = true }, Voice)
  record.handle = handle
  sourceCall(source, "play")
  beginFade(record, record.volume, options.fadeIn or cue.fadeIn)
  self._stats.started = self._stats.started + 1
  self:_emit("voiceStarted", { cue = cueName, voice = handle, result = result, priority = priority })
  return handle, result
end

function Audio:setBusVolume(name, volume, fade)
  local bus = self._buses[name]
  if not bus then return false end
  bus.volume = clamp(volume, 0, 1)
  beginFade(bus, bus.volume, fade, function() self:_emit("fadeComplete", { bus = name }) end)
  self._busGainsDirty = true
  self:_emit("busChanged", { bus = name, volume = bus.volume, muted = bus.muted })
  return true
end

function Audio:setBusMuted(name, muted)
  local bus = self._buses[name]
  if not bus then return false end
  bus.muted = muted
  self._busGainsDirty = true
  self:_emit("busChanged", { bus = name, volume = bus.volume, muted = muted })
  return true
end

function Audio:pauseBus(name, fade)
  if not self._buses[name] then return false end
  for _, record in ipairs(self:_activeFor(nil, name)) do record.handle:pause(fade) end
  return true
end

function Audio:resumeBus(name, fade)
  if not self._buses[name] then return false end
  for _, record in ipairs(self:_activeFor(nil, name)) do if record.paused then record.handle:resume(fade) end end
  return true
end

function Audio:stopBus(name, fade)
  if not self._buses[name] then return 0 end
  local voices = self:_activeFor(nil, name)
  for _, record in ipairs(voices) do self:_stopRecord(record, fade, "bus-stopped") end
  return #voices
end

function Audio:setListenerPosition(x, y, z) if love.audio.setPosition then love.audio.setPosition(x, y, z) end end
function Audio:setListenerVelocity(x, y, z) if love.audio.setVelocity then love.audio.setVelocity(x, y, z) end end
function Audio:setListenerOrientation(fx, fy, fz, ux, uy, uz) if love.audio.setOrientation then love.audio.setOrientation(fx, fy, fz, ux, uy, uz) end end

function Audio:_sceneTiming(scene)
  local analysis = scene.analysis and self._analysis[scene.analysis] or nil
  local analyzedBpm = analysis and analysis.confidence >= self.analysisConfidence and analysis.bpm or nil
  return scene.bpm or analyzedBpm, scene.offset or (analysis and analysis.beatOffset) or 0, analysis
end

function Audio:_startScene(name, transition)
  local scene = self._music[name]
  if not scene then return "unknown-state" end
  local old = self._musicRuntime
  local fade = transition and transition.fade or 0
  if old then for _, voice in pairs(old.layers) do voice:stop(fade) end end
  local runtime = { name = name, scene = scene, layers = {}, beat = -1, bar = -1, startedAt = self._clock }
  for layerName, layer in pairs(scene.layers or {}) do
    local cue = { source = layer.source, bus = layer.bus or scene.bus or "music", volume = layer.volume or 1, looping = layer.looping ~= false, priority = layer.priority or 100000 }
    local internalName = "__music:" .. name .. ":" .. layerName
    self._cues[internalName] = cue
    local voice = self:play(internalName, { fadeIn = fade })
    runtime.layers[layerName] = voice
  end
  self._musicRuntime = runtime
  self._pendingMusic = nil
  self:_emit("musicTransitionStarted", { state = name })
  self:_emit("musicTransitionCompleted", { state = name })
  return "started"
end

function Audio:setMusicState(name, transition)
  local scene = self._music[name]
  if not scene then return "unknown-state" end
  transition = transition or {}
  local quantize = transition.quantize or "immediate"
  if quantize == "immediate" or not self._musicRuntime then return self:_startScene(name, transition) end
  local bpm = self:_sceneTiming(self._musicRuntime.scene)
  if not bpm then return "analysis-pending" end
  self._pendingMusic = { name = name, transition = transition, quantize = quantize, afterBeat = self._musicRuntime.beat }
  self:_emit("musicTransitionScheduled", { state = name, quantize = quantize })
  return "scheduled"
end

function Audio:setMusicLayer(name, volume, fade)
  local runtime = self._musicRuntime
  local voice = runtime and runtime.layers[name]
  return voice and voice:fadeTo(volume, fade) or false
end

function Audio:_updateMusic()
  local runtime = self._musicRuntime
  if not runtime then return end
  local bpm, offset, analysis = self:_sceneTiming(runtime.scene)
  if not bpm then return end
  local leader
  for _, voice in pairs(runtime.layers) do if voice and validVoice(voice) then leader = voice._record.source break end end
  local position = leader and leader.tell and leader:tell("seconds") or self._clock - runtime.startedAt
  local loopRegion = runtime.scene.loopRegion or runtime.scene.loop
  if type(loopRegion) == "table" then
    local loopStart = loopRegion.start or loopRegion[1] or 0
    local loopEnd = loopRegion.finish or loopRegion[2]
    if loopEnd and position >= loopEnd then
      for _, voice in pairs(runtime.layers) do
        if voice and validVoice(voice) then sourceCall(voice._record.source, "seek", loopStart, "seconds") end
      end
      position = loopStart
      runtime.beat = math.floor(math.max(0, position - offset) * bpm / 60) - 1
      runtime.bar = -1
    end
  end
  local beat = math.floor(math.max(0, position - offset) * bpm / 60)
  local signature = runtime.scene.signature or { 4, 4 }
  for index = runtime.beat + 1, beat do
    self:_emit("beat", { state = runtime.name, beat = index, position = position })
    local bar = math.floor(index / (signature[1] or 4))
    if bar > runtime.bar then runtime.bar = bar self:_emit("bar", { state = runtime.name, bar = bar, position = position }) end
  end
  runtime.beat = math.max(runtime.beat, beat)
  if self._pendingMusic then
    local pending = self._pendingMusic
    local boundary = beat > pending.afterBeat and (pending.quantize == "beat" or (pending.quantize == "bar" and beat % (signature[1] or 4) == 0))
    if boundary then self:_startScene(pending.name, pending.transition) end
  end
  if leader and leader.tell then
    runtime.drift = 0
    for _, voice in pairs(runtime.layers) do
      if voice and validVoice(voice) and voice._record.source ~= leader then
        local source = voice._record.source
        local drift = source.tell and math.abs(source:tell("seconds") - position) or 0
        runtime.drift = math.max(runtime.drift, drift)
        if drift > self.driftTolerance and source.seek then source:seek(position, "seconds") end
      end
    end
  end
  runtime.position = position
  if analysis and analysis.energy and #analysis.energy > 0 then
    local hz = analysis.envelopeHz or (#analysis.energy / math.max(analysis.duration or 1, 0.001))
    runtime.energy = analysis.energy[math.min(#analysis.energy, math.floor(position * hz) + 1)] or 0
  end
end

function Audio:update(dt)
  self._clock = self._clock + dt
  for _, bus in pairs(self._buses) do
    if bus._fade then self._busGainsDirty = true end
    updateFade(bus, dt)
  end
  if self._busGainsDirty then self:_refreshBusGains() end
  for _, record in ipairs(self._voices) do
    if record.active then
      local finishedFade = updateFade(record, dt)
      if record.active then
        sourceCall(record.source, "setVolume", (record._gain or 1) * self:_busGain(record.bus))
        if not record.paused and not record.stopping and not sourcePlaying(record.source) and not (record.source and record.source.isPlaying == nil) then
          self:_release(record, "ended")
        elseif finishedFade then self:_emit("fadeComplete", { cue = record.cueName, voice = record.handle }) end
      end
    end
  end
  self:_updateMusic()
  for index = #self._analysisJobs, 1, -1 do
    local job = self._analysisJobs[index]
    local result = job._channel:pop()
    if result then
      if job._cancelled then
        -- The worker cannot be forcefully stopped safely; discard its result.
      elseif result.ok then
        job._status = "ready"
        job.result = result.analysis
        self._analysis[job.path] = result.analysis
        self._analysisStatus[job.path] = "ready"
        self:_emit("analysisReady", { path = job.path, analysis = result.analysis })
        if job._cachePath then
          love.filesystem.createDirectory("tua-audio-cache")
          love.filesystem.write(job._cachePath, "return " .. serialize(result.analysis))
        end
      else
        job._status = "failed"
        job.error = result.error
        self._analysisStatus[job.path] = "failed"
        self:_emit("analysisFailed", { path = job.path, reason = result.error })
      end
      for _, listener in ipairs(job._listeners) do listener(job.result, job.error) end
      table.remove(self._analysisJobs, index)
    elseif not job._thread:isRunning() and job._thread:getError() then
      job._status = "failed"
      job.error = job._thread:getError()
      self:_emit("analysisFailed", { path = job.path, reason = job.error })
      table.remove(self._analysisJobs, index)
    end
  end
  local control = self._control:get()
  local musicState = self._musicRuntime and self._musicRuntime.name or nil
  if control.activeVoices ~= self._activeVoiceCount or control.musicState ~= musicState then
    self._control:update(self._updateControl)
  end
end

function Audio:snapshot()
  if self._busGainsDirty then self:_refreshBusGains() end
  local buses = {}
  for name, bus in pairs(self._buses) do buses[name] = { gain = self:_busGain(name), volume = bus.volume, muted = bus.muted, activeVoices = self:_activeCount(nil, name), fading = bus._fade ~= nil } end
  local music = { state = self._musicRuntime and self._musicRuntime.name or nil, pending = self._pendingMusic and self._pendingMusic.name or nil,
    position = self._musicRuntime and self._musicRuntime.position or nil, layerDrift = self._musicRuntime and self._musicRuntime.drift or 0,
    energy = self._musicRuntime and self._musicRuntime.energy or 0 }
  return {
    activeVoices = self._activeVoiceCount, pooledVoices = #self._pool,
    started = self._stats.started, stolen = self._stats.stolen, rejected = self._stats.rejected,
    buses = buses, music = music, analysis = self._analysisStatus,
  }
end

function Audio:requestAnalysis(path, options)
  options = options or {}
  local job = { path = path, _status = "pending", _cancelled = false, _listeners = {} }
  function job:status() return self._status end
  function job:cancel() if self._status ~= "pending" then return false end self._cancelled = true self._status = "cancelled" return true end
  function job:onComplete(listener) self._listeners[#self._listeners + 1] = listener return listener end
  if not (love.thread and love.sound and love.filesystem) then
    job._status = "unsupported"
    self:_emit("analysisFailed", { path = path, reason = "LÖVE thread, sound, and filesystem modules are required" })
    return job
  end
  if love.data and love.data.hash and love.data.encode then
    local bytes = love.filesystem.read(path)
    if bytes then
      local digest = love.data.encode("string", "hex", love.data.hash("string", "sha256", bytes))
      job._cachePath = "tua-audio-cache/" .. digest .. "-v1.lua"
      if love.filesystem.getInfo(job._cachePath) then
        local loader = love.filesystem.load(job._cachePath)
        local cached = loader and loader()
        if cached then
          job._status = "ready"
          job.result = cached
          self._analysis[path] = cached
          self._analysisStatus[path] = "ready"
          self:_emit("analysisReady", { path = path, analysis = cached, cached = true })
          return job
        end
      end
    end
  end
  local code = [[
local path, options, channel = ...
local ok, result = pcall(function()
  local data = love.sound.newSoundData(path)
  local rate = data:getSampleRate()
  local channels = data:getChannelCount()
  local frames = data:getSampleCount()
  local hz = math.max(1, math.min(200, options.envelopeHz or 20))
  local window = math.max(1, math.floor(rate / hz + 0.5))
  local energy, onset = {}, {}
  local squareSum, peak = 0, 0
  for start = 0, frames - 1, window do
    local finish = math.min(frames - 1, start + window - 1)
    local sum = 0
    for frame = start, finish do
      local mono = 0
      for channelIndex = 1, channels do mono = mono + data:getSample(frame, channelIndex) end
      mono = mono / channels
      sum = sum + mono * mono
      squareSum = squareSum + mono * mono
      peak = math.max(peak, math.abs(mono))
    end
    energy[#energy + 1] = math.sqrt(sum / math.max(1, finish - start + 1))
  end
  onset[1] = 0
  for index = 2, #energy do onset[index] = math.max(0, energy[index] - energy[index - 1]) end
  local tempoMin, tempoMax = options.tempoMin or 60, options.tempoMax or 200
  local minLag = math.max(1, math.floor(60 * hz / tempoMax + 0.5))
  local maxLag = math.min(#onset - 1, math.floor(60 * hz / tempoMin + 0.5))
  local bestLag, best, total = nil, 0, 0
  for lag = minLag, maxLag do
    local score = 0
    for index = lag + 1, #onset do score = score + onset[index] * onset[index - lag] end
    total = total + score
    if score > best then best, bestLag = score, lag end
  end
  local bpm = bestLag and 60 * hz / bestLag or nil
  local confidence = total > 0 and math.min(1, best / total * math.max(1, maxLag - minLag + 1)) or 0
  return { path = path, duration = frames / rate, channels = channels, sampleRate = rate, envelopeHz = hz,
    rms = math.sqrt(squareSum / math.max(1, frames)), peak = peak, bpm = bpm,
    confidence = confidence, beatOffset = 0, beatGrid = {}, energy = energy,
    low = {}, mid = {}, high = {}, onset = onset }
end)
channel:push(ok and { ok = true, analysis = result } or { ok = false, error = tostring(result) })
]]
  local fileData = love.filesystem.newFileData(code, "tuaaudio_analyzer.lua")
  job._channel = love.thread.newChannel()
  job._thread = love.thread.newThread(fileData)
  job._thread:start(path, options, job._channel)
  self._analysisJobs[#self._analysisJobs + 1] = job
  self._analysisStatus[path] = "pending"
  return job
end

local function create(options)
  assert(type(options) == "table", "audioSystem expects an options table")
  nextSystemId = nextSystemId + 1
  local buses = {}
  for name, source in pairs(options.buses or {}) do
    buses[name] = copy(source)
    buses[name].volume = buses[name].volume or 1
    buses[name]._gain = buses[name].volume
    buses[name].muted = buses[name].muted or false
  end
  if not buses.master then buses.master = { volume = 1, _gain = 1, muted = false } end
  local self = setmetatable({
    maxVoices = options.maxVoices or 64, overflow = options.overflow or "steal",
    driftTolerance = options.driftTolerance or 0.03,
    analysisConfidence = options.analysisConfidence or 0.35,
    _id = nextSystemId, _clock = 0, _events = signal(),
    _control = store({ activeVoices = 0, musicState = nil }),
    _buses = buses, _cues = copy(options.cues), _effects = copy(options.effects),
    _music = copy(options.music), _analysis = options.analysis or {}, _analysisStatus = {},
    _voices = {}, _pool = {}, _activeVoiceCount = 0,
    _stats = { started = 0, stolen = 0, rejected = 0 },
    _busGains = {}, _busGainState = {}, _busGainsDirty = true,
    _effectNames = {},
    _analysisJobs = {},
  }, Audio)
  self._updateControl = function(state)
    state.activeVoices = self._activeVoiceCount
    state.musicState = self._musicRuntime and self._musicRuntime.name or nil
  end
  for name in pairs(self._effects) do self._effectNames[name] = "tua_audio_" .. self._id .. "_" .. name end
  return self
end

return create

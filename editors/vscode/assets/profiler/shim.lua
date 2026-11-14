-- Loaded only by Tua's temporary profiling launcher, never by a game build.
local reportPath, stopPath, durationSeconds, options = ...
options = options or {}
local M = {}
local MAX_FRAMES, MAX_STACKS = 2048, 512
local frames, stacks, frameCount, stackCount, sampleCount = {}, {}, 0, 0, 0
local droppedSamples, profile, profileError = 0, nil, nil
local startTime, ended = nil, false
local updateMs, drawMs, drawCalls, luaKb = nil, nil, nil, nil
local gcSamples, vmStates = 0, {}
local errorMessage, errorTraceback
local originalUpdate, originalDraw
local trace, traceError

local function clock()
  return love and love.timer and love.timer.getTime and love.timer.getTime() or os.clock()
end

local function finite(value)
  return type(value) == "number" and value == value and value ~= math.huge and value ~= -math.huge
end

local function number(value)
  return finite(value) and string.format("%.6f", value) or "null"
end

local function quote(value)
  return '"' .. tostring(value):gsub('[%z\1-\31\\"]', function(char)
    return string.format("\\u%04x", string.byte(char))
  end) .. '"'
end

local function writeReport(reason)
  if ended then return end
  ended = true
  if profile then pcall(profile.stop) end
  if trace then trace.stop() end
  if originalUpdate then love.update = originalUpdate end
  if originalDraw then love.draw = originalDraw end
  local now = clock()
  local parts = {
    '{"version":1,"reason":', quote(reason),
    ',"durationMs":', number(startTime and (now - startTime) * 1000 or 0),
    ',"frameCount":', tostring(frameCount),
    ',"sampleCount":', tostring(sampleCount),
    ',"droppedSamples":', tostring(droppedSamples),
    ',"profileError":', profileError and quote(profileError) or "null",
    ',"errorMessage":', errorMessage and quote(errorMessage) or "null",
    ',"errorTraceback":', errorTraceback and quote(errorTraceback) or "null",
    ',"frames":['
  }
  local retained = math.min(frameCount, MAX_FRAMES)
  for index = frameCount - retained + 1, frameCount do
    local item = frames[(index - 1) % MAX_FRAMES + 1]
    if index > frameCount - retained + 1 then parts[#parts + 1] = "," end
    parts[#parts + 1] = '{"index":' .. index .. ',"wallMs":' .. number(item.wallMs) ..
      ',"updateMs":' .. number(item.updateMs) .. ',"drawMs":' .. number(item.drawMs) ..
      ',"drawCalls":' .. number(item.drawCalls) .. ',"luaKb":' .. number(item.luaKb) ..
      ',"gcSamples":' .. tostring(item.gcSamples) .. '}'
  end
  parts[#parts + 1] = '],"stacks":['
  local first = true
  for stack, count in pairs(stacks) do
    if not first then parts[#parts + 1] = "," end
    first = false
    parts[#parts + 1] = '{"stack":' .. quote(stack) .. ',"count":' .. tostring(count) .. '}'
  end
  parts[#parts + 1] = '],"vmStates":{'
  first = true
  for state, count in pairs(vmStates) do
    if not first then parts[#parts + 1] = "," end
    first = false
    parts[#parts + 1] = quote(state) .. ":" .. tostring(count)
  end
  parts[#parts + 1] = '},"traceError":' .. (traceError and quote(traceError) or "null")
  local traced, recent, dropped = {}, {}, 0
  if trace then traced, recent, dropped = trace.snapshot() end
  parts[#parts + 1] = ',"droppedTraces":' .. tostring(dropped) .. ',"traces":['
  for index, item in ipairs(traced) do
    if index > 1 then parts[#parts + 1] = "," end
    parts[#parts + 1] = '{"kind":' .. quote(item.kind) .. ',"name":' .. quote(item.name) ..
      ',"location":' .. (item.location and quote(item.location) or "null") ..
      ',"calls":' .. tostring(item.calls) .. ',"totalMs":' .. number(item.totalMs) ..
      ',"worstMs":' .. number(item.worstMs) .. '}'
  end
  parts[#parts + 1] = '],"recentTraces":['
  for index, item in ipairs(recent) do
    if index > 1 then parts[#parts + 1] = "," end
    parts[#parts + 1] = '{"kind":' .. quote(item.kind) .. ',"name":' .. quote(item.name) ..
      ',"location":' .. (item.location and quote(item.location) or "null") ..
      ',"elapsedMs":' .. number(item.elapsedMs) .. '}'
  end
  parts[#parts + 1] = "]}"
  local temporary = reportPath .. ".tmp"
  local file = io.open(temporary, "wb")
  if not file then return end
  file:write(table.concat(parts))
  file:close()
  os.rename(temporary, reportPath)
end

function M.beforeGame()
  if type(options.trace) ~= "table" or next(options.trace) == nil then return end
  local ok, result = pcall(function()
    return assert(loadfile(options.tracePath))()(options.trace)
  end)
  if ok then trace = result else traceError = tostring(result) end
end

local function start()
  startTime = clock()
  local ok, value = pcall(require, "jit.profile")
  if not ok or type(value) ~= "table" or type(value.start) ~= "function" then
    profileError = "LuaJIT statistical profiler is unavailable in this LÖVE runtime"
    return
  end
  profile = value
  local started, reason = pcall(profile.start, "li10", function(thread, count, vmState)
    sampleCount = sampleCount + count
    vmStates[vmState] = (vmStates[vmState] or 0) + count
    if vmState == "G" then gcSamples = gcSamples + count end
    local stack = profile.dumpstack(thread, "pl\n", 8)
    if #stack > 1024 then stack = stack:sub(1, 1024) end
    if stacks[stack] then stacks[stack] = stacks[stack] + count
    elseif stackCount < MAX_STACKS then stacks[stack] = count; stackCount = stackCount + 1
    else droppedSamples = droppedSamples + count end
  end)
  if not started then profileError = tostring(reason); profile = nil end
end

function M.install()
  local originalErrorHandler = love.errorhandler
  love.errorhandler = function(message)
    errorMessage = tostring(message)
    errorTraceback = debug and debug.traceback and debug.traceback(errorMessage, 2) or errorMessage
    writeReport("error")
    if originalErrorHandler then return originalErrorHandler(message) end
  end
  local originalRun = love.run
  if type(originalRun) ~= "function" then
    profileError = "LÖVE did not provide love.run"
    writeReport("unavailable")
    return
  end
  love.run = function(...)
    local frame = originalRun(...)
    if type(frame) ~= "function" then
      profileError = "love.run did not return a frame function"
      writeReport("unavailable")
      return frame
    end
    originalUpdate, originalDraw = love.update, love.draw
    if type(originalUpdate) == "function" then
      love.update = function(...)
        local before = clock()
        local result = originalUpdate(...)
        updateMs = (clock() - before) * 1000
        return result
      end
    end
    if type(originalDraw) == "function" then
      love.draw = function(...)
        local before = clock()
        local result = originalDraw(...)
        drawMs = (clock() - before) * 1000
        if love.graphics and love.graphics.getStats then
          local ok, stats = pcall(love.graphics.getStats)
          if ok and type(stats) == "table" then drawCalls = stats.drawcalls end
        end
        return result
      end
    end
    start()
    local nextStopCheck = 0
    return function(...)
      if ended then return frame(...) end
      local before = clock()
      local result = frame(...)
      frameCount = frameCount + 1
      if frameCount % 10 == 1 then luaKb = collectgarbage("count") end
      frames[(frameCount - 1) % MAX_FRAMES + 1] = {
        wallMs = (clock() - before) * 1000, updateMs = updateMs,
        drawMs = drawMs, drawCalls = drawCalls, luaKb = luaKb, gcSamples = gcSamples,
      }
      updateMs, drawMs, drawCalls, gcSamples = nil, nil, nil, 0
      local now = clock()
      if not ended and durationSeconds > 0 and now - startTime >= durationSeconds then
        writeReport("duration")
      elseif not ended and now >= nextStopCheck then
        nextStopCheck = now + 0.1
        local file = io.open(stopPath, "rb")
        if file then file:close(); writeReport("manual") end
      end
      if result then writeReport("game-ended") end
      return result
    end
  end
end

return M

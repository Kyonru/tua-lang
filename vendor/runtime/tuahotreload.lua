--
-- tuahotreload: live code reloading for `tua dev`.
--
-- `tua dev` appends `require("tuahotreload").install()` to the generated
-- main.lua and rewrites `.tuahotreload` after every successful rebuild. This
-- module watches that file and, when it changes:
--
-- * reloads each loaded game module whose source changed, swapping its new
--   functions into the existing module table so code that already holds the
--   table, and objects whose metatable it is, use the new functions while
--   keeping their data;
-- * restarts the game when main.lua or conf.lua changed.
--
-- Errors raised by a reload, love.update, or love.draw are shown on screen
-- instead of ending the session; saving a fix resumes the game.
--
-- Copyright (c) 2026 Kyonru. MIT License, the same license as Tua.
--

local M = {}

local SIGNAL = ".tuahotreload"
local POLL_SECONDS = 0.2
local NOTICE_SECONDS = 2

-- Modules never reloaded.
local SKIP = {
  tuahotreload = true,
}

local tracked = {}
local entry_sources = {}
local generation = nil
local since_poll = 0
local notice, notice_left = nil, 0
local failure = nil
local callbacks = {}
local installed = false

local function module_path(name)
  return (name:gsub("%.", "/")) .. ".lua"
end

local function read(path)
  if not love.filesystem.getInfo(path, "file") then
    return nil
  end
  return love.filesystem.read(path)
end

local function report(message)
  failure = message
  print("[tua dev] " .. message)
end

local function announce(message)
  notice, notice_left = message, NOTICE_SECONDS
  print("[tua dev] " .. message)
end

-- Record the source of every loaded module that comes from the game.
local function track_loaded_modules()
  for name in pairs(package.loaded) do
    if type(name) == "string" and not tracked[name] and not SKIP[name] then
      local source = read(module_path(name))
      if source then
        tracked[name] = source
      end
    end
  end
end

-- Copy `new` into `old`: functions and new keys are replaced or added, data
-- already in `old` is kept, and nested tables are updated in place.
local function swap(old, new, seen)
  if seen[old] then
    return
  end
  seen[old] = true
  for key, value in pairs(new) do
    local current = rawget(old, key)
    if type(value) == "table" and type(current) == "table" then
      swap(current, value, seen)
    elseif type(value) == "function" or current == nil then
      rawset(old, key, value)
    end
  end
  local old_meta, new_meta = getmetatable(old), getmetatable(new)
  if type(old_meta) == "table" and type(new_meta) == "table" and old_meta ~= new_meta then
    swap(old_meta, new_meta, seen)
  end
end

local function reload(name, source)
  local chunk, load_error = loadstring(source, "@" .. module_path(name))
  if not chunk then
    report(load_error)
    return false
  end
  local ok, result = pcall(chunk, name)
  if not ok then
    report(tostring(result))
    return false
  end
  local old = package.loaded[name]
  if type(old) == "table" and type(result) == "table" then
    swap(old, result, {})
  elseif result ~= nil then
    package.loaded[name] = result
  end
  return true
end

local function poll()
  local current = read(SIGNAL)
  if current == nil or current == generation then
    return
  end
  generation = current

  for path, source in pairs(entry_sources) do
    if read(path) ~= source then
      print("[tua dev] " .. path .. " changed; restarting")
      love.event.quit("restart")
      return
    end
  end

  local reloaded = {}
  local all_ok = true
  for name, source in pairs(tracked) do
    local latest = read(module_path(name))
    if latest and latest ~= source then
      tracked[name] = latest
      if reload(name, latest) then
        reloaded[#reloaded + 1] = name
      else
        all_ok = false
      end
    end
  end
  track_loaded_modules()

  if all_ok and #reloaded > 0 then
    failure = nil
    table.sort(reloaded)
    announce("reloaded " .. table.concat(reloaded, ", "))
  end
end

local function call(name, ...)
  local callback = callbacks[name]
  if not callback or failure then
    return
  end
  local ok, message = xpcall(callback, debug.traceback, ...)
  if not ok then
    report(message)
  end
end

local function draw_overlay()
  if not (failure or notice) then
    return
  end
  local graphics = love.graphics
  graphics.push("all")
  graphics.origin()
  local width = graphics.getWidth()
  if failure then
    local font = graphics.getFont()
    local _, lines = font:getWrap(failure, width - 16)
    graphics.setColor(0.35, 0, 0, 0.9)
    graphics.rectangle("fill", 0, 0, width, #lines * font:getHeight() + 16)
    graphics.setColor(1, 1, 1, 1)
    graphics.printf(failure, 8, 8, width - 16)
  else
    graphics.setColor(0, 0, 0, 0.6)
    graphics.rectangle("fill", 0, 0, width, 24)
    graphics.setColor(1, 1, 1, 1)
    graphics.print(notice, 8, 5)
  end
  graphics.pop()
end

-- Route love.update and love.draw through hot-reload wrappers that survive
-- the game assigning new callbacks later.
local function wrap_callbacks()
  local wrappers = {
    update = function(dt)
      since_poll = since_poll + dt
      if since_poll >= POLL_SECONDS then
        since_poll = 0
        poll()
      end
      if notice then
        notice_left = notice_left - dt
        if notice_left <= 0 then
          notice = nil
        end
      end
      call("update", dt)
    end,
    draw = function()
      call("draw")
      draw_overlay()
    end,
  }
  for name in pairs(wrappers) do
    callbacks[name] = rawget(love, name)
    rawset(love, name, nil)
  end
  setmetatable(love, {
    __index = function(_, key)
      return wrappers[key]
    end,
    __newindex = function(table, key, value)
      if wrappers[key] then
        callbacks[key] = value
      else
        rawset(table, key, value)
      end
    end,
  })
end

function M.install()
  if installed then
    return
  end
  installed = true
  entry_sources["main.lua"] = read("main.lua")
  entry_sources["conf.lua"] = read("conf.lua")
  generation = read(SIGNAL)
  track_loaded_modules()
  if getmetatable(love) == nil then
    wrap_callbacks()
  end
  print("[tua dev] hot reload is watching for rebuilds")
end

return M

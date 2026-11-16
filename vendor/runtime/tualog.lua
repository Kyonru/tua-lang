-- Tua's optional, Lua 5.1-compatible game logger. The editor recognizes the
-- level and scope tags and maps the generated Lua call site back to Tua.

---@alias TuaLogLevel "trace"|"debug"|"info"|"warn"|"error"

---@class TuaLogFileOptions
---@field path string Relative path inside the LOVE save directory.
---@field bufferBytes? number Full-buffer size in bytes; 8192 by default.

---@class TuaLogScreenOptions
---@field lines? number Maximum visible messages from 1 to 50; 8 by default.
---@field maxBytes? number Maximum UTF-8 bytes per message from 64 to 2048; 512 by default.

---@class TuaLogOptions
---@field console? boolean Print messages to standard output; true by default.
---@field file? TuaLogFileOptions Append messages to a buffered LOVE save file.
---@field screen? TuaLogScreenOptions Keep a bounded on-screen log overlay.

---@class TuaLogScope
---@field trace fun(message: any, ...: any) Log only when the level is trace.
---@field debug fun(message: any, ...: any) Log only when the level is debug or trace.
---@field info fun(message: any, ...: any) Write an informational message.
---@field warn fun(message: any, ...: any) Write a warning.
---@field error fun(message: any, ...: any) Write an error message; this does not throw.
---@class TuaLogger
---@field trace fun(message: any, ...: any)
---@field debug fun(message: any, ...: any)
---@field info fun(message: any, ...: any)
---@field warn fun(message: any, ...: any)
---@field error fun(message: any, ...: any)
---@field scope fun(name: string): TuaLogScope Create a logger with a filterable module name.
---@field setLevel fun(level: TuaLogLevel) Set the minimum level for all scopes.
---@field configure fun(options: TuaLogOptions) Replace output destinations.
---@field flush fun() Flush a configured file sink.
---@field close fun() Flush and close a configured file sink.
---@field draw fun(x?: number, y?: number) Draw the configured on-screen log overlay.
---@field clearScreen fun() Clear retained on-screen messages.

local levels = { trace = 1, debug = 2, info = 3, warn = 4, error = 5 }
local minimum = levels.info
local console_enabled = true
local file_sink = nil
local screen_sink = nil

local function screen_text(value, max_bytes)
  if #value <= max_bytes then return value end
  local finish = max_bytes - 3
  while finish > 0 do
    local byte = value:byte(finish + 1)
    if not byte or byte < 128 or byte > 191 then break end
    finish = finish - 1
  end
  return value:sub(1, finish) .. "..."
end

local function emit(level, scope, message, ...)
  if levels[level] < minimum then return end
  if not console_enabled and not file_sink and not screen_sink then return end
  local text = tostring(message)
  local screen_limit = screen_sink and not console_enabled and not file_sink
    and screen_sink.maxBytes or nil
  if screen_limit then text = screen_text(text, screen_limit) end
  if select("#", ...) > 0 and (not screen_limit or #text < screen_limit) then
    local values = { text }
    local length = #text
    for index = 1, select("#", ...) do
      local value = tostring(select(index, ...))
      if screen_limit then value = screen_text(value, screen_limit) end
      values[#values + 1] = value
      length = length + 1 + #value
      if screen_limit and length >= screen_limit then break end
    end
    text = table.concat(values, "\t")
  end
  text = text:gsub("\r", "\\r"):gsub("\n", "\\n")
  local prefix = "[" .. level .. "] [" .. scope .. "] "
  if console_enabled or file_sink then
    local caller = debug and debug.getinfo and debug.getinfo(3, "Sl")
    local source = caller and caller.source
    local location = source and source:sub(1, 1) == "@"
      and (source:sub(2) .. ":" .. caller.currentline) or nil
    local line = prefix .. text .. (location and " (" .. location .. ")" or "")
    if console_enabled then print(line) end
    if file_sink then
      local ok, err = file_sink:write(line .. "\n")
      if not ok then
        local failed = file_sink
        file_sink = nil
        failed:close()
        print("[error] [Log] file output stopped: " .. tostring(err))
      elseif level == "error" then
        local flushed, flush_err = file_sink:flush()
        if not flushed then print("[error] [Log] file flush failed: " .. tostring(flush_err)) end
      end
    end
  end
  if screen_sink then
    local screen = screen_sink
    screen.entries[screen.next] = screen_text(prefix .. text, screen.maxBytes)
    screen.next = screen.next % screen.lines + 1
    screen.count = math.min(screen.count + 1, screen.lines)
    screen.dirty = true
  end
end

local function logger(scope)
  local value = {}
  for level in pairs(levels) do
    value[level] = function(message, ...) emit(level, scope, message, ...) end
  end
  return value
end

---@type TuaLogger
local Log = logger("game")
---@param name string
---@return TuaLogScope
function Log.scope(name)
  assert(type(name) == "string" and name ~= "" and not name:find("]", 1, true)
    and not name:find("%c"), "Log.scope needs a single-line name without ]")
  return logger(name)
end
---@param level TuaLogLevel
function Log.setLevel(level)
  assert(levels[level], "Log.setLevel needs trace, debug, info, warn, or error")
  minimum = levels[level]
end
---@param options TuaLogOptions
function Log.configure(options)
  assert(type(options) == "table", "Log.configure needs an options table")
  local next_screen
  if options.screen then
    local settings = options.screen
    assert(type(settings) == "table", "Log.configure screen needs an options table")
    local lines, max_bytes = settings.lines or 8, settings.maxBytes or 512
    assert(type(lines) == "number" and lines % 1 == 0 and lines >= 1 and lines <= 50,
      "Log.configure screen lines must be an integer from 1 to 50")
    assert(type(max_bytes) == "number" and max_bytes % 1 == 0 and max_bytes >= 64 and max_bytes <= 2048,
      "Log.configure screen maxBytes must be an integer from 64 to 2048")
    next_screen = { entries = {}, next = 1, count = 0, lines = lines,
      maxBytes = max_bytes, dirty = true }
  end
  local next_file
  if options.file then
    local settings = options.file
    assert(type(settings) == "table" and type(settings.path) == "string" and settings.path ~= "",
      "Log.configure file needs a relative path")
    local relative = settings.path:sub(1, 1) ~= "/" and not settings.path:match("^%a:")
      and not settings.path:find("\\", 1, true)
    for segment in settings.path:gmatch("[^/]+") do
      if segment == ".." then relative = false end
    end
    assert(relative,
      "Log.configure file path must stay inside the LOVE save directory")
    local bytes = settings.bufferBytes or 8192
    assert(type(bytes) == "number" and bytes >= 256 and bytes <= 1048576 and bytes % 1 == 0,
      "Log.configure bufferBytes must be an integer from 256 to 1048576")
    assert(love and love.filesystem and love.filesystem.newFile,
      "Log.configure file output requires LOVE filesystem")
    local directory = settings.path:match("^(.*)/[^/]+$")
    if directory then
      local made, make_err = love.filesystem.createDirectory(directory)
      assert(made, "Log.configure could not create log directory: " .. tostring(make_err))
    end
    local err
    next_file, err = love.filesystem.newFile(settings.path, "a")
    assert(next_file, "Log.configure could not open log file: " .. tostring(err))
    local buffered, buffer_err = next_file:setBuffer("full", bytes)
    if not buffered then
      next_file:close()
      error("Log.configure could not buffer log file: " .. tostring(buffer_err), 2)
    end
  end
  local previous = file_sink
  local previous_screen = screen_sink
  file_sink = next_file
  screen_sink = next_screen
  console_enabled = options.console ~= false
  if previous then previous:close() end
  if previous_screen and previous_screen.text and previous_screen.text.release then
    previous_screen.text:release()
  end
end
function Log.flush()
  if not file_sink then return end
  local ok, err = file_sink:flush()
  assert(ok, "Log.flush failed: " .. tostring(err))
end
function Log.close()
  local previous = file_sink
  file_sink = nil
  if previous then previous:close() end
end
---@param x? number
---@param y? number
function Log.draw(x, y)
  local screen = screen_sink
  if not screen or screen.count == 0 then return end
  assert(love and love.graphics and love.graphics.newText, "Log.draw needs LOVE graphics")
  if screen.dirty then
    local visible = {}
    for index = 1, screen.count do
      local slot = (screen.next - screen.count + index - 2) % screen.lines + 1
      visible[index] = screen.entries[slot]
    end
    local value = table.concat(visible, "\n")
    if screen.text then screen.text:set(value)
    else screen.text = love.graphics.newText(love.graphics.getFont(), value) end
    screen.width, screen.height = screen.text:getWidth() + 8, screen.text:getHeight() + 8
    screen.dirty = false
  end
  x, y = x or 8, y or 8
  love.graphics.push("all")
  love.graphics.origin()
  love.graphics.setColor(0, 0, 0, 0.75)
  love.graphics.rectangle("fill", x - 4, y - 4, screen.width, screen.height)
  love.graphics.setColor(1, 1, 1, 1)
  love.graphics.draw(screen.text, x, y)
  love.graphics.pop()
end
function Log.clearScreen()
  local screen = screen_sink
  if not screen then return end
  screen.entries, screen.next, screen.count, screen.dirty = {}, 1, 0, true
end
return Log

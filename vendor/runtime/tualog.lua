-- Tua's optional, Lua 5.1-compatible game logger. The editor recognizes the
-- level and scope tags and maps the generated Lua call site back to Tua.

---@alias TuaLogLevel "trace"|"debug"|"info"|"warn"|"error"

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

local levels = { trace = 1, debug = 2, info = 3, warn = 4, error = 5 }
local minimum = levels.info

local function emit(level, scope, message, ...)
  if levels[level] < minimum then return end
  local values = { tostring(message) }
  for index = 1, select("#", ...) do
    values[#values + 1] = tostring(select(index, ...))
  end
  local caller = debug and debug.getinfo and debug.getinfo(3, "Sl")
  local source = caller and caller.source
  local location = source and source:sub(1, 1) == "@"
    and (source:sub(2) .. ":" .. caller.currentline) or nil
  local text = table.concat(values, "\t"):gsub("\r", "\\r"):gsub("\n", "\\n")
  print("[" .. level .. "] [" .. scope .. "] " .. text .. (location and " (" .. location .. ")" or ""))
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
return Log

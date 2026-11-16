local Log = require("tualog")
local original_love, original_print = love, print
local printed, files = {}, {}
_G.print = function(line) printed[#printed + 1] = line end
_G.love = { filesystem = {
  createDirectory = function(directory)
    assert(directory == "logs")
    return true
  end,
  newFile = function(path, mode)
    assert(path == "logs/game.log" and mode == "a")
    local file = { writes = {}, flushes = 0, closes = 0 }
    function file:setBuffer(kind, bytes)
      assert(kind == "full" and bytes == 4096)
      return true
    end
    function file:write(line)
      self.writes[#self.writes + 1] = line
      return true
    end
    function file:flush() self.flushes = self.flushes + 1; return true end
    function file:close() self.closes = self.closes + 1; return true end
    files[#files + 1] = file
    return file
  end,
} }

local ok, err = pcall(function()
  Log.configure({ console = false, file = { path = "logs/game.log", bufferBytes = 4096 } })
  Log.info("loaded", 2)
  assert(#printed == 0, "file-only logging must not print")
  assert(#files[1].writes == 1 and files[1].writes[1]:match("^%[info%] %[game%] loaded\t2"))
  Log.error("failed")
  assert(files[1].flushes == 1, "error messages must be durable at once")
  Log.flush()
  assert(files[1].flushes == 2)
  assert(not pcall(Log.configure, { file = { path = "../escape" } }))
  assert(files[1].closes == 0, "invalid reconfiguration must keep the old file")
  Log.close()
  assert(files[1].closes == 1)

  Log.configure({ console = false })
  local conversions = 0
  Log.info(setmetatable({}, { __tostring = function() conversions = conversions + 1; return "unused" end }))
  assert(conversions == 0, "disabled output must skip message formatting")
end)

Log.configure({ console = true })
_G.love, _G.print = original_love, original_print
assert(ok, tostring(err))

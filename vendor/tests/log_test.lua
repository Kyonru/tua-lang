local Log = require("tualog")
local lines = {}
local original_print = print
_G.print = function(line) lines[#lines + 1] = line end

local function write_messages()
  Log.debug("hidden")
  Log.info("loaded", 7)
  Log.scope("combat").warn("low health", 2)
  Log.error("failed") -- logging an error must not throw
end

local ok, err = pcall(write_messages)
_G.print = original_print
assert(ok, tostring(err))
assert(#lines == 3, "debug should be filtered by default")
assert(lines[1]:match("^%[info%] %[game%] loaded\t7 %(.*log_test.lua:%d+%)$"), lines[1])
assert(lines[2]:match("^%[warn%] %[combat%] low health\t2 %(.*log_test.lua:%d+%)$"), lines[2])
assert(lines[3]:match("^%[error%] %[game%] failed %(.*log_test.lua:%d+%)$"), lines[3])

Log.setLevel("debug")
local seen
_G.print = function(line) seen = line end
Log.scope("combat").debug("visible")
_G.print = original_print
assert(seen and seen:match("^%[debug%] %[combat%] visible"), tostring(seen))
assert(not pcall(Log.setLevel, "invalid"), "invalid log levels should fail early")
assert(not pcall(Log.scope, "bad]\nname"), "scope names must not break console tags")

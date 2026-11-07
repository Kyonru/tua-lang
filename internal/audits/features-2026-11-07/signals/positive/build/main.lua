local __tua_signal = require("tuasignal")
local events = __tua_signal()
local count = 0
local calls = {}
local h = events:on("enemy.hit", function(a, b)
  count = count + a
  calls[#calls + 1] = "hit"
end)
events:emit("enemy.hit", 2, nil)
events:off("enemy.hit", h)
assert(not events:has("enemy.hit"))
local first = events:once("once", function() count = count + 3 end)
events:emit("once")
events:emit("once")
assert(count == 5)
events:declare("never", function(value) error("schema ran") end)
events:emit("never", 2)
print("positive", count)

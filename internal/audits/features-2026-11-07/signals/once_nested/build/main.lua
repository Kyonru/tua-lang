local __tua_signal = require("tuasignal")
local events = __tua_signal()
local nested = false
local count = 0
events:on("hit", function()
  if not nested then
    nested = true
    events:emit("hit")
  end
end)
events:once("hit", function() count = count + 1 end)
events:emit("hit")
print("once count", count)
assert(count == 1, "once callback must run at most once")

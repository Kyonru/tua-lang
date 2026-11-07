local __tua_state_machine = require("tuastatemachine").create
local machine = __tua_state_machine({
 initial = "idle",
 events = { {name="current", from="idle", to="running"} },
})
local fired = machine:current()
print(fired)

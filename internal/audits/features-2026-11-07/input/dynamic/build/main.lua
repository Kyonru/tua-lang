local __tua_input = require("baton").new
local config={controls={go={"key:a"}}}
local c=__tua_input(config)
print(c:get("go"))

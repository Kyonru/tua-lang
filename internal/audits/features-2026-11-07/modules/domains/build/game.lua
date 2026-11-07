local __tua_store = require("tuastore")
local game = __tua_store({score=0,paused=false})
return {game=game}

local __tua_object_pool = require("tuaobjectpool")
local pool = __tua_object_pool({create=function() return {x=0} end,reset=function(b) print(string.upper(b.name)) end})
local b = pool:acquire()
pool:release(b)

local __tua_object_pool = require("tuaobjectpool")
local pool = __tua_object_pool({create=function() return {x=0} end,dispose=function(b) print(string.upper(b.name)) end,initial=1})
pool:clear()

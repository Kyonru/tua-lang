local function use(cb) cb(1,"a") end
use(function(x) assert(x == 1) end)
local row = {value=3}
row.value=nil
assert(row.value==nil)
print('short callback and optional field deletion: pass')

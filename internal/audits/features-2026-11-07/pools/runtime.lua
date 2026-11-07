local objectPool = dofile(arg[1] .. '/vendor/runtime/tuaobjectpool.lua')
-- Reentrant reset: one nested release is finite and should not duplicate ownership.
local pool, nested
pool=objectPool({create=function() return {} end, reset=function(o)
 if not nested then nested=true; print('inner_release',pool:release(o)) end
end})
local o=pool:acquire();print('outer_release',pool:release(o))
print('reentrant_counts',pool:activeCount(),pool:availableCount(),pool:capacity())
local a,b=pool:acquire(),pool:acquire()
print('duplicate_checkouts',a==b)
-- Reset can clear a scene while releasing its last object.
local p;p=objectPool({create=function() return {} end,reset=function() p:clear() end})
local x=p:acquire();p:release(x)
print('clear_during_reset',p:activeCount(),p:availableCount(),p:capacity(),p:owns(x))
-- Finite guard prevents this probe from actually allocating indefinitely.
local made=0
local q=objectPool({create=function() made=made+1;if made>4 then error('probe safety stop') end;return {} end})
local ok,err=pcall(function() q:prewarm(math.huge) end)
print('infinite_prewarm',ok,made,tostring(err))
-- Error on reset retains active identity; clearing inactive/active values is supported.
local r=objectPool({create=function() return {} end,reset=function() error('reset fail') end})
local y=r:acquire();assert(not pcall(function() r:release(y) end));assert(r:activeCount()==1 and r:owns(y) and r:availableCount()==0)
print('reset_exception_control','PASS')

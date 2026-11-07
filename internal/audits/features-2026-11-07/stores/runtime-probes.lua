local root = assert(arg[1])
local store = assert(loadfile(root .. '/vendor/runtime/tuastore.lua'))()
-- Positive controls: Lua callbacks ignore extra arguments.
for count = 0, 3 do
  local s = store({score=0}); local calls=0
  local f
  if count==0 then f=function() calls=calls+1 end
  elseif count==1 then f=function(value) assert(value==1);calls=calls+1 end
  elseif count==2 then f=function(value,previous) assert(value==1 and previous==0);calls=calls+1 end
  else f=function(value,previous,state) assert(value==1 and previous==0 and state.score==1);calls=calls+1 end end
  s:watch('score',f);s:update(function(state) state.score=1 end)
  assert(calls==1); print('PASS watch callback arity '..count)
end
-- Optional field deletion is natural Lua and keyed watchers notify with nil.
local s=store({score=1});local changed=false
s:watch('score',function(value,previous) assert(value==nil and previous==1);changed=true end)
s:update(function(state) state.score=nil end);assert(changed)
print('PASS optional field deletion notifies watcher')
-- Reentrant set exposes the actual API return identity.
local g=store({score=0}); local first={score=1};local second={score=2}
g:subscribe(function(state) if state==first then g:set(second) end end)
local result=g:set(first)
print('REENTRANT_SET returned_next='..tostring(result==first)..' returned_current='..tostring(result==second))
assert(result==second)
-- A serializer adapter sees only explicit notifications; nested updates are shallow.
local h=store({score=1,child={x=1}});local saves=0;local previousChild
h:subscribe(function(state,previous) saves=saves+1;previousChild=previous.child end)
h:update(function(state) state.child.x=2 end)
assert(saves==1 and previousChild.x==2)
print('PASS documented shallow state snapshots and subscription persistence adapter')

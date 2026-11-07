local root=assert(arg[1]);package.path=root..'/vendor/runtime/?.lua;'..package.path
local signal=require('tuasignal')
local b=signal(); local count=0
b:once('normal',function() count=count+1 end);b:emit('normal');b:emit('normal');assert(count==1)
local selfCount=0
b:once('self',function() selfCount=selfCount+1;b:emit('self') end);b:emit('self');assert(selfCount==1)
local h=b:on('remove',function() end);b:on('remove',h);assert(#b:listeners('remove')==2)
local copy=b:listeners('remove');copy[1]=nil;assert(#b:listeners('remove')==2);b:off('remove',h);assert(not b:has('remove'))
local errors=0
local bad=b:on('error',function() error('intentional') end);assert(not pcall(function() b:emit('error') end));b:off('error',bad);b:on('error',function() errors=errors+1 end);b:emit('error');assert(errors==1)
print('PASS normal once, self-reentrant once, duplicate removal, listener copy, recovery')

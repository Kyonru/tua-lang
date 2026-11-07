function love.load()
 local S=require('sheets')
 local p=S:player('idle')
 local count=0
 debug.sethook(function() count=count+1; if count>100 then error('AUDIT_INSTRUCTION_BUDGET: update failed to terminate') end end,'',10000)
 local ok,err=pcall(function() p:update(0.1) end)
 debug.sethook()
 print('ZERO_DURATION_UPDATE',ok,err,'timer',p.animation.timer,'frame',p.animation.position)
 love.event.quit()
end

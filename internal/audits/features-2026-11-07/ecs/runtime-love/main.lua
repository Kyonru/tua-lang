local base = '/Users/kyonru/Documents/projects/Experiments/tlua/internal/audits/features-2026-11-07/ecs'
function love.load()
 print("runtime",_VERSION,jit.version)
 for _,name in ipairs({"happy","false_value","missing_get","missing_remove"}) do
 package.path=base.."/"..name.."/build/?.lua;"..package.path
 package.loaded.tuaecs=nil
 local ok,err=pcall(dofile,base.."/"..name.."/build/main.lua")
 print(name,ok,err)
 end
 love.event.quit(0)
end

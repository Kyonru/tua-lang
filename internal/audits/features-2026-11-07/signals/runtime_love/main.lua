function love.load()
 print("runtime",_VERSION,jit and jit.version or "no jit")
 package.path = "/Users/kyonru/Documents/projects/Experiments/tlua/vendor/runtime/?.lua;" .. package.path
 local signal = require("tuasignal")
 local events = signal()
 local nested, count = false, 0
 events:on("hit", function() if not nested then nested = true; events:emit("hit") end end)
 events:once("hit", function() count = count + 1 end)
 events:emit("hit")
 print("once_nested",count)
 local pattern = signal()
 local seen = 0
 pattern:on("enemy.hit", function(a,b,c) assert(a==1 and b==nil and c==3); seen=seen+1 end)
 pattern:emitPattern("^enemy%.", 1,nil,3)
 assert(seen==1)
 print("pattern_nil_payload",seen)
 love.event.quit(count==1 and 0 or 1)
end

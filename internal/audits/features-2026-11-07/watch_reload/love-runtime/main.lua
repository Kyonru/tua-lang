local quit = love.event.quit
function love.load()
 local ok, err = pcall(dofile, '/Users/kyonru/Documents/projects/Experiments/tlua/internal/audits/features-2026-11-07/watch_reload/runtime.lua')
 print('suite=' .. tostring(ok) .. ' ' .. tostring(err))
 quit(ok and 0 or 1)
end

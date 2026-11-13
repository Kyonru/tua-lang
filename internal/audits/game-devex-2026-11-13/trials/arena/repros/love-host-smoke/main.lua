print('LOVE_HOST_MAIN_LOADED')
local count=0
function love.load() print('LOVE_HOST_LOAD') end
function love.update() count=count+1; if count>=3 then print('LOVE_HOST_UPDATE_3'); love.event.quit(0) end end
function love.draw() end

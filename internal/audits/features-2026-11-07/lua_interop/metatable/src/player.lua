local Player = {}
Player.__index = Player
function Player.new(name) return setmetatable({name=name,x=0,active=true},Player) end
function Player:update(dt) self.x=self.x+100*dt end
return Player

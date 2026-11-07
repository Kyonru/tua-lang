local Bomb = {}
Bomb.__index = Bomb
Bomb.__type_id = 1
Bomb.__is = {
  [1] = true,
}

local function Bomb_init(self)

  self.value = 2

  self.value = 1
end

local function Bomb_explode(self)

  error("class failure")
  return self.value

end
Bomb.explode = Bomb_explode

local function Bomb_new()
  return setmetatable({
    value = 2,
    value = 1,
  }, Bomb)
end

Bomb.new = Bomb_new

local bomb = Bomb.new()
Bomb_explode(bomb)

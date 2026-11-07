---@alias Kind "friendly" | "hostile"
---@class Enemy
---@field name string Display name
---@field hp number
---@field kind Kind
---@param name string
---@return Enemy
local function makeEnemy(name)
 return {name=name,hp=20,kind="hostile"}
end
---@param head string
---@vararg number
---@return number
local function sum(head, ...) local n=0 for _,v in ipairs({...}) do n=n+v end return n end
---@overload fun(value: string): string
---@overload fun(value: number): number
local function identity(value) return value end
---@return number, string
local function pair() return 3,"three" end
return {makeEnemy=makeEnemy,sum=sum,identity=identity,pair=pair}

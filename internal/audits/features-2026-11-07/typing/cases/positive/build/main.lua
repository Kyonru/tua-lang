local rows = {}
table.insert(rows, {name="Mina",score=3})
local scores = {}
scores.Mina = 3
local n = scores.Mina
if n then print(n + 1) end
for _, row in ipairs(rows) do print(row.name, row.score) end
local function label(prefix, suffix)
  if prefix == nil then
    prefix = "ok"
  end
 return prefix .. (suffix or "!")
end
print(label())
local state = nil
state = 5
if state then print(state+1) end
local a = "dynamic"
local b = a
print(b)
do local state = "shadow" print(state) end

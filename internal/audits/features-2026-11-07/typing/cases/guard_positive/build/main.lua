local function use(row)
 if row.value == nil then
  return 0
 end
 return row.value.n
end
print(use({value={n=3}}))

-- Bounded prefix instructions. No load/loadstring, callbacks, clocks or gameplay state.
local Conditions = {}
local binary = {
  ["and"] = true,
  ["or"] = true,
  ["=="] = true,
  ["!="] = true,
  ["~="] = true,
  ["<"] = true,
  ["<="] = true,
  [">"] = true,
  [">="] = true,
}
local function scalar(value)
  return type(value) == "string" or type(value) == "boolean"
    or (type(value) == "number" and value == value and math.abs(value) < math.huge)
end
function Conditions.compile(code)
  assert(type(code) == "table" and getmetatable(code) == nil and #code > 0 and #code <= 256,
    "condition needs 1 to 256 instructions")
  local count = 0
  for key in pairs(code) do
    assert(type(key) == "number" and key % 1 == 0 and key >= 1 and key <= #code,
      "condition instructions must be dense")
    count = count + 1
  end
  assert(count == #code, "condition instructions must be dense")
  local cursor = 1
  local function read(depth)
    assert(depth <= 32, "condition nesting exceeds 32")
    local instruction = code[cursor]
    assert(
      type(instruction) == "table" and getmetatable(instruction) == nil,
      "missing condition instruction"
    )
    cursor = cursor + 1
    for key in pairs(instruction) do
      assert(key == "op" or key == "value" or key == "path", "unknown condition instruction field")
    end
    local op = instruction.op
    local node = { op = op }
    if op == "value" then
      assert(scalar(instruction.value) and instruction.path == nil, "invalid condition scalar")
      node.value = instruction.value
    elseif op == "path" then
      assert(
        type(instruction.path) == "string" and instruction.value == nil,
        "invalid condition path"
      )
      local root, tail = instruction.path:match("^(%a+)%.(.+)$")
      assert(root == "context" or root == "locals", "invalid condition root")
      node.root, node.keys = root, {}
      for key in tail:gmatch("[^.]+") do
        assert(key:match("^[A-Za-z_][A-Za-z0-9_]*$"), "invalid condition field")
        node.keys[#node.keys + 1] = key
      end
      assert(#node.keys > 0 and #node.keys < 32 and table.concat(node.keys, ".") == tail,
        "invalid condition path")
    else
      assert(instruction.value == nil and instruction.path == nil, "unexpected condition operand")
      if op == "not" then node.left = read(depth + 1)
      elseif binary[op] then
        node.left = read(depth + 1)
        node.right = read(depth + 1)
      else assert(op == "nil", "unknown condition operation") end
    end
    return node
  end
  local root = read(1)
  assert(cursor == #code + 1, "extra condition instructions")
  return root
end
local function boolean(value)
  assert(value == nil or type(value) == "boolean", "condition needs boolean or nil")
  return value == true
end
local function evaluate(node, facts, locals)
  local op = node.op
  if op == "nil" then return nil end
  if op == "value" then return node.value end
  if op == "path" then
    local value = node.root == "context" and facts or locals
    for _, key in ipairs(node.keys) do
      if value == nil then return nil end
      assert(
        type(value) == "table" and getmetatable(value) == nil,
        "condition path needs a plain record"
      )
      value = rawget(value, key)
    end
    return value
  end
  local left = evaluate(node.left, facts, locals)
  if op == "not" then return not boolean(left) end
  if op == "and" then return boolean(left) and boolean(evaluate(node.right, facts, locals)) end
  if op == "or" then return boolean(left) or boolean(evaluate(node.right, facts, locals)) end
  local right = evaluate(node.right, facts, locals)
  if op == "==" or op == "!=" or op == "~=" then
    assert(left == nil or right == nil or (scalar(left) and scalar(right) and type(left) == type(right)),
      "equality needs matching scalars or nil")
    if op == "==" then return left == right end
    return left ~= right
  end
  if left == nil or right == nil then return false end
  assert(type(left) == "number" and type(right) == "number",
    "ordered comparison needs numbers")
  if op == "<" then return left < right end
  if op == "<=" then return left <= right end
  if op == ">" then return left > right end
  return left >= right
end
function Conditions.test(node, facts, locals)
  if node == nil then return true end
  return boolean(evaluate(node, facts, locals))
end
return Conditions

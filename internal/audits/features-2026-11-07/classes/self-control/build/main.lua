local Node = {}
Node.__index = Node
Node.__type_id = 1
Node.__is = {
  [1] = true,
}

local function Node_init(self)

  local owner = self
  self.next = owner

end

local function Node_new()
  local self = setmetatable({}, Node)
  Node_init(self)
  return self
end

Node.new = Node_new

local node = Node.new()
assert(node.next == node, "self reference lost")
print("control ok")

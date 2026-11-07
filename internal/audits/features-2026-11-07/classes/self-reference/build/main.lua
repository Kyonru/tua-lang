local Node = {}
Node.__index = Node
Node.__type_id = 1
Node.__is = {
  [1] = true,
}

local function Node_init(self)

  self.next = self

end

local function Node_new()
  return setmetatable({
    next = self,
  }, Node)
end

Node.new = Node_new

local node = Node.new()
assert(node.next == node, "self reference lost")
print("ok")

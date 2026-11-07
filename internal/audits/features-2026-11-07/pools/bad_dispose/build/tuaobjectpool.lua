local function objectPool(options)
  assert(type(options) == "table", "objectPool options must be a table")
  assert(type(options.create) == "function", "objectPool options.create must be a function")
  assert(options.reset == nil or type(options.reset) == "function", "objectPool options.reset must be a function")
  assert(options.dispose == nil or type(options.dispose) == "function", "objectPool options.dispose must be a function")

  local create = options.create
  local reset = options.reset
  local dispose = options.dispose
  local available = {}
  -- A missing entry is foreign, true is active, and false is available.
  local state = {}
  local availableCount = 0
  local activeCount = 0
  local capacity = 0

  local function createObject()
    local object = create()
    assert(object ~= nil, "objectPool options.create returned nil")
    assert(state[object] == nil, "objectPool options.create returned an object already owned by this pool")

    capacity = capacity + 1
    return object
  end

  local pool = {}

  function pool:acquire()
    local object

    if availableCount > 0 then
      object = available[availableCount]
      available[availableCount] = nil
      availableCount = availableCount - 1
    else
      object = createObject()
    end

    state[object] = true
    activeCount = activeCount + 1
    return object
  end

  function pool:release(object)
    if state[object] ~= true then
      return false
    end

    if reset then
      reset(object)
    end

    state[object] = false
    activeCount = activeCount - 1
    availableCount = availableCount + 1
    available[availableCount] = object
    return true
  end

  function pool:prewarm(count)
    assert(type(count) == "number" and count >= 0 and count == math.floor(count), "objectPool prewarm count must be a non-negative integer")

    local created = 0
    while availableCount < count do
      local object = createObject()
      state[object] = false
      availableCount = availableCount + 1
      available[availableCount] = object
      created = created + 1
    end

    return created
  end

  function pool:clear()
    if dispose then
      for object in pairs(state) do
        dispose(object)
      end
    end

    available = {}
    state = {}
    availableCount = 0
    activeCount = 0
    capacity = 0
  end

  function pool:owns(object)
    return state[object] ~= nil
  end

  function pool:activeCount()
    return activeCount
  end

  function pool:availableCount()
    return availableCount
  end

  function pool:capacity()
    return capacity
  end

  local initial = options.initial or 0
  pool:prewarm(initial)

  return pool
end

return objectPool

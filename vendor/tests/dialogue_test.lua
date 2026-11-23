local runtime = arg[1] or "vendor/runtime"
package.path = runtime .. "/?.lua;" .. package.path
local Dialogue = require("tuadialogue")
local source = {
  entry = "hello",
  nodes = {
    hello = { kind = "line", text = "Hello", next = "question" },
    question = {
      kind = "choice",
      options = {
        { id = "again", text = "Again", target = "hello" },
        { id = "leave", text = "Leave", target = "done" },
      },
    },
    done = { kind = "end" },
  },
}
local first, second = Dialogue.new(source), Dialogue.new(source)
assert(first:snapshot().state == "idle" and not first:advance())
first:start()
source.nodes.hello.text = "external mutation"
local view = first:snapshot()
assert(view.node.text == "Hello" and #view.node.options == 0)
view.node.text = "renderer mutation"
assert(first:snapshot().node.text == "Hello")
assert(not first:choose("leave"))
assert(first:advance())
assert(first:snapshot().node.text == "")
assert(not first:advance() and not first:choose("missing"))
view = first:snapshot()
view.node.options[1].target = "done"
assert(first:choose("again") and first:snapshot().nodeId == "hello")
assert(second:snapshot().state == "idle")
assert(first:advance() and first:choose("leave"))
assert(first:snapshot().state == "completed" and not first:advance())
first:start()
local generation = first:snapshot().generation
assert(first:cancel() and not first:cancel())
assert(first:snapshot().generation > generation)
assert(not pcall(function() first:start(false) end))
first:start("question")
assert(first:snapshot().state == "awaiting_choice")
first:dispose()
first:dispose()
assert(first:snapshot().state == "disposed" and not first:advance())
assert(not pcall(function() first:start() end))
for _, invalid in ipairs({
  { entry = "absent", nodes = {} },
  { entry = "a", nodes = { a = { kind = "line", text = "x", next = "absent" } } },
  { entry = "a", nodes = { a = { kind = "choice", options = {} } } },
  { entry = "a", nodes = { a = { kind = "unknown" } } },
}) do
  assert(not pcall(Dialogue.new, invalid))
end
print("Dialogue: isolated sessions, owned snapshots, explicit progression and lifecycle passed")

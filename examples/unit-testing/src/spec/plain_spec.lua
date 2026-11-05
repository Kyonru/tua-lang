-- Existing Lua specs work alongside typed Tua specs.
describe("plain Lua #unit", function()
  it("shares the normal Lua runtime", function()
    assert.same({ 1, 2 }, { 1, 2 })
  end)
end)

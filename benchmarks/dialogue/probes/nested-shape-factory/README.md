# Nested shape factory inference probe

This is diagnostic evidence, not a runnable example or a golden test that accepts
incorrect checking. Create `tua.toml` with `source = "src"`, then these modules.
With the M2 compiler, `tua check` accepts the numeric name and readonly mutation,
reporting only the unused `name` local. Expected: reject both invalid operations.
The supported `Dialogue.context` builtin rejects them in maintained regressions.

## src/factory.tua

```tua
local function context<C, L>(schemas: { facts: { defaults: C, copy: (C) -> C }, locals: { defaults: L, copy: (L) -> L } }): {
  facts: Readonly<C>,
  locals: Readonly<L>,
  new: (C) -> {
    getContext: (self: any) -> Readonly<C>,
    getLocals: (self: any) -> Readonly<L>,
    updateContext: (self: any, value: C) -> nil,
    setLocals: (self: any, value: L) -> nil,
  },
}
  error("declaration")
end
return { context = context }
```

## src/context.tua

```tua
local factory = require("factory")
return factory.context({
  facts = Save.shape(Save.record({player = Save.record({name = Save.string("")}), nickname = Save.optional(Save.string(""))})),
  locals = Save.shape(Save.record({visits = Save.number(0)})),
})
```

## src/forward.tua

```tua
return require("context")
```

## src/main.tua

```tua
local Definition = require("forward")
local player = Definition.new({ player = { name = "Ada" } })
local facts = player:getContext()
local name: string = facts.player.name
player:setLocals({ visits = 2 })

player:updateContext({ player = { name = 42 } })
player:getContext().player.name = "changed"
```

Replacing the nested `Save.shape(...)` expressions with explicitly typed ordinary
`defaults`/`copy` records restores the diagnostics after the M2 module-graph fix.

The exploratory multiline generic factory also exposed an unstable formatter
layout: `tua fmt` followed immediately by `tua fmt --check` wanted to reformat the
same factory again. This input is preserved verbatim above; it is not claimed as
passing the formatting gate. Production context sources and the runnable example
pass their separate formatting checks. Generic aliases changed inference behavior
and were not used to conceal this reproduction. Both findings need focused
follow-up; neither is required by the supported direct `Dialogue.context` API.

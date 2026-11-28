---
icon: lucide/git-branch
---

# Context and conditional flow

Use typed facts and player-owned locals to bind text and select branches or available replies. All expressions are restricted to data, not host code.

[Dialogue overview](index.md)

## Typed context and conversation locals

Use a shared context definition when scripts need player facts or their own counters.
The host supplies facts explicitly; reading a binding never calls a gameplay method.
[Run the complete example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/context).

### 1. Declare serializable data in Tua

```tua title="src/story/context.tua"
return Dialogue.context({
  facts = Save.shape(Save.record({
    player = Save.record({ name = Save.string(""), coins = Save.number(0) }),
    nickname = Save.optional(Save.string("")),
  })),
  locals = Save.shape(Save.record({ visits = Save.number(0) })),
})
```

`Save.shape` creates a headless validator and copier; it does **not** create a save
slot, access a filesystem, or need LÖVE. Records, dense arrays, optional fields,
strings, finite numbers, booleans and string enums use the existing
[save schema rules](../save-contracts.md). Required fields need defaults. Functions,
userdata, metatables, cycles and unknown fields are rejected. Project engine objects
must be projected into plain data first: store an entity ID and copied facts, not
the live entity object. See [typed checkpoint locals](checkpoints-and-history.md#restore-typed-dialogue-locals) for what can be serialized.

Keep this module definitions-only. It may be shared or re-exported through another
Tua module, but must not import a scene which imports it back. Use the direct
`Dialogue.context(...)` and `Save.shape(...)` built-ins to preserve specialized
inference; wrapping them in an untyped or generic factory can lose payload types.

### 2. Bind named placeholders in the script

```text title="src/welcome.tdialogue"
@dialogue 1
@context "story.context"
@bind name context.player.name
@bind nickname context.nickname default "traveler"
@bind visits locals.visits
[start]
// @id welcome.hello
: Hello %{name} (%{nickname}). Visits: %{visits}. (end)
```

Declare one `@context` and all `@bind` directives before labels. `context` reads
host facts; `locals` reads this player's private variables. Bindings are field paths,
not expressions: no calls, indexing, arithmetic or assignment. A placeholder must
have a binding, and its final field must be a string, number or boolean. Optional
fields, including optional parents, require a compatible scalar `default`. `false`
and zero are values; only a missing/nil value uses the fallback.

The compiler checks imported/re-exported paths and fallback types. Editor completion
and hover show fields; definition navigation from a binding path leads to the context
factory declaration. Inside `%{name}`, completion suggests declared scalar bindings,
hover shows the imported field type and bound path, and Go to Definition jumps to
`@bind name`. This works in plain/rich lines, choices and disabled-choice reasons,
even while the placeholder is unfinished. It replaces only the name, preserving
braces and surrounding prose. Unsaved binding and imported-context edits refresh
the results. Duplicate or malformed headers suppress binding suggestions; escaped
`%%` and rich-mark attributes are not placeholder references.

Limits are 256 bindings and 32 path segments (including the root). `%{name}` and `%%`
use the same literal-safe formatting as localization. Inserted text is never parsed
again as a placeholder, markup or dialogue code.

### 3. Create players and refresh their data

```tua title="src/main.tua"
local context = require("story.context")
local scene = require("welcome")
local facts = { player = { name = "Ada", coins = 3 } }
local player = context.new(scene, facts)
player:start()

facts.player.coins = 8
player:updateContext(facts)
player:setLocals({ visits = 2 })
local view = player:getContext()
local localView = player:getLocals()
```

`context.new(scene, facts, options?)` validates and copies facts and local defaults.
Every player owns its copies. Mutating the original facts, schema defaults, document
or a getter result cannot change another player's state. `getContext()` and
`getLocals()` return owned, recursively read-only typed views. Lua callers can mutate
their returned copies without altering the player.

`updateContext(value)` and `setLocals(value)` accept complete replacements, not
patches. They validate before committing and refresh current text immediately.
Invalid data or a failing resolver leaves the previous state and presentation intact.
These methods do not change visit/generation, execute actions, or replay timed events.
Renderers honor `presentationRevision` and `revealComplete`, as with locale changes.

| Operation | Facts | Locals | Execution |
| --- | --- | --- | --- |
| `new` | Copy supplied facts | Copy captured defaults | Idle |
| `start`, including restart | Keep current facts | Reset to captured defaults | New generation and visit |
| `advance`, `choose`, loops | Keep | Keep | Normal transition/new visit |
| `updateContext`, `setLocals` | Replace selected data | Replace selected data | Refresh presentation only |
| `cancel` | Keep for inspection | Keep for inspection | Discard pending events |
| `dispose` | Release | Release defaults and validators too | Further context access fails |

Localization options remain `{locale, resolve, args?}`. Use `I18n.dialogue` for the
existing catalog integration; it receives bound values alongside optional extra
arguments. Extra arguments cannot override a binding. Custom resolvers return final
formatted text; the player does not interpolate their output a second time. Bindings
work without localization or a catalog runtime.

### Compatibility

Context-bound scripts emit document schema **2**; context-free scripts continue to
emit schema **1**. The authoring header remains `@dialogue 1`. Use the paired compiler
and runtime, or check `Dialogue.schemaVersion >= 2` with a custom runtime replacement.
Create a bound script through its matching `context.new`, not `Dialogue.new`; a
mismatched definition fails at runtime. Existing structured and schema-one documents,
renderers, localization, character portraits and event clocks remain supported.

## Conditions, branches and available choices

Conditions read the latest **accepted** context and locals. Mutating the original
host table has no effect until `updateContext` or `setLocals` accepts a replacement.
The [conditional shop example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/conditions)
builds and runs without graphics or audio.

### Route through a branch label

For this section, define the shop's facts in their own module:

```tua title="src/shop/context.tua"
return Dialogue.context({
  facts = Save.shape(Save.record({
    coins = Save.number(0),
    unlocked = Save.boolean(false),
    secretUnlocked = Save.boolean(false),
    bonus = Save.optional(Save.number(0)),
  })),
  locals = Save.shape(Save.record({ visits = Save.number(0) })),
})
```

```text
@dialogue 1
@context "shop.context"
[gate]
@branch context.unlocked and context.coins >= 20 [target:shop] [else:closed]
[shop]
// @id shop.welcome
: Welcome! (end)
[closed]
// @id shop.closed
: Come back later. (end)
```

A branch is the only content of its label. Both destinations are required; forward
references are allowed and checked. It advances automatically, without adding a
blank utterance or requiring input. Ordinary lines still wait for acknowledgement.

The small expression language supports `context.field` / `locals.field`, finite
numbers (including negative numbers), double-quoted JSON strings, `true`, `false`,
`nil`, parentheses, `not`, `and`, `or`, `==`, `~=`, `!=`, `<`, `<=`, `>` and `>=`.
Calls, indexing, arithmetic and assignments are rejected. Analysis never runs the
imported context module.

Precedence, highest first: parentheses, `not`, comparisons, `and`, `or`. Boolean
operators accept booleans or nil and return booleans; nil means false, while numbers
and strings are not implicitly truthy. `and`/`or` evaluate left to right and skip
unneeded operands. Comparisons require matching scalar types; ordered comparisons
are numeric only. This avoids language/locale-dependent string ordering.

Optional paths yield nil if an intermediate value is absent. Equality can test nil;
an ordered comparison with a missing value is false. For example,
`context.bonus != nil and context.bonus > 0` and `context.bonus > 0` both reject a
missing bonus. Use `not (context.coins >= 20)`, rather than `not context.coins >= 20`.
Unknown paths and wrong operand types are diagnosed even in an unreachable operand.
Completion, hover and definition navigation work on condition paths across imports.

### Hide or disable a reply

```text
[shop]
// @id shop.prompt
: What would you like?
// @id shop.buy
-> Buy [target:bought]
@enabled context.coins >= 20
// @id shop.buy.reason
@reason Requires 20 coins.
// @id shop.secret
-> Secret offer [target:bought]
@visible context.secretUnlocked
// @id shop.leave
-> Leave [target:done]
@fallback closed
```

Declare `@visible` / `@enabled` immediately after their choice; each may occur once.
An omitted condition means true. A hidden reply is absent from snapshot options.
A visible, disabled reply has `enabled = false` and cannot be chosen. Its optional
`reason` is localized using the preceding stable `// @id`; it uses the same bindings,
translation extraction and coverage checks as line/choice text. Enabled replies omit
the reason. Schema-three snapshots expose `enabled = true` for available replies;
older snapshots may omit it, so compatible renderers treat only `false` as disabled.

Conditional choice groups require `@fallback label`. If no **visible and enabled**
reply remains, entering or refreshing the group automatically follows that target.
To let players read disabled reasons, include an enabled exit reply. This is an
explicit branch, not an implicit selection of the first disabled option.

`choose(id)` rechecks the current accepted facts and returns false for a hidden,
disabled or unknown reply. An old UI snapshot never grants permission. The host must
still validate and apply gameplay actions such as spending money; dialogue navigation
does not perform purchases. Updating facts refreshes the current group without
replaying events or restarting the visit unless its fallback changes the current
node. A failing resolver leaves the previous facts and presentation intact.

### Bound automatic work and inspect faults

```tua
local player = context.new(scene, facts, { transitionBudget = 32 })
player:start()
local view = player:snapshot()
if view.issue then
  print(view.issue.code, view.issue.nodeId)
end
```

The default budget is 256 automatic edges per transition/refresh; `transitionBudget`
accepts integers from 1 to 4,096. Branch and no-choice fallback edges consume it.
A loop that exceeds the budget enters `faulted` with an owned issue record:
`{code = "transition_budget_exceeded", message, nodeId}`. No partial line events
are emitted. `advance` and `choose` return false; `update` returns no events.
Changing facts or locale does not restart a faulted player. Inspect it, then use
`start` to retry explicitly, `cancel`, or `dispose` to clean up. Restart resets locals
to captured defaults; a fault is not a resumable checkpoint boundary.

Conditions are limited to 8 KiB of authored expression text, 256 lexical tokens,
32 AST levels and at most 256 emitted instructions. Runtime documents validate the
instruction/depth limits too. The evaluator reads owned plain records and never uses
`load`, `loadstring`, a host method, or a wall clock. Each automatic edge evaluates
bounded data, and a refresh resolves its route once.

Assets using conditional flow or reasons emit schema **3**, requiring
`Dialogue.schemaVersion >= 3`. Adding event IDs upgrades the asset to schema **4**
and requires `Dialogue.schemaVersion >= 4`. The authoring header remains
`@dialogue 1`. Without event IDs, context-only assets still emit schema 2 and plain
assets still emit schema 1. Existing line, event, localization and context APIs
remain compatible.

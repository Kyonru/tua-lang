---
icon: lucide/message-square-text
---

# Comments And Documentation

Use comments to explain intent and documentation comments to describe an API.
Tua also reads supported LuaDoc/LuaCATS annotations in both `.tua` and `.lua`
files. These are structured comments, not runtime decorators.

## Ordinary Comments

```tua
-- Keep the previous frame's position for interpolation.
local previousX = 0

--[[
The simulation updates at a fixed rate.
Rendering interpolates between simulation steps.
]]
```

`--` comments extend to the end of the line. Lua long comments can span several
lines; use matching `--[=[ ... ]=]` delimiters if the text contains `]]`.
Ordinary comments and TODO notes do not establish types or API documentation.

## Document A Function

Put a contiguous `---` block immediately above the declaration. Avoid a blank
line or an ordinary `--` comment between that block and the declaration.

```tua
--- Clamp incoming damage to the remaining health.
---@param health number Current health before the hit.
---@param amount number Incoming damage.
---@return number Remaining health, never below zero.
local function applyDamage(health: number, amount: number): number
  return math.max(0, health - amount)
end

print(applyDamage(100, 25))
```

The native `: number` annotations describe the Tua signature. The `---` text
explains its behavior; the `---@param` and `---@return` descriptions enrich
editor documentation. Keep types in comments consistent with the native
signature: contradictory annotations can produce a conflict diagnostic.

For plain Lua, the same comments can provide types without native annotations:

```lua
--- Clamp incoming damage to the remaining health.
---@param health number Current health before the hit.
---@param amount number Incoming damage.
---@return number Remaining health, never below zero.
local function applyDamage(health, amount)
  return math.max(0, health - amount)
end
```

A comma-separated return list describes multiple returned values:

```tua
--- Return the panel's logical dimensions.
---@return number, number Width and height in pixels.
local function dimensions(): (number, number)
  return 320, 180
end
```

Native multiple-return signatures use parentheses. The LuaDoc return list does
not. A LuaDoc tuple such as `[number, string]` describes one positional table
value; it is not a multiple-return list. Named return slots and repeated
`---@return` lines are not a substitute for the supported comma-separated list.

## Document A Class Method

```tua
final class Health {
  value: number = 100

  --- Apply damage and report whether health is exhausted.
  ---@param amount number Incoming damage.
  ---@return boolean Whether health reached zero.
  damage(amount: number): boolean {
    self.value = math.max(0, self.value - amount)
    return self.value == 0
  }
}
```

Resolved method hovers include the description and parameter/return tags,
including methods reached through imported instances. Native class method
signatures remain the source for their types. Do not assume every LuaDoc tag
has the same effect on native class syntax as on a Lua function declaration.
See [Classes](language.md#classes) for imports, inheritance, and editor support.

## Choose The Right Annotation

| Need | Use |
| --- | --- |
| Type a Tua parameter, local, or return | Native `: Type` syntax; infer ordinary locals where possible. |
| Describe a Lua function contract | `---@param`, `---@return`, `---@vararg`, `---@overload`. |
| Describe a Lua record or module shape | `---@class`, `---@field`, `---@alias`, `---@type`. |
| Mark a resolved API as deprecated | `---@deprecated` with optional replacement guidance. |
| Warn about a discarded direct call result | `---@nodiscard`. |
| Map a declaration library to runtime modules | Configured `type_libraries` and `---@meta`. |
| Suppress a Tua diagnostic | `-- tua-ignore-line TLxxxx`, `-- tua-ignore-next-line TLxxxx`, or `-- tua-ignore-file TLxxxx`. |

`---@class` describes a type; it does not construct a runtime class. Likewise,
`---@generic` does not add generics to Tua. Tags including `@generic`, `@cast`,
`@as`, and `@diagnostic` are recognized metadata without their full LuaLS type
checking behavior. In particular, use Tua's ignore directives instead of
expecting `---@diagnostic disable` to suppress a Tua diagnostic.

The [LuaDoc reference](lua-interop.md#luadoc-support) owns the complete supported
tag list and type forms. [Typing In Tua](typing.md) explains native annotations.

## Use The Extension

- Type `---@` and invoke completion to see the currently offered tag subset.
  The parser supports more tags than this completion list currently offers.
- In supported annotation type positions, complete builtins and project aliases,
  for example after `---@param player Pl`.
- On a line containing only `---` immediately above a simple function declaration,
  invoke completion and select **document function**. It inserts parameter and
  explicit return annotations with description placeholders. Review and fill
  those descriptions; the generator does not infer behavioral prose.
- Hover a documented usage, use signature help while entering arguments, or go
  to definition to read its source documentation. Resolved imported functions
  and methods can carry documentation across files.
- Enable malformed-annotation diagnostics in `tua.toml`:

```toml
[diagnostics]
doc_annotations = true
```

The matching VS Code setting is `tua.diagnostics.docAnnotations`. This option
is off by default. It validates supported annotation syntax; it does not
promise typo detection for arbitrary tags or synchronization with every rename.

The documentation generator is currently limited: native class methods,
multiline signatures, complex nested parameter types, and updating an existing
comment block need more work. It is a completion item, not a dedicated
"generate/update documentation" command. Existing descriptions are not
continuously synchronized with parameter or return changes.

To include type comments in generated Lua, set `[emit].annotation_mode =
"luacats"`. This separate output option does not control whether the editor
reads your source comments. See [annotation-preserving emission](lua-interop.md#preserve-types-for-lua-tools).

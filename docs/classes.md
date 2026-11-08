---
icon: lucide/boxes
---

# Classes

Use a class when an object owns both state and methods. Tua lowers classes to
Lua tables and metatables; instances are constructed with `Name.new(...)` and
methods should be called with `instance:method(...)`.

For data without object behavior, start with a [typed table](typing.md#aliases-and-table-shapes).
For module organization, see [Modules And Imports](modules.md).

## Create An Instance

```tua
open class Entity {
  x: number
  y: number

  init(x: number, y: number) {
    self.x = x
    self.y = y
  }

  update(dt: number) {
  }
}

final class Particle extends Entity {
  speed: number = 120
  trail: number[]

  init(x: number, y: number) {
    super(x, y)
    self.trail = {}
  }

  override final update(dt: number) {
    self.x += self.speed * dt
    table.insert(self.trail, self.x)
  }
}

local particle = Particle.new(10, 20)
particle:update(0.016)
```

## Inheritance And Polymorphism

`class` is final by default; `open class` is required for inheritance.
`final class` may be written explicitly. Instance fields, `init`, methods,
`override`, `final` methods, and `super(...)` are supported. Static members,
private members, getters/setters, generic classes, mixins, and multiple
inheritance are not currently supported.

For a runnable cross-module example, see
[`polymorphism.tua`](https://github.com/Kyonru/tua-lang/blob/main/examples/features/src/polymorphism.tua)
and its actor module. A base-typed array dispatches overridden methods to each
subclass. Unrelated objects can join a shared loop through explicitly typed
table/function adapters that bind their receivers. Tua also supports
method-only [protocol contracts](#protocol-contracts) for explicitly conforming
classes. Abstract classes and methods are not supported.

## Protocol Contracts

A protocol describes methods that a class promises to provide. Declare each
method's parameter and return types, then list the protocol after `implements`:

```tua
protocol Updatable {
  update(dt: number): nil
}

final class Particle implements Updatable {
  elapsed: number = 0

  update(dt: number): nil {
    self.elapsed += dt
  }
}

final class HudClock implements Updatable {
  ticks: number = 0

  update(dt: number): nil {
    self.ticks += dt
  }
}

local function updateAll(items: Updatable[], dt: number): nil
  for _, item in ipairs(items) do
    item:update(dt)
  end
end

local items: Updatable[] = { Particle.new(), HudClock.new() }
updateAll(items, 0.5)
```

The classes remain unrelated: each explicitly implements the contract, and
calls through `Updatable[]` dispatch to the object's class method. The checker
reports missing or incompatible methods and does not infer conformance merely
because a class happens to have a method with the same name. A method inherited
from a base class can satisfy a protocol requirement.

Protocols can be exported and consumed as type-only imports; those imports and
the protocol declaration are erased from Lua. See [Sharing Types Between
Modules](modules.md#share-types-without-loading-code) for module boundaries.
Protocol bodies currently describe methods only. They do not declare fields,
default implementations, or protocol inheritance. Dedicated protocol
implementation navigation and refactoring are still being completed. The
[feature-tour example](https://github.com/Kyonru/tua-lang/blob/main/examples/features/src/protocols.tua)
exercises this language feature.

Conformance does not add a wrapper, proxy, or generated interface table. The
class still lowers to its normal Lua table and metatable, and a call through a
protocol-typed value remains a virtual colon call. For example:

```tua
local function update(value: Updatable, dt: number): nil
  value:update(dt)
end
```

emits:

```lua
local function update(value, dt)
  value:update(dt)
end
```

The annotation and protocol name disappear; Lua's method lookup still selects
the receiver's implementation and the colon call still passes `value` as
`self`. A type-only protocol import adds no runtime `require`. The generated
class remains an ordinary Tua class, including the usual constructor and
runtime class metadata used by supported `is` checks; `implements` itself adds
no runtime metadata.

## Fields And Method Syntax

Class fields use the same native types as locals and aliases, including `T[]`,
dictionaries, unions, function types, and scoped `Self`. Assigning `{}`
to an annotated array or dictionary field is valid. Fields not declared by the
class or an ancestor are rejected; use `any` only when a deliberately dynamic
object is required.

Method parameters and returns accept the same native type surface, including
table-shaped return types. Unrecognized text in a class body reports `TL1008`
instead of being discarded during class lowering.

Constructors return the declared class type, and methods keep their exact
parameter and return types across hover, completion, signature help, checking,
definition, outlines, workspace symbols, and static module exports. Static
`require` and `import` boundaries also preserve nominal class identity and
inheritance, so two structurally identical exported classes are not
interchangeable. Duplicate `init` declarations, inheritance cycles, extending
non-open classes, missing or spurious `override`, and override signature drift
are diagnosed.

Class methods use `update(dt: number) { ... }`, without the Lua `function`
keyword. Close the method with `}`. Ordinary Lua blocks and nested functions
inside that body still close with `end`. A Lua-style method declaration directly
inside a class produces a class diagnostic with this syntax guidance.

## Import A Constructor And Its Instance Type

Place the cursor on a class name and choose **Export instance type** to create
its exported alias. The action exports an existing private alias when available
and avoids duplicate exports or name collisions. In a consumer, type completion
can then insert the type-only import.

Export an instance type alias when another module needs to annotate a value
created by the runtime class:

```tua
-- dialogue.tua
final class Dialogue {
  text: string

  init(text: string) {
    self.text = text
  }
}

export type DialogueInstance = Dialogue
return { Dialogue = Dialogue }
```

```tua
-- main.tua
import type { DialogueInstance } from "./dialogue"
local DialogueModule = require("dialogue")

local dialogue: DialogueInstance = DialogueModule.Dialogue.new("Hello")
dialogue.text = "Goodbye"
```

The type-only import supplies the instance type; the runtime `require` supplies
the constructor value. Imported instances retain class member completion and
method signatures. Use colon calls for instances reached through module
boundaries so the generated Lua passes the receiver explicitly.

## Documentation And Navigation

Class names, fields, and methods participate in source definition, references,
and hover documentation. Place a `---` description immediately above a method
to show it when hovering a resolved method usage, including imported instances. Native
parameter and return annotations supply the displayed signature. Add
`---@param name Type description` and `---@return Type description` lines
to include parameter and return descriptions in the hover. See
[Comments And Documentation](comments.md) for placement, examples, and supported
annotation behavior.

Class names, fields, and methods also support rename. This includes accesses through locally inferred instances and
instances exported from static modules; edits target the original class syntax,
not generated Lua. Dynamic values typed as `any` remain outside this guarantee.

## Constructor Requirements

Inherited fields may be redeclared only when their resolved type is unchanged.
Every field whose type excludes `nil` must also be initialized on every path
that returns from the constructor. A field default, a definite `self.field =`
assignment, or a guaranteed `super(...)` call can satisfy the contract;
assigning in only one conditional branch cannot. Fields whose type accepts
`nil` may remain absent, matching Lua's field semantics.

Those correctness checks are always-on `TL1008` errors. Standard class lints
separately report constructors that can return without `super(...)` (`TL4057`),
direct calls to overridable methods during an open class's initialization
(`TL4058`), and type-only inherited field redeclarations with no child default
(`TL4059`). They obey `diagnostics.rules` and exact-code suppressions. Empty
overrides, overrides that replace rather than call parent behavior, exported
methods with no local use, and open classes with no known local subclass remain
valid and do not warn.

Inside `init` and instance methods, `self` resolves to the enclosing class.
Completion after `self.` and member hover include declared and inherited fields
and methods with their exact types.

`init` supports typed varargs. Generated constructors forward every declared
parameter, including `...`, and standalone `super(...)` or `super.method(...)`
calls preserve their argument expressions. Member calls such as
`helper.super()` remain ordinary Lua calls. A subclass's constructor signature
is independent of its parent's signature; the arguments passed to
`super(...)` are checked separately against the parent initializer.

A child class may appear before its parent in the same file. Tua gives the
later parent a stable forward table, so inherited methods and `super` calls work
once the parent declaration has executed; source order does not cause the
parent declaration to run early.

## Editing Incomplete Classes

While you are typing, the editor can recover missing closing braces on a class
and its methods, retaining member suggestions and surrounding local bindings.
Valid Lua blocks inside methods keep their scopes. The source still reports an
unclosed-class error and cannot be built until you add the missing braces;
editor recovery does not modify your file or guess missing Lua `end` keywords.

## Initialization Order And Generated Lua

Generated Lua uses ordinary tables, `__index`, local method functions, and
per-class `__type_id` / `__is` metadata. `Class.new(...)` constructs instances.
Within each class, the `init` body runs before that class's field defaults; this
ordering is preserved across inheritance and constructor optimization. A default
therefore overwrites a value assigned to the same field by `init`. For a value
computed in `init`, declare the field without a default and initialize it there.
`ClassDefaultAfterInit` (`TL4077`) points to defaults that follow a direct
source-written `self.field = ...` assignment in the same initializer. It is a
standard suppressible warning, not a change to constructor execution order:


```tua
final class Spacing {
  value: number
  init(width: number) {
    self.value = width / 16
  }
}
```

When a constructor is proven to contain only field defaults, `self.field = ...`,
and a flattenable `super(...)`, Tua emits a direct table constructor. Otherwise
it preserves explicit init calls. Calls to known final methods on known class
instances can lower to direct local function calls; non-final calls keep normal
virtual dispatch. Locally inferred native class instances accept both
`instance.method(...)` and `instance:method(...)`; use Lua's colon form for
instances obtained through module or dynamic boundaries so the receiver is
explicit in the emitted Lua. A dot call there would run without `self`, and
`TL1004` reports it with the colon call to use instead. Writing the colon form
everywhere keeps code correct when a class moves to another module.

# Independent cross-check: whitespace-sensitive native class tooling

Run from repository root:

```sh
node internal/audits/devex-2026-11-06/typing/crosscheck-class-whitespace.js
```

Raw source, exact positions, CLI output, hover/completion/signature responses and
pull diagnostics: `class-whitespace-crosscheck.json`.

Distinct fixture: `final class Tank`, `fuel: number`, `init(capacity: number)` and
`getFuel(cost: number): number`. The only difference between the two projects is
spaces around `=` in the constructor assignment:

```tua
local tank = Tank.new(12) -- spaced control
local tank=Tank.new(12)   -- compact variant (in a separate project)
```

| Surface | Spaced | Compact |
|---|---|---|
| Completion at `tank:` | `fuel`, `getFuel` | empty |
| Receiver hover | `Tank` | `any` |
| Signature inside `tank:getFuel(1)` | `(cost: number): number` | null |
| Hover `tank.fuel` field | number | null |
| Hover inferred method-result local | number | any |
| CLI check of `local wrong: string = tank.fuel` | TL1003 expected string, found number | same TL1003 |
| LSP diagnostics of that negative assignment | same TL1003 | same TL1003 |

No incomplete syntax is needed. Both sources are valid apart from the identical,
intentional negative assignment used to test checker consistency. All probe
coordinates are on later identical lines, so changing the constructor line's
length cannot misplace the request. Field/type/class names differ from the
original Counter fixture. This rules out the initial constructor-name or
particular-init-body explanation.

`crates/tua-core/src/semantic.rs:1118–1141` scans preceding source lines using the
literal prefix `format!("local {name} = ")`, matching the observed whitespace
sensitivity. A syntax/scoped-binding backed constructor fact should replace this
text-layout predicate; do not patch additional spacing spellings individually.

**Assessment:** High is justified for a developer-experience audit: ordinary
legal formatting loses the principal class-instance editing features, contrary
to whitespace-insensitive language semantics. Bound the impact correctly:
checking still identifies the wrong assignment in this reproduction, so this is
not evidence that compact syntax bypasses type checking. Not Critical. If the
project uses a severity scale restricted to runtime/checker breakage, Medium
would be defensible; under the audit's stated “ordinary typed workflows lose key
tooling” High definition, retain High.

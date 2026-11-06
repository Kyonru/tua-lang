# Bounded native class constructor follow-up

This follow-up separates the previously correlated explicit-init fixture from
its actual whitespace-sensitive local-binding defect. No fixes were made.

Reproduce:

```sh
node internal/audits/devex-2026-11-06/completion/class-init-variants.js
```

The script tests ten class-body variants (no init; empty init; unused parameter;
assigned parameter; renamed parameter; field without default; multiline init;
different field name; multiline init/method; init after method). Within each
variant it records the combined call layout and isolated direct/local receivers.
Two additional layout controls are limited to the assigned-parameter variant.
Sources and exact queries are preserved in `class-init-variants-evidence.json`.

## CMP-07 — Native class instance inference depends on spaces around assignment

**Category:** semantic correctness / class completion, hover and signature help.
**Severity:** High. **Confidence:** high. **Status:** confirmed protocol with
controlled source edits; exact matching implementation located. Independently
confirmed by the typing agent using Tank/fuel/getFuel; see
[raw cross-check](../typing/class-whitespace-crosscheck.json) and
[independent review](../typing/REVIEW-class-whitespace.md).
Both spacing forms still correctly reject a deliberately wrong string assignment
to the numeric field: checker unsoundness is not claimed for this finding. This is a distinct defect from expression-hover
provenance (HOV-03), although they can occur in the same file.

### Minimal reproduction

```tua
class Counter {
 value: number = 0
 init(value: number) { self.value = value }
 read(): number { return self.value }
}
local counter=Counter.new(1)
counter:read()
print(counter.value)
```

Query completion just after `counter:` at line 6, character 8; hover inside
`counter` at line 6, character 2; signature help just after `read(` at line 6,
character 13. Positions are zero-based UTF-16 and the fixture is ASCII.

**Expected:** `read` and `value` members; `counter: Counter`; signature
`function counter:read(): number`. Whitespace around `=` is insignificant Lua/Tua
syntax. `docs/classes.md:8` and `:76–83` promise constructor instances and exact
method types across these editor surfaces.

**Actual:** completion `[]`, receiver hover `local counter: any`, signature null.
Changing only the declaration to `local counter = Counter.new(1)` gives both
members, `counter: Counter`, and the expected method signature. The same effect
survives changing `print(counter.value)` to `print(counter)`, so it is not
backward inference from a later field access.

Evidence: `class-init-variants-evidence.json`, `assigned-arg`, isolated layouts
`local-only`, `local-compact`, `local-counter-print`, and matching request labels.
Ten body/init variants all provide useful local members/signatures with the
spaced binding. Thus **explicit init, its argument name, field default, or inline
method layout are not the necessary cause**. The earlier explicit-init/minimal
fixtures happened to use compact assignment; the passing no-init fixture used
spaces. The initial correlation was not itself a root-cause diagnosis.

### Root-cause evidence

`crates/tua-core/src/semantic.rs:1118–1146` implements
`class_instance_local_type` by scanning prior source lines. It builds:

```rust
let prefix = format!("local {name} = ");
```

and uses `trimmed.strip_prefix(&prefix)` before recognizing `Class.new(...)`.
This literal prefix requires exactly the spelling of spaces in the passing
control. The fallback is used by `type_of_name` at `semantic.rs:1078`; compact
assignment cannot enter it. This establishes the textual matching failure.
Underlying native constructor facts already exist in the environment
(`types/environment.rs:911–924` registers generated constructor return types),
so the correct direction is to expose those through the shared expression and
binding resolver rather than expanding the line-scanning pattern.

### Existing coverage and recommended fix

Native class completion tests commonly use formatted `local name = Class.new`
fixtures. Earlier no-init controls in the hover lane also use that form. There
is no whitespace-neutral constructor-result matrix in the inspected tests.
Testing only formatter-normalized examples masks this editing-time failure.

Resolve local assignment expressions through scope/HIR declaration identity and
source/parser provenance, then resolve the native constructor's result through
shared class metadata. Do not simply replace the literal prefix with another
regex: that would still mishandle multiline assignments, same-line multiple
statements, shadowing, reassignment and constructor aliases. Keep gradual `any`
only when the receiver identity is genuinely unknown.

Regression: same valid program with spaced/compact/multiline assignments, no init
and explicit init, nested scopes and a shadowed constructor; compare inferred
receiver type, members, method signature and checker behavior. Include both open
unsaved buffers and saved files. Acceptance: formatting a program must not be
required to recover the instance type. Approximate effort **M**, shared semantic
infrastructure; high priority with declaration identity and provenance fixes.

## U-01 — Direct native constructor chain loses receiver; causal link unresolved

This is a **verified symptom with unresolved root**, not an additional independently attributed
root cause and not silently omitted.

Across all ten class-body variants, the isolated direct call
`Counter.new(...):read()` gives empty member completion and null signature help.
The corresponding spaced local receiver works. Some direct method hovers also
show the false generated expression type/range documented in HOV-03, but the
absence of completion/signature is not explained solely by that hover traversal.

This cannot be fixed by changing whitespace in a local assignment: a direct
receiver has no local assignment for the source-scanning fallback at all. It may
be the same missing constructor-result authority that the textual workaround
covers, or another source-coordinate problem in expression receiver resolution.
The audit did not experimentally distinguish these paths. The integrated backlog
should explicitly retain the direct-chain regression under shared constructor
expression resolution, and avoid asserting that a whitespace-only fix resolves
it. Test direct method tokens and signature help separately from whole-expression
hover after HOV-03/CMP-07 remediation.

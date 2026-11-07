# Classes and polymorphism audit

Revision and candidate: shared BASELINE.md, 0.6.0 release executable. One new confirmed High defect; no product changes. Tested ordinary inheritance and imported class workflows behave as documented, subject to the known nominal assignability failures in the release baseline.

## Execution and coverage

From repository root:

```
python3 internal/audits/features-2026-11-07/classes/probe.py
node internal/audits/features-2026-11-07/classes/editor.js
```

Both harnesses use the absolute repository `target/release/tua` and `/opt/homebrew/bin/lua` (Lua 5.4.8). Every project and emitted file is retained here. `results.json` captures CLI status/stdout/stderr and runtime assertions; `editor-results.json` captures actual stdio LSP responses and the imported workflow build/runtime results. No cargo rebuild or desktop automation.

| Area | Status and evidence |
| --- | --- |
| Inheritance / virtual dispatch / super.method / final method / Self return | Tested: positive fixture builds and prints `positive ok`; base-typed array invokes subclass method |
| Init/default ordering including same-type inherited redeclaration | Tested: default-order prints `defaults ok`; defaults overwrite init as documented |
| Forward parent | Tested: forward prints `forward ok` |
| Self identity in constructor | Tested: failing self-reference; passing local-capture self-control |
| Final class and final method enforcement | Tested: negative rejects both with TL1008 |
| Required-field definite initialization | Tested independently: uninitialized rejects one-branch-only assignment with TL1008 |
| Override signature drift | Tested independently: override-drift rejects return mismatch with TL1008 |
| Missing implicit parent arguments | Tested: implicit-parent rejects with TL1004 |
| Instance type import + constructor require + virtual dispatch | Tested: module builds two files and Lua prints `6` |
| Imported hover, signature, completion, definition | Tested actual LSP: exact Actor method signature/documentation, dt parameter, value/update members, definition in actor.tua; empty diagnostics |
| Constructor flattening and relevant tests | Inspected classes.rs; emit/tests.rs:393; check/tests.rs:3445 |
| Rename/references/outlines/export action, typed varargs, duplicate init, cycles | Not exercised in this bounded pass |
| LuaJIT/LOVE execution | Not exercised; Lua 5.4 runtime used |
| Known record-to-class nominal acceptance | Known release-baseline failure; not rediscovered or counted as new |

## CLASS-01 — Flattening constructor self references loses object identity

**High; confirmed defect; new relative to reviewed roadmap/prior audit reports.** A valid constructor storing itself in a field passes check and build, but the emitted optimized constructor reads an unrelated/global `self`. With no such global, the required field disappears (`nil`), corrupting identity and violating its non-null `Self` type.

Reproduction: `self-reference/src/main.tua` declares `next: Self`, assigns `self.next = self` in init, then asserts `Node.new().next` is that instance. Run the harness above, or change directory to `classes/self-reference`, run the absolute release binary with `check` and `build`, then `/opt/homebrew/bin/lua build/main.lua`.

Expected: check/build succeed and assertion passes. Actual: check/build exit 0; Lua exits 1 with `self reference lost`. `self-reference/build/main.lua:14` constructs `{ next = self }` without a local self binding. The unused init function correctly contains `self.next = self`. Control `self-control/src/main.tua` introduces `local owner = self` and stores owner; it forces explicit allocation/init and prints `control ok`. This is distinct from documented defaults-after-init ordering.

Root evidence: `crates/tua-core/src/classes.rs:1187` accepts and collects simple assignments; `:2302` substitutes whole identifiers; `:2317` accepts the identifier `self` as simple. `:995` emits a table literal without allocating a receiver local first. The fast path therefore moves receiver-dependent expressions out of their lexical binding.

Existing test blind spot: `crates/tua-core/src/emit/tests.rs:393` checks generated text for parameter/default ordering; `crates/tua-core/src/check/tests.rs:3445` checks simple parameter assignments and final calls. Neither validates runtime self identity across optimized and fallback constructors.

Smallest maintainable improvement: make constructor flattening reject values that reference the initializer receiver (including defaults and substituted parent arguments), preserving the existing allocation/init path. Keep this decision in the shared class lowering analysis. Regression: execute emitted Lua for a self-typed field assigned `self`, compare with the local-capture control, and cover inherited self-reference assignments and receiver defaults if supported.

## Separate design questions

Protocols/interfaces, generic classes, abstract classes and multiple inheritance are explicitly unimplemented in docs/classes.md and deferred in ROADMAP.md:148–156. They are known design gaps, not defects. Current typed adapters remain a documented option; this pass does not propose new syntax. No blockers prevented the executed probes.

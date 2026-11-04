# Simulated Tua development trials — November 2026

Two agents independently implemented small game features against baseline
`e766bcf`. They wrote and changed real Tua files, checked and built them, and
queried the actual stdio language server, including unsaved, incomplete edits.
This is a synthetic programming exercise, not a human usability study or a
VS Code GUI test. It supplements the earlier zombie-project audit.

## Scenarios

- **Gameplay:** four modules containing Actor/Enemy inheritance, a typed actor
  array, spawning, damage, and a cross-file event handler. Generated Lua was
  executed to verify construction, inherited methods, virtual dispatch through
  the base array, and event delivery.
- **UI:** a Label class with Font/Text, LoveColor, a nullable imported instance,
  multi-return dimensions, a fluent Self-returning setter, and a measurement
  helper moved across modules. Both original projects passed check and build.

Agents saved exact sources, request positions, responses, and replay scripts.
The local evidence bundle is `target/tua-devex`; temporary working projects are
`/private/tmp/tua-devex-gameplay` and `/private/tmp/tua-devex-ui`. Permanent
regressions are in the core analysis/hover/types tests and LSP navigation and
completion tests. No zombie source files were changed during these trials.

Initial probes that accidentally targeted the annotation colon or `love.draw`
declaration were corrected. They are not counted as product failures.

## Fixed

| Finding and reproduction | Result | Commit |
| --- | --- | --- |
| `local target = actors[1]; target:damage(n)` and `for _, actor in ipairs(world.actors)` had completion/signatures but no method definition. | Navigation now uses the shared semantic name resolver for inferred receivers. | `bb157ec` |
| The first statement inside `if title then` inferred `width, height = title:getSize()` as `any`. | RHS evaluation immediately before the statement remains inside the branch's narrowing boundary; both values infer `number`. | `ae01698` |
| `font`/`value` parameter uses inside class bodies hovered as an unrelated builtin or expression, with incorrect ranges. | Hover converts source positions to parser positions explicitly; nested shadowing is covered. Existing emitter coordinate behavior is preserved. | `1b05dad` |
| `local width = love.graphics.newCanvas(...):getWidth()` inferred `Canvas`. | Local/return inference uses the final call's return list, including multiple values and imported factory results. A void call does not invent a receiver for the next method. | `a2d5785` |
| Editing an event handler to `print(payload.)` or `payload. .. " hit"` lost completion. | Token-aware recovery preserves the expression and callback scope; existing cached recovered facts feed module/type/usage indexes. Tests cover cold incomplete files and unsaved LSP edits. Release replay also restores standalone `title:` completion in the UI scenario. | `b152f7d` |

These are separate behavior commits. Changelog bullets for class hover and
recovery were staged alongside neighboring commits while agents worked; the
implementation and regression changes remain separated by behavior.

## Remaining defects and concrete next work

| Priority | Reproduction / observed result | Workaround and next step |
| --- | --- | --- |
| High | `title:setValue("x"):draw(` has no signature; completion after the returned expression is incomplete. This affects inferred and annotated receivers and LOVE constructor chains. | Split the chain into named locals. Add a shared expression-receiver context used by completion, signatures, hover and navigation; current contexts accept identifier chains. Test completed and incomplete expressions together. |
| High | Imported `LabelInstance` method returning `Self` displays `any`; an inferred constructor receiver can retain `Label`. | Avoid relying on fluent calls across the explicit instance alias. Preserve nominal class identity through recursive project-type expansion, then rehydrate members through that identity. Do not remove recursive LuaCATS safeguards or merely change displayed text. |
| Medium | `local dead = target:damage(amount)` is locally `boolean`, but emitting it through another module yields handler parameter `dead: any`. | Annotate `local dead: boolean`. Feed project-aware call-result facts into event indexing with bounded dependency/cycle handling. Target and numeric amount already propagate. |
| Medium | Class parameter member completion still has source/parser coordinate inconsistencies beyond the corrected hover path. | Use a typed local as needed. Make coordinate contracts explicit across semantic entry points; avoid heuristic double mapping. |
| Low, needs isolation | At an empty `Text:getWidth(` argument slot, signature selection prefers the index overload over the zero-argument overload. | Both overloads remain available. Distinguish an empty argument slot from a supplied first argument before changing overload selection globally. |

The last item is an observed usability issue, not yet a fully isolated general
policy defect. No performance improvement is claimed from these tiny projects.
Large-project cold typing, require analysis, hot reload, and format-on-save
need separate traces and live-editor validation.

## What Tua needs next — implementation epics

Each epic is a behavior-sized delivery with its own acceptance checks and commit.
The remaining-defects table above remains the backlog; completing one of these
scoped deliveries does not imply that every semantic limitation is resolved.

### Epic 1: Editor agreement during typing

**Delivered:** shared callback-field types now drive field hovers with exact
identifier ranges; inferred loop bindings hover with their actual type; function
parameter scopes include recovered empty bodies so a lone `payload.` retains
completion. No synthetic runtime statements are inserted for recovery.

**Acceptance matrix:**

| Scenario | Checks |
| --- | --- |
| Inferred, annotated, and nullable imported class instances | Hover, completion, signature help, definition |
| Imported base-class array iteration | Receiver hover, completion, signature help, definition |
| Cross-file event record payload | Field type/range hover and completion |
| Callback edited to one incomplete member access | Cold snapshot completion and unsaved LSP completion |
| LOVE chained return values and class parameter hovers | Existing trial regressions retained |

Core `devex_*` tests and the LSP `devex_event_field_*` test exercise this matrix.
Fluent expression receivers, imported `Self`, emitted-call payload inference,
and class parameter completion remain separate semantic follow-ups.

### Epic 2: Less friction importing class types

**Delivered:** the **Export instance type** action on a class name creates an
exported alias or promotes an existing private alias. It avoids duplicate exports
and collisions with values, types, or type imports. Existing type completion and
unknown-type import actions consume the export. Core tests apply the provider
edit and check a typed consumer; an LSP test resolves auto-import completion
after the provider changes without saving. No new type syntax is introduced.

### Epic 3: Clearer syntax-error guidance

**Planned delivery:** explain native class-method brace syntax when a class
contains Lua-style `function`/`end` declarations. Preserve ordinary Lua bodies,
including nested functions, and do not change the language grammar.
Validate location, actionable message, and unaffected valid syntax.

### Epic 4: Keep abstractions evidence-led

**Planned delivery:** a runnable, documented polymorphism example demonstrating
base-typed collections, overridden dispatch, imported instance types, and the
boundary between shared ancestry and unrelated behaviors. Verify generated
Lua behavior; record a concrete future protocol use case without claiming
protocols, generics, or multiple inheritance are implemented.

## Positive results and limits

Imported construction and factories, inherited/overridden dispatch, base-typed
arrays, explicit instance aliases, normal LOVE method docs/signatures/virtual
definitions, cross-file method docs and rename, and moving a typed helper across
modules all worked. Event-field completion updated after an unsaved producer
rename. A wrong damage argument produced a project diagnostic at the argument.

The trials did not launch a graphical game or validate an installed extension.
They do not establish subjective editor usability or zombie-scale performance.
Final verification and artifact checks are recorded in `target/tua-devex`.

Final verification passed: `RUST_MIN_STACK=16777216 make verify` (with local
loopback permission), `make docs-build`, `make lsp-features-smoke`, both trial
check/build runs, gameplay Lua runtime, and rebuilt-server replays. The release
replay confirms chained Canvas width is `number`. `make extension-package`
passed and the resulting VSIX passed archive integrity checks. Detailed logs
and SHA-256 are in `target/tua-devex`.

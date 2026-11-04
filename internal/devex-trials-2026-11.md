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
| Medium | An object payload completes fields correctly, but hovering `payload.amount` can return the enclosing `(payload: any) -> any` expression and range. | Use completion or an explicit handler annotation. Resolve the smallest field expression using the same project event facts. |
| Medium | A loop receiver can complete Actor members and now navigate correctly while its own hover still says `expression: any`. | Use an explicit typed local if hover is needed. Align loop-variable hover with the semantic resolver. |
| Medium | A standalone unfinished `payload.` as the only callback statement can lose its scope when recovery removes the line. The UI scenario with other statements now completes after `title:`; the empty-body case remains. | Keep an enclosing expression such as `print(payload.)`, or complete the method name. Preserve scope extent while recovering an otherwise empty body. |
| Medium | Class parameter member completion still has source/parser coordinate inconsistencies beyond the corrected hover path. | Use a typed local as needed. Make coordinate contracts explicit across semantic entry points; avoid heuristic double mapping. |
| Low, needs isolation | At an empty `Text:getWidth(` argument slot, signature selection prefers the index overload over the zero-argument overload. | Both overloads remain available. Distinguish an empty argument slot from a supplied first argument before changing overload selection globally. |

The last item is an observed usability issue, not yet a fully isolated general
policy defect. No performance improvement is claimed from these tiny projects.
Large-project cold typing, require analysis, hot reload, and format-on-save
need separate traces and live-editor validation.

## What Tua needs next

1. **Editor agreement during typing.** A conformance matrix should cover inferred
   and annotated values, nullable guards, imported aliases, event callbacks,
   classes, LOVE objects, and incomplete expressions across all editor surfaces.
   Successful compilation alone misses these failures.
2. **Less friction importing class types.** Runtime `require`, explicit type
   import, and an exported instance alias work, but are verbose. Discoverable
   type auto-import and annotation actions would improve the common workflow.
3. **Clearer syntax-error guidance.** Mixing class-method braces with Lua
   function `end` syntax caused an initial development mistake. Better targeted
   diagnostics would help without changing the language.
4. **Keep abstractions evidence-led.** The gameplay trial demonstrates useful
   current single-inheritance polymorphism. These findings justify fixing editor
   consistency before adding generics, protocols, or multiple inheritance. A
   protocol feature should be driven by a concrete unrelated-class API need.

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

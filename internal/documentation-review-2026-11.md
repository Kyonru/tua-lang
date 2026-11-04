# Documentation and annotation review — November 2026

Reviewed the production changes since `b40dbb4`, their changed public guides,
and the annotation parser, completion provider, type environment, highlighting,
and extension settings. This was a focused source/documentation review with
headless LSP probes, not another installed-editor or graphical runtime audit.
The previous [development trials](devex-trials-2026-11.md) remain the inventory
of unresolved fluent-chain, event inference, and coordinate-consistency issues.

## Findings addressed

- Comment-writing instructions were scattered through the Lua interop, class,
  and LSP guides. Added the discoverable `docs/comments.md` guide, linked from
  navigation and the owning reference pages. It separates comments, native
  type annotations, LuaDoc typing, metadata, and Tua diagnostic directives.
- Existing `document function` completion was not explained as a workflow.
  The guide now describes its trigger, placeholders, and limits, without
  promising class-method or multiline-signature generation.
- A real LSP probe produced `---@return (number, number)` for a native multiple
  return signature. Checking that generated comment produced TL1001/TL1006.
  Completion now emits a comma-separated LuaDoc return list, and the parser
  and function-body typing retain every return slot. Regressions insert the
  generated comment and check its contract, validate nested return type shapes,
  reject missing slots, and follow a Lua module's second return into a Tua hover.
- A build probe found the same multiple-return distinction lost in LuaCATS
  emission: `(): (number, string)` became `---@return [number, string]`.
  Emitted function and exported callable annotations now keep the return list;
  a regression parses the generated Lua and verifies both return slots.
- The LoveColor section was under language boundaries and the lazy-emission
  paragraph was after architecture links. Moved them into their owning sections.
- Qualified the class source-map description: source-backed body lines map to
  their method; generated code can map to the enclosing class.
- Clarified that `@meta` can identify configured declaration-library modules,
  while recognizing `@diagnostic` does not implement LuaLS suppression behavior.

## Extension improvements, in priority order

1. **Explain every supported tag in completion.** The parser recognizes more
   tags than `complete_annotation_tags` offers. Add missing `deprecated`,
   `nodiscard`, and declaration/metadata tags; show syntax, a short example,
   and whether the tag affects checking or is metadata only. Derive the list
   from a shared tag description rather than another extension-owned registry.
2. **Generate documentation from parsed signatures.** Add a generate/update
   action for native class methods, multiline functions, nested function/table
   parameter types, defaults, and varargs. The current generator partly splits
   signature text on commas and only finds ordinary function annotations.
   Preserve existing prose, update matching parameter names, and never invent
   descriptions or silently change native types.
3. **Keep comments aligned with code.** Offer targeted fixes for missing/stale
   parameter tags and ensure rename updates the matching parameter reference
   in docs. Type disagreement is already checked in supported ordinary
   function contracts; this is not a request to duplicate that diagnostic.
4. **Navigate and explain annotations.** Type names in annotation positions
   should use the same completion/navigation source as native types. Hovering
   a tag should explain its actual Tua effect and link to the local guide.
   Explain that `@generic`, `@cast`, and `@as` do not provide full LuaLS semantics.
5. **Test editing states, not only finished examples.** Use one matrix for Lua
   functions, native functions, class methods, imports, partial tags, and
   generated snippets. Insert generated snippets and check them, rather than
   asserting only the produced string. Keep native and parser source positions
   explicit across these queries.

These are proposed follow-ups, not shipped features. The extension should
present semantic results from tua-core/LSP rather than implement its own
annotation parser in JavaScript.

## Remaining review limits

No new blocker was established in the reviewed hot-reload, lazy-emission,
class provenance, or offline-reference changes. That is not a claim that every
combination works: the earlier trial report records known limitations, and
class coordinate handling plus partial-source project facts remain important
regression-test boundaries. This pass does not remeasure cold-start or save
performance and does not rebuild the previously delivered VSIX.

# Rich dialogue presentations

Run `tua check`, `tua build`, then `(cd build && lua main.lua)`.

This headless example uses typed schema-8 documents with explicit `richText`
templates, matching plain fallback templates, emphasis, leading/trailing pauses,
scoped reveal rates, literal placeholder values and a localized template callback.
A renderer can draw `node.text` or use `node.presentation.runs`. Text shaping,
colors, animation, font and audio choices remain with the host.

The `.tdialogue` `@text rich` directive is not enabled yet. This example exercises
the runtime path that future compiler emission will target. The normal localization
adapter `I18n.dialogueTemplate` can supply `resolveTemplate` in configured projects.
See [the dialogue guide](../../docs/dialogue.md#rich-presentations-and-timing).

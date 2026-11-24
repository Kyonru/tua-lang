# Rich dialogue presentations

Run `tua check`, `tua build`, then `(cd build && lua main.lua)`.

This headless example uses an authored `@text rich` script and typed schema-8 documents with explicit `richText`
templates, matching plain fallback templates, emphasis, leading/trailing pauses,
scoped reveal rates, literal placeholder values and a localized template callback.
A renderer can draw `node.text` or use `node.presentation.runs`. Text shaping,
colors, animation, font and audio choices remain with the host.

The authored script emits the same rich runtime representation as typed documents.
The normal localization
adapter `I18n.dialogueTemplate` can supply `resolveTemplate` in configured projects.
See [the dialogue guide](../../docs/dialogue.md#rich-presentations-and-timing).

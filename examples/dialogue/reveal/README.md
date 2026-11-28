# Grapheme-aware dialogue reveal

Run `tua check`, `tua build`, then `(cd build && lua main.lua)`.
This silent, headless example uses Unicode combining marks, emoji and flags, Indic
and CJK text, independent controllers, reduced motion, and literal placeholder text.

The host presents snapshots, updates reveal, then reports actual completion to the
dialogue player. Optional cues are bounded presentation data; no audio is played.
See [reveal documentation](../../../docs/dialogue/presentation.md#reveal-text-by-grapheme-cluster)
for lifecycle tokens, shaping responsibilities and the remaining styled-text work.
Generated output is not committed.

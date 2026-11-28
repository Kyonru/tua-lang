# Character expressions

Run `tua check`, `tua build`, then `(cd build && lua main.lua)`.
The headless example shares two characters across two independent players, selects
static and animated portrait metadata, and checks per-line reset behavior. Asset
IDs are illustrative: no files are loaded, and no GPU resources are needed.

For a graphical host, supply the images and use the optional static-image or
`spriteSheet` [adapter recipes](../../../docs/dialogue/characters-and-audio.md#select-character-expressions).
Keep sprite players per presentation; never put mutable animation state in the cast.
Generated output is not committed.

The additional `media.tdialogue` script demonstrates shared typed speech profiles,
per-line overrides/silence, voice cue locale/fallback policy and pronunciation hints.
These are owned descriptions only; this headless example does not load or play audio.

Try completing an `@expression` name after a character utterance. Suggestions use
that character's declared expressions and replace the whole name, including dots
or hyphens. Choices and duplicate expression metadata end that editing context.
See [expression authoring](../../../docs/dialogue/editor.md#complete-character-expressions).

Inside an `@voice` cue string, use completion to reuse an authored cue in this
project, or Find References to locate other dialogue uses. These queries do not
load audio or resolve files; the host owns cue-to-recording mappings. See
[voice-cue authoring](../../../docs/dialogue/editor.md#find-authored-voice-cues).

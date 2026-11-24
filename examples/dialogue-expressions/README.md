# Character expressions

Run `tua check`, `tua build`, then `(cd build && lua main.lua)`.
The headless example shares two characters across two independent players, selects
static and animated portrait metadata, and checks per-line reset behavior. Asset
IDs are illustrative: no files are loaded, and no GPU resources are needed.

For a graphical host, supply the images and use the optional static-image or
`spriteSheet` [adapter recipes](../../docs/dialogue.md#select-character-expressions).
Keep sprite players per presentation; never put mutable animation state in the cast.
Generated output is not committed.

The additional `media.tdialogue` script demonstrates shared typed speech profiles,
per-line overrides/silence, voice cue locale/fallback policy and pronunciation hints.
These are owned descriptions only; this headless example does not load or play audio.

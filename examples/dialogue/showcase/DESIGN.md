# Dialogue showcase design

## Intent

This example is for a developer or writer comparing dialogue presentations while
reading an actual conversation. The words, speaker and next decision lead. The stage
is flat and quiet; there is no decorative harbor simulation. H hides technical controls
and diagnostics until requested. The three characters are the visual anchor.

## Visual grammar

- Cinematic: dark slate, fine brass framing, an overlapping bust on roomy windows.
- Storybook: a smaller asymmetrical pale speech card with a tail and Lora body text.
- Transmission: narrow, right-aligned, channel identity and restrained signal marks.
- Subtitles: no containing panel; speaker, a short rule, text and input cue.
- Portrait location is independent of style. At small sizes, cinematic busts become
  inset portraits to protect the text measure. An above-panel portrait sits at the
  upper right so it cannot cover the title at 640×480.

`theme.tua` owns named palettes. Their colors come from the story's materials:
slate water, oxidized metal, brass, pale chart paper. The stage is #131B1F in the dark
styles; storybook uses a desaturated green stage. Body text is 21px Manrope or 22px
Lora, speaker names 16px, utility labels 12–13px, and the story title 32px Lora.
Manrope is bundled at regular weight rather than relying on a variable font's default.

Keep a 32px stage margin and 28px panel inset. Measure full text before revealing it,
so lines and choices do not shift as glyphs arrive. Stable line height, wrapped replies,
auto-scrolled choice focus, and explicit manual scrolling protect long/localized content.
The footer belongs to input hints, not diagnostics. Do not revive the always-visible
four-line developer HUD or place a decorative animation behind the text.

## References and interpretation

The user supplied local Artstyle reference images. These were inspected directly:

- `ui-1145360-0.webp` (Hades): portrait breaking the box and a clear speaker identity.
  Borrow the spatial relationship, not its frame decoration or character art.
- `ui-607050-4.webp` (Wargroove): compact speech card and readable speaker naming.
- `ui-965230-2.webp`: portraits can anchor opposite sides rather than occupying a
  mandatory slot inside every panel.
- `ui-987720-1.webp` and `ui-678950-20.webp`: sparse subtitle treatments can leave
  the text unframed. This motivates the fourth style.
- `ui-435530-9.webp`: a communication interface can have its own channel identity.

The local files were found in the user's downloaded Artstyle `dialogue-examples`
folder. They are references only and are not included in the repository.
[Artstyle dialogue reference index](https://artstyle.games/interfaces?q=dialogue+boxes).
The linked YouTube video and Artstyle guide could not be fetched during this work;
no claim is made about their contents.

The story follows the useful distinction between information hubs and consequential
choices, and uses a small relationship variant after reconvergence. See
[GameAnalytics: dialogue trees](https://www.gameanalytics.com/blog/how-to-write-perfect-dialogue-trees-for-games).

## Keep checking

Capture all four styles, all four portrait positions, both sizes, and French text.
Inspect the last choice after keyboard scrolling, the disabled explanation, concurrent
radio, and completed states. The old crash/empty-panel/locale/priority/focus tests remain.
Procedural portraits are deliberately replaceable: use `node.portrait` as the lookup ID
for your own licensed illustration or animation adapter.

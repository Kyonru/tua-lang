# Dialogue showcase design

## Intent

This example is for a developer or writer comparing dialogue presentations while
reading an actual conversation. The words, speaker and next decision lead. The stage
is flat and quiet; there is no decorative harbor simulation. H hides technical controls
and diagnostics until requested. The three characters are the visual anchor.

## Original visual grammar (keys 1–4)

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

## Additional reference styles (keys 5–7)

The original four styles remain available. Additions use their own palettes and
frame module so the new treatments do not replace the original ones.

- **Mythic frame:** the supplied Hades screenshot's pale reading panel, gold double
  edge, cut corners and dark angled nameplate. The portrait can break the frame.
- **Tactics box:** the Wargroove screenshot's stepped rectangular frame, pale inset,
  burgundy speaker tab and small diamond cue. Pixelify Sans adds the pixel type treatment.
- **Field terminal:** the supplied communication-window screenshot's physical green
  surround, dark screen, signal-status rail and channel identity. Space Mono gives
  it a separate typographic rhythm. Status-rail marks are decorative indicators,
  not interactive buttons.

Mara, Ivo and Beacon now use bundled DiceBear Lorelei avatars. Fixed seeds and
explicit component options keep identity stable across expression variants. The
slate square and sage circular portrait backgrounds are unchanged, including when
the portrait overlaps a frame. No request is made to DiceBear during play.

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

Capture all seven styles, all four portrait positions, both sizes, and French text.
Inspect the last choice after keyboard scrolling, the disabled explanation, concurrent
radio, and completed states. The old crash/empty-panel/locale/priority/focus tests remain.
Bundled Lorelei portraits are deliberately replaceable: use `node.portrait` as the lookup ID
for your own licensed illustration or animation adapter.

## Rich text

Named line treatments live in `rich_styles.tua`. Their decorations do not alter
layout metrics: fully measured text keeps wrapping stable while it reveals. Motion
is limited to small offsets; M disables it. Colors adapt to dark and pale panels.
Emphasis, underline and highlight may combine, while story prose uses effects only
on meaningful phrases. F3 is the deliberately denser sampler. Preserve plain history,
literal placeholders and localized templates when adding treatments.

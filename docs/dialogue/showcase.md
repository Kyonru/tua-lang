---
icon: lucide/gamepad-2
---

# LÖVE showcase

Explore complete dialogue presentations and borrow host adapters for your own game.

[Dialogue overview](index.md)

## Try the LÖVE showcase

[Beacon Harbor](https://github.com/kyonru/tua-lang/tree/main/examples/dialogue/showcase)
uses the same player for seven presentations: the original cinematic, storybook,
transmission and subtitle styles, plus mythic, tactics and terminal frames. Lorelei
character portraits retain their separate slate-and-sage backgrounds.
The longer story includes a question hub, three outcomes, and a relationship-dependent
ending variant. Portraits can sit left, right, above the panel, or be hidden.
It includes a second independent announcement player, English/French templates,
shared characters, conditional replies, typed actions, chatter, voice, waits,
checkpoint restore and a bounded transcript.

From the repository root:

```sh
cd examples/dialogue/showcase
tua check
tua build
love build
```

Press **1–4** for the original styles, **5–7** for the additional reference-based
styles, and **O** to cycle portrait placement.
**H** opens controls/diagnostics, **Tab** opens the scrollable recent transcript, and
**F2** switches between the longer story and the short feature demonstration.
**F3** opens a rich-text tour with nested emphasis, highlights, underline/strike,
thought/whisper text, colors, motion, scoped reveal speed and pauses. These are
showcase renderer treatments built on the existing rich-text marks.
**F4** opens a separate character-staging conversation: opposite portrait slots,
speaker-specific bubble anchors, portrait-free speech and a per-line position override.
The typed host layout uses stable speaker/text IDs and survives language changes.
**Space** finishes reveal, then advances;
**Up/Down** and **Enter** select replies. **F** changes reveal speed, **M** reduces
motion, **S** toggles subtitles and **V** mutes recorded voice. **L** switches
language, **P** pauses, **R** resets, **Q/N** queue conversations/announcements,
and **F5/F9** capture/restore an in-memory checkpoint. **[ / ]** scroll content (Page Up/Down remain aliases). The recent-conversation
panel also has clickable Older/Newer buttons and a scroll-position indicator.

The F4 staging tour always reserves modal priority. Elsewhere, cinematic, mythic
and tactics modes give the conversation modal priority and suspend radio; the other
styles permit concurrent announcements. The background stays quiet so dialogue is
the focus. These are example host policies. The engine does not decide
whether your game pauses or which channel owns input. Two conversation and four
announcement waiting slots reject overflow. A restore creates a fresh player and
stops stale presentation/audio; it does not replay a settled token action.

The checkpoint demonstration is session-local. It is not a crash-safe gameplay
save: use the [durable-action recipe](https://github.com/kyonru/tua-lang/tree/main/examples/dialogue/durable)
when dialogue changes persistent inventory or rewards. Host preferences, transcript
privacy and external effects remain the game's responsibility.

### Borrow one adapter at a time

| Need | Start with |
| --- | --- |
| Plain conversations | `snapshot.node.text`, explicit `advance`/`choose`; [basics](https://github.com/kyonru/tua-lang/tree/main/examples/dialogue/basics) |
| Custom styled text | `presenter.tua`: semantic run names, owned fonts, portrait layouts, wrapping and scroll metrics |
| Chatter or recorded voice | `audio.tua` plus an injected backend; [focused audio example](https://github.com/kyonru/tua-lang/tree/main/examples/dialogue/audio) |
| Non-pausing messages | A separate player; optionally borrow the bounded host queue |
| Headless testing | `controller.tua` with fake backends, executed by `tests/controller.lua` |

The example's fonts and Sources are created by the LÖVE host and released on
shutdown. Reduced motion disables wave/shake and completes text immediately.
Subtitles and keyboard focus are explicit controls. This is not a screen-reader
bridge or a claim of bidirectional layout/full text shaping.

### Presentation inspiration and inline images

[SYSL-Text](https://github.com/sysl-dev/SYSL-Text) is useful inspiration for named
presentation styles, per-textbox defaults, wrapping/scrolling, animated text and
character sounds. Beacon Harbor demonstrates those ideas with the existing Tua
run/reveal contracts; it does not embed SYSL-Text or execute inline function tags.
Its `wave`, `warning` and `radio` style names are host mappings, not new core tags.
Unknown style names fall back to ordinary readable text. Actions stay declared in
`.tdialogue` and are dispatched through typed handlers.

Inline icons/images are intentionally **not added to the grammar** in the current dialect. Before adopting them, a host should define all of the following:

- Intrinsic size, baseline, line-height contribution, wrapping and missing-asset behavior.
- A localized spoken/plain-text alternative that remains meaningful in history,
  subtitles, translation extraction and renderers without image support.
- Grapheme/reveal and voice timing: whether an icon consumes a reveal step, and
  what happens on instant reveal, skip and restore.
- An allowlisted resource mapping and explicit ownership; no file paths or code
  execution in untrusted prose.

For now, use a character portrait or render a host icon beside an ordinary localized
word such as “token.” A decorative icon can be omitted; a meaningful one must not
replace the only readable description of the action. Any future core syntax needs
separate layout, localization, accessibility and lifecycle tests.

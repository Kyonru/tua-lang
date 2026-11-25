# Beacon Harbor

One dialogue player contract, three interchangeable LÖVE presentations. Run from
this directory:

```sh
tua fmt --check
tua check
tua build
love build
```

No game assets or third-party graphics are required. Portraits and scenery are
procedural; the original synthetic WAVs are test fixtures, not recorded actors.

| Key | Action |
| --- | --- |
| 1 / 2 / 3 | Conversation panel / NPC bubble / radio presentation |
| Space / Enter | Reveal immediately, advance or choose |
| Up / Down | Focus a reply (disabled replies explain why) |
| Page Up / Page Down | Scroll the conversation |
| F | Normal / 3× text reveal speed |
| M | Reduced motion and instant reveal |
| S / V | Subtitles / voice mute (voice clock continues) |
| L | English / French templates |
| P / R | Pause both channels / reset conversation and queues |
| Q / N | Queue a conversation / non-pausing announcement |
| F5 / F9 | Capture / restore an in-memory checkpoint |
| Escape | Quit and release owned resources |

Panel conversations pause the little harbor simulation and suspend radio. Bubble
and radio presentations leave it running. Manual pause freezes both players and
sound. Two conversation and four announcement **waiting** slots reject new items
when full; active items are additional. Priority, suspension, overflow and reset
are policies in `controller.tua`, not rules imposed by the engine.

`scene.tdialogue` demonstrates shared characters and expressions, rich localized
placeholders, a disabled choice, typed host actions, shown/revealed events, a wait,
chatter, recorded voice and a bounded eight-entry / 2 KiB transcript. Setting
`host.player():updateContext({ name = "Ada", permit = false })` disables the token
reply. The default permit is granted. Radio has its own player and audio controller.

The token action is session-local demonstration state. F5/F9 restores a settled
conversation into a fresh player without reissuing that action. This is **not** a
crash-safe game-save transaction; use the independent [durable example](../durable/README.md)
for persisted gameplay effects, receipts and recovery. Settings and checkpoint
remain in memory; reset deliberately clears queues and transcript, not earned tokens.

## Verify

```sh
python3 verify.py --tua ../../../target/release/tua
python3 verify.py --tua ../../../target/release/tua --love /path/to/love
```

The headless oracle runs the emitted controller on Lua. The optional native replay
uses the same controller, real fonts/Sources, 640×480 and 1280×720 windows, multiple
players, both languages, keyboard choices, pause, bounded queues, restore and
repeated resets. It requires a new output directory (see `--output`); `--video`
adds frame capture and a silent MP4 when FFmpeg is available. The native audio
example separately tests its shared source-pool design with JIT enabled and disabled.

## Adopt only what you need

- **A simple panel:** load a script with `Dialogue.new`, present `snapshot.node.text`,
  then `advance`/`choose` explicitly. None of this example's audio or queues is required.
- **Styled reveal:** use `Dialogue.reveal` and map semantic run names in your own
  presenter. `presenter.tua` owns fonts, wrapping, colors, shadows and optional wave/shake.
- **Radio:** use a separate player and let gameplay continue; borrow the bounded
  queue and priority policy only if your host needs it.
- **Audio:** `audio.tua` consumes snapshots and an injected backend. `backend.tua`
  owns ten native Sources across two players. No resources are loaded per frame.
- **Headless:** require `controller` with fake backends; neither controller nor story
  needs graphics or the `main` module. See `tests/controller.lua`.

The presenter illustrates grapheme-aware wrapping and explicit keyboard focus. It
does not implement a screen-reader bridge, bidirectional layout or full shaping.
Radio is intentionally concise; longer interactive content belongs in the scrollable
conversation presenter. Host actions are declared; rich text never executes code.

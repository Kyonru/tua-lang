# Dialogue: runnable headless examples

Run conversations, choices, shared characters and timed events without a LÖVE
window. This example uses the development dialogue feature intended for the
release after 0.11.0.

## Run it

From this directory, select your rebuilt development compiler:

```sh
../../../target/release/tua check
../../../target/release/tua build
cd build
lua main.lua
```

The host prints `Ready?` and then uses assertions to check the remaining behavior.
A successful run exits without an assertion error. No image or audio assets are
required: portrait paths and sound events are data until a graphical host handles
them. Generated Lua is not committed.

## Read the example in this order

| File | What it demonstrates |
| --- | --- |
| [intro.tdialogue](src/intro.tdialogue) | Inline character, two choices, a loop and an acknowledged final line |
| [main.tua](src/main.tua) | Starting players, reading snapshots, choosing by ID and advancing clocks |
| [characters.tua](src/characters.tua) | Shared typed names and an optional portrait |
| [arrival.tdialogue](src/arrival.tdialogue) | Shared character, hold/fade hints and both event timing anchors |
| [departure.tdialogue](src/departure.tdialogue) | Reusing the cast across consecutive lines |

`require("intro")` imports a typed document. Using `Dialogue` automatically
includes the player runtime. A renderer can consume the same snapshots; guard
the optional `node` before reading its fields.

## Try a change

- Change Guide's display name in `characters.tua`, rebuild and rerun. Both shared
  scenes read that export. Update the host's expected-name assertions too.
- In `arrival.tdialogue`, compare `@event shown 0.2 portrait.pulse` with
  `@event revealed 0.25 sound.play "welcome"`. The host explicitly reports reveal
  completion between these two events.
- Use Rename Symbol on `[start]` in `intro.tdialogue` to update its local targets.
  Rename uses the full label and leaves text IDs/prose alone. Update any host-side
  entry strings separately; they are not inferred rename references.
- Change an event delay and adjust the corresponding `update()` interval to see
  when it becomes due. Merely passing time to `shown` does not start `revealed`.
- Type `Cast.` at a speaker position in the updated extension to see completion;
  hover or go to definition to inspect the shared character.

`portrait.pulse` and `sound.play` are example host-defined names, not built-in
commands. The host asserts the returned records; it does not animate or play audio.
The portrait path `portraits/guide.png` is illustrative and is not loaded here.

`tua watch` rebuilds changed scripts. Invalid edits retain the previous valid
output, and text IDs must be unique across project dialogue files. Restart this
host after a rebuild to use the new document.

See the [dialogue guide](../../../docs/dialogue.md) for complete authoring examples,
event dispatch, timing rules, pause policies, translations and the API reference.
With configured localization, text-ID rename also updates catalog keys and typed
localization calls in the same project. Computed host strings and action identity
migrations remain manual. Declare `@entry label` in a script header for sections
started directly by the host; this suppresses unreachable-section warnings without
changing the default entry. See [reachability diagnostics](../../../docs/dialogue.md#diagnose-unreachable-sections).

## Compiler frontend harness

For frontend development, compile one script from the repository root:

```sh
cargo run -q -p tua-core --example dialogue_compile -- examples/dialogue/basics/src/intro.tdialogue > /tmp/intro-dialogue.lua
```

After building this example, run the standalone data host from this directory:

```sh
lua asset_host.lua /tmp/intro-dialogue.lua
```

The harness uses the same bundled player. Its generated output stays outside
source control.

Use `tua fmt` (or `tua fmt src/intro.tdialogue`) to align the dialogue structure
without rewriting prose, escapes, IDs or choice order. `tua fmt --check` validates
the layout, and editor Format Document uses the same implementation. Incomplete
scripts are left unchanged; dialogue range/on-type formatting is not enabled.

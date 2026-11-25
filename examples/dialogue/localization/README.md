# Localized dialogue

From this directory, using the newly built Tua compiler:

```sh
tua check
tua build
(cd build && lua main.lua)
tua dialogue-extract > /tmp/dialogue-source.json
```

The headless example uses two simultaneous players, English/French plural lines,
unexpanded template selection with fallback/plural metadata, a shared localized character, a mid-line locale change, timed events and stable
choice IDs. French intentionally omits `harbor.bye`: expect coverage warnings and
the English fallback at runtime. No images, audio or private game assets are needed.

Open `src/scene.tdialogue` and run **Tua: Show Localization Explorer** to compare
source prose, translations and missing coverage. Hover/definition on text IDs
connect scripts to catalog entries. Copy the snapshot into any custom renderer;
on `presentationRevision` changes honor `revealComplete` to show refreshed text fully.

See [the dialogue guide](../../../docs/dialogue.md) for options, ownership, fallback,
argument validation, extraction/merge policy and runtime replacement requirements.

To deliberately change a text identity, use Rename Symbol on an ID in
`src/scene.tdialogue`, entering a new final key segment. Review the workspace edit:
it includes matching locale keys and recognized typed localization calls, without
changing dialogue prose or flow labels. Find References can show this relationship
before renaming. Existing destination keys and duplicate IDs are rejected; finish
malformed script edits before refactoring.

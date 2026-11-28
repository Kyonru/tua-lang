---
icon: lucide/users
---

# Characters, portraits and audio

Reuse a cast across scenes, select expressions and describe voice cues. The host owns actual image loading, animation and audio playback.

[Dialogue overview](index.md)

## Share characters and portraits { #share-characters-between-scripts }

Use an ordinary typed module when several scenes share a cast.

`src/characters.tua`:

```tua
type Character = { name: string, portrait?: string }

local Guide: Character = {
  name = "Harbor guide",
  portrait = "portraits/guide.png",
}

-- A character can have a name without a portrait.
local Captain: Character = { name = "Captain" }

return { Guide = Guide, Captain = Captain }
```

`src/arrival.tdialogue`:

```text
@dialogue 1
@characters Cast "characters"

[start]
// @id arrival.welcome
Cast.Guide: Welcome to the harbor. (end)
@timing hold 3 fade 0.35
@event shown 0.2 portrait.pulse
@event revealed 0.25 sound.play "welcome"
```

`src/departure.tdialogue` can import the same characters:

```text
@dialogue 1
@characters Cast "characters"

[start]
// @id departure.farewell
Cast.Guide: Safe travels.
// @id departure.ready
Cast.Captain: All aboard! (end)
```

`Cast` is a script-local alias. For a nested file such as
`src/dialogue/characters.tua`, use `@characters Cast "dialogue.characters"`.

In the updated extension, typing `Cast.` at a speaker position suggests character
exports. Hover shows their types and comments; go-to-definition opens the typed
module. These features also work with unsaved character edits.

### Read character metadata in your renderer

```tua
local document = require("arrival")
local player = Dialogue.new(document)
player:start()

local node = player:snapshot().node
if node then
  print(node.speaker or "Narrator")
  print(node.text)
  local portrait = node.portrait
  if portrait then print("Portrait asset:", portrait) end
end

player:dispose()
```

A portrait is an **asset identifier string**, not a loaded image. The example can
print `portraits/guide.png` without that file existing; a graphical host must
provide the asset and load it. Tua does not select a portrait layout, image loader
or animation system.

Each used character must have a required string `name`; `portrait` may be a string
or nil. Named `expressions` are described below. Other fields remain available in the character module but are not copied
into dialogue snapshots. Type aliases and module re-exports work normally.

Character metadata is read when the emitted module loads. Each player then owns
a document copy. Editing a character does not mutate existing players or reload
Lua's module cache; rebuild and recreate/reload the relevant state in your host.

## Select character expressions

Declare named expression records on a shared character. Each expression needs a
nonempty `portrait` asset ID and may name an `animation` for the host's adapter.
Keep the record fields visible in its type (or use inference) so the editor can
suggest the names. A widened dictionary cannot describe a closed set of names.

```tua
return {
  Guide = {
    name = "Guide",
    portrait = "portraits/guide.png",
    expressions = {
      happy = { portrait = "portraits/guide-happy.png" },
      talking = { portrait = "portraits/guide-sheet.png", animation = "talk" },
    },
  },
}
```

Place `@expression` after its utterance and before any choices. It works on an
imported character, including a re-exported character; narration and local
`@character` declarations have no expression registry.

```text
@dialogue 1
@characters Cast "characters"
[start]
// @id greeting.happy
Cast.Guide: Welcome!
@expression happy
// @id greeting.default
Cast.Guide: Back to my default portrait. (end)
```

`snapshot().node` exposes `expression`, the resolved `portrait`, and optional
`portraitAnimation`. Omission resets the next line to the character's default
portrait, or no portrait if there is none. Selection never persists implicitly
between lines, speakers, or players. Repeat the directive when another line needs
the same expression. Locale changes preserve selection on the current line.

Unknown names and incompatible selected metadata types produce authoring diagnostics.
Nonempty-string constraints validate at construction. Dynamic Lua documents also
validate there and reject unknown names rather than silently falling back. The selected portrait and animation strings are copied into each
player; later edits to the shared record or a returned snapshot cannot change an
existing player. No sprite timer or GPU resource is stored in the shared cast.

Expression name completion works while typing a directive. Hover shows its metadata
type; go-to-definition opens the owning character declaration, including re-exports.
These queries inspect source facts and do not load images. Asset IDs remain opaque
strings: the host resolves them and handles missing files or unsupported animations.
The compiler does not prove that an animation name exists in an arbitrary host
registry. Existing sprite-sheet calls retain their own animation-name checking.

Scripts selecting expressions emit schema **7**, requiring
`Dialogue.schemaVersion >= 7`. Portrait-only scripts keep their previous schema,
and the new runtime accepts older schemas. See the runnable
[expression example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/expressions)
for two speakers, two players, and reset behavior.

### Optional static-image adapter

Load/cache images in the host, then use the resolved asset ID. This LÖVE recipe
assumes the named image exists; adapt missing-asset handling to your game:

```tua
local images = {
  ["portraits/guide-happy.png"] = love.graphics.newImage("portraits/guide-happy.png"),
}

local node = player:snapshot().node
if node then
  local portrait = node.portrait
  if portrait then
    local image = images[portrait]
    if image then love.graphics.draw(image, 24, 24) end
  end
end
```

Share cached image resources if useful. Keep layout, tint, and transitions local to
each dialogue presentation. Image lifetime and cleanup remain the host's responsibility.

### Optional animated adapter with `spriteSheet`

Use an ordinary [sprite sheet](../sprites.md) and create a **separate sprite player
for each dialogue presentation**. The following recipe assumes a sheet containing
four 32 × 32 frames in its first row; it is not a required dialogue runtime:

```tua
local GuideSheet = spriteSheet({
  image = "portraits/guide-sheet.png",
  frame = { width = 32, height = 32 },
  animations = { talk = { from = 0, to = 3, row = 0, fps = 8 } },
})

local leftPortrait = GuideSheet:player("talk")
local rightPortrait = GuideSheet:player("talk")

-- In the left presentation's update, after a new snapshot is accepted:
local node = player:snapshot().node
if node and node.portrait == "portraits/guide-sheet.png" then
  if node.portraitAnimation == "talk" then
    leftPortrait:play("talk", true)
  else
    leftPortrait:stop()
  end
end

-- In love.update(dt), advance each active presentation independently.
leftPortrait:update(0.1)
rightPortrait:update(0.05)
-- In love.draw(), draw only the active animated presentations.
leftPortrait:draw(24, 24)
rightPortrait:draw(200, 24)
```

Call selection/restart once when `(generation, visit, presentationRevision)` changes,
not on every frame; otherwise `play(..., true)` continually restarts the animation.
Hide or discard the previous sprite player when the new snapshot has no animated
portrait, or when the dialogue cancels/disposes. Switching to a static expression
must also stop showing the old animation. The host may choose whether a locale-only
presentation revision restarts animation. Never cache sprite players on characters;
only cache the sheet/image resources. This keeps simultaneous conversations independent.

## Describe voices and character chatter

Audio descriptions are optional schema-10 data. Tua validates and owns them; the host
loads assets, selects samples, plays/stops sources, and decides how to handle missing
audio. Subtitles remain available with no audio system or LÖVE dependency.

Declare a shared profile in the character module:

```tua
return {
  Guide = {
    name = "Guide",
    speech = Dialogue.speechProfile({
      sampleSet = "guide.chatter",
      pitchMin = 0.9,
      pitchMax = 1.1,
      variation = 0.2,
      gain = 0.6,
      minCueInterval = 0.05,
      cadence = "character",
    }),
  },
}
```

The constructor provides typed fields and validates numeric bounds at runtime.
Pitch limits must be ordered within 0.25–4; variation and gain are 0–1; the minimum
cue interval is 0–60 seconds. `sampleSet` is a nonempty host identifier, at most
256 UTF-8 bytes. Cadence is `"character"` or `"word"`.

Choose the profile explicitly on a line:

```text
@dialogue 1
@characters Cast "characters"
[start]
// @id greeting
Cast.Guide: Welcome.
@speech character
@voice {"cue":"guide.greeting","sourceLocale":"en","fallback":"silent"}
// @id aside
Cast.Guide: A quiet aside. (end)
@speech off
```

`@speech character` requires an imported character with a valid, required `speech`
profile. Missing or incorrectly typed profiles fail checking; missing dynamic
profiles fail module loading. Use `@speech off` to mark explicit silence, or provide
a complete JSON profile to override the shared one:

```text
@speech {"sampleSet":"guide.soft","pitchMin":0.8,"pitchMax":1.0,"variation":0.1,"gain":0.3,"minCueInterval":0.2,"cadence":"word"}
```

Each directive follows its utterance before choices and occurs at most once per line.
Omission means no speech preference; metadata never carries over from another line.
Explicit selection preserves earlier scripts' schema selection. Scripts with either
audio directive emit schema 10, which also supports prior waits, rich text, actions
and expressions. Older replacement runtimes need schema-10 support for these scripts.

Typed line records accept `speech = profile`, `speech = "off"`, and
`voice = { cue = "guide.greeting", sourceLocale = "en", fallback = "silent" }`.
Snapshots expose owned `node.speech` and `node.voice` records. Mutating a source
profile or returned snapshot cannot change a running player's descriptions.

### Voice locale and fallback policy

A voice cue is a logical asset identifier (1–256 bytes), not a path that Tua opens.
`sourceLocale` names its source recording locale (1–64 bytes). Identifiers may not
contain ASCII control characters; locale tags are opaque identifiers, not normalized.

The voice snapshot adds `locale`: the player's requested locale, or `sourceLocale`
when no locale was selected. This remains independent of subtitle catalog fallback.
The host first attempts the cue in that requested locale:

- `fallback = "silent"`: if unavailable, show subtitles without recorded voice.
- `fallback = "source"`: if unavailable, try the source-locale recording; otherwise
  remain silent.

The host determines availability and applies the policy; Tua does not claim a cue
exists or start playback. Locale refresh updates the descriptor without emitting an
audio command. Use player generation/visit/presentation identity to avoid replaying
a recording unintentionally.

`mixSpeech` is optional and defaults to false. Adapters should suppress chatter
**while recorded voice is actually playing**, unless mixing is explicitly enabled.
A missing recording does not by itself suppress available chatter. `@speech off`
always requests no chatter; it does not disable a separately declared recorded cue.
Rich `[pronounce="hint"]…[/pronounce]` runs provide existing pronunciation metadata;
hints are not synthesized or interpreted as commands by the core.

### Adapter responsibilities

Profiles are descriptors in this slice: they do not automatically configure
`Dialogue.reveal` or produce sound. Its existing cues remain grapheme-based.
Adapters must map cadence (including language-appropriate word segmentation), apply
profile intervals/gain/pitch variation, and use private deterministic RNG rather
than gameplay RNG. They also own volume/mute, bounded polyphony, missing-asset
fallback, audio suspension, skip policy and lifecycle cleanup. The optional
[dialogue audio example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/audio)
implements these responsibilities with typed host code and ordinary LÖVE Sources.
It includes two character voices, fixed source pools, a private pitch RNG, recording
locale fallback, mute, pause/resume, silent finish and lifecycle cleanup. Replace
its original synthetic recording fixture with your licensed voice acting.

Run `tua build` then `love build` inside `examples/dialogue/audio`. Space finishes
text while keeping the recording, then advances and stops it. P pauses/resumes,
M mutes chatter, V mutes recordings, and R restarts. This is a copyable host policy,
not a new required audio engine. Its ASCII word-boundary callback is replaceable
for other languages; pronunciation metadata is not synthesized speech. For pending actions, use the explicit [host skip policy](player.md#choose-an-advance-and-skip-policy) instead of bypassing their state.

The [character presentation example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/expressions)
includes shared profiles, voice locale metadata, overrides, silence and plain-line reset.

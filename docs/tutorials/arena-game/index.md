---
icon: lucide/gamepad-2
---

# Build an arena game

Build one game a chapter at a time. Begin with an animated character and a run you
can pause or restart. Add combat, enemy decisions, sound, shaders and a localized
between-wave conversation to that same scene. Save cleared-wave progress and preferences;
then inspect and measure the same game with reproducible workloads.

Each chapter has a **self-contained runnable checkpoint**. Start from its
source, experiment, then carry what you learned into the next chapter. No imports
from other examples or special tutorial framework are required.

| Chapter | What you can play |
| --- | --- |
| [1. The playable loop](playable-loop.md) | Start a run, move an animated character, pause, resume and restart |
| [2. Combat and waves](combat-waves.md) | Aim, fire, damage enemies and clear waves |
| [3. Enemy decisions](enemy-decisions.md) | Survive enemies that patrol, chase and attack |
| [4. Give the game sound](sound.md) | Add cues, music scenes and independent volume controls |
| [5. Scoped shader feedback](visual-feedback.md) | Load, initialize and safely scope a hit shader |
| [6. Conversation and language](conversation-and-language.md) | Talk between waves and change the UI/dialogue locale |
| [7. Save progress and preferences](save-progress.md) | Resume a cleared wave, remember settings and handle save recovery |
| [8. Profile the game](profile-the-game.md) | Inspect live state, capture hot paths and compare repeatable workloads |

You need [Tua and LÖVE](../../installation.md), basic Lua-style functions and tables,
and a checkout of the repository for the bundled source and assets. Character art
is Kenney CC0. The scene stays deliberately simple so you can see what each system
does. Small API-only examples remain on their feature reference pages.

## Follow the series

Start with chapter 1 and run its complete checkpoint before changing anything. Each
chapter explains what to add to the previous game and links to the finished source.
The code excerpts explain those files; they are not additional modules to paste into
the finished checkpoint. Compare your changes with the next folder as you progress.

Every checkpoint owns its assets and typed source. You can copy one independently;
keep its `tua.toml` and `src/` together. Build output is generated, so edit `.tua` files
and rebuild. Playing needs Tua and LÖVE; Python, Node and the regression commands in
example READMEs are optional contributor tools, not prerequisites for the first game.

The chapters gradually add controls and change N from immediate wave spawning to a
conversation. Read each chapter's controls before playing. Chapters 7 and 8 share the
`tua-arena-tutorial` save identity and schema, so they can resume the same progress.
Use the confirmed reset UI, or choose another LÖVE identity for an independent experiment.

After chapter 8, keep the small default wave and choose one change to explore: new
behavior, another weapon, or a different presentation. Reuse the existing lifecycle
and outcome checks before treating a faster measurement as a successful change.

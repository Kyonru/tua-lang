---
icon: lucide/gamepad-2
---

# Build an arena game

Build one game a chapter at a time. Begin with a character you can move and a run
you can pause or restart. Chapter 2 adds weapons and enemies to that
same scene. Later chapters are planned for enemy decisions, sound, shaders,
conversation, localization, saving and profiling.

Each finished chapter has a **self-contained runnable checkpoint**. Start from its
source, experiment, then carry what you learned into the next chapter. No imports
from other examples or special tutorial framework are required.

| Chapter | What you can play |
| --- | --- |
| [1. The playable loop](playable-loop.md) | Start a run, move an animated character, pause, resume and restart |
| [2. Combat and waves](combat-waves.md) | Aim, fire, damage enemies and clear waves |

You need [Tua and LÖVE](../../installation.md), basic Lua-style functions and tables,
and a checkout of the repository for the bundled source and assets. Character art
is Kenney CC0. The scene stays deliberately simple so you can see what each system
does. Small API-only examples remain on their feature reference pages.

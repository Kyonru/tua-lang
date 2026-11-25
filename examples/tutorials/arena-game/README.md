# Build an arena game

Follow one evolving game through runnable chapter checkpoints. Each folder includes
its source, assets and tests and can be copied independently. Later chapters extend
the previous one; there is no shared hidden framework or separate `latest` copy.

| Chapter | Outcome | Status |
| --- | --- | --- |
| [01: the playable loop](checkpoints/01-playable-loop/README.md) | Move, animate, follow, pause and restart | Runnable |
| [02: combat and waves](checkpoints/02-combat-waves/README.md) | Fire projectiles at live enemies; manage hits, waves and feedback | Runnable |
| [03: enemy decisions](checkpoints/03-enemy-decisions/README.md) | Patrol, chase, attack, survive and restart after game over | Runnable |
| [04: sound](checkpoints/04-sound/README.md) | Hear cues/music, adjust buses, pause and mute | Runnable |
| [05: visual feedback](checkpoints/05-visual-feedback/README.md) | Scope hit shaders, preload and recover from errors | Runnable |
| [06: conversation and language](checkpoints/06-conversation-and-language/README.md) | Talk between waves and switch English/French presentation | Runnable |
| [07: save progress](checkpoints/07-save-progress/README.md) | Resume cleared waves and restore preferences with explicit recovery | Runnable |
| [08: profile the game](checkpoints/08-profile-the-game/README.md) | Inspect bounded workloads and compare a measured allocation change | Runnable |

Start in `checkpoints/01-playable-loop`, then run `tua build` and `love build`.
Read the [series guide](../../../docs/tutorials/arena-game/index.md).
Existing standalone examples remain supported while this series is assembled.

## Continuous qualification

From the repository root, run `make arena-tutorials-check` to build the current
compiler and check every chapter from a fresh source copy. It runs formatting,
warning-free type checking, emission and the headless Lua suites. Use
`LUA=luajit` to match the Linux CI gate. Each invocation keeps its report and logs
under a fresh `target/arena-tutorials/run.*/report` directory.

CI uploads these logs on success or failure. This headless gate does not run native
LÖVE, GPU, audio, save-process, inspector or performance checks. For full native
qualification, run from repository root:

```sh
python3 scripts/qualify-arena-tutorials.py --tua /path/to/tua \
  --lua luajit --love /path/to/love --output /new/report/path
```

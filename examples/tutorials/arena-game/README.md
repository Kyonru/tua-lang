# Build an arena game

Follow one evolving game through runnable chapter checkpoints. Each folder includes
its source, assets and tests and can be copied independently. Later chapters extend
the previous one; there is no shared hidden framework or separate `latest` copy.

| Chapter | Outcome | Status |
| --- | --- | --- |
| [01: the playable loop](checkpoints/01-playable-loop/README.md) | Move, animate, follow, pause and restart | Runnable |
| [02: combat and waves](checkpoints/02-combat-waves/README.md) | Fire projectiles at live enemies; manage hits, waves and feedback | Runnable |
| [03: enemy decisions](checkpoints/03-enemy-decisions/README.md) | Patrol, chase, attack, survive and restart after game over | Runnable |

Start in `checkpoints/01-playable-loop`, then run `tua build` and `love build`.
Read the [series guide](../../../docs/tutorials/arena-game/index.md).
Existing standalone examples remain supported while this series is assembled.

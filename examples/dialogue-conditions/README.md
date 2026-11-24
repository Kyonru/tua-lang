# Conditional dialogue

Use a compiler containing M3:

```sh
tua check
tua build
(cd build && lua main.lua)
```

The shop demonstrates typed branches, optional facts, hidden versus disabled replies,
localized-reason IDs, explicit no-choice fallback, stale UI rejection and bounded
branch-loop faults. The example asserts navigation; the game still performs and
validates purchases. Renderer, clocks, audio and input remain host-owned.

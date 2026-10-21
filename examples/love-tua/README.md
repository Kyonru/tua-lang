# LOVE + Tua Showcase

This small bullet-hell loop keeps ordinary Lua and LOVE structure visible while
showing where Tua adds value:

- ECS owns the player's typed position, velocity, and rotation.
- An object pool reuses typed projectiles.
- A store owns score, shot, and wave state.
- A state machine checks pause/resume transitions.
- Baton-backed input derives control and movement-pair names.
- `ecs.tua`, `global.tua`, `renderer.tua`, and `types.tua` demonstrate typed
  module exports shared by the main LOVE callbacks.
- `patterns.lua` stays handwritten Lua and is copied unchanged.
- `lib/feel/init.lua` stays a vendored Lua runtime while
  `lib/feel/types/feel.lua` supplies declaration-only editor and checker types.

Build and run it from the repository root:

```bash
make check-love-tua
make run-love-tua
```

Generated Lua is written to the ignored `build/` directory.

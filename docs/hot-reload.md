---
icon: lucide/flame
---

# Hot Reload

`tua dev` builds the project, starts LOVE on the output, and keeps both in
sync: every time you save, it rebuilds and the running game picks up the new
code without restarting.

```bash
tua dev
```

```text
watching src/**/*
initial build: built 6 file(s)
dev: started love build
changed 1 file(s)
rebuild: rebuilt 1 file(s)
dev: sent reload 2 to the game
[tua dev] reloaded enemy
```

In VS Code, run **Tua: Run LOVE Project With Hot Reload**, or use its flame
button in a Tua editor's title bar. It runs `tua dev` in a task with the
`tua.love.command` executable.

`tua dev` looks for `love` on `PATH`, then `/Applications/love.app` on macOS.
Set `TUA_LOVE` or pass `--love PATH` to use another executable. Closing the
game ends `tua dev`.

## What Reloads

| You change | The game |
| --- | --- |
| A module loaded with `require` | Swaps in the new code and keeps running. |
| A shader loaded by a module | Compiles it first, then reloads its loading modules even when their Lua is unchanged. |
| `main.tua` or `conf.tua` | Restarts, since their top-level code sets the game up. |
| A file with errors | Keeps running the last good build; `tua dev` prints the diagnostics. |

A module reload runs the new file and copies its functions into the table the
game already has. Code that captured the module, and objects whose metatable
is a class from it, use the new functions straight away. Values already in the
table, such as `M.score` or a cached list, are kept, so the game's state
survives the edit.

!!! warning "Keep state where reloading can see it"
    The new functions run with the new file's local variables. A module-level
    `local enemies = {}` starts empty again after a reload, while
    `M.enemies = {}` keeps its contents. Keep state that should survive edits in
    module tables, a [store](stores.md), or your game objects.

A module that returns a function rather than a table is replaced; callers that
already captured the old function keep it until they look it up again.

## Shader Edits

Saving a shader referenced by `shader "path.glsl"` or a literal
`love.graphics.newShader("path.glsl")` reloads the loaded modules that use it.
The runtime compiles the changed source inside the running LOVE process first.
A compilation error appears in the console and overlay, while the last working
shader and game keep running. Saving a fix retries the reload. Shaders loaded
by `main.lua` or `conf.lua` trigger a restart only after successful validation.

As with code reloads, module-level local state resets. Shader objects exported
in module tables are replaced; callers that cached the old object directly
keep that object, so look up the module's shader or call its drawing function.
Dynamic filenames and assets outside the copied source tree are not tracked.

## Errors While Playing

An error in `love.update` or `love.draw`, including one introduced by a
reload, does not end the session. The game pauses, shows the message and
stack trace in a red bar, and resumes after you save a fix. Errors in other
callbacks use LOVE's normal error screen.

## How It Works

After each error-free build, `tua dev` writes `tuahotreload.lua` into the
output, appends `require("tuahotreload").install()` to the end of
`main.lua`, and rewrites `.tuahotreload`. The game checks that file a few
times a second and compares each loaded module's source with what it last
loaded. The signal also carries the full shader dependency map, so a game
that misses an intermediate generation still detects shader edits. Because the hook is appended, source-map line numbers stay correct.

`tua build` and `tua package` regenerate `main.lua` without the hook, so a
leftover `tuahotreload.lua` in the output is never loaded, and packages never
include it.

---
icon: lucide/package
---

# Packaging Games

`tua package` builds the project and packages the output for players. It
always writes a `.love` archive, and it can also fuse a standalone Windows or
macOS build with a LOVE runtime you provide.

```bash
tua package
```

```text
built 6 file(s)
packaged dist/my-game.love (184.2 KiB)
```

Packages go to `dist/` in the project root. `tua init` adds this directory to
`.gitignore`.

In VS Code, **Tua: Package Game...** asks which package to build and runs the
same command in a task. For fused builds it uses `tua.love.windowsRuntime` or
`tua.love.macosApp`, and asks for the runtime when the setting is empty.

## What Goes In

The archive holds the build output directory: generated Lua, copied assets,
bundled runtimes such as `tuatween.lua`, and your `conf.lua`. These are left
out:

- source maps (`.lua.map` and `.lua.map.json`), which only the debugger uses;
- Tua sources and `tua.toml`, for projects that build in place;
- `dist/` itself, hidden files, and symbolic links.

The output must contain `main.lua` at its root, which LOVE runs. Entries are
stored uncompressed, since images and audio are already compressed, and carry
a fixed timestamp, so the same build produces a byte-identical archive.

`--name` sets the file name; it defaults to the project directory's name.

```bash
tua package --name "Star Dust"
```

## Windows

Download the 64-bit zipped LOVE 11.5 for Windows from
[love2d.org](https://love2d.org), extract it, and pass that directory:

```bash
tua package --target windows --love ~/Downloads/love-11.5-win64
```

```text
dist/windows/my-game/
  my-game.exe     love.exe with my-game.love appended
  SDL2.dll, OpenAL32.dll, love.dll, lua51.dll, mpg123.dll, msvcp120.dll, msvcr120.dll
  license.txt
```

Zip the `my-game` folder to distribute it. The executable still carries the
LOVE icon and version details; change them with a resource editor if you need
to.

## macOS

Pass LOVE's `love.app`:

```bash
tua package --target macos --love /Applications/love.app --identifier com.example.mygame
```

This writes `dist/macos/my-game.app`, a copy of `love.app` with `my-game.love`
in `Contents/Resources`. Its `Info.plist` is renamed to the game, uses the
`--identifier` bundle identifier (default `io.tua.<name>`), and no longer
claims `.love` files.

Changing `love.app` invalidates its signature. On a Mac, `tua package` signs
the result ad hoc so it runs on that machine. Before distributing it, sign it
with your Developer ID and notarize it, or players will see a warning that the
app is damaged or from an unidentified developer.

## Other Platforms

Players on Linux, or anyone with LOVE installed, can run the `.love` file
directly with `love my-game.love`. Mobile and web builds use LOVE's own
Android, iOS, and love.js toolchains, which take the `.love` archive as input.

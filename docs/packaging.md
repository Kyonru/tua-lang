---
icon: lucide/package
---

# Packaging Games

`tua package` builds the project and packages the output for players. It
always writes a `.love` archive, and it can also build a standalone Windows or
macOS app or a static web site with a runtime you provide.

```bash
tua package
```

```text
built 6 file(s)
packaged dist/my-game.love (184.2 KiB)
```

Packages go to `dist/` in the project root. `tua init` adds this directory to
`.gitignore`.

In VS Code, select the package icon in the **Tua → Run Configuration** view or
run **Tua: Package Game...** from the Command Palette. Tua uses the active
file's project; if several projects are open and no project file is active,
it asks which one to package. Then choose a target:

- **Any desktop (.love):** a portable archive for Linux, Windows, or macOS;
  players need LÖVE installed.
- **Windows app (.exe):** a fused app folder. Select an extracted LÖVE for
  Windows directory containing `love.exe` when prompted.
- **macOS app (.app):** a fused app bundle. Select `love.app` when prompted.
- **Web (HTML5):** a static site. Select the root of the LÖVE 11.5 standalone
  web player when prompted. You must serve the site with the headers below.

Desktop and web builds reuse `tua.love.windowsRuntime`, `tua.love.macosApp`,
or `tua.love.webPlayer` when set.
The package runs in a VS Code task. On success, the extension offers to reveal
the generated file or folder or copy its path. On failure, read the **Package
Game** task output for the compiler or packaging error. Files are written to
the selected project's `dist/` directory. The macOS signing and notarization
requirements below still apply before distribution.

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
Android and iOS toolchains take the `.love` archive as input.

## Web

Download the [LÖVE 11.5 standalone web player](https://github.com/2dengine/love.js)
and pass its root directory (the one containing `player.js` and `11.5/love.wasm`):

```bash
tua package --target web --web-player ~/Downloads/love.js --name my-game
```

This writes `dist/web/my-game/` with `index.html`, a content-named
`game-<hash>.love` archive, the player,
its licenses, and a `README.txt`. The web player is supplied separately;
Tua does not download it or include it in the extension. Only known runtime
files are copied, so other files in the player folder do not enter the build.
The archive name changes when its contents change, avoiding the player's
URL-based cache serving an old game after redeployment.

Serve the directory over HTTP(S), with these response headers:

```text
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
Content-Security-Policy: script-src 'self' 'unsafe-eval'
```

The upstream `.htaccess` is copied when present for Apache hosts configured
with `mod_headers` and `AllowOverride`. Other hosts need equivalent headers.
Opening `index.html` with `file://` is insufficient. Browser audio and
fullscreen need user interaction; LuaSocket and native desktop dependencies
may not work. Test your game in its target browsers before distributing it.

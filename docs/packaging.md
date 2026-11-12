---
icon: lucide/package
---

# Packaging Games

`tua package` builds the project and packages the output for players. It
always writes a `.love` archive. It can also build a standalone Windows or
macOS app, a static web site, or a configured custom package.

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
- **Custom targets:** named commands from `[[package_pipelines]]` in `tua.toml`.
  The picker reads their names from the project after you select it.

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
directly with `love my-game.love`. Android and iOS toolchains can use the
`.love` archive as input.

## Web

Download the [LÖVE 11.5 standalone web player](https://github.com/2dengine/love.js)
and pass its root directory (the one containing `player.js` and `11.5/love.wasm`):

```bash
tua package --target web --web-player ~/Downloads/love.js --name my-game
```

This writes `dist/web/my-game/` with `index.html`, a content-named
`game-<hash>.love` archive, the player, its licenses, hosting header files,
and a `README.txt`. The web player is supplied separately;
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

Tua includes `_headers` for static deployments on
[Netlify](https://docs.netlify.com/manage/routing/headers/) and
[Cloudflare Pages](https://developers.cloudflare.com/pages/configuration/headers/).
Publish `dist/web/my-game/` as the site's root so `_headers` is in the
published directory.
It keeps the player's `.htaccess` when present, otherwise writes one with the
headers above and the `application/wasm` MIME type. Apache must allow
`.htaccess` overrides and enable `mod_headers`; verify the policy actually
appears in HTTP responses. Other hosts need equivalent configuration.

### Test locally

The [miniserve](https://github.com/svenstaro/miniserve) static server accepts
the required headers directly. From the packaged directory:

```bash
cd dist/web/my-game
miniserve -i 127.0.0.1 --index index.html \
  --header 'Cross-Origin-Opener-Policy:same-origin' \
  --header 'Cross-Origin-Embedder-Policy:require-corp' \
  --header "Content-Security-Policy:script-src 'self' 'unsafe-eval'" .
```

Open `http://127.0.0.1:8080/`, then inspect the Network panel for these
headers and `Content-Type: application/wasm` on `11.5/love.wasm`.
The generated `README.txt` carries the same local test command with the site.
Opening `index.html` with `file://`, or serving it with a server that does not
send the headers, is insufficient.

### Other hosts

For a dedicated Nginx location serving this directory:

```nginx
add_header Cross-Origin-Opener-Policy "same-origin" always;
add_header Cross-Origin-Embedder-Policy "require-corp" always;
add_header Content-Security-Policy "script-src 'self' 'unsafe-eval'" always;
```

For a dedicated Caddy site:

```caddyfile
header {
  Cross-Origin-Opener-Policy same-origin
  Cross-Origin-Embedder-Policy require-corp
  Content-Security-Policy "script-src 'self' 'unsafe-eval'"
}
```

Configure your server's `.wasm` MIME type as `application/wasm`. The
[HTML section of the LÖVE project template guide](https://github.com/Oval-Tutu/bootstrap-love2d-project/blob/main/USAGE.md#html)
has additional deployment examples.

On itch.io, upload the web directory as an HTML game and enable
[SharedArrayBuffer support](https://itch.io/t/2025776/experimental-sharedarraybuffer-support)
in the game's Embed Options. `_headers` and `.htaccess` do not configure
itch.io's CDN. Test the actual hosted page, since the embedding policy and
browser behavior can differ from local hosting.

Browser audio and fullscreen need user interaction; LuaSocket and native
desktop dependencies may not work. Cross-origin isolation can also block
third-party resources that do not opt in. Test your game in its target browsers
before distributing it. [The player documents its limitations](https://github.com/2dengine/love.js).

## GitHub Actions Packaging

From an initialized Tua project, run:

```bash
tua actions init
```

This creates `.github/workflows/tua-game.yml` if it is absent. Review and
commit that file in your game repository. The generated workflow uses a pinned
Tua `v0.7.0` source tag and Cargo to install the CLI, then runs `tua check` and
`tua package --target love`. It does not need Tua on GitHub Packages, a
personal access token, or a separate Tua action. The pinned version supports
the generated portable `.love` workflow; update the pin only after verifying a
newer Tua release. Projects using language or configuration features introduced
after `v0.7.0` need a newer published Tua source revision for this workflow.

Every push and manual run uploads the `.love` file as an Actions artifact named
`love-game` for 14 days. Download it from the workflow run's **Artifacts** area,
or with the [GitHub CLI](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/download-workflow-artifacts?tool=cli):

```bash
gh run download RUN_ID -n love-game
```

A pushed game tag beginning with `v` also runs a release job. That job downloads
the artifact from the **same run** and attaches it to a **draft** GitHub Release.
Review the draft and publish it when ready. Only the release job receives
`contents: write`; ordinary builds have read-only repository permissions.
The command leaves any existing workflow unchanged, including your edits.

The workflow packages the portable `.love` target immediately. Windows,
macOS, web, mobile, and custom targets have separate runtime or toolchain
requirements. Add jobs for them using the [target commands above](#packaging-games)
and the relevant toolchains, then verify those jobs on GitHub-hosted runners.
For web builds, supply the standalone player and use a Tua CLI version with
`--target web`; the generated `v0.7.0` pin predates that target. GitHub Release
CLI archives can replace the source installation after a suitable version is
published and verified.

## Custom Package Pipelines

Add a named `[[package_pipelines]]` table to `tua.toml` when a
target needs its own packaging tool.
Tua builds the game and passes the `.love` archive to your command. The command
writes its finished files into an empty staging directory; Tua publishes that
directory at `dist/custom/<target>/<game-name>/` only after a successful exit.
If the command fails, times out, or leaves that directory empty, the previous package is
kept. Pipelines run only when you explicitly select their target; `check`,
`build`, and the default `tua package` command do not run them.

```toml title="tua.toml"
[[package_pipelines]]
name = "portable-port"
label = "Portable port"
command = ["python3", "scripts/package_port.py"]
timeout_ms = 300000
```

```bash
tua package --list-targets
tua package --target custom:portable-port --name "My Game"
```

The command runs from the project root without an implicit shell. A program
path containing `/` or `\` is resolved from that root; a bare program name
uses `PATH`. It receives these absolute paths and values through the
environment:

| Variable | Value |
| --- | --- |
| `TUA_PACKAGE_ARCHIVE` | Built `.love` archive |
| `TUA_PACKAGE_OUTPUT_DIR` | Empty staging directory for finished files |
| `TUA_PACKAGE_BUILD_DIR` | Generated Lua build directory |
| `TUA_PACKAGE_PROJECT_ROOT` | Project root and command working directory |
| `TUA_PACKAGE_NAME` | Sanitized game name |
| `TUA_PACKAGE_TARGET` | Configured pipeline name |

Arguments that are exactly `{archive}`, `{output}`, `{build}`, `{root}`, `{name}`,
or `{target}` are replaced with the matching value. This lets a CLI tool use
the handoff directly, for example
`command = ["port-packer", "{archive}", "{output}"]`. Other arguments are passed
literally; there is no shell expansion.

For a minimal script that proves the handoff, put this in
`scripts/package_port.py`. Replace the zip operation with the packaging steps
required by your target runtime:

```python
import os
from pathlib import Path
from zipfile import ZipFile

archive = Path(os.environ["TUA_PACKAGE_ARCHIVE"])
output = Path(os.environ["TUA_PACKAGE_OUTPUT_DIR"])
name = os.environ["TUA_PACKAGE_NAME"]
with ZipFile(output / f"{name}.zip", "w") as bundle:
    bundle.write(archive, arcname="game.love")
```

Names use ASCII letters, digits, `-`, and `_`; they must be unique. Commands
are arrays of program and argument strings, and `timeout_ms` defaults to five
minutes (maximum 30 minutes). `label` is optional and defaults to the name.
**Treat configured commands as executable code.**
The VS Code command requires a trusted workspace. Tua does not download a
port toolchain or assert that a `.love` archive runs unchanged on that port.

Projects you might investigate for custom targets include
[LÖVE Potion](https://github.com/lovebrew/lovepotion),
[LOVE-WrapLua](https://github.com/LukeZGD/LOVE-WrapLua),
[lovejs-player](https://codeberg.org/alexjgriffith/lovejs-player),
[LoveDos](https://github.com/rxi/lovedos), and
[Lutro](https://lutro.libretro.com/). Their APIs and packaging contracts differ.
[LÖVR](https://github.com/bjornbytes/lovr) is a separate VR framework and may
require code migration; the historical
[LoveFTW](https://bitbucket.org/T-BoneISS/l-veftw) project likewise needs
independent compatibility checks.

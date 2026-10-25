# README screenshot captures

These PNGs are unedited captures of the production-bundled Tua extension in an
isolated VSCodium window, at 1440×960 with the optional Indigo Carbon HC theme.
The source example is `examples/starfall`; the script adds a demo store and a
sprite declaration using `examples/features/src/assets/player.png`. The runtime
inspector receives real snapshots from the temporary game, not fabricated data.

From `editors/vscode`, with dependencies installed and `bin/tua` staged:

```sh
npm run build
VSCODE_EXECUTABLE_PATH=/Applications/VSCodium.app/Contents/MacOS/VSCodium \
  node scripts/readme-screenshots.js
```

Use `VSCODE_EXECUTABLE_PATH` for a different local VS Code/VSCodium executable.
LÖVE must be available through the extension's normal executable discovery.
`TUA_CAPTURE_OUTPUT=/tmp/tua-screenshots` writes review captures elsewhere.

The script uses a temporary profile/workspace, invokes the actual feature
commands, waits for populated previews, captures through loopback CDP, stops the
game, and removes its temporary workspace. It never changes your normal editor
settings or game project. A local graphical session is required. The temporary
game has its own save identity and an invisible game window.

Inspect every capture after regeneration. Keep graphs fully visible, expand the
runtime values, and check for diagnostics, loading states, errors, or clipped
controls. Keep README captions and alt text consistent with the captured UI.

# November 12 follow-up qualification

Environment: macOS arm64, VS Code 1.142.0 development host, bundled `examples/starfall` copied to a temporary project, current release-built Tua server. The prior comparison is the existing `editors/vscode/tua-0.7.0.vsix` (SHA-256 `094ecf7d460640f0ab3ca0eb04011bb332e024dfc306330f89ea69b1955fdc9a`). No user game project was opened or modified. The Starfall files are a bundled sample, so these results do not represent a full private game workspace.

## Status against the follow-up recommendations

| Recommendation | Current evidence | Status |
| --- | --- | --- |
| FUP-01: inferred class through two module hops | Core and LSP regressions, a direct LSP probe, and an installed host check already pass; see [FUP-01](FINDINGS.md). | Fixed before this pass. |
| Visible completion and previous version | Twelve typed-dot suggestions in Starfall were timed in both the current host and prior 0.7.0 package. The current host also logs the completion-provider entry and return. | Partly qualified; exact LSP wire-send/server-receive and paint isolation remain. |
| Real graph and non-PNG previews | Actual `tua/moduleGraph` output for a generated 121-module project matched rendered node/link counts and a module source URI. Valid WAV, Ogg, OTF, and GLSL assets were opened in the host. | Partly qualified; hand-authored large game fidelity and font visualization remain. |
| Longer inspector soak | A two-minute Starfall pilot and a fifteen-minute automated game run passed. | Qualified for this automated macOS workload; manual UI and other platforms remain. |

## Real-game editing found a distinct module export defect

In Starfall's `src/main.tua`, replacing `world.reset()` with an unsaved `world.` showed only methods and collection fields in completion. `world.width`, `world.height`, and `world.spawnTimer` were absent even though `world.width` hover reported a number. Before the fix, a direct `tua/moduleGraph` request listed later method assignments but omitted the fields from the returned local table initializer. The project graph only retained exports with a source span, and the HIR path did not provide those initializer-key spans. Commit `b594e38a` recovers exact initializer-key spans from the parsed table once per module and keeps the exports. A core module-index regression and LSP completion/edit and graph-span regressions pass. The same installed-host Starfall run now shows `width`, `height`, `spawnTimer`, and the existing methods and collections. This issue is separate from FUP-01.

## Visible completion comparison

Command (requires a working VS Code host and local loopback):

```sh
node internal/audits/feature-followup-2026-11-11/starfall-visible-run.js
TUA_STARFALL_BASELINE_VSIX=editors/vscode/tua-0.7.0.vsix node internal/audits/feature-followup-2026-11-11/starfall-visible-run.js
```

Each run copies bundled Starfall to a temporary workspace, edits an unsaved `world.` at the first `world.reset()` call, types a dot twelve times, and waits for matching visible suggestion labels. The first sample is cold; the other eleven are warm. Raw samples are in [current](starfall-visible.json) and [prior package](starfall-visible-baseline.json).

| Boundary | Current | Prior package |
| --- | ---: | ---: |
| First trigger to visible labels | 197.9 ms | 171.5 ms |
| Warm trigger to visible labels, median / maximum | 124.6 / 129.5 ms | 121.4 / 130.2 ms |
| Warm trigger to completion-provider entry, median | 3.0 ms | Not instrumented |
| Warm provider entry to return, median | 17.4 ms | Not instrumented |
| Warm provider return to visible labels, median | 102.9 ms | Not instrumented |

The current warm median is 3.2 ms above the prior package in these eleven samples; this is too small and too short a run to establish a regression. The roughly 103 ms post-provider interval is an observed client-side interval, **not** a pure widget paint measurement. The test hook writes a JSON line synchronously at provider return, and the renderer observer records when the expected labels appear. The script does not timestamp LSP wire send or server receive, and cross-process timestamps are approximate. Do not set a latency budget from these numbers alone. The previous package lacked the three numeric exports, so its visible-label assertion uses its available methods; the candidate sets differ.

## Compiler-produced graph and other asset types

Command:

```sh
node internal/audits/feature-followup-2026-11-11/preview-stress-run.js
```

The runner generates 120 linked Tua modules plus `main.tua`, requests `tua/moduleGraph` from the release server, and renders that response in an actual VS Code webview. The server returned 121 modules and 120 edges; the DOM contained 121 module nodes and 120 connections. The final module's exported `value` field was `number`, and double-clicking that module posted the exact compiler URI. The final measured render-to-populated-DOM interval was 68.4 ms. [Raw host data](preview-stress.json) also retains the separate 500-node synthetic graph stress. This generated fixture checks real server-to-view data shape and one source link; it does not establish fidelity for a hand-authored large game or verify every source link.

Valid bundled WAV and Ogg files opened in VS Code's audio player through `tua.assets.preview` ([WAV](native-audio-wav.png), [Ogg](native-sfx_bump-ogg.png)). A GLSL file opened as shader source ([capture](native-wave-glsl.png)). A valid OTF font opened in a binary-file warning rather than a glyph preview ([capture](native-sniglet-otf.png)). This is a font-preview capability gap in the current delegate-to-`vscode.open` workflow. The host run did not assert playback sound quality or shader runtime behavior. The command and screenshots establish only that the correct file was opened and what VS Code displayed.

## Inspector soak

The existing inspector soak runner now accepts `TUA_SOAK_GAME=starfall`. It copies and builds the bundled sample in a temporary project, starts the smoke-game loop in a small invisible LOVE window, and runs both inspector-enabled and plain LOVE processes. It checks snapshots, pause/refresh, polling rates, forced reconnects, process completion, and post-GC Lua/Node heap changes. Temporary driver changes suppress save writes and extend the smoke loop; the bundled source stays untouched. A two-minute pilot returned 217 snapshots, three reconnects, -5.49 KiB active Lua heap change, and a 267.63 KiB Node heap range.

```sh
TUA_SOAK_GAME=starfall TUA_SOAK_SECONDS=900 TUA_SOAK_OUTPUT=internal/audits/feature-followup-2026-11-11/inspector-starfall-15m.json node editors/vscode/scripts/runtime-inspector-soak.js
```

The [15-minute raw result](inspector-starfall-15m.json) has `completed: true`, 1,725 snapshots, zero truncated snapshots or errors, and 29 forced disconnect/reconnect recoveries. The observed 1,000/200/500 ms polling rates were 1.00/4.66/2.00 snapshots per second. Mean active Lua heap change from the first to last three post-warm samples was -5.49 KiB; the plain game process changed -0.02 KiB. Post-GC Node heap range was 585.76 KiB, below the harness's 2 MiB bound. The two LOVE processes exited successfully.

The automated start does not prove fifteen minutes of continuous active gameplay; Starfall can enter its game-over scene. It does exercise the real compiled game loop, inspector lifecycle, and LOVE host over the interval. It is not a manual long-session inspector UI test.

The original fixture mode was rerun after adding the game option:

```sh
TUA_SOAK_SECONDS=60 TUA_SOAK_OUTPUT=/tmp/tua-inspector-fixture-60s.json node editors/vscode/scripts/runtime-inspector-soak.js
```

It completed with 100 snapshots, one recovered disconnect, no errors, 25.09 KiB active Lua heap growth, and an 84.10 KiB post-GC Node heap range. Its 67 intentionally truncated snapshots exercise the fixture's payload limit; the Starfall workload had none. This checks that the default mode still works, without substituting for its earlier ten-minute run.

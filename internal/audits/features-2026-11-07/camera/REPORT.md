# Camera feature audit — 2026-11-07

Core workflows passed; five bounded findings were reproduced (three medium, two low). No product changes. Binary baseline: target/release/tua 0.6.0 at the revision/hash recorded in ../BASELINE.md.

## Execution and scope

From repository root:
- `python3 internal/audits/features-2026-11-07/camera/probe.py` — checks/builds isolated projects and executes emitted Lua; raw cli.log.
- `/opt/homebrew/bin/lua internal/audits/features-2026-11-07/camera/runtime.lua` — raw runtime.log. Uses the build-copied camera runtime and **LOVE graphics doubles**, Lua 5.4.8; no actual LOVE graphics qualification claimed.
- `node internal/audits/features-2026-11-07/camera/lsp.js` — real stdio LSP, raw lsp.json.

| Area | Status/evidence |
| --- | --- |
| Default options, follow, smoothing, bounds, zoom, lead, shake decay | Existing camera suite passes; runtime.lua reruns it against emitted runtime |
| Five styles, zoom, split viewport roundtrip | Passed extra probes |
| Smoothing timestep independence | One 0.2 step equals two 0.1 steps for static target |
| Rotation + bounds; shake + culling; pixel-perfect odd viewport | Reproduced findings below |
| Scissor restoration and detach after callback error | Existing suite passes with doubles |
| CLI option/style/method argument diagnostics | Rejects orbit, incomplete bounds, string trauma |
| Alias and chained helper emission | Alias fails; chained Camera.new():follow works |
| Editor hover/signature/completion | Correct constructor options, follow(x:number,y:number), camera methods; no happy-path diagnostics |
| Multiple-return typing | wx:number, wy:any, explicitly documented limitation |
| Actual LOVE rendering / visual pixel-art appearance | Not exercised in this bounded pass |

## Findings

### CAMERA-01 — Aliasing Camera skips runtime injection (medium, confirmed defect)

- Reproduction: `local C = Camera; local cam = C.new(); print(cam.x)`
- Expected: A built-in used as a value must be injected and bundled, as the no-import contract promises.
- Actual: check and build succeed, but runtime crashes indexing nil C; build emits only one file, without tuacamera.lua.
- Source: crates/tua-core/src/emit/mod.rs:1856.
- Test blind spot: Existing built-in emission coverage uses direct member calls, not a bare module value.
- Smallest improvement and regression: Detect unshadowed built-in value references using shared scope-aware emission infrastructure; include aliases, arguments, and returns in CLI-to-runtime tests.
- Evidence: cli.log; alias/src/main.tua; alias/build/main.lua.
- Known status: no matching camera issue located in ROADMAP.md or prior origin-main audit/reports searched. CAMERA-01 overlaps the shared helper-injection class observed in the concurrent vector audit; deduplicate at synthesis.

### CAMERA-02 — Rotated views escape world bounds (medium, confirmed defect)

- Reproduction: `Camera.new({rotation=math.pi/4,bounds={x=0,y=0,w=2000,h=2000}}):lookAt(-100,-100)`
- Expected: The documented world area must contain the view when the world is large enough.
- Actual: Center clamps to (400,300), but visibleBounds starts at (-94.9747,-194.9747).
- Source: vendor/runtime/tuacamera.lua:188.
- Test blind spot: Bounds tests have zero rotation; rotated conversion tests have no bounds.
- Smallest improvement and regression: Reuse rotated half-extents in bounds clamping; test all four visible corners after lookAt/update with rotation and zoom.
- Evidence: runtime.lua; runtime.log.
- Known status: no matching camera issue located in ROADMAP.md or prior origin-main audit/reports searched. CAMERA-01 overlaps the shared helper-injection class observed in the concurrent vector audit; deduplicate at synthesis.

### CAMERA-03 — Pixel-perfect drawing and coordinate conversion disagree (low, confirmed defect)

- Reproduction: `Camera.new({pixelPerfect=true,viewport={x=0,y=0,w=401,h=301}}); compare attach's translation against toScreen(0,0).`
- Expected: Conversions should match the rendered coordinates, including pixel rounding.
- Actual: Drawing puts the camera center at (201,151), while toScreen returns (200.5,150.5).
- Source: vendor/runtime/tuacamera.lua:286; vendor/runtime/tuacamera.lua:314; vendor/runtime/tuacamera.lua:324.
- Test blind spot: Roundtrip alone cannot detect mutually consistent conversion functions that disagree with rendering; tests use even viewport dimensions.
- Smallest improvement and regression: Share rounded viewport center between attach and conversions; compare transformed points and toScreen for odd viewport dimensions.
- Evidence: runtime.lua; runtime.log.
- Known status: no matching camera issue located in ROADMAP.md or prior origin-main audit/reports searched. CAMERA-01 overlaps the shared helper-injection class observed in the concurrent vector audit; deduplicate at synthesis.

### CAMERA-04 — visibleBounds excludes part of the shaken view (medium, confirmed defect)

- Reproduction: `Camera.new({shakeDecay=0}); shake(1); update(0.1); compare visibleBounds with toWorld(0,0).`
- Expected: The documented rectangle in view should include the drawn world, so culling does not hide visible objects.
- Actual: Visible top-left is (-408.1951,-294.8351), but reported bounds are (-400,-300,800,600), omitting visible pixels on the left.
- Source: vendor/runtime/tuacamera.lua:339.
- Test blind spot: Shake and visibleBounds are tested separately; visibleBounds uses logical self.x/y instead of viewCenter.
- Smallest improvement and regression: Derive visible bounds from transformed viewport corners/shared rendered center, incorporating shake and rounding; assert all corners are inside returned bounds.
- Evidence: runtime.lua; runtime.log.
- Known status: no matching camera issue located in ROADMAP.md or prior origin-main audit/reports searched. CAMERA-01 overlaps the shared helper-injection class observed in the concurrent vector audit; deduplicate at synthesis.

### CAMERA-05 — Platformer preset contradicts documented deadzone proportions (low, confirmed defect)

- Reproduction: `Camera.new({style='platformer'}):deadzoneRect() with 800x600 viewport.`
- Expected: Documented preset proportions should match the preset users select.
- Actual: Deadzone is 100 pixels wide and 200 high; source annotation and docs describe wide horizontally and short vertically.
- Source: docs/camera.md:80; vendor/runtime/tuacamera.lua:78.
- Test blind spot: Existing test asserts the 100-pixel width but never compares it with height or the documentation.
- Smallest improvement and regression: Decide intended proportions, then align preset or description and test both dimensions; avoid silently changing game feel without documenting it.
- Evidence: runtime.lua; runtime.log.
- Known status: no matching camera issue located in ROADMAP.md or prior origin-main audit/reports searched. CAMERA-01 overlaps the shared helper-injection class observed in the concurrent vector audit; deduplicate at synthesis.

## Capability ideas and remaining decisions

- Define numeric domain expectations (positive finite zoom, nonnegative viewport sizes/dt) and fail clearly at runtime. The zero-zoom probe currently passes type checking and yields NaN coordinates; there is no documented numeric-range contract, so this is a design question rather than an additional confirmed defect.
- Preserve multiple numeric returns through shared LuaDoc semantic inference; the current loss of the second coordinate is accurately documented.
- Document that rotation is ignored in deadzone tests (runtime comment), or consider rotated deadzone support if user demand warrants it.
- Multi-target, cinematic, and parallax features are explicitly outside this starter camera's scope; no missing-feature defect is asserted.

# Tua 0.7.0 preview and explorer audit

Audit date: 2026-11-11 (Asia/Tokyo). Starting revision:
`b49ceb0019c98267d339c7d864440cccc635800c`. The working tree was clean
before this audit. This report covers the changes made here; it does not certify
every language or runtime behavior in 0.7.0.

## Assessment

The sprite, state-machine, and behavior-tree previews have usable interaction
models: refresh, source navigation, and narrow layouts passed in a real VS Code
webview. The module, ECS, object-pool, signal, audio, and health views also load
in that host. Static graphs remain separate from live game state; the runtime
inspector is the live view. Four editor findings were fixed. The main reliability
problem was a shared refresh race across several views. A fifth accessibility
finding concerned missing editor-colored keyboard focus rings in graph views.

The candidate VSIX can be installed for beta testing after the checks below.
Public release still depends on the repository's platform, upgrade, provenance,
and published-artifact gates in `internal/release-readiness.md`. Local webview
tests do not prove every populated graph against an independent large game.

## Inventory and coverage

“Browser” means a real VS Code extension-host webview controlled through CDP;
it is not a static HTML render. “Gallery” means an unedited capture from the
production extension bundle and Starfall example. “Inspected” means source and
existing tests were reviewed without new end-to-end exercise of that path.

| Surface | UI source | Evidence in this audit | Remaining limit |
| --- | --- | --- | --- |
| Sprite animation | `src/love/sprite-preview/` | Browser playback, focus, restore, bad-image feedback, retry, narrow layout; populated gallery | No pixel oracle for every image format |
| State machine | `src/state-machines/` | Browser routes, focus, navigation, restore, narrow layout; populated gallery | Static topology only |
| Behavior tree | `src/behavior-trees/preview.js` | Browser nesting, child order, collapse/restore, navigation, narrow layout | No live tick/status overlay |
| Module graph | `src/modules/` | Populated gallery; empty browser/narrow layout; stale-response regression | Large graph interaction not profiled here |
| Signal graph | `src/signals/signal-graph.js` | Empty browser/narrow layout, visible guidance, stale-response regression | Populated graph actions not browser-tested here |
| ECS explorer | `src/ecs/` | Empty browser/narrow layout, existing extension-host integration, stale-response regression | Populated graph not browser-tested here |
| Object-pool explorer | `src/object-pools/` | Empty browser/narrow layout, source inspection, stale-response regression | Populated graph not browser-tested here |
| Audio Explorer | `src/audio/index.js` | Populated browser fixture, source navigation, retry, narrow layout, stale-response regression | No audible playback or independent pipeline project |
| Asset Explorer and asset preview | `src/love/assets/` | Controller/tree source inspection and existing unit coverage | Native VS Code media preview appearance not captured |
| Runtime inspector | `src/inspector/` | Real runtime gallery; existing value, session and navigation unit tests reviewed | Long-session and multi-player game qualification pending |
| Server health | `src/health/` | Empty browser/narrow layout and stale-response regression | Populated health panel not browser-tested here |
| Generated Lua preview | `src/editor/generated-lua.js` | Source and existing extension-host test inspected | VS Code text editor, not a custom webview |

The offline LOVE API reference and Explain Type quick pick were inspected for
scope but excluded from preview scoring: they are documentation and analysis
navigation, not visual previews of game declarations or assets.

## Fixed findings

### PV-01 · Out-of-order refresh could replace newer results

**Severity:** Medium · **Confidence:** High · **Status:** Fixed, regression-tested.

**Reproduction:** Open any affected view, trigger Reload twice while the first
server request is pending, and resolve the second response first. Previously,
the first response later replaced the newer graph. Module, signal, ECS, health,
and audio paths lacked a request sequence check. Object pools already had a
local sequence; they now use the same guard as the other views. This could show
stale declarations or health readings during active game editing.

**Evidence:** `editors/vscode/test/unit/preview-refresh.test.js` issues two
deferred requests for each of six views and verifies that resolving the older
request last leaves the current HTML intact. The fix uses
`src/shared/render-gate.js`, invalidates pending requests on disposal, and
checks panel identity before writing. Related code is in `src/modules/module-graph.js`,
`src/signals/signal-graph.js`, `src/ecs/explorer.js`,
`src/object-pools/explorer.js`, `src/audio/index.js`, and `src/health/index.js`.
The previous tests checked open/reuse or view models, not response order.

### PV-02 · Audio Explorer had inconsistent error, navigation, and webview behavior

**Severity:** Medium · **Confidence:** High · **Status:** Fixed, browser-tested.

**Reproduction:** Make `tua/audioGraph` fail, or open the view beside a narrow
editor and use its source action. The prior loading/error HTML was a bare
paragraph with no themed document or retry action; the success document did
not use the shared CSP-bearing webview builder. Navigation used an audio-only
message format and handler.

**Evidence:** `src/audio/index.js` now uses `shared/webview.js` and
`shared/locations.js`. The browser suite checks source action messages, narrow
layout, and the retry control after a server error. The rendered screenshots
were inspected at wide and narrow widths. Existing tests had no rendered audio
webview coverage.

### PV-03 · Signal empty state was outside a narrow pane

**Severity:** Medium · **Confidence:** High · **Status:** Fixed, browser-tested.

**Reproduction:** Open Signal Graph with no indexed calls in a narrow editor
pane. Its graph kept a 720-pixel minimum width, centering “No SignalBus calls
found” beyond the visible portion. The screenshot showed an apparently blank
graph. The filter-empty path had the same layout risk.

**Evidence:** `src/signals/signal-graph.js` removes the graph minimum width
only while empty and gives specific guidance. The browser test checks that
the empty message's bounds are visible inside the viewport. The captured
`/private/tmp/tua-preview-audit/signal-graph-narrow.png` was reviewed.

### PV-04 · Preview headers exposed long absolute source paths

**Severity:** Low · **Confidence:** High · **Status:** Fixed, gallery-reviewed.

**Reproduction:** Open sprite, state-machine, or behavior-tree preview in a
project with a long path. The header displayed the full temporary or user path,
truncating useful context in narrow panes. The prior release gallery reproduced
this with `/var/folders/.../Starfall/src/...`.

**Evidence:** The three views now display workspace-relative source labels via
`shared/locations.js` while retaining full source identity for navigation and
state, and the sprite/behavior header exposes the full location in a tooltip.
The regenerated populated screenshots show `src/main.tua` and
`src/sprites.tua`; each of the five gallery images was inspected.

### PV-05 · Graph controls lacked consistent themed keyboard focus

**Severity:** Low · **Confidence:** Medium · **Status:** Fixed, source-reviewed.

**Reproduction:** Tab through module, signal, ECS, object-pool, and health view
controls. Their styles contained no explicit `:focus-visible` rule, and some
search inputs removed the native outline. The latter retained a border change,
but focus was less visible than in sprite, state-machine, and behavior-tree
previews. The new rules use the VS Code focus token and a 2-pixel outline.

**Evidence:** Each affected template now includes `:focus-visible`; the browser
suite confirms the views render and function after the CSS change. This audit
did not automate keyboard traversal of every control, so the confidence is
medium. The previous tests did not inspect focused states for these views.

## Coverage gaps and follow-up

### Release gate · generics example disagreed with the formatter

**Severity:** Medium · **Confidence:** High · **Status:** Fixed in a separate commit.

The full `make ci-check` reached `profile-examples-smoke` and failed because
`tua fmt --check` would reformat the newly added `examples/features/src/generics.tua`.
Running the formatter changed spaces around generic angle brackets. The
formatter's existing regression test specifies that style, so the example was
aligned with it and rechecked with `tua check` and `tua fmt --check` from
`examples/features`. A future formatter design pass could revisit the visual
style of generic arguments, but it is separate from this release gate.

These are limits of this audit, not confirmed defects. The most useful next
editor exercise is a populated signal/ECS/object-pool graph in a large,
independent game, including filtering and source actions at narrow widths.
Runtime inspector stability over a long play session and native asset preview
appearance also need installed-editor qualification. The browser suite now
has a small empty-view/narrow fixture for those graphs, but it does not replace
that exercise. A future behavior-tree live tick/status overlay would need an
explicit runtime protocol and is a design decision, not a missing static-preview
bug.

## Verification record

| Command or check | Result |
| --- | --- |
| `node --test editors/vscode/test/unit/preview-refresh.test.js` | 6 pass; stale response order across six views |
| `TUA_PREVIEW_SCREENSHOTS=/private/tmp/tua-preview-audit npm --prefix editors/vscode run test:webviews` | Pass in installed VS Code extension host; screenshots reviewed |
| `make extension-server && make extension-screenshots` | Pass; five populated gallery images regenerated from the final source and visually reviewed |
| `make extension-screenshots-check` | Pass as part of screenshot command |
| `make ci-check` | Pass after correcting a preview test double and a generics example formatter mismatch; includes 1,925 core and 448 LSP tests, example checks, editor checks, docs and benchmark smoke checks |
| `make extension-real-server-test` | Pass in a real VS Code extension host against the staged 0.7.0 server; LSP completion, navigation, hover, diagnostics, and generics/protocol interactions exercised |
| `npm audit --prefix editors/vscode --audit-level=high` | Pass: 0 vulnerabilities of any severity; required a network-enabled retry after sandbox DNS failure |
| `make extension-package` | Pass; packaged `editors/vscode/tua-0.7.0.vsix` (40 files, 8.83 MB) |
| `unzip -t`, package manifest, bundled binary `--version` | Pass; archive intact and both extension and server report 0.7.0 |
| Isolated `code --install-extension ... --force` and `--list-extensions --show-versions` | Pass in `/private/tmp/tua-vsix-user-070` and `/private/tmp/tua-vsix-extensions-070`; listed `solenodontelabs.tua@0.7.0` |

The first screenshot attempt failed because `editors/vscode/bin/tua` had not yet
been staged (`spawnSync ... ENOENT`). `make extension-server` staged the current
0.7.0 release binary; the subsequent real capture passed. No user project or
normal VS Code profile was modified by the isolated browser/gallery runs.

## Installable artifact

`editors/vscode/tua-0.7.0.vsix` is the locally built, installable macOS arm64
beta candidate (the bundled server is a Mach-O arm64 executable).
SHA-256: `775bba65594352eec20468b9eaf3ff3dd4881dc5fab79b600989c53b1a53db88`.
Install with `code --install-extension editors/vscode/tua-0.7.0.vsix --force`
from the repository root, or use VS Code's **Install from VSIX** command.
The VSIX is an ignored build artifact, so it is not part of the audit commit.
Public publication remains gated by cross-platform CI and the other release
qualifications described above.

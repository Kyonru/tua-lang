# Editor and LSP audit lane

Starting revision: `c08edbca` on 2026-11-11. Local host: VS Code 1.142.0 on macOS arm64; `target/release/tua --version` reported 0.7.0. This lane covers T09–T13. It does not certify every language-server feature; the language lane and lead own shared semantic behavior and the full coverage ledger.

## Checks run

| Check | Outcome | Scope |
| --- | --- | --- |
| `npm --prefix editors/vscode run test:webviews` | Pass in installed VS Code extension host after local loopback permission | Rendered webviews, focus, navigation, refresh, narrow layout, selected state, and populated Signal/ECS/pool relationships. The first sandbox attempt failed `listen EPERM 127.0.0.1`; it was an environment restriction, not a product failure. |
| `TUA_PREVIEW_SCREENSHOTS=/private/tmp/tua-feature-preview npm --prefix editors/vscode run test:webviews` | Pass | Captured rendered sprite, state-machine, behavior-tree, audio, and empty/narrow graph views; inspected the behavior-tree capture. Selected captures are in `captures/`. |
| `npm --prefix editors/vscode run check` | Pass: 189 JS tests, grammar/theme/config/activation checks | Extension unit suite and activation smoke. Theme checks reported text minimum 4.72:1 and focus minimum 3.89:1; this is not a manual contrast audit of every preview. |
| `TUA_REAL_SERVER_PATH=/Users/kyonru/Documents/projects/Experiments/tlua/target/release/tua npm --prefix editors/vscode run test:vscode-real` | Pass, exit 0 | Real-server extension-host workflow on an isolated generated project, including generic/protocol hover, completion, definitions, rename, unsaved updates, inlay hints, and signatures. It used the pre-edit 0.7.0 release binary; final bundled-artifact testing remains a separate qualification step. |
| `TUA_REPRESENTATIVE_FIXTURES=project-100 TUA_REPRESENTATIVE_COLD_REPEAT=2 TUA_REPRESENTATIVE_WARM_REPEAT=5 TUA_REPRESENTATIVE_CONTENTION_REPEAT=20 TUA_REPRESENTATIVE_BACKGROUND_BURST=4 TUA_REPRESENTATIVE_PROFILE_OUT=/private/tmp/tua-editor-audit-profile.json node editors/vscode/scripts/lsp-representative-profile.js target/release/tua` | Pass | Release-binary protocol replay on a synthetic 100-file, 1,401-line project. First sandbox attempt failed `spawn EPERM` in the memory sampler; local-permission retry passed. |

The bounded protocol replay measured completion cold p95 207.51 ms, warm p95 0.55 ms; hover cold p95 189.43 ms, warm p95 1.86 ms; definition cold p95 261.45 ms, warm p95 0.28 ms; contended completion p95 52.81 ms with 72/80 cancelled, zero stale, and 20/20 non-empty inline results. These are protocol request timings, **not VS Code key-to-menu latency**. The release binary predates this lane's webview-only edits. The JSON trace remains at `/private/tmp/tua-editor-audit-profile.json` and should be copied only if useful for a retained benchmark artifact.

## ED-01: explorer filters reset on refresh

**Category:** editor usability/correctness · **Severity:** Medium · **Confidence:** High · **Status:** confirmed in a real VS Code webview; fixed and regression-tested in this lane.

**Impact:** During active editing, refreshing an ECS, pool, module, or signal view returned the user to the initial filter and mode. This made a selected subset disappear from view just when the user was checking whether a change affected it. ECS and pool views also needed to avoid carrying a filter to a different focused source file.

**Minimal reproduction:** Open a rendered ECS Explorer, enter `player` in Filter, choose Systems, then replace the webview HTML with an updated analysis revision. Before the fix, `#filter.value` was empty and Components was active. `editors/vscode/test/webview/suite.js` now runs this sequence for all four views. The pre-fix ECS assertion failed with `AssertionError: ECS Explorer preserves filtering after refresh`; after the fix, the installed-host suite passes. The fixture also checks that moving ECS focus from `file:///one.tua` to `file:///two.tua` resets the old filter.

**Likely root cause:** `src/ecs/template.js`, `src/object-pools/template.js`, `src/modules/template.js`, and `src/signals/signal-graph.js` initialized filter/mode/selection variables to defaults on each HTML load, while their controllers replace `webview.html` for refreshes. The render gate prevented stale responses but did not preserve user view state. Sprite and behavior-tree previews already used `vscode.getState()/setState()`, which provided the local pattern.

**Fix direction and coverage:** Each of the four templates now stores namespaced, small view state through the VS Code webview API. ECS/pool restoration is scoped to the focused source URI; module restoration also respects the command's initial tab. Options that disappear from a refreshed signal graph are cleared. The installed-host regression asserts filter/mode retention across analysis revisions, and source-focus reset for ECS. `docs/lsp.md` describes the behavior. Follow-up coverage could add populated-selection cases and cross-workspace switching; the current test does not prove those paths.

## ED-02: signal event name hidden by bus metadata

**Category:** preview visual correctness · **Severity:** Medium · **Confidence:** High · **Status:** confirmed from a populated VS Code screenshot and browser geometry; fixed and regression-tested in this lane.

**Impact:** The graph's event node hid `enemy.hit`, the value a developer needs to identify the event, while showing the bus path and call count. The inspector below still showed the event name, but a graph with multiple events would force selection or scrolling to identify each node.

**Minimal reproduction:** Render a signal graph containing one `enemy.hit` event on `EnemyEvents`, with declaration, listener, and emit calls in three files. `captures/signal-graph-clipped-before.png` shows only `EnemyEvents / /game/events.tua / 3 calls` in the event node; `captures/signal-graph-populated.png` shows the name after the fix. The installed-host assertion `label.getBoundingClientRect().width >= 50` failed before the CSS change and passed after it.

**Likely root cause:** `src/signals/signal-graph.js` placed the event name and potentially long metadata in one flex row; `.node span { flex: 0 0 auto }` could consume the node's width and shrink the name to zero. The prior suite covered empty/narrow states, not a populated node with realistic metadata.

**Fix direction and coverage:** The node now stacks its primary name above truncated metadata, while the full label and metadata remain available through the node title. The installed-host test checks event-name geometry, all three call roles, and source navigation. This is a local presentation fix; no compiler or protocol model changed.

## T09–T13 status

| ID | Status in this lane | Evidence and limit |
| --- | --- | --- |
| T09 LSP feature set | Partly tested | Real-server installed extension-host suite exercised generic/protocol hover, completion, definitions, rename, unsaved updates, inlay hints, and signatures. The 100-file protocol profile exercised completion, hover, definition, inlay hints, contention, and inline completion; neither is a correctness matrix for every LSP method. |
| T10 unsaved/cache/project lifecycle | Partly tested | Protocol profile included contended edits and stale-result assertions; webview refresh suite tested response ordering and state. Multi-project switch, long-session RSS, and exact editor key-to-menu timing remain unqualified in this lane. |
| T11 API/MCP bridge | Inspected only | Capability exists in `docs/lsp.md` and server request registration. This lane did not run an API/MCP privacy or trust-boundary exercise. |
| T12 setup/theme/toolbar/offline refs/health | Partly tested | `npm run check` covered extension activation, theme tokens, and 189 unit cases; rendered Server Health empty/narrow view passed. Actual first-run setup and populated health interpretation were not retested. |
| T13 domain editor integrations | Partly tested | Installed webview suite covered representative preview actions, this lane's refresh regression, and small populated Signal/ECS/pool graphs with source actions. Large graphs, native asset appearance, and long-running runtime inspector were not established here. |

## Verification limits

No automated installed-editor measurement of a user's keystroke to visible suggestion menu was available in this lane. The installed webview test uses a controlled fixture and CDP, not a real game project. Screenshot captures show the VS Code host but do not prove source/model fidelity for every populated graph. Local platform was macOS arm64; Windows/Linux and another LSP client were not exercised in this lane.

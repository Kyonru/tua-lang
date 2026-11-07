# Project graphs and explorers audit

All seven exercised actual LSP interfaces returned coherent results for the bounded fixture. No confirmed protocol defect. Two extension feature gaps were reproduced through the real sidebar provider under a minimal VS Code mock; these are not counted as confirmed defects because documentation does not promise live sidebar updates or complete file inventory.

## Commands and evidence

From repository root:

- `node internal/audits/features-2026-11-07/explorers/probe.js` — actual `target/release/tua lsp` stdio process; six phases, seven requests per phase; raw results in `responses.json`, diagnostics in `diagnostics.json`.
- `node internal/audits/features-2026-11-07/explorers/sidebar-probe.js` — production provider with mocked VS Code API, fed actual server responses; `sidebar-results.json`.
- `node --test editors/vscode/test/unit/workspace-sidebar.test.js` — 2 passed; `sidebar-tests.log`.

The first harness draft mistakenly used `output` instead of supported `out` in tua.toml, making initial project discovery incomplete. This was corrected and the entire probe rerun; final evidence includes three indexed modules and the expected two-module cycle. This is a harness error, not a finding.

## Coverage

| Surface | Status | Evidence / limit |
| --- | --- | --- |
| Module graph | Exercised | Tua + Lua inventory, resolved import edges, exports, reverse dependencies, two-module cycle; unsaved edits and watcher creation/deletion |
| Signal graph | Exercised | Declaration, listener, emitter, payload, source ranges; unsaved name change reflected |
| ECS graph | Exercised | Component, read/write system, standalone query, linked component IDs; unsaved rename and watcher lifecycle |
| Object pool graph | Exercised | Pool and matched acquire/release metadata; no leak insight; unannotated callback reports `any`, not separately counted |
| Audio graph | Exercised | Static buses, empty cues, unsaved bus rename, conservative empty result for dynamic config |
| State machine inspect | Exercised | Initial state, wildcard and ordinary routes, declaration ranges; unsaved initial-state edit; dynamic and empty configurations yield no static items |
| Asset inventory | Exercised, narrow | Empty inventory requests remain valid across all phases; actual asset lifecycle handled in separate asset lane |
| Source navigation | Protocol/provider inspected | Ranges and URIs preserved in responses and sidebar conversion; actual editor click/reveal not exercised |
| Sidebar updates/filter | Mock replay | Cached graph survives unsaved edit until manual refresh; Lua filtered from Project files |
| Graph rendering, filters, sprite preview, state-machine edit synchronization | Inspected/limited | No GUI session; cannot claim visual layout, focus, webview message delivery, or live rendered update behavior verified |

## EXPLORERS-01 — Sidebar graph cache does not follow unsaved or filesystem changes

- Severity: low. Category: feature gap. Status: reproduced; new relative to searched prior audit reports and ROADMAP.
- Reproduce: run both probes above; initial sidebar Signals contains `player.hit`. Feed the provider the actual server's unsaved response containing `player.changed`, then expand again. Sidebar still shows `player.hit`, request count remains 1. Manual `tua.workspace.refresh` returns `player.changed`.
- Expected improvement: a current-project sidebar follows relevant source changes, including externally created/deleted source files. Actual implementation subscribes only to document saves and workspace-folder changes; unsaved edits and filesystem notifications have no refresh subscription.
- Evidence: `sidebar-results.json`; `responses.json` initial/unsaved and created/deleted phases. Source: `editors/vscode/src/workspace/sidebar.js:27`, `:80`.
- Qualification: docs/lsp.md does not explicitly promise live sidebar refresh. This is a visible consistency gap, not a breach of the documented source-edit-following state-machine preview contract. Dedicated graph webviews have Reload controls and were not labeled defective for manual refresh.
- Existing test blind spot: `workspace-sidebar.test.js` verifies explicit refresh and client replacement, not event-driven invalidation.
- Small improvement: debounced relevant document/file lifecycle invalidation shared by the sidebar, retaining lazy graph requests. Regression: register the provider, emit edit/create/delete notifications, and assert refreshed graph/file children without manual refresh; also keep an explicit save-only policy option if intended.

## EXPLORERS-02 — Project files omits Lua sources that Modules includes

- Severity: low. Category: feature gap. Status: reproduced; new relative to searched prior reports and ROADMAP.
- Reproduce: actual moduleGraph includes fixture `interop.lua` with `sourceKind: lua`; sidebar production directory filtering over entries `main.tua`, `interop.lua`, `tua.toml` yields only `main.tua`, `tua.toml`.
- Expected improvement: Lua sources supported and indexed by Tua should be reachable from Project files, matching Modules. Actual extension hardcodes `tua|toml` suffixes at `editors/vscode/src/workspace/sidebar.js:70`.
- Evidence: `responses.json` initial/moduleGraph, `sidebar-results.json` files; `sidebar-probe.js`.
- Qualification: documentation promises Lua in the module graph (which passes), not an exhaustive Project files tree. Hence feature gap rather than confirmed defect.
- Existing test blind spot: sidebar tests cover graphs and cache, not directory filtering.
- Small improvement: include indexed Lua source files; ideally derive source visibility from project facts rather than independently maintaining suffix/output rules. Regression: mixed Lua/Tua project directory test, preserving exclusions for generated/vendor paths.

No product files changed, no installs, no GUI launch, no subagents, no runtime-domain findings recounted. Configuration/output-root filtering, multi-root workspace switching, asynchronous request/render races and nonempty asset UI remain unverified.

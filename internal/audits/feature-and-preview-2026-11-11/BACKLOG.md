# Prioritized improvement backlog

This is a risk-ranked follow-up list, not a release promise. Confirmed fixes already committed are in FINDINGS.md. Effort is approximate and excludes unavailable platform/host setup. Prioritize shared correctness over one-off view patches.

| Priority | Kind | Shared cause / work | Dependency and approximate effort | Acceptance |
| --- | --- | --- | --- | --- |
| P1 | Editor correctness | Preserve imported class identity through an inferred forwarding function; hover currently shows `AgentInstance` while member completion is empty. | Shared module export/type resolver; medium. See [FUP-01](../feature-followup-2026-11-11/FINDINGS.md). | Class → typed factory → inferred forwarding function → consumer has matching check, hover, completion, signature, and definition across saved/unsaved edits. |
| P1 | Verification | Run the new ECS/class/helper interaction regressions under pinned LuaJIT and final packaged server on trusted CI. | Local fixes committed; Linux runtime and release gates; small. | CI executes negative cross-module `is`, inheritance positive, nullable ECS access, imported schema, owned-helper refresh/override, and tree/ECS/pool runtime without a silent skip. |
| P1 | Editor integration | Keep the real-server-to-rendered graph oracle for Signal/ECS/pool data and source actions, extending only for observed data-shape drift. | `c9232605` landed; small to maintain. | Installed-host test opens generated project, renders all three views, and navigates to expected source URI/line. |
| P2 | Correctness qualification | Risk-selected assets × sprites × shaders and hot reload × helper inclusion × source-map freshness. | Graphical LOVE host and deterministic editing fixture; medium. | One game scenario verifies data, rendering/validation, source actions, failed reload, and recovery without stale helper or map. |
| P2 | Editor responsiveness | Attribute the measured visible-menu interval and repeat on a real game and previous version. Synthetic installed-host and server p50/p95 are recorded in the [follow-up](../feature-followup-2026-11-11/EDITOR_PERFORMANCE.md). | Client/renderer trace and representative game; medium. | Timestamp trigger, client send, server response, widget paint, and format-on-save; set a variance-aware budget only after a comparable baseline. |
| P2 | Preview qualification | Verify real large-project graph fidelity, populated server health, non-PNG native assets, and longer inspector sessions. Synthetic 500-node graphs, native PNG, and a 10-minute LOVE soak passed in the [follow-up](../feature-followup-2026-11-11/PREVIEWS.md). | Installed editor plus LOVE graphical host; medium to large. | Compare displayed graph data and source links with compiler output; exercise accessibility and recovery on a real game and additional asset types. |
| P2 | Compatibility | Previous-release upgrade/rollback, Windows/Linux platform packages, alternate LSP client, and sustained memory. | Release candidates and platform hosts; large. | Each matrix cell has versioned command, outcome, artifact hash, and explicit blocker; no local macOS smoke substitutes for other platforms. |

## Implementation sequence

1. Run final core, LSP, CLI, examples, docs, extension, and packaging gates after the last edit; use Linux CI for pinned LuaJIT and graphical LOVE evidence.
2. Qualify the unjoined runtime/editor boundaries with small fixtures. Measure latency and memory against a baseline before changing caches or UI architecture.
3. Use the [reconciled historical ledger](PRIOR.md) and observed task blockage to choose the next product improvement; continue deeper feature-method/configuration checks as evidence becomes available.

## Completed during this audit

Nested imported ECS world/component schema propagation, nullable `get`/`remove` checking, and local-world/imported-component hover landed separately in `14d9a1a2`, `68c13600`, and `82b8674f`, with core, CLI, and real LSP regressions. The class identity, SignalBus vararg, owned-helper, preview, and feature-example wording fixes are listed in FINDINGS.md. Pinned Linux CI and broader qualification remain separate from these local fixes.

The November 11 [follow-up](../feature-followup-2026-11-11/README.md) individually rechecked all 21 historical IDs that had remained unverified and recorded installed-editor latency, 500-node previews, native PNG appearance, and a ten-minute LOVE inspector soak. Deeper feature-method/configuration claims remain a separate documentation audit.

## Decisions rather than defects

- Live behavior-tree tick overlays need a runtime data contract and lifecycle design; their absence from the static preview is not a regression.
- Broader generic/protocol language machinery, structural ECS callbacks, and additional graph visualizations should be scoped against real game tasks before implementation. They are not required to fix the confirmed defects above.

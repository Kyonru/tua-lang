# Prioritized improvement backlog

This is a risk-ranked follow-up list, not a release promise. Confirmed fixes already committed are in FINDINGS.md. Effort is approximate and excludes unavailable platform/host setup. Prioritize shared correctness over one-off view patches.

| Priority | Kind | Shared cause / work | Dependency and approximate effort | Acceptance |
| --- | --- | --- | --- | --- |
| P1 | Verification | Run the new ECS/class/helper interaction regressions under pinned LuaJIT and final packaged server on trusted CI. | Local fixes committed; Linux runtime and release gates; small. | CI executes negative cross-module `is`, inheritance positive, nullable ECS access, imported schema, owned-helper refresh/override, and tree/ECS/pool runtime without a silent skip. |
| P1 | Editor integration | Keep the real-server-to-rendered graph oracle for Signal/ECS/pool data and source actions, extending only for observed data-shape drift. | `c9232605` landed; small to maintain. | Installed-host test opens generated project, renders all three views, and navigates to expected source URI/line. |
| P2 | Correctness qualification | Risk-selected assets × sprites × shaders and hot reload × helper inclusion × source-map freshness. | Graphical LOVE host and deterministic editing fixture; medium. | One game scenario verifies data, rendering/validation, source actions, failed reload, and recovery without stale helper or map. |
| P2 | Editor responsiveness | Measure actual keystroke-to-visible-menu delay and format-on-save in an installed editor, alongside cold/warm LSP protocol timings. | UI automation and representative games; medium. | Reproducible p50/p95 with machine/project metadata and a variance-based regression budget; server, extension, and rendering costs separated. |
| P2 | Preview qualification | Native asset appearance, populated server health, large graph filtering/navigation, and runtime-inspector disconnect/long session. | Installed editor plus LOVE graphical host; medium to large. | Task-based host capture or interaction record for each, exact source/data fidelity, accessibility and recovery results. |
| P2 | Compatibility | Previous-release upgrade/rollback, Windows/Linux platform packages, alternate LSP client, and sustained memory. | Release candidates and platform hosts; large. | Each matrix cell has versioned command, outcome, artifact hash, and explicit blocker; no local macOS smoke substitutes for other platforms. |
| P3 | Documentation and inventory | Revalidate historical November 7 finding IDs and deeper feature-method/configuration claims. | 74 prior headings and 369+ current inventory rows; large. | Each historical ID has current reproduction or linked regression and a defensible fixed/open/blocked status; every grouped capability's distinct oracle is visible in COVERAGE.md. |

## Implementation sequence

1. Run final core, LSP, CLI, examples, docs, extension, and packaging gates after the last edit; use Linux CI for pinned LuaJIT and graphical LOVE evidence.
2. Qualify the unjoined runtime/editor boundaries with small fixtures. Measure latency and memory against a baseline before changing caches or UI architecture.
3. Reconcile historical reports as a separate evidence task, then choose the next product improvement by observed task blockage rather than by checklist count.

## Completed during this audit

Nested imported ECS world/component schema propagation, nullable `get`/`remove` checking, and local-world/imported-component hover landed separately in `14d9a1a2`, `68c13600`, and `82b8674f`, with core, CLI, and real LSP regressions. The class identity, SignalBus vararg, owned-helper, preview, and feature-example wording fixes are listed in FINDINGS.md. Pinned Linux CI and broader qualification remain separate from these local fixes.

## Decisions rather than defects

- Live behavior-tree tick overlays need a runtime data contract and lifecycle design; their absence from the static preview is not a regression.
- Broader generic/protocol language machinery, structural ECS callbacks, and additional graph visualizations should be scoped against real game tasks before implementation. They are not required to fix the confirmed defects above.

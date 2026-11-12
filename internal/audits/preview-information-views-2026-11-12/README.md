# Tua preview and information-view audit — 12 November 2026

Starting revision: `b21dfb48` (clean worktree). This pass reviews the extension views against the question a LOVE developer is trying to answer. The existing preview plan and the [November 11 review](../feature-and-preview-2026-11-11/PREVIEWS.md) supplied the baseline; the [host/soak recheck](../feature-followup-2026-11-11/HOST_AND_SOAK_RECHECK.md) supplied historical installed-package and inspector evidence. The current host run used production templates inside VS Code 1.142.0 and a disposable project. See [Inventory](INVENTORY.md) and [Verification](VERIFICATION.md).

## Outcome

Two confirmed medium-severity information/workflow problems were fixed in separate unsigned local commits:

| ID | Problem | Developer impact | Fix |
| --- | --- | --- | --- |
| PV-01 | The Module Graph's `Roots` filter was clipped in a narrow editor. | A developer could not select a key way of answering “why is this module in my build?” without finding a hidden horizontal scroll. | Filters wrap into two visible rows at narrow widths; host regression added. [Before](captures/module-before.png) · [After](captures/module-after.png) |
| PV-02 | Server Health displayed `0ms` p95 and `0%` hit/warm rates before any samples/files existed. | The view appeared to report measured instant requests and a poor cache rather than absence of evidence. | Unmeasured values say “No samples” or “No files”; measured values remain numeric. Unit and host regressions and troubleshooting note added. [Before](captures/health-before.png) · [After](captures/health-after.png) |

No large redesign was justified by the small populated fixtures alone. The sprite, state-machine, behavior-tree, ECS, pools, signal, and audio views already expose domain facts and source actions, although their large-project fidelity and some host states remain unqualified. The state-machine capture initially appeared clipped; inspection showed a deliberately restored selection and horizontal scroll after the suite had interacted with it. This is not counted as a defect.

## Design decisions

**Module dependencies.** Approach A was a responsive version of the existing dependency canvas and inspector. Approach B was a full dependency table/tree optimized for keyboard tracing. The observed obstruction was the filter toolbar, so A was selected for this fix. B may be worthwhile only after a large graph demonstrates that tracing and source navigation are actually slower on the canvas. The [before/after captures](captures/module-before.png) are rendered prototypes of A in the installed editor host.

**Server health.** Approach A was to keep the existing metric summary but distinguish absent samples. Approach B was a triage-first timeline showing queue, cache, and request causes over time. The server currently returns snapshots rather than a time series, so B would invent an unsupported history. A was implemented; a real slow-project study should determine whether to add a bounded history contract later.

**Signals/ECS/pools.** A generic node diagram would put every relationship into the same shape. The current domain-specific inspectors give an event's declaration/listener/emitter roles, a component's system read/write access, and a pool's acquire/release lifecycle. A task-first ledger or access matrix is a plausible alternative for large datasets, but this pass has only small populated fixtures. We retained the current layouts pending measured large-project tasks. [Signal capture](captures/signals.png) · [ECS capture](captures/ecs.png) · [Behavior-tree capture](captures/behavior-tree.png).

## Next evidence to gather

1. Compare compiler responses with rendered Module, Signal, ECS, and Pool views on one representative large project, including conflicts, unresolved imports, and many-to-many edges. Record time to locate a specific relationship and all source targets.
2. Capture native audio/video/image/shader asset previews and offline reference/type workflows in an installed extension, including focus return and high-contrast content. The current synthetic webview suite does not prove those native editor surfaces.
3. Repeat the runtime-inspector soak in a long real LOVE session. The historical ten-minute synthetic run passed; it does not prove a multi-hour game session.
4. Measure actual narrow webview dimensions and focus at high zoom. The host's workbench viewport override does not always resize an already-mounted webview iframe in lockstep; screenshots and assertions must distinguish the two targets.

These are qualification gaps, not newly confirmed defects. Stores has no dedicated static preview: it is visible as live state in the Runtime Inspector. Whether a static store dependency view would improve debugging is a capability/design question requiring a concrete workflow.

# Feature audit follow-up: editor latency, previews, historical cases

**Revision:** `ab1b69c506f962ab239e015d2db4c38764fd592a` (2026-11-11, macOS arm64). **Scope:** the remaining installed-editor/performance qualifications and 21 historical IDs from the [feature and preview audit](../feature-and-preview-2026-11-11/README.md). This pass investigated and documented behavior; it made no product-code changes.

**November 12 follow-up:** The [installed-host and soak recheck](HOST_AND_SOAK_RECHECK.md) closes the two explorer host-evidence gaps, fixes a newly reproduced sidebar refresh race, and replaces the default soak's pre-GC heap oracle. The results below describe the November 11 baseline; use the addendum for the current outcomes.

**FUP-01 resolution:** The cross-file inferred factory forwarding gap described below is now fixed and rechecked in core, LSP, and an actual VS Code host. See the [finding's resolution note](FINDINGS.md#fup-01--a-second-inferred-factory-hop-loses-class-member-completion).

**November 12 qualification:** The [Starfall, graph, asset, and inspector qualification](NOV12_QUALIFICATION.md) rechecks the remaining recommendations against the bundled game and a previous packaged extension. It also records a separate returned-local module export defect found during the real-game completion run and its fix. The 15-minute Starfall inspector soak passed with 1,725 snapshots and 29 recovered disconnects.

The editor was responsive in the tested synthetic workloads, with a visible warm suggestion median around 161 ms for an imported table and around 108 ms for a one-hop imported class factory. Keyboard-invoked class hover appeared around 10 ms median; pointer hover took 320–325 ms including VS Code's hover delay. Format-on-save took 12.7 ms median for a two-line file and 16–33 ms for a 1,001-line file. These are actual VS Code host timings, not just server replies. The server's 100-module warm completion p95 was 0.47 ms, so the remaining visible delay needs a client/renderer trace before attribution. The native PNG preview rendered; 500-node signal and module graphs filtered and opened source without renderer errors. A ten-minute LOVE inspector soak passed with forced GC, recovering from 19 forced disconnects; the default-Node invocation failed an unreliable heap-range assertion.

**One new Medium editor defect was confirmed:** an unannotated forwarding function in a second module preserves `AgentInstance` on hover but loses class member completion in the consumer. The installed suggestion widget remained hidden. Direct factory use and an explicit return annotation on the forwarding function both worked. [FUP-01](FINDINGS.md) has the fixture, exact outputs, likely cause, and regression direction.

All 21 earlier unverified IDs were revisited. The 74-item historical ledger now has **72 fixed/rechecked, one rechecked design question, and one superseded documentation claim**. At this baseline, the two explorer rows relied on current unit tests; the [November 12 replay](HOST_AND_SOAK_RECHECK.md) now verifies them in an installed host and records a refresh-race fix. The explicit import-cleanup action still changes initialization behavior by design and is warned about in the docs.

## Review files

- [Editor and formatter latency](EDITOR_PERFORMANCE.md) — visible menu, hover, file switching, and saves.
- [New finding](FINDINGS.md) and [cross-file fixture](fixtures/crossfile) — class/factory forwarding loss.
- [Large previews and native asset](PREVIEWS.md) — rendered host result and [PNG capture](native-asset.png).
- [Historical recheck](PRIOR_RECHECK.md) — all 21 dispositions.
- [Verification log](VERIFICATION.md) — commands, raw data links, environment, and limits.
- [Installed-host and soak recheck](HOST_AND_SOAK_RECHECK.md) — packaged extension filesystem replay and retained-memory measurement.
- [November 12 qualification](NOV12_QUALIFICATION.md) — Starfall visible completion versus a previous package, compiler-produced graph, non-PNG assets, and extended inspector soak.

## Recommended next work

1. Close the remaining completion timing gap with exact LSP wire-send/server-receive and widget-paint marks. The new trace measures trigger, completion-provider entry/return, and visible labels in Starfall against a previous package; its final stage includes client scheduling and rendering.
2. Check graph fidelity and navigation in a hand-authored large game. The new compiler-produced 121-module fixture establishes DOM count parity, but it is generated, and non-PNG qualification found a font-preview gap.
3. Exercise the inspector in a sustained interactive game session and on other platforms. The 15-minute automated Starfall run passed but can spend time in the game-over scene.

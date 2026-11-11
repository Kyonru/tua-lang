# Feature audit follow-up: editor latency, previews, historical cases

**Revision:** `ab1b69c506f962ab239e015d2db4c38764fd592a` (2026-11-11, macOS arm64). **Scope:** the remaining installed-editor/performance qualifications and 21 historical IDs from the [feature and preview audit](../feature-and-preview-2026-11-11/README.md). This pass investigated and documented behavior; it made no product-code changes.

The editor was responsive in the tested synthetic workloads, with a visible warm suggestion median around 161 ms for an imported table and around 108 ms for a one-hop imported class factory. Keyboard-invoked class hover appeared around 10 ms median; pointer hover took 320–325 ms including VS Code's hover delay. Format-on-save took 12.7 ms median for a two-line file and 16–33 ms for a 1,001-line file. These are actual VS Code host timings, not just server replies. The server's 100-module warm completion p95 was 0.47 ms, so the remaining visible delay needs a client/renderer trace before attribution. The native PNG preview rendered; 500-node signal and module graphs filtered and opened source without renderer errors. A ten-minute LOVE inspector soak passed with forced GC, recovering from 19 forced disconnects; the default-Node invocation failed an unreliable heap-range assertion.

**One new Medium editor defect was confirmed:** an unannotated forwarding function in a second module preserves `AgentInstance` on hover but loses class member completion in the consumer. The installed suggestion widget remained hidden. Direct factory use and an explicit return annotation on the forwarding function both worked. [FUP-01](FINDINGS.md) has the fixture, exact outputs, likely cause, and regression direction.

All 21 earlier unverified IDs were revisited. The 74-item historical ledger now has **72 fixed/rechecked, one rechecked design question, and one superseded documentation claim**; the two explorer rows rely on current unit tests rather than a fresh installed-host filesystem event sequence. The explicit import-cleanup action still changes initialization behavior by design and is warned about in the docs. None of those 21 is a newly confirmed product defect.

## Review files

- [Editor and formatter latency](EDITOR_PERFORMANCE.md) — visible menu, hover, file switching, and saves.
- [New finding](FINDINGS.md) and [cross-file fixture](fixtures/crossfile) — class/factory forwarding loss.
- [Large previews and native asset](PREVIEWS.md) — rendered host result and [PNG capture](native-asset.png).
- [Historical recheck](PRIOR_RECHECK.md) — all 21 dispositions.
- [Verification log](VERIFICATION.md) — commands, raw data links, environment, and limits.

## Recommended next work

1. Fix FUP-01 in shared module type provenance, then add a project/LSP regression for the full chain and edit recovery.
2. Instrument trigger → client send → server response → widget paint for visible completion; measure an actual game project and a previous version before setting a latency target.
3. Recheck the sidebar's live filesystem events and a real large graph against compiler output; extend native preview qualification to other asset types.
4. Make the inspector soak's heap assertion measure retained memory reliably by using forced GC or post-GC samples; then repeat on longer game sessions.

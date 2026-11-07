# External API and MCP audit

Candidate 0.6.0, shared baseline 9d490dc. Three confirmed defects: two medium, one low. No product changes, network calls, model calls, Lua execution, or Cargo rebuilds. Applied security-auditor, test-auditor, architecture-steward, audit-lead and language-auditor guidance.

Reproduce all probes: `python3 internal/audits/features-2026-11-07/api_mcp/probe.py`. Python 3 launches the absolute `target/release/tua api|mcp --stdio --workspace …/api_mcp/fixture`. Exact requests, responses, process exits, stderr and fixture hashes are in `evidence.json`; indexes below are zero-based array indexes. Source and outside canary live entirely within this lane. The harness deliberately changes and restores one fixture during refresh testing; before/after hashes match, no overlay file is created. This is bounded read-only evidence, not filesystem syscall tracing.

| Surface | Coverage and result |
| --- | --- |
| API lifecycle/framing | Tested pre-init rejection, initialize, shutdown, malformed JSON recovery, unknown method, oversized 8 MiB frame rejection, invalid UTF-8 fail-closed exit |
| MCP lifecycle/framing | Tested pinned 2025-11-25 initialization, 12 advertised tools called, structured results, unknown tool/error method, CRLF, EOF/shutdown |
| Privacy/path boundaries | All three source-returning API methods rejected without sourceContext; opt-in completion worked; relative traversal and absolute outside path rejected; outside-file symlink skipped during indexing and preview rejected |
| Analysis | Diagnostics, semantic facts, graph, references, asset metadata, formatting and code actions exercised; fixture graph/assets/references are minimal/empty, not a full semantic correctness oracle |
| Completion | Context acquisition and candidate `1` accepted; old context after overlay rejected as stale; MCP bogus context rejected as stale |
| Overlay/refresh | Existing-file overlay survives refresh and clear restores disk; disk edit invisible until refresh, visible afterward; revision and new-overlay aggregation defects below |
| Cancellation | Cancellation notification silently consumed followed by successful ping. No active cancellation implementation; synchronous dispatch inspected, no concurrent long-request cancellation stress |
| Not exercised | Native Windows path behavior, SDK interoperability, sockets (none offered), concurrent clients, adversarial nesting/CPU exhaustion, type-library root/symlink permutations, UTF-16 position semantics |

## API-MCP-01 — Refresh reuses analysis revisions across changed snapshots

**Medium · confirmed defect · new relative to searched prior audit Markdown and ROADMAP.md.**

Reproduction: events 19–28. Set an overlay: revision becomes 6. Clear it: revision falls to 5. Change `src/main.tua` on disk from the incomplete completion fixture to `local disk_changed: string = "new"`, query semantic facts, refresh, query again. Both different snapshots report revision 5; after refresh the new binding appears. Expected: session revisions distinguish semantic snapshots and cannot move backwards, so consumers can invalidate revision-keyed results. Actual: revision aliases changed facts. This does not establish an incorrect completion acceptance: source fingerprints provide an additional guard and tested stale validation correctly rejected.

Source: `crates/tua-cli/src/api.rs:161` replaces the host with `load_analysis`'s newly initialized host (`:608`); returned revision comes directly from host at `:273`/`:316`. Docs claim both servers reuse one AnalysisHost for the session. Existing overlay test at `:995` checks overlay presence/content but not revisions or disk changes. Smallest improvement: refresh inputs through the existing host (including removal/config/root updates), retaining its revision semantics. Regression: same file count, changed file contents and overlay clear must each produce a distinct increasing revision; unchanged queries retain it. Include API and MCP refresh adapters.

## API-MCP-02 — Project diagnostics omit new unsaved overlay files

**Medium · confirmed defect · new.**

Reproduction: events 60–68. `setOverlay` on nonexistent `src/new.tua` with `local broken: number = "wrong"`. Path-specific diagnostics returns TL1003 and TL4005. Parameterless diagnostics returns only the original main file's parse error, before and after refresh. Module graph does include the new overlay. Expected: project diagnostics include analyzed unsaved files, consistent with per-file diagnostics and module graph. Actual: project-wide clients miss the type error until the document exists on disk.

Source: `crates/tua-cli/src/api.rs:286` enumerates `indexed_paths`, populated only by disk discovery at `:618`; `setOverlay` at `:238` never extends that set. Existing lifecycle test at `:995` overlays an existing file only. Smallest improvement: derive effective project paths from the analysis snapshot or a shared union of disk and overlay paths; preserve disk-only inventory labeling if useful. Regression: add unsaved erroneous file, compare project/per-file diagnostics, refresh, clear, and verify error disappears only on clear. Keep unrelated/ignored file policy explicit.

## API-MCP-03 — JSON-RPC envelope and notification handling is inconsistent

**Low · confirmed defect · new.**

Reproduction: events 30–31 and 53–56. API accepts `jsonrpc:"1.0"` and an object ID `{}` and echoes the object ID in a purported JSON-RPC 2.0 success response. API and MCP initialization notifications (no ID) produce responses with `id:null`. Expected under the advertised JSON-RPC 2.0 contract: validate version/ID shape and never answer notifications. Actual: malformed envelopes execute methods; selected notifications generate unsolicited responses. This is interoperability/robustness, not demonstrated privilege escalation.

Source: `crates/tua-cli/src/api.rs:172` and `:541` extract ID/method without shared envelope validation; initialization branches run before the notification check. Existing framing tests `:958` and `:984` cover JSON syntax/CRLF only. Smallest improvement: shared request-envelope validation and notification-aware response suppression before either adapter dispatch. Regression matrix: version missing/wrong, object/array IDs, scalar messages, initialization/ping/shutdown notifications, normal request IDs, malformed JSON recovery. Preserve documented no-batch policy if intentionally selected; current docs do not define it.

## Capability and protocol gaps (not additional defect totals)

- Both adapters are synchronous; API cancellation is unknown and MCP cancellation notifications are ignored. Document this limitation, and add cancellation only through shared query work if long operations become material. Do not claim the no-op notification probe demonstrates active cancellation.
- MCP exposes no overlay tools, matching its documented 12-tool list. Clients needing unsaved editor buffers must use the API; consider making that distinction explicit in docs.
- API documentation calls initialization “negotiate protocol version 1” but its example supplies no version and implementation never checks one. Define whether this is version discovery or negotiation before adding compatibility machinery.
- MCP advertises restrictive input schemas but dispatch does not enforce additional properties (the harness intentionally supplies extra fields to all tools). Required-field validation remains per method. Treat strict schema enforcement as a design choice; no harmful outcome established.
- No high-severity path escape or source-gate bypass was demonstrated. The skipped symlink is not evidence that all configuration-root/symlink combinations are safe.

Known/new check: searched ROADMAP.md and earlier audit Markdown for API/MCP, revisions and symlink issues; none identifies these three concrete cases. Earlier shared project-ownership findings concern other boundaries. No skill update needed.

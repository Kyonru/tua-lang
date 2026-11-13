---
name: tua-performance
description: Investigate or improve Tua typing latency, LSP and compiler cost, preview refreshes, runtime inspector overhead, build time, or memory use. Use for measured performance work across the toolchain, not a routine code change with no performance claim.
---

# Tua Performance

Optimize only measured work that cannot help the user at that moment. Keep
editor results, diagnostics, preview freshness, and emitted Lua correct.

## Attribute The Cost

- Record revision, binary hash, host, fixture, sample count, and baseline
  failures. Use bundled or consented projects; keep private game source out of
  reports. Compare cold, warm, post-edit, contended, and long-session paths.
- Separate physical trigger-to-widget time from client dispatch, server
  request, response conversion, and paint. A fast LSP response does not prove
  a fast visible menu; a delayed menu does not prove a slow server.
- Use the existing representative LSP profiler, editor replay, Server Health,
  cache counters, real-server host tests, and inspector soak where relevant.
  Read the harness before trusting its cursor anchors or memory samples.
- Trace work across compiler snapshot, module/export resolution, request
  scheduling, extension event listeners, and webview rendering. Avoid deriving
  a root cause from one cold sample or a synthetic project alone.

## Fix Without Losing DevEx

- Preserve current-document and imported type freshness, cancellation,
  diagnostics, and format-on-save behavior. A hidden panel may defer visual
  work only if it catches up when visible; diagnostic producers must still
  publish their hints. Keep source navigation and live runtime state current.
- Prefer shared analysis or a bounded client refresh gate over suppressing a
  feature. Check that a candidate actually reduces work and that relevant
  source, dependency, or workspace edits still invalidate it.
- Add a regression that asserts behavior and request counts or timing where
  stable. Re-run the same workload after the final edit, then the affected
  crate, LSP, extension-host, and runtime gates. Report protocol and visible
  measurements separately, including outliers and platform limits.

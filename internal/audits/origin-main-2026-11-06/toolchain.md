# Toolchain, test integration and trust-boundary review

Compared the new testing CLI/discovery/report/coverage pipeline, test source-root isolation, watch/build/check/fmt integration, extension Test Explorer/model/process/coverage, command registration, offline references, and trusted CI changes against local origin/main. Source inspection is distinguished from executed tests below; no external service or cross-platform result is implied.

## TOOL-01 — Test Explorer cancellation could retain the queue indefinitely

Confirmed by tracing runProcess: timeout/cancellation/output cap only sends SIGTERM on Unix; a child can register a handler and continue holding its pipes. The Promise resolves on close, so the serialized Test Explorer queue remains blocked. This affects ordinary hanging tests, not only malicious code. New regression starts a real Node child that ignores SIGTERM, requires discovery timeout rejection within a bounded interval, and passes after a one-second force-kill fallback. Existing tests cancelled only children with the default signal handler.

Fixed in editors/vscode/src/testing/process.js. Repeated stop requests share one escalation timer, cleanup clears it, process groups retain existing descendant termination. Windows retains taskkill /T /F; Windows execution is not qualified on this host.

Verification: node --test editors/vscode/test/unit/testing.test.js — six passed, including ignored-SIGTERM timeout. No shell interpretation of user arguments; Unicode output control remains passing.

## TOOL-02 — CLI test output capture was unbounded

The new CLI used Command::output, retaining arbitrarily large stdout/stderr before remapping. The extension's 16-MiB reader limit cannot protect the CLI child while it buffers before emitting. This is a concrete resource-bound gap; no claim of measured OOM/exploit is made.

Fixed with capped concurrent stream capture (8 MiB each), continuing to drain both pipes to avoid backpressure deadlocks. Exit status and structured JSON report/coverage are preserved; human output explains truncation. Independent review caught an interaction with the extension's combined-output limit. Report-backed editor runs now drain/truncate logs too, while discovery remains fail-closed because its stdout is JSON. A real 17-MiB child preserves status 7 and bounded output. Regression exercises output beyond pipe capacity on both streams and a nonzero status. Process execution remains explicit trusted-workspace action. Long-running legitimate tests have no new arbitrary runtime timeout.

## Checked boundaries and remaining limits

- Test Explorer discovery and execution check workspace trust before running a configured executable. Continuous testing is explicitly enabled; edits alone refresh discovery.
- Spawn uses argument arrays with shell:false. CLI filter/tags are passed as arguments, source selectors reject parent/absolute traversal, temporary outputs use existing path guards.
- Static discovery parses tests rather than executing them. Dedicated roots are included in checking/formatting and excluded from production build/watch according to shared project policy.
- JSON result and coverage files are distinct from stdout, and coverage validates completion/schema. Raw output is not treated as a trustworthy test event stream.
- Annotation hover trusts only its specific offline-guide command. LOVE reference embeds bundled HTML under a restrictive no-network policy; upstream catalog integrity is covered separately by generator drift CI, not re-fetched here.
- Trusted-main/reusable/manual CI includes Rust/platform tests, real Linux Busted, runtime examples, extension unit and editor-host tests. No missing-untrusted-PR trigger finding.
- Captured raw JSON report size, descendants retaining pipes after a finished CLI test, workspace-folder changes during an active Test Explorer session, native Windows process termination and extended continuous-run sessions remain robustness/qualification areas. They are not represented as reproduced failures.

## Integrated handoff

The checkpoints above preserve investigation history, including corrected fixture mistakes and intermediate failures. [Final integrated verification](VERIFICATION.md) is authoritative for the finished working tree, rebuilt-server replay and test VSIX; earlier counts and intermediate binaries are not final package provenance.

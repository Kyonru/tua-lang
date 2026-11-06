# Protocol and project integration

## WF-02 — Workspace-owned module resolution

The module-path inventory already retained workspace ownership; the module graph flattened source/output roots. Added an explicit `build_with_workspaces` path using the same ProjectModuleWorkspace facts. Existing standalone build callers preserve explicit-root behavior. Each document resolves through the longest containing project root, using that project's require roots and corresponding source/output roots.

Regression: two roots export incompatible `shared` values, including a Lua provider. Hover and definitions must use their own project in either folder order. A core graph regression also requires a missing local module to remain missing when another workspace has it, both initially and after removing the local provider. Follow-up regressions also cover conflicting per-workspace declaration libraries, a third workspace with no overlay, and an explicitly configured shared library outside the workspace roots. Declaration aliases now retain the same project ownership as module resolution; standalone callers preserve their existing explicit-root policy.

Initial focused test targeted the module name rather than its member; corrected to exact member before asserting field types. Full first LSP run passes the corrected workspace isolation test. Prior devex evidence remains unchanged.

## CMP-01 — Invoked completion and project augmentation

Preserved current-file candidate inference, but routed its result through the same snapshot autoimport augmentation used by other request contexts. Extracted augmentation rather than duplicating import discovery. Permanent regression tests omitted/invoked/retrigger contexts and resolved import edits.

This deliberately restores missing project facts rather than claiming the previous local-only path was semantically complete. The cache regression now requires indexing both project files once and reusing them across subsequent requests, instead of forbidding the second file from ever being parsed. Final current-host replay is recorded in the integrated verification report.

## CMP-06 — Snippet capability negotiation

Initialize records snippetSupport, defaulting absent support to false. Initial completion and resolve share normalization: template-only snippets are omitted; callable candidates retain plain identifiers when snippets are unsupported. Supported clients keep snippets. Tests cover true/false/absent support and resolve. Existing snippet-specific tests now explicitly advertise the capability.

## Test harness correction

A preemption test's needle `o` matched the first `local` token, not its intended new identifier. Its replacement multiline anchor exposed the old position helper's same-line-only arithmetic. UTF-16 migration owns corrected helpers so tests derive positions from the exact text rather than guess offsets.

## Integrated handoff

The checkpoints above preserve investigation history, including corrected fixture mistakes and intermediate failures. [Final integrated verification](VERIFICATION.md) is authoritative for the finished working tree, rebuilt-server replay and test VSIX; earlier counts and intermediate binaries are not final package provenance.

# Remaining backlog implementation — November 2026

This pass follows `01a642f`. Each implemented behavior receives a separate commit;
remaining design/resource-dependent work is not silently marked complete.
Artifacts and verification logs are in `target/remaining-backlog/`.

## Completed

- `88bd6a6`: dedicated test roots shared by analysis/check/format/test discovery,
  isolated test output, and production/watch/package spec exclusion. Verified
  with 1,639 core tests (one ignored), 375 LSP tests, all 124 CLI integration
  tests, config/model regressions and the real three-test Busted example.
- `6fd9fe7`: static Busted discovery and structured JSON reporting, including
  mapped native/Lua locations, stdout isolation and stale-report prevention.
  Parser, argument, real Busted report and documentation checks pass.

## In progress

- VS Code Test Explorer: lazy discovery, selected runs, cancellation, and
  opt-in continuous runs. Unit and extension activation checks pass; installed
  editor qualification remains part of the final release pass.
- Richer Busted contracts, coverage and explicit LOVE runtime testing.
- Annotation generation/update, documentation-aware parameter rename and
  annotation navigation/recovery.
- Further cold responsiveness, installed-editor/visual/long-session evidence,
  migration/upgrade qualification and final package.

The user replied “yes” to the request for a concrete reference/implementation
reproduction, without a symbol, file or expected destination. That report is
still unspecified; existing navigation regressions remain the baseline.

Test Explorer follows the [VS Code Testing API](https://code.visualstudio.com/api/extension-guides/testing),
using core/CLI discovery instead of a second Lua parser in the extension.

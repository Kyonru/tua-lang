# Typing-lane verification log

- Revision: `d7aed2e5f1297b80a948e055985375bceaff6f0c`.
- `node internal/audits/devex-2026-11-06/typing/run.js`: completed 64 isolated
  CLI scenarios, 94 hover/completion/signature requests and 64 pull-diagnostic
  requests. No harness errors in final results. The 36 accepted and 28 rejected
  CLI programs include deliberately invalid inputs and suspected defects;
  these counts are outcomes, not a passing-test count.
- Final raw output: `results.json`; source/configuration: `cases.json`.
- `lua internal/audits/devex-2026-11-06/typing/runtime-oracles.lua`: passed three
  assertions/prints for final-call expansion and missing scalar/parenthesized
  assignment slots. Lua 5.4.8; no LuaJIT execution claimed.
- `node internal/audits/devex-2026-11-06/typing/verify-multi-root.js`: independent
  reproduction of wrong-project module resolution; exact output in
  `multi-root-crosscheck.json`. This is a confirmed defect, not a passing result.
- Independent shadow/provenance cross-checks also included in the full corpus;
  earlier isolated evidence retained in `results-shadowed.json` and
  `results-provenance-crosscheck.json` for review references.
- Source inspection: docs/typing.md, docs/language.md, docs/classes.md,
  docs/architecture.md; core checking/type inference/semantic/narrowing paths;
  existing check/type tests referenced individually in REPORT.md.
- No native product tests added or changed. Lead's baseline workspace suite
  independently passed (1,640 core, 375 LSP, 127 CLI integration); this lane did
  not duplicate that expensive full-suite run.
- No GUI automation in this lane. All editor observations here are actual stdio
  LSP protocol responses, not VS Code rendering/client behavior.
- First exploratory probes included an invalid assert anchor and some root-name
  positions where a final field was intended; those harness issues were corrected
  and the whole corpus rerun. They are not product findings.
- Existing repository work preserved. Only this audit's typing directory and
  ignored generated target projects were written. No commits or publications.

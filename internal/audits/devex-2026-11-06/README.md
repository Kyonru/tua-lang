# Tua developer-experience audit — 2026-11-06

Tua's straightforward single-project editing paths are useful, but several
boundaries still undermine trust: symbol identity, contextual typing, native
class receiver recovery, completion insertion and source coordinates. The audit
found **18 confirmed defects (8 High, 10 Medium)**, with every High finding
independently reproduced. One additional constructor-chain symptom is recorded
with unresolved root-cause attribution and excluded from the defect count.

The most consequential findings are:

- Non-BMP text can cause rename to corrupt surrounding source (**WF-01**),
  confirmed by applying the edit in actual VS Code.
- A trailing comment on a typed function header panics CLI analysis and an LSP
  diagnostic worker; a suppression quick fix can produce it (**CMP-02**).
- Workspace projects can resolve one another's same-name modules (**WF-02**),
  and nested same-name functions can resolve the outer contract (**HOV-02**).
- Invalid callback returns and call-result property assignments pass checking
  despite useful declared/editor types (**TYP-01**, **TYP-02**).
- Native class assistance depends on assignment whitespace (**CMP-07**), and
  invoked identifier completion omits available project autoimports (**CMP-01**).

Ordinary annotated/inferred locals, simple records, direct class members,
imported LuaDoc contracts, many Lua/LOVE documentation paths, nested-call active
parameter selection and single-root dependency updates worked in the exercised
cases. Unsaved changes, closing overlays, watched dependency replacement,
configuration reload and invalid-syntax recovery received direct protocol probes.
The existing workspace test suite passed, demonstrating important gaps in its
current editor and semantic-boundary coverage rather than absence of defects.

## Read the results

1. [Consolidated findings](FINDINGS.md) — impact, severity and links to full
   reproductions, source references, evidence and regression directions.
2. [Prioritized backlog](BACKLOG.md) — separate correctness, capability,
   usability and performance work, with dependencies, effort and acceptance criteria.
3. [Coverage matrix](COVERAGE.md) — all 58 planned areas investigated:
   57 tested and one architecture row inspected only. “Tested” includes failing
   behavior and does not mean every permutation passed.
4. [Verification log and reproduction instructions](VERIFICATION.md) — commands,
   outcomes, cross-checks, environment, corrections and limits.
5. [Architecture and test mapping](ARCHITECTURE.md) — shared infrastructure,
   consumption paths and why isolated surface fixes would be fragile.

Full lane reports: [typing](typing/REPORT.md),
[completion](completion/REPORT.md), [hover/signatures](hover/REPORT.md),
[workflows and performance](workflow/REPORT.md).
Machine-readable inventories: [findings](findings.json), [coverage](coverage.json).

## Recommended first implementation sequence

First protect source integrity and analysis availability: fix UTF-16 boundaries
and typed-header trivia. Then establish project-scoped module identity and
lexical declaration identity. Improve canonical constructor/callable/expression
facts before adding editor-specific fallbacks. Follow with a shared completion
edit planner, metadata/value-list corrections and provenance-aware hover.
The backlog supplies narrower acceptance criteria for each independent change.

## Evidence and limits

Baseline: `d7aed2e5f1297b80a948e055985375bceaff6f0c`, initially clean.
Investigation only: no product changes, commits, pushes or publications.
Reports and portable probes are contained here; generated scratch is ignored
under `target/devex-audit-2026-11-06/` or disposable OS temporary directories.

Verification included the release build, full workspace tests, 64 typing
scenarios, 45 completion scenarios in three request/capability modes, focused
hover/signature trials, independent cross-reproductions, Lua runtime controls,
three bundled-project checks, lifecycle probes and selected actual VS Code
provider/UI/edit operations. Successful observational probe execution is not a
behavioral pass. All major audit areas were investigated, not every possible
program or platform.

Performance measurements cover three bundled projects on this host: cold
completion medians approximately 37–118 ms, warm completion about 0.1–0.2 ms,
and post-edit completion approximately 4–14 ms. They are protocol round trips,
not end-to-end typing latency or proof of improvement over prior versions.

Other editors were assessed only at protocol level. Cross-platform runtime,
external-service, exhaustive LOVE runtime/catalog, full-game visual and
hours-long editing qualification remain outside this audit. Source-backed root
causes have not been validated by patches. The detailed reports explicitly
separate defects, capability gaps, uncertain attribution and unverified ideas.

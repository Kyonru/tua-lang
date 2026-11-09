# LSP Peer Capability Review And Tua Plan

Date: 2026-11-09  
Repository revision reviewed: `1c0a140fd4c9f664ce576576d773247df0e07319`

## Summary

The Kotlin LSP's published LSP feature list does not reveal a missing standard
request in Tua. Tua already advertises and documents completion, diagnostics,
quick fixes, semantic tokens, organize imports, rename, formatting, hover and
documentation, call hierarchy, and folding. Tua also implements signature
help, type definition, implementation, references, document links/highlights,
selection ranges, code lenses, inlay hints, and workspace symbols.

The important distinction is feature presence versus feature depth. Kotlin's
IntelliJ implementation may provide more language-specific completion,
inspections, and refactorings; enabling the same LSP method does not establish
parity in quality. JetBrains' broader product also integrates JVM build
systems, test/run/debug workflows, and library sources. Tua already has its
own LOVE run/debug/package workflows and Busted-backed testing. Gradle, Maven,
Bazel, Spring, and JVM module-language-level support do not fit Tua's language
or project model.

The useful Tua work is therefore targeted workflow polish, not adding duplicate
LSP methods:

1. Generate correct, clearly unfinished class methods for missing protocol
   requirements through a compiler-backed code action.
2. Audit and improve navigation from external Lua dependencies and generated
   library declarations to their best available source or documentation.
3. Profile real large-workspace open, completion, navigation, and indexing
   workflows. Add visible progress only for measured long operations; keep
   keystroke requests cancellable and responsive.
4. Expand refactorings one validated, high-value action at a time after
   collecting Tua-specific use cases.
5. Keep a protocol-level cross-editor regression suite as the stable contract;
   keep VS Code-specific testing, debug, and UI behavior at the extension edge.

## Comparison

| Capability | Kotlin LSP / JetBrains listing | Tua state | Tua opportunity |
| --- | --- | --- | --- |
| Completion, diagnostics, quick fixes | Listed as IntelliJ-powered | Present; Tua diagnostics and code actions use shared core analysis | Compare quality with concrete Tua fixtures; do not equate provider registration with semantic parity. |
| Semantic highlighting | Listed | Full/range/delta semantic tokens and TextMate scopes | No missing protocol surface. Improve classifications only against a reproducible editor case. |
| Organize imports | Listed | Organize, sort, and remove-unused-import source actions | Existing capability. Use compiler-confirmed edits, preserve comments, and test idempotence. |
| Rename, navigation, hover/docs | Listed | Definition, type definition, implementation, references, rename, document links, hover, offline LOVE API docs | Investigate any library navigation that stops at generated stubs or declaration metadata. Current `.tua`, indexed `.lua`, LuaDoc, LOVE, and virtual schema routes are already documented. |
| Signature help | Listed by current JetBrains docs | Present, including nested call retriggering and imported/generated signatures | No missing protocol request; focus on language-semantic correctness. |
| Call hierarchy | Listed | Present in Tua LSP | No missing endpoint. Improve project/library coverage only with a failing example. |
| Folding | Listed | Present with functions, blocks, tables, aliases, and comments | No missing endpoint. |
| Code lenses | Kotlin feature list/release notes describe run/debug lenses for `main`; Tua advertises code lenses for references | Present for references to exports and type aliases; tests use VS Code Testing API | Do not add a game-run lens mechanically. Consider test lenses only if they improve on the existing gutter/Test Explorer flow. |
| Run/debug/tests | JetBrains extension provides JVM run/debug and tests | Tua extension provides LOVE run/debug/package and Busted test execution | Different runtime/domain; no JVM debugger/build integration needed. Keep LSP methods separate from DAP and VS Code Testing API. |
| Project import/build | Gradle, Maven, Bazel, per-module JDK/language levels, Spring integration | Tua has `tua.toml`, project module graph, Lua/Tua/LOVE runtime | Not a gap to copy. Improve Tua config diagnostics or workspace selection only from Tua use cases. |
| Library sources | JetBrains docs advertise library documentation/source access; release notes mention source download for Gradle | Tua has source/module navigation and generated LOVE/Lua library declarations/docs; arbitrary dependency source lookup is not established as a uniform capability | Verify external dependency target coverage, licensing, and source availability, then design source navigation over the existing module/type indexes. |
| Large-project indexing | Marketplace advertises fast indexing and monorepo analysis; releases mention indexing percentage, faster indexing, persisted workspace models | Tua has revisioned snapshots, incremental reuse, health/cache/latency reporting, lazy diagnostics; no general progress percentage or persistent semantic index surfaced in the LSP docs | Measure before adding persistence. Progress is appropriate only for explicit long-running work, not every interactive request. |
| Snippets/live templates | Kotlin release notes describe `main`, `sout`, `if` templates | Tua supplies language and documentation snippets, plus generated call snippets | No obvious missing capability; compare coverage from real Tua code patterns. |

Tua's current protocol inventory is documented in
[`docs/lsp.md`](../docs/lsp.md), and its advertised capabilities are registered
in `crates/tua-lsp/src/server/capabilities.rs`. The feature inventory should
remain factual; the recommendations below concern behavior depth and editor
workflow rather than endpoint count.

## Proposed Epics

### Epic 1 — Generate missing protocol methods

**Priority:** High; exact Tua-specific gap already identified in the
generics/protocol plan.  
**Size:** M.  
**Owner:** `tua-core` code actions and shared protocol conformance facts; thin
LSP and VS Code adapters.

- Offer an action on a class missing one or more methods required by an
  `implements` clause.
- Derive method names, parameter and return types, docs, and source insertion
  from the resolved protocol declaration. Account for inherited methods and
  multiple protocol obligations.
- Label generated bodies as TODO/unimplemented; do not generate a fabricated
  successful return value.
- Suppress the action if imported identity, edit range, or requirements are
  ambiguous. Apply one validated source edit and preserve comments/formatting.

**Acceptance:** positive and negative core cases for local/imported protocols,
inherited satisfaction, multiple contracts, and incomplete class bodies; LSP
code-action response and VS Code apply test; result formats and rechecks without
new diagnostics except the deliberate TODO marker/diagnostic.

### Epic 2 — Complete source-backed library navigation

**Priority:** Medium; begin with a capability audit, not an assumed defect.  
**Size:** S for inventory, M/L if new source indexing is justified.  
**Owner:** shared module/type indexes, core navigation/hover, then LSP mapping.

- Inventory definitions for bundled Lua, configured Lua modules, LOVE entries,
  LuaDoc declarations, generated stubs, and dependency modules.
- For each, record whether Tua navigates to original source, declaration,
  generated stub, documentation page, or nowhere, and verify hover/document
  links agree with definition.
- Where real source is available and legally distributable, index/read it
  through existing workspace and module infrastructure; otherwise provide a
  stable documentation target. Never guess into an unresolved dynamic `require`.
- Preserve source identity and URI/range encoding; respect workspace trust and
  avoid network retrieval during typing.

**Acceptance:** a table-driven definition/hover/document-link suite across the
source classes above, including missing, duplicate, and dynamic dependency
cases; no new request-time filesystem or network scans.

### Epic 3 — Measured workspace progress and responsiveness

**Priority:** Medium; qualification first.  
**Size:** S for instrumentation/replay, M if a specific long operation needs
progress or cancellation changes.  
**Owner:** analysis host and LSP scheduler; VS Code presents progress.

- Measure cold workspace open, warming, completion, definition/workspace
  symbols, and dependency edits on the existing game ports and a wide synthetic
  module graph. Record p50/p95, memory, revision, and cache state.
- Reuse Tua's existing health, cache, queue, and request-latency telemetry.
- Add standard LSP work-done progress only for operations shown to take long
  enough to benefit, with cancellation and stale-revision behavior tested.
- Investigate persistent indexing only if same-workspace restart cost is
  significant and the saved model can be safely revisioned and bounded.

**Acceptance:** repeatable before/after traces, no interactive-latency
regression, no stale progress/results after edits, and memory/retention evidence
for any persistence change.

### Epic 4 — Add high-value refactorings from Tua workflows

**Priority:** Medium; use a short user-workflow inventory to pick the first
action.  
**Size:** XS/M per action.  
**Owner:** core code-action/refactor query and LSP adapter.

- Do not aim for an IntelliJ-sized catalogue. Rank actions by recurring Tua
  tasks and correctness risk.
- Candidate first actions: implement missing protocol methods (Epic 1), safe
  module move with require/import updates, and create a test from a function.
  Existing rename, file-rename, import, and test workflows may already cover
  some of these; verify first to avoid duplicate commands.
- Each action must use semantic identity, return versioned edits, refuse
  ambiguous/dynamic edits, and have core plus real-LSP application tests.

**Acceptance:** every action has an exact supported/declined behavior, edit
collision checks, an unsaved-buffer case, and a compile/check-after-apply test.

### Epic 5 — Cross-editor LSP contract suite

**Priority:** Medium; continuous qualification.  
**Size:** S/M.  
**Owner:** `tua-lsp` tests and release tooling.

- Keep language semantics tests in `tua-core` and protocol payload tests in
  `tua-lsp` independent of VS Code.
- Add a small client-compatibility matrix for a VS Code host and at least one
  non-VS-Code LSP client, covering UTF-16 edits, cancellation, snippets,
  diagnostics refresh, code actions, and server shutdown.
- Keep tasks, debug adapter, and Testing API tests in the VS Code extension;
  those are not generic LSP capabilities.

**Acceptance:** reproducible local/CI client versions and fixtures; failures
distinguish client quirks from protocol-server defects.

## Source-Code Inspiration And License Boundary

The public `Kotlin/kotlin-lsp` repository root contains an Apache-2.0 license,
and the inspected `LSKotlinOrganizeImportsCodeActionProvider.kt` file carries an
Apache-2.0 source header. It is a narrow provider: it declares language,
title, command and source-action kind, asks Kotlin's semantic import optimizer
for the result, then returns one LSP text edit. The useful pattern is the
small protocol adapter around an authoritative semantic operation. Tua already
has the same separation in its core code-action queries and LSP protocol
conversion; do not port Kotlin PSI or IntelliJ APIs.

The repository README also says the implementation relies on proprietary
JetBrains Air/Fleet components and is only partially open-source. Some
directories include `OWNERSHIP` metadata. The Marketplace extension is
distributed under JetBrains' Free Plugin License according to its release
notes, rather than the repository's Apache license. Therefore:

- Reuse only source files whose own header and repository ownership/license
  metadata clearly allow it; preserve license, copyright, NOTICE, and modified
  file notices required by Apache-2.0.
- Do not copy proprietary implementation, closed-source interfaces, Marketplace
  extension assets/code, or code whose upstream dependency license is unclear.
- Prefer architectural ideas and independently implemented Rust code. The
  provider above is illustrative; Tua's existing shared semantic contracts are
  the source of truth.

## Sources

- [Kotlin LSP README and feature list](https://github.com/Kotlin/kotlin-lsp)
- [Kotlin LSP Apache-2.0 license](https://github.com/Kotlin/kotlin-lsp/blob/main/LICENSE.txt)
- [Organize imports provider source](https://github.com/Kotlin/kotlin-lsp/blob/main/features-impl/kotlin/src/com/jetbrains/ls/api/features/impl/kotlin/codeActions/LSKotlinOrganizeImportsCodeActionProvider.kt)
- [Kotlin LSP release notes](https://github.com/Kotlin/kotlin-lsp/blob/main/RELEASES.md)
- [JetBrains Java and Kotlin extension overview](https://www.jetbrains.com/help/intellij-vscode/About-instance.html)
- [Marketplace listing](https://marketplace.visualstudio.com/items?itemName=JetBrains.intellij-server)
- [Tua LSP documentation](../docs/lsp.md)
- [Tua LSP capability registration](../crates/tua-lsp/src/server/capabilities.rs)

# Origin/main audit and remediation — 2026-11-06

**Commit status:** subsequently committed at the user’s request; see [commit handoff](COMMIT_HANDOFF.md). References to uncommitted changes below preserve the original audit checkpoint.

Comparison: local `origin/main` **b40dbb445f7d8ba36e9312909be7c5d2949ac4af**
to starting HEAD **d7aed2e5f1297b80a948e055985375bceaff6f0c**: 203 changed files.
The pre-existing untracked developer-experience audit was preserved. No commits,
pushes or publications were made. This report describes additional working-tree
fixes; it does not pretend every defect was introduced by the compared commits.

The change set substantially expands Tua's editor semantics, native testing,
class/module interoperability and documentation. The highest risks were at
shared boundaries: declaration identity, project ownership, source coordinates,
contextual callable checking and emission trivia. Four coordinated lanes audited
those boundaries and the CLI/extension/runtime integration, then implemented
reproducible fixes that do not require a language-design decision.

## Results and review path

The fixes address the **18 prior confirmed defect families**, including the
constructor-chain symptom previously left unattributed, and **three additional
runtime/testing issues**: private helper closures during hot reload, unresponsive
test-child termination and unbounded CLI output capture. Independent review
extended the identity fixes to inferred wrapper returns and declaration-library
overlays, and caught interactions between CLI/editor output limits and legal
method-call whitespace. Those are consolidated under their existing causes. Three subsequent game-development
trials found one additional formatter indentation defect, independently reproduced
and fixed using the existing AST layout traversal. The consolidated total is
**22 confirmed defect families: 8 High and 14 Medium**. Severity reflects user
impact; runtime/testing additions and the indentation defect are Medium.

- [Typing and language contracts](typing.md): contextual callbacks, varargs,
  call-result fields, Lua value lists, lexical callable identity and project overlays.
- [Editor semantics](editor.md): constructors/chains, completion insertion,
  hover documentation/provenance, signatures and additional domain checks.
- [Emission and runtime](emission.md): comment-preserving lowering, class/source-map
  controls and Lua/LuaJIT hot-reload behavior.
- [Protocol and projects](protocol-project.md), [Unicode boundary](utf16.md):
  source-aware conversions, request parity, capabilities and workspace isolation.
- [Toolchain and trust boundaries](toolchain.md): test discovery/report/coverage,
  cancellation, bounded logs, output safety and trusted CI integration.
- [Independent integration review](independent-integration-review.md): findings
  that were caught during cross-review, rejected hypotheses and verification.
- [Changed-file inventory](file-coverage.md): every file in the comparison mapped
  to a lane. This is a coverage inventory, not a universal correctness claim.
- [Game-building trials](games/README.md): three independently developed playable LOVE games, protocol evidence, runtime smoke tests and findings.
- [Follow-up backlog](BACKLOG.md): remaining qualification, capability and design work.
- [Verification](VERIFICATION.md): final gates, rebuilt-server reproductions,
  performance observations and explicit qualification limits.

## Most consequential changes

1. Non-BMP text no longer shifts standard LSP edits/navigation; the same explicit
   source-aware conversion is used by custom graph/inspector/model responses.
2. Module and declaration-library resolution retain the importing project's
   ownership. A missing dependency cannot silently resolve from another project.
3. Same-name functions retain lexical identity in checking and inferred wrapper
   returns, rather than merely showing the same wrong signature on several surfaces.
4. Contextual callback returns, native typed varargs, terminal fields after calls,
   multi-return argument expansion and missing assignment slots retain contracts.
5. Native constructors work across whitespace/layout and direct chains; hover
   uses original-source provenance and preserves method identity/documentation.
6. Completion keeps autoimports across triggers, respects client capabilities,
   and produces valid edits for suffixes, filenames and existing LOVE calls.
7. Typed header comments no longer panic emission, and reloaded constructor
   helper closures retain preserved class tables.
8. Test cancellation and output bounds compose correctly across CLI and extension;
   large logs do not replace structured test status with a false failure.

## Decisions and qualification still requiring separate work

No new generics/protocols, multiple inheritance, class initialization order or
arbitrary closure-effect system was invented. Existing dynamic `any` policy is
preserved. Automatic conversion of invalid dot-key candidates to bracket syntax,
exact default-expression display, richer annotation authoring and type-based
overload ranking remain capability/design work, not broken-code fixes disguised
as feature additions.

This host cannot establish native Windows/Linux behavior, every LOVE API overload,
external-service correctness, full-game visual parity or hours-long editing
stability. Runtime tests on installed LOVE's LuaJIT exercise repository fixtures,
not full application ports. Current-host protocol latency is reported separately
from end-to-end typing latency and historical benchmark workloads.

The existing documentation/examples continue to teach supported syntax; focused
language/editor/testing/hot-reload docs and Unreleased notes are updated. No
new example language feature was introduced solely to demonstrate a regression.

## Final verification and local package

The composed final gate passed **2,276 Rust tests** (three intentionally ignored), **166 extension tests**, strict Clippy, metadata/language checks and docs validation. Fourteen rebuilt-server replay runners completed; all eight applied-edit fixtures check successfully. All three game probes and native smoke runs passed, and isolated VS Code confirmed autoimport visibility and correct Unicode rename. See [verification](VERIFICATION.md) for exact evidence and limits.

[Installable local VSIX for macOS arm64](../../../editors/vscode/tua-0.5.1-origin-audit.vsix). The archive was verified and its bundled server matches the final replay hash. This is an unpublished test build retaining version 0.5.1.

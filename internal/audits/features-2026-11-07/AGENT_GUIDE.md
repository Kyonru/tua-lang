# Feature audit instructions

Audit only. Preserve all product code and pending release changes. No commits,
pushes, installs into normal profiles, external messages, or product fixes.
Write all fixtures, raw evidence and REPORT.md under your assigned feature
subdirectory of internal/audits/features-2026-11-07/.

Read AGENTS.md if found (none at repository root), tua-audit-lead,
tua-language-auditor, and relevant feature skills from .codex/skills. Read the
owning docs and inspect implementation and existing tests. Use the already-built
absolute target/release/tua (version 0.6.0) for CLI probes. Prefer existing Lua
runtimes for emitted output; use actual LOVE when appropriate and feasible.
Avoid competing cargo rebuilds or full suites: focused cargo tests are allowed
if necessary, but first prefer CLI/runtime/LSP reproduction. No fabricated API
expectations: distinguish documented contract, reasonable expectation, and ideas.

Cover positive workflow, error/edge behavior, configuration/missing options,
compiler-to-runtime consistency, editor assistance where relevant, docs and test
gaps. Test actual public interfaces, not source inspection alone. Identify
at least one exercised representative workflow; keep negative probes minimal.
Do not invent findings to fill a quota. Record gaps even when no bug is found.

REPORT.md: concise assessment; commands/runtimes; tested/inspected/blocked matrix;
each finding with stable FEATURE-NN ID, severity, category (confirmed defect,
feature gap, design question, risk), exact reproduction, expected/actual output,
source file:line, existing test blind spot, smallest maintainable improvement,
regression test. Preserve raw evidence. Check issues against ROADMAP.md and prior
audit/release reports: note known vs new. Be careful about fixture/harness errors.
Prioritize up to five well-supported findings over dozens of speculation items.

When done, send lead a short summary (max ~500 words), report path, test outcomes,
confirmed severity counts, top improvements, blockers. No universal claims. Other
agents share filesystem; own your feature folder only. Lead will cross-check high
issues and reconcile overlaps. Do not spawn additional agents.

Environment notes: LOVE is at /Applications/LOVE.app/Contents/MacOS/love
(not on PATH); /opt/homebrew/bin/lua and /opt/homebrew/bin/busted are present.
Existing actual-LSP harness: editors/vscode/scripts/lsp-profile-common.js exports
LspClient, pathToFileURL, delay, completionItems. See prior audit probes under
internal/audits/devex-2026-11-06/{hover,completion}/probe.js for usage patterns,
but write your own fixtures/output to your assigned folder. Do not overwrite
prior evidence. Desktop editor automation must be serialized through the lead;
CLI, stdio LSP, and isolated Lua/LOVE runtime probes can run independently.

For synthesis, also write findings.json as an array of {id,title,category,severity,
status,summary,evidence:[relative paths],known:boolean} when feasible. Only count
confirmed defects in defect totals; feature gaps/design questions stay separate.
The inventory covers 31 features; keep each pass bounded and hand off clear
shared-root symptoms rather than exhaustively expanding adjacent features.

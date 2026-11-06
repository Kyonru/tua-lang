# Audit baseline

- Revision: `d7aed2e5f1297b80a948e055985375bceaff6f0c`.
- Working tree clean before audit; no applicable AGENTS.md found in ancestors or repository.
- Scope: investigation only; no fixes, commits, pushes, publications or external messages.
- Reports and portable fixtures: this directory. Large/generated logs: `target/devex-audit-2026-11-06/`.
- Tools: available local Rust, Node, Lua/Busted, LOVE, VS Code. Tool versions/hashes recorded in verification log.
- Skill set read: audit-lead, language-auditor, test-auditor, lsp-parity, semantic-resolver, lua-defaults, love-devex, architecture-steward.
- Matrix built before delegation: 58 topical rows, grouped into language (T), completion (C), hover/signatures (H), workflows/integration (W).
- Severity: Critical = widespread data loss/execution failure; High = ordinary typed workflows produce materially wrong results or lose key tooling; Medium = narrower correctness/usability failure; Low = polish. Feature gaps/design questions are separate from confirmed defects.
- Root-cause claims are hypotheses unless established by code-path inspection; actual protocol/CLI observations are distinguished.

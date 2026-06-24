---
name: tua-verification
description: Choose and run the right checks for Tua changes. Use before finalizing code, docs, examples, VS Code extension, LSP, CLI, parser, checker, emitter, LOVE, asset, or roadmap changes, especially when deciding between focused tests and full workspace verification.
---

# Tua Verification

Verify according to risk and blast radius.

## Common Commands

- Formatting: `cargo fmt --all`
- Core tests: `cargo test -p tua-core`
- LSP tests: `cargo test -p tua-lsp`
- CLI unit tests: `cargo test -p tua-cli --lib`
- CLI integration tests: `cargo test -p tua-cli --test cli`
- Lints: `cargo clippy --workspace --all-targets -- -D warnings`
- Docs build: `make docs-build`
- JSON metadata: `make json-check`
- Whitespace: `make diff-check`

## Feature-Specific Checks

- Parser/checker/emitter: run focused `cargo test -p tua-core <filter>`, then
  `cargo test -p tua-core`.
- LSP behavior: run `cargo test -p tua-lsp` and `make lsp-features-smoke`.
- CLI/build behavior: run CLI tests and the relevant `make build-*` target.
- Examples: run `make check-example EXAMPLE=...` and `make build-example
  EXAMPLE=...`.
- Docs/nav: run `make docs-build`, `make json-check`, and `make diff-check`.
- LOVE runtime: run `make love-test` only when LOVE is available and the change
  needs runtime validation.

## Reporting

In the final answer, mention commands run and any skipped checks. If a known
long-running or environment-dependent test was not run, say that plainly.

## Skill Maintenance

If this skill is stale, unclear, or missing guidance discovered while using it,
update `SKILL.md` in the same change when the update is safe and scoped.
Regenerate `agents/openai.yaml` if the name, description, or purpose changes,
then validate the skill. If updating it would distract from the user request,
note the needed skill update explicitly.

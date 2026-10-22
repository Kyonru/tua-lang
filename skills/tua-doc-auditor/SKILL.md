---
name: tua-doc-auditor
description: Audit Tua documentation, examples, diagnostics references, CLI and LSP claims, navigation, and roadmap language against implemented behavior. Use for repository-wide documentation truthfulness and learning-path audits; use tua-doc-updater when applying known documentation edits.
---

# Tua Documentation Auditor

Find where a user would form the wrong mental model, fail to discover a working
feature, or copy an example that no longer represents the product.

## Sources Of Truth

- Start with the focused pages under `docs/` (`language.md`, `typing.md`,
  `cli.md`, `lsp.md`, `linter.md`, and the feature pages); each owns the
  contract for its topic.
- Treat a mismatch among the documented contract, implementation, and tests
  as a three-way discrepancy. Route suspected compiler defects through
  `tua-language-auditor`; never rewrite the contract merely to match unverified
  behavior.
- Cross-check focused pages, `README.md`, CLI help, config schema, diagnostic
  registry, LSP capabilities, extension manifests, examples, `CHANGELOG.md`,
  `ROADMAP.md`, `docs/llms.txt`, generated root `llms.txt`, build targets, CI,
  and `zensical.toml`.
- Treat `internal/` as the home for contributor-only implementation plans,
  benchmark methodology, evaluation evidence, and maintainer workflows. Flag
  those topics when they distract from supported user workflows in `docs/`.
- Build or exercise a documented command or example when its accuracy is in
  doubt. Do not call prose stale only because wording differs from code.
- Keep archived documents historical; do not treat them as current promises.

## Audit Lenses

- factual drift between implementation, defaults, diagnostics, and prose;
- missing prerequisites, failure behavior, platform constraints, and upgrade
  guidance;
- features implemented but undiscoverable from navigation or task-oriented
  journeys;
- examples that compile without teaching the claimed behavior, or generated
  output that has drifted;
- roadmap entries stated as shipped behavior, and shipped behavior still
  described as future work;
- absolute claims such as "all" or "every" whose build and documentation lanes
  omit configured examples or capabilities;
- notable shipped features missing from release notes and primary discovery
  surfaces;
- missing contributor, support, and private vulnerability-reporting paths;
- inconsistent terminology and unsupported guarantees.
- audience mixing: repository paths, phase histories, tuning evidence, and CI
  mechanics embedded in pages meant to help users install or use Tua;
- repetition that restates a focused page without helping readers choose or
  apply the feature.

Report exact document and implementation evidence, user impact, and the
smallest correction. Separate a docs defect from a product usability gap. When
edits are requested, route them through `tua-doc-updater` and verify the docs
site plus affected examples. Run `make llms-drift-check` when the machine-readable
documentation contract is in scope.

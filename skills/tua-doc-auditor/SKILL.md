---
name: tua-doc-auditor
description: Audit Tua documentation, examples, diagnostics references, CLI and LSP claims, navigation, and roadmap language against implemented behavior. Use for repository-wide documentation truthfulness and learning-path audits; use tua-doc-updater when applying known documentation edits.
---

# Tua Documentation Auditor

Find where a user would form the wrong mental model, fail to discover a working
feature, or copy an example that no longer represents the product.

## Sources Of Truth

- Start with `docs/specification.md` for the language contract.
- Treat a mismatch among the normative specification, implementation, and tests
  as a three-way discrepancy. Route suspected compiler defects through
  `tua-language-auditor`; never rewrite the contract merely to match unverified
  behavior.
- Cross-check focused pages, `README.md`, CLI help, config schema, diagnostic
  registry, LSP capabilities, extension manifests, examples, `CHANGELOG.md`,
  `ROADMAP.md`, `docs/llms.txt`, generated root `llms.txt`, build targets, CI,
  and `zensical.toml`.
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

Report exact document and implementation evidence, user impact, and the
smallest correction. Separate a docs defect from a product usability gap. When
edits are requested, route them through `tua-doc-updater` and verify the docs
site plus affected examples. Run `make llms-drift-check` when the machine-readable
documentation contract is in scope.

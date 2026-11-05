# Internal contributor guides

These documents record Tua's implementation plans, evaluation methods, and
maintainer workflows. They are useful when changing the repository, but they
are not part of the public language and toolchain documentation.

- [Development and verification](development.md)
- [Completion benchmarks](completion-benchmarks.md)
- [November 2026 editor performance audit](editor-performance-audit-2026-11.md)
- [LLM language evaluation](llm-language-evaluation.md)
- [November 2026 simulated development trials](devex-trials-2026-11.md)
- [November 2026 documentation and annotation review](documentation-review-2026-11.md)
- [Fuzzing](fuzzing.md)
- [Release readiness](release-readiness.md)
- [Local inline-completion plan](better-autocompletion.md)
- [Lint review and enhancement plan](lint-enhancement.md)
- [Shader editor support plan](shader-editor-support.md)

User-facing behavior belongs under [`docs/`](../docs/). Promote an internal
document there only when it describes a supported workflow or contract rather
than implementation history or test evidence.

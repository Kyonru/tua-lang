# Internal contributor guides

These documents record Tua's implementation plans, evaluation methods, and
maintainer workflows. They are useful when changing the repository, but they
are not part of the public language and toolchain documentation.

- [Development and verification](development.md)
- [Completion benchmarks](completion-benchmarks.md)
- [LLM language evaluation](llm-language-evaluation.md)
- [Fuzzing](fuzzing.md)
- [Local inline-completion plan](better-autocompletion.md)
- [Lint review and enhancement plan](lint-enhancement.md)

User-facing behavior belongs under [`docs/`](../docs/). Promote an internal
document there only when it describes a supported workflow or contract rather
than implementation history or test evidence.

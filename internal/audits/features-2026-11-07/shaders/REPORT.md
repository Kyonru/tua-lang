# Shader authoring and validation audit

Baseline: 9d490dcd299bd25218c9fa4adc83e080e617453c, existing release 0.6.0 binary; no product edits. Two confirmed defects: **one medium, one low**. Ordinary static shader authoring, metadata and fallback compilation worked in the exercised cases.

## Exercised workflows and evidence

Reproduce from repository root with `node internal/audits/features-2026-11-07/shaders/probe.js`, then `node internal/audits/features-2026-11-07/shaders/validators.js` and `node internal/audits/features-2026-11-07/shaders/compile-lsp.js`. Scripts use absolute target/release/tua, isolated projects, actual stdio LSP, and no mocked validator. Validators script temporarily sets fixture configuration and restores it.

| Area | Outcome | Evidence |
|---|---|---|
| Static asset and literal inline positive checks | Passed | cli.json |
| Number/vector incompatibility, unused uniform, typo suggestion | TL2015/TL2014/TL2013 as expected | cli.json |
| Shadowed `love` loader | No erroneous missing-asset/uniform diagnosis | cli.json |
| Reassigned receiver | No guessed uniform error; intentionally undefined assignment emits unrelated TL4004 | cli.json |
| Asset/inline completion and definition | Correct names and declaration coordinates | lsp.json |
| Shader unsaved overlay and close | `alpha` changes to `altitude`, then restores `alpha` | lsp.json |
| Auto fallback, explicit glslang, inline compilation error | Valid shaders pass, bad inline line 3 emits TL2016 at line 3 | validators.json |
| Trusted LSP worker and inline unsaved correction | TL2016 at zero-based line 2; final publication clears compiler errors | compile-lsp.json |
| Missing selected tools with deny-warnings | One skip note, success as documented | validators.json |
| Build and emitted Lua syntax | Both positive fixtures build; Lua 5.4 luac accepts emitted files | build.json, fixtures/*/build |
| Shader tool detection | glslang usable, LOVE process exits without response | shader-tools.json |
| Include source | Validator rejects unsupported plain #include; metadata does not expand includes | cli.json, validators.json |

LÖVE actual graphics/runtime execution is **host-blocked**: `/Applications/LOVE.app/Contents/MacOS/love` was explicitly selected for `shader-tools` and returned `LOVE shader validator exited without a response`. glslang is `/opt/homebrew/bin/glslangValidator`. Build syntax checks are not a claim of actual shader sends under LÖVE. Windows, GLES target, graphics-device behavior, separate vertex/pixel files and full editor rendering were not exercised. Includes have no documented supported resolution contract, so their rejection is recorded as a boundary, not a defect. Shader-tools reports preferred="love" even when unavailable; code intentionally represents preference, not selected working backend, so no defect is claimed.

## SHADERS-01 — Macro-defined uniform names trigger false missing-uniform errors

- **Medium, confirmed defect, new in this audit (not an attribution to the pending release changes).** Valid macro names block `tua check` with TL2013. docs/language.md:735–740 and docs/lsp.md:757 promise conservative handling of macro-generated declarations.
- Exact reproduction: `cd internal/audits/features-2026-11-07/shaders/fixtures/macro` and run the repository's absolute `target/release/tua check`. Source loads `effect.glsl` and sends `alpha`; GLSL has `#define NAME alpha`, `extern number NAME;`, and uses `NAME` in `effect`.
- Expected: no definitive absent-uniform error for macro-expanded declarations (or correctly resolve `alpha`). Actual: exit 1, `TL2013 ... has no uniform alpha ... it declares NAME`.
- Oracle: `glslangValidator -S frag internal/audits/features-2026-11-07/shaders/fixtures/macro/wrapped.frag` exits 0. Wrapped source uses the same vendored desktop/glsl1/pixel header as the product. `macro-validator.json` preserves command/result; `validators.json` preserves public CLI failure.
- Root: crates/tua-core/src/asset/shader_uniforms.rs:167 checks preprocessor lines only for literal `uniform`/`extern` text. Name macros therefore survive the guard and get recorded as literal identifiers by the declaration scan at :183 onward.
- Test blind spot: crates/tua-core/src/asset/tests.rs:304 covers `#define PARAM(name) uniform float name;` but not macros used as uniform identifiers or types.
- Smallest maintainable improvement: mark declaration metadata uncertain when declaration name/type depends on a macro, then share that uncertainty across checking/completion/hover. Conservative opt-out is sufficient; a full GLSL preprocessor is unnecessary.
- Regression: CLI/core fixture for the exact macro above must accept the valid send (without falsely recommending NAME); add identifier/type macro variants to the shared scanner tests.

## SHADERS-02 — Shader hover embeds a GLSL fence inside a Tua fence

- **Low, confirmed defect, new in searched audit reports.** Protocol output has malformed Markdown structure for both asset and inline uniform hovers.
- Exact reproduction: run probe.js and inspect `lsp.json` hover.contents.value for either case. Actual begins ` ```tua\n```glsl\nextern number alpha;\n``` ` and ends with another fence after the prose. Expected one GLSL code block followed by readable source location/value guidance.
- Root: crates/tua-core/src/analysis/shader_queries.rs:160 constructs a full Markdown block, while crates/tua-lsp/src/server/protocol.rs:182 unconditionally wraps hover.contents in a Tua block. This is a shared rendering-contract problem, not a reason for shader-specific protocol parsing.
- Test blind spot: crates/tua-lsp/src/server/tests/shaders.rs:281 onward asserts hover substrings such as `Unread uniform`, without checking Markdown shape; core tests check semantic contents.
- Smallest maintainable improvement: represent code language/code and prose separately in shared hover metadata, or explicitly distinguish Markdown from plain declaration content at the serializer boundary.
- Regression: actual protocol assertion for a shader hover should have one `glsl` fence and no enclosing `tua` fence; preserve ordinary Tua declaration hover behavior.
- Desktop rendering was not exercised; malformed serialized Markdown is directly observed.

## Context and bounded gaps

Reviewed docs/language.md, docs/lsp.md shader sections, docs/cli.md shader-tools/validation, ROADMAP.md and shader CHANGELOG entries, scanner/query/validator implementations, core shader tests, LSP shader tests, and prior audit searches. Neither defect appears in ROADMAP.md or the searched prior REPORT.md files; current changelog advertises shader features but does not identify these issues. No tests were added or product files modified. The shared baseline already ran 389 LSP tests; this pass used direct public-interface probes instead of rebuilding that suite.

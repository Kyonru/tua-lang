# Working on Tua

Tua is typed Lua tooling for LuaJIT and LÖVE. It includes a language, compiler,
CLI, LSP, VS Code extension, and opt-in game helpers. It keeps ordinary Lua and
LÖVE workflows available; do not treat a Tua helper as a required framework.

## Sources of truth

- Read the focused feature page under `docs/` and a runnable source in
  `examples/features` before changing or recommending a game API. Start at
  `docs/language.md` for the current feature index. `ROADMAP.md` records pending
  work, not implemented behavior.
- `docs/architecture.md` describes ownership. `tua-core` owns syntax, types,
  project facts, diagnostics, emitted Lua, and IDE queries; `tua-lsp` adapts
  those facts to LSP; `tua-cli` owns commands and filesystem output; the VS Code
  extension presents them. Avoid a second semantic resolver in an adapter.
- `docs/llms.txt` is the source of the model language contract. Run
  `make generate-llms` after editing it: root `llms.txt` is generated and is
  embedded in the compiler, `tua api`, MCP, and completion prompts. Offline
  model evaluation cards live under `benchmarks/llm-language/cards/`; keep
  their claims and reference tasks aligned. Check the generated file, model
  contract tests, and evaluation corpus.

## Game features and emitted code

- In `.tua`, built-in module values such as `ECS`, `Tween`, `Camera`, `Rot`,
  `BehaviorTree`, and `AudioDesign` need no user-written `require`. Tua includes
  their Lua runtimes only when referenced. Plain `.lua` files use explicit
  imports. Factory functions such as `vec2`, `signal`, and `store` keep lower
  camel case; their type names use PascalCase. The older `ecs` and `tween`
  spellings remain compatible. Read each feature page for its exact API and
  replacement rules; do not invent a universal registration or plugin system.
- Type annotations, type imports, protocols, and generic applications erase
  from emitted Lua. Runtime behavior must remain compatible with Lua 5.1,
  LuaJIT, and supported LÖVE versions. Test meaningful game behavior on emitted
  Lua, including absence of unused helpers, when touching lowering or runtimes.
- Static previews describe source-derived declarations. The opt-in runtime
  inspector shows sampled live state. Keep those claims and refresh behavior
  distinct. A performance fix must preserve current diagnostics, navigation,
  preview freshness, and visible editor assistance.

## Source readability

- Treat Tua's shared formatter defaults as the baseline for CLI, editor, examples,
  and agent-written source. Write one statement per line; expand multi-action
  blocks and use one field per line in multiline tables. Small single-action
  guards and simple expressions can remain compact.
- Run `tua fmt` and `tua fmt --check` in affected Tua projects before committing.
  Type checking, runtime tests, `cargo fmt`, and `git diff --check` do not replace
  the Tua formatting check. Preserve unrelated project changes.
- Keep authored test/harness code as readable as production code. Intentionally
  compact or malformed fixture *strings* may remain unchanged when layout is the
  behavior under test. Plain Lua assets need a separate formatting gate; do not
  imply `tua fmt --check` checked them automatically.
- If formatting is unstable or changes program behavior, retain a regression and
  fix the shared formatter. Do not silently skip the gate or add an editor-only
  style implementation. Keep mechanical cleanup separate from behavior changes.

## Tooling and verification

- Repository skills live under `skills/tua-*`; `.codex/skills/` contains their
  discovery aliases. Use `tua-skill-maintainer` for changes to this guidance.
  Route feature composition through `tua-framework-devex`, new game APIs
  through `tua-domain-design`, previews through `tua-editor-previews`, game
  distribution through `tua-game-packaging`, AI API/prompts through
  `tua-ai-bridge`, and measured responsiveness work through
  `tua-performance`. The language,
  semantic, LSP, Lua, LOVE, docs, examples, test, architecture, security,
  review, and release skills cover their named areas.
- Use focused checks while changing code, then `tua-verification` to select
  the affected integration gate. `make ci-check` is the broad local gate;
  installed VS Code and LÖVE checks need their respective hosts and must be
  reported separately. A passing protocol benchmark does not establish visible
  suggestion or hover latency.
- The API and MCP adapters are explicitly started, local, read-only analysis
  transports. Source excerpts require the documented capability or tool call;
  neither transport runs project Lua or applies edits. Preserve workspace
  confinement, overlay freshness, and those privacy boundaries.
- `internal/` is intentionally Git-ignored. Keep local audit evidence there
  when useful; do not force-add it or import another project's private source
  without the user's authorization. Preserve pre-existing work. A package or
  release preparation request does not by itself authorize pushing, tagging,
  or publishing.

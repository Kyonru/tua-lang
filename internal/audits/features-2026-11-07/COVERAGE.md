# Feature coverage matrix

Each of the 31 feature families received a dedicated agent. “Exercised” means representative executable probes completed; it does not mean every option passed. Each report distinguishes execution, inspection, mocks and blocked checks.

| Feature | Guide | Status | Evidence and limits |
|---|---|---|---|
| State stores | docs/stores.md | Exercised; bounded | [stores](stores/REPORT.md) |
| Signals and events | docs/signals.md | Exercised; bounded | [signals](signals/REPORT.md) |
| State machines | docs/state-machines.md | Exercised; bounded | [state_machines](state_machines/REPORT.md) |
| Entity component systems | docs/ecs.md | Exercised; bounded | [ecs](ecs/REPORT.md) |
| Object pools | docs/object-pooling.md | Exercised; bounded | [pools](pools/REPORT.md) |
| Input and bindings | docs/input.md | Exercised; bounded | [input](input/REPORT.md) |
| Audio systems and analysis | docs/audio.md | Exercised; bounded | [audio](audio/REPORT.md) |
| Sprites and animation | docs/sprites.md | Exercised; bounded | [sprites](sprites/REPORT.md) |
| Vectors and game math | docs/vectors.md | Exercised; bounded | [vectors](vectors/REPORT.md) |
| Camera | docs/camera.md | Exercised; bounded | [camera](camera/REPORT.md) |
| Tweening | docs/tweening.md | Exercised; bounded | [tween](tween/REPORT.md) |
| Classes and polymorphism | docs/classes.md | Exercised; bounded | [classes](classes/REPORT.md) |
| Types, inference and narrowing | docs/typing.md | Exercised; bounded | [typing](typing/REPORT.md) |
| Functions, defaults, multi-returns and syntax lowering | docs/language.md | Exercised; bounded | [functions](functions/REPORT.md) |
| Modules, imports and declaration libraries | docs/modules.md | Exercised; bounded | [modules](modules/REPORT.md) |
| Lua interop and annotations | docs/lua-interop.md | Exercised; bounded | [lua_interop](lua_interop/REPORT.md) |
| LOVE catalog and callbacks | docs/language.md | Exercised; bounded | [love](love/REPORT.md) |
| Assets and pipelines | docs/asset-pipelines.md | Exercised; bounded | [assets](assets/REPORT.md) |
| Shader authoring and validation | docs/lsp.md | Exercised; bounded | [shaders](shaders/REPORT.md) |
| Formatter | docs/formatter.md | Exercised; bounded | [formatter](formatter/REPORT.md) |
| Linter, diagnostics and quick fixes | docs/linter.md | Exercised; bounded | [linter](linter/REPORT.md) |
| Busted testing, coverage and Test Explorer | docs/testing.md | Exercised; bounded | [testing](testing/REPORT.md) |
| Project configuration, init and builds | docs/configuration.md | Exercised; bounded | [projects](projects/REPORT.md) |
| Watch, dev and hot reload | docs/hot-reload.md | Exercised; bounded | [watch_reload](watch_reload/REPORT.md) |
| Debugger, source maps and runtime inspector | docs/debugging.md | Exercised; bounded | [debugging](debugging/REPORT.md) |
| Completion, hover, signatures and navigation | docs/lsp.md | Exercised; bounded | [editor](editor/REPORT.md) |
| Project graphs and explorers | docs/lsp.md | Exercised; bounded | [explorers](explorers/REPORT.md) |
| Deterministic inline completion and Ollama | docs/lsp.md | Exercised; bounded | [inline](inline/REPORT.md) |
| External API and MCP | docs/external-api-bridge.md | Exercised; bounded | [api_mcp](api_mcp/REPORT.md) |
| Game packaging and toolchain distribution | docs/packaging.md | Exercised; bounded | [packaging](packaging/REPORT.md) |
| Installation, setup, theme and documentation | docs/installation.md | Exercised; bounded | [setup_docs](setup_docs/REPORT.md) |

## Cross-cutting coverage

| Area | Status | Evidence / boundary |
|---|---|---|
| Command ownership | Inspected | All 105 contributed commands assigned exactly once in command-ownership.json; not 105 executed GUI commands |
| Settings | Inspected | 74 inventoried in extension-surface.json; not every combination exercised |
| LSP shared regression suite | Executed | 389 passing tests; lsp-baseline.log |
| Bundled runtime suites | Executed | Four passing suites; runtime-baseline.log |
| High findings | Independently exercised | All eight reproduced by lead; cross-checks/README.md |
| Native runtime and debugger | Partially exercised | Actual LOVE/DAP/macOS fusion successes; per-lane restrictions remain |
| VS Code GUI | Limited | Protocol/provider/mocked-host checks do not qualify every desktop workflow |
| Audio/input/rendering | Limited | Source/device/rendering doubles where stated; no comprehensive hardware/pixel qualification |
| Ollama generation | Blocked | Local endpoint refused connection; deterministic protocol logic exercised |
| Windows/Linux installation and distribution | Inspected / limited | No full native platform matrix; Windows fusion structure only |
| External publication/services | Not exercised | No publish, registry install, remote workflow or external messaging |
| Long sessions/performance | Not qualified | Bounded cancellation and latency observations only; no sustained benchmark |
| Architecture | Inspected alongside probes | Shared checker/resolver/lowering/runtime paths traced in feature reports |

See [verification](VERIFICATION.md) and the independent [coverage review](reviews/coverage-review.md).

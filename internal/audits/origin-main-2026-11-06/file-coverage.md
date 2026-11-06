# Changed-file coverage inventory

All files in the comparison are assigned below. Assignment identifies the relevant review lane, not that every changed line has an independent runtime test. Lane reports and final verification distinguish source inspection, regression tests, protocol replay and environment limitations. Historical evidence files are inspected for claim consistency rather than rewritten.

## configuration/examples/gates/generators (21)

- `.github/workflows/ci.yml`
- `config/schema.json`
- `config/templates/default.toml`
- `examples/features/README.md`
- `examples/features/src/inference.tua`
- `examples/features/src/main.tua`
- `examples/features/src/polymorphism.tua`
- `examples/features/src/polymorphism/actors.tua`
- `examples/features/src/timer_services.tua`
- `examples/linter/src/classes.tua`
- `examples/type-libraries/hump/timer.lua`
- `examples/unit-testing/README.md`
- `examples/unit-testing/src/counter.tua`
- `examples/unit-testing/tests/counter_spec.tua`
- `examples/unit-testing/tests/plain_spec.lua`
- `examples/unit-testing/tua.toml`
- `scripts/generate-annotation-guide.js`
- `scripts/generate-love-api-extra.lua`
- `scripts/generate-love-api-signatures.lua`
- `scripts/generate-love-api.lua`
- `scripts/love-api-overrides.lua`

## documentation/evidence (49)

- `CHANGELOG.md`
- `CONTRIBUTING.md`
- `README.md`
- `ROADMAP.md`
- `SUPPORT.md`
- `docs/annotations.md`
- `docs/architecture.md`
- `docs/audio.md`
- `docs/camera.md`
- `docs/classes.md`
- `docs/cli.md`
- `docs/comments.md`
- `docs/configuration.md`
- `docs/debugging.md`
- `docs/examples.md`
- `docs/formatter.md`
- `docs/hot-reload.md`
- `docs/index.md`
- `docs/installation.md`
- `docs/language.md`
- `docs/learn.md`
- `docs/linter.md`
- `docs/lsp.md`
- `docs/lua-basics.md`
- `docs/lua-interop.md`
- `docs/luahelper-code-matrix.md`
- `docs/modules.md`
- `docs/quickstart.md`
- `docs/releases.md`
- `docs/signals.md`
- `docs/sprites.md`
- `docs/testing.md`
- `docs/troubleshooting.md`
- `docs/tweening.md`
- `docs/typing.md`
- `internal/README.md`
- `internal/completion-benchmarks.md`
- `internal/completion-latency-followup-2026-11.md`
- `internal/devex-trials-2026-11.md`
- `internal/documentation-review-2026-11.md`
- `internal/documentation-usability-2026-11.md`
- `internal/editor-performance-audit-2026-11.md`
- `internal/ordered-backlog-2026-11.md`
- `internal/releasing.md`
- `internal/remaining-backlog-2026-11.md`
- `internal/roadmap-implementation-2026-11.md`
- `internal/unit-testing-2026-11.md`
- `internal/upstream-port-audit-2026-11.md`
- `zensical.toml`

## toolchain/CLI (19)

- `crates/tua-cli/src/args/mod.rs`
- `crates/tua-cli/src/args/tests.rs`
- `crates/tua-cli/src/build/mod.rs`
- `crates/tua-cli/src/build/watch.rs`
- `crates/tua-cli/src/check/mod.rs`
- `crates/tua-cli/src/command/mod.rs`
- `crates/tua-cli/src/formatter/mod.rs`
- `crates/tua-cli/src/lib.rs`
- `crates/tua-cli/src/source/mod.rs`
- `crates/tua-cli/src/testing.rs`
- `crates/tua-cli/src/testing/coverage.lua`
- `crates/tua-cli/src/testing/coverage.rs`
- `crates/tua-cli/src/testing/discovery.rs`
- `crates/tua-cli/src/testing/report.rs`
- `crates/tua-cli/src/testing/reporter.lua`
- `crates/tua-cli/src/traceback/mod.rs`
- `crates/tua-cli/tests/cli.rs`
- `crates/tua-cli/tests/gameplay.rs`
- `crates/tua-cli/tests/helper_defaults.rs`

## shared semantics/project/domain (57)

- `crates/tua-core/resources/annotation-tags.json`
- `crates/tua-core/src/analysis/database.rs`
- `crates/tua-core/src/analysis/editor_queries.rs`
- `crates/tua-core/src/analysis/host.rs`
- `crates/tua-core/src/analysis/mod.rs`
- `crates/tua-core/src/analysis/navigation_queries.rs`
- `crates/tua-core/src/analysis/project_queries.rs`
- `crates/tua-core/src/analysis/recovery.rs`
- `crates/tua-core/src/analysis/shader_queries.rs`
- `crates/tua-core/src/analysis/shader_unsent.rs`
- `crates/tua-core/src/analysis/snapshot.rs`
- `crates/tua-core/src/analysis/tests.rs`
- `crates/tua-core/src/annotation_tags.rs`
- `crates/tua-core/src/asset/inline_shaders.rs`
- `crates/tua-core/src/asset/mod.rs`
- `crates/tua-core/src/code_action/mod.rs`
- `crates/tua-core/src/code_action/tests.rs`
- `crates/tua-core/src/completion/context.rs`
- `crates/tua-core/src/completion/mod.rs`
- `crates/tua-core/src/completion/query.rs`
- `crates/tua-core/src/completion/tests.rs`
- `crates/tua-core/src/config/mod.rs`
- `crates/tua-core/src/config/parser.rs`
- `crates/tua-core/src/config/tests.rs`
- `crates/tua-core/src/definition/mod.rs`
- `crates/tua-core/src/definition/tests.rs`
- `crates/tua-core/src/diagnostic/mod.rs`
- `crates/tua-core/src/ecs.rs`
- `crates/tua-core/src/folding_range/mod.rs`
- `crates/tua-core/src/folding_range/tests.rs`
- `crates/tua-core/src/hover/mod.rs`
- `crates/tua-core/src/hover/tests.rs`
- `crates/tua-core/src/lib.rs`
- `crates/tua-core/src/linter/class_rules.rs`
- `crates/tua-core/src/linter/mod.rs`
- `crates/tua-core/src/linter/rules.rs`
- `crates/tua-core/src/linter/tests.rs`
- `crates/tua-core/src/love_api_signatures.rs`
- `crates/tua-core/src/love_conf.rs`
- `crates/tua-core/src/lua/mod.rs`
- `crates/tua-core/src/module_graph/mod.rs`
- `crates/tua-core/src/module_index/mod.rs`
- `crates/tua-core/src/module_index/tests.rs`
- `crates/tua-core/src/project_model/mod.rs`
- `crates/tua-core/src/project_model/tests.rs`
- `crates/tua-core/src/references/mod.rs`
- `crates/tua-core/src/semantic.rs`
- `crates/tua-core/src/semantic_domains.rs`
- `crates/tua-core/src/semantic_index.rs`
- `crates/tua-core/src/semantic_index/tests.rs`
- `crates/tua-core/src/signal.rs`
- `crates/tua-core/src/signature_help/mod.rs`
- `crates/tua-core/src/signature_help/tests.rs`
- `crates/tua-core/src/source_text.rs`
- `crates/tua-core/src/testing.rs`
- `crates/tua-core/src/type_database/mod.rs`
- `crates/tua-core/src/usage_index.rs`

## language contracts (8)

- `crates/tua-core/src/check/mod.rs`
- `crates/tua-core/src/check/tests.rs`
- `crates/tua-core/src/narrowing.rs`
- `crates/tua-core/src/scope/mod.rs`
- `crates/tua-core/src/types/environment.rs`
- `crates/tua-core/src/types/mod.rs`
- `crates/tua-core/src/types/model.rs`
- `crates/tua-core/src/types/tests.rs`

## emission/runtime (14)

- `crates/tua-core/src/classes.rs`
- `crates/tua-core/src/emit/builtin_calls.rs`
- `crates/tua-core/src/emit/lowering.rs`
- `crates/tua-core/src/emit/lowering/operation.rs`
- `crates/tua-core/src/emit/mod.rs`
- `crates/tua-core/src/emit/tests.rs`
- `crates/tua-core/src/emit/vec2_fusion.rs`
- `crates/tua-core/src/parser/doc.rs`
- `crates/tua-core/src/parser/mod.rs`
- `crates/tua-core/src/parser/model.rs`
- `crates/tua-core/src/parser/normalize.rs`
- `crates/tua-core/src/parser/tests.rs`
- `vendor/runtime/tuahotreload.lua`
- `vendor/tests/hotreload_test.lua`

## protocol/project/editor (8)

- `crates/tua-lsp/src/server/handlers.rs`
- `crates/tua-lsp/src/server/tests/code_actions.rs`
- `crates/tua-lsp/src/server/tests/completion.rs`
- `crates/tua-lsp/src/server/tests/diagnostics.rs`
- `crates/tua-lsp/src/server/tests/hover.rs`
- `crates/tua-lsp/src/server/tests/navigation.rs`
- `crates/tua-lsp/src/server/tests/recovery.rs`
- `crates/tua-lsp/src/server/tests/scheduling.rs`

## extension/editor (27)

- `editors/vscode/README.md`
- `editors/vscode/docs/screenshots/README.md`
- `editors/vscode/docs/screenshots/capture.json`
- `editors/vscode/docs/screenshots/module-graph.png`
- `editors/vscode/docs/screenshots/runtime-inspector.png`
- `editors/vscode/docs/screenshots/sprite-animation.png`
- `editors/vscode/docs/screenshots/state-machine.png`
- `editors/vscode/docs/screenshots/workspace.png`
- `editors/vscode/package.json`
- `editors/vscode/resources/annotation-guide.md`
- `editors/vscode/resources/love-api/NOTICE.md`
- `editors/vscode/resources/love-api/index.html`
- `editors/vscode/scripts/activation-smoke.js`
- `editors/vscode/scripts/lsp-editor-profile.js`
- `editors/vscode/scripts/lsp-editor-profile.test.js`
- `editors/vscode/src/annotation-guide.js`
- `editors/vscode/src/extension.js`
- `editors/vscode/src/love/api-reference.js`
- `editors/vscode/src/love/game-commands.js`
- `editors/vscode/src/testing/coverage.js`
- `editors/vscode/src/testing/index.js`
- `editors/vscode/src/testing/model.js`
- `editors/vscode/src/testing/process.js`
- `editors/vscode/test/unit/annotation-guide.test.js`
- `editors/vscode/test/unit/game-commands.test.js`
- `editors/vscode/test/unit/love-api-reference.test.js`
- `editors/vscode/test/unit/testing.test.js`

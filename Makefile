.DEFAULT_GOAL := help

CARGO ?= cargo
NPM ?= npm
NPX ?= npx
CODE ?= code
LOVE ?= love
LUA ?= lua
VSCE ?= $(NPX) --no-install vsce
RSVG_CONVERT ?= rsvg-convert
LOVE_API_ROOT ?=
LOVE_API_REVISION := $(shell sed -n '1p' scripts/love-api-revision.txt)
PROJECT_VERSION := $(shell node scripts/sync-version.js --print)

TUA := $(CARGO) run -q -p tua-cli --
VSCODE_DIR := editors/vscode
VSIX := $(VSCODE_DIR)/tua-$(PROJECT_VERSION).vsix
EXAMPLE ?= examples/love-minimal
ECS_BASE_REF ?= $(shell git describe --tags --abbrev=0 2>/dev/null)
ECS_LOVE_FLAGS ?=
STORE_BASE_REF ?= $(shell git describe --tags --abbrev=0 2>/dev/null)
SPRITE_BASE_REF ?= $(shell git describe --tags --abbrev=0 2>/dev/null)
SPRITE_LOVE_FLAGS ?=
UNAME_S := $(shell uname -s 2>/dev/null)
EXE_SUFFIX := $(if $(filter MINGW% MSYS% CYGWIN%,$(UNAME_S)),.exe,)
DEBUG_TUA := target/debug/tua$(EXE_SUFFIX)
RELEASE_TUA := target/release/tua$(EXE_SUFFIX)
VSCODE_TUA := $(VSCODE_DIR)/bin/tua$(EXE_SUFFIX)
LOGO_SVG := logo.svg
VSCODE_LOGO_SVG := $(VSCODE_DIR)/assets/logo.svg
VSCODE_LOGO_PNG := $(VSCODE_DIR)/assets/logo.png
DOCS_ICON_SVG := docs/assets/icon.svg
DOCS_ICON_PNG := docs/assets/icon.png

.PHONY: help
help: ## Show available targets.
	@awk 'BEGIN { FS = ":.*##"; printf "tua project targets:\n\n" } /^[a-zA-Z0-9_.-]+:.*##/ { printf "  %-28s %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

.PHONY: fmt
fmt: ## Format the Rust workspace.
	$(CARGO) fmt --all

.PHONY: fmt-check
fmt-check: ## Check Rust formatting without modifying files.
	$(CARGO) fmt --all -- --check

.PHONY: test
test: ## Run the full Rust test suite.
	$(CARGO) test --workspace

.PHONY: generate-love-api
generate-love-api: ## Regenerate all LOVE catalogs from the pinned love-api checkout.
	@test -n "$(LOVE_API_ROOT)" || { echo "error: set LOVE_API_ROOT=/path/to/love-api"; exit 1; }
	@test "$$(git -C "$(LOVE_API_ROOT)" rev-parse HEAD)" = "$(LOVE_API_REVISION)" || { echo "error: LOVE API checkout must be at $(LOVE_API_REVISION)"; exit 1; }
	lua scripts/generate-love-api.lua "$(LOVE_API_ROOT)"
	lua scripts/generate-love-api-signatures.lua "$(LOVE_API_ROOT)"
	lua scripts/generate-love-api-extra.lua "$(LOVE_API_ROOT)"
	rustfmt crates/tua-core/src/love_api.rs crates/tua-core/src/love_api_signatures.rs crates/tua-core/src/love_api_extra.rs

.PHONY: generate-config-schema
generate-config-schema: ## Regenerate VS Code and docs config metadata from config/schema.json.
	node scripts/generate-config-schema.js

.PHONY: generate-llms
generate-llms: ## Regenerate the root llms.txt language contract.
	node scripts/generate-llms.js

.PHONY: sync-version
sync-version: ## Synchronize package versions from the Rust workspace version.
	node scripts/sync-version.js

.PHONY: set-version
set-version: ## Set every package version; usage: make set-version VERSION=0.2.9.
	node scripts/sync-version.js --set "$(VERSION)"

.PHONY: release-version
release-version: ## Validate, commit, and create the local vVERSION tag.
	@test -n "$(VERSION)" || { echo "error: set VERSION, for example make release-version VERSION=0.2.9"; exit 1; }
	node scripts/release-version.js "$(VERSION)"

.PHONY: version-drift-check
version-drift-check: ## Verify every package uses the Rust workspace version.
	node scripts/sync-version.js --check

.PHONY: release-tool-test
release-tool-test: ## Test deterministic release archives and changelog extraction.
	node --test scripts/sync-version.test.js scripts/release-version.test.js
	PYTHONPATH=scripts python3 -m unittest scripts/package_release_test.py

.PHONY: config-schema-drift-check
config-schema-drift-check: ## Verify VS Code and docs config metadata matches config/schema.json.
	node scripts/generate-config-schema.js --check

.PHONY: llms-drift-check
llms-drift-check: ## Verify root llms.txt matches the documented contract.
	node scripts/generate-llms.js --check

.PHONY: love-api-drift-check
love-api-drift-check: ## Verify generated LOVE catalogs match the pinned source checkout.
	@test -n "$(LOVE_API_ROOT)" || { echo "error: set LOVE_API_ROOT=/path/to/love-api"; exit 1; }
	@test "$$(git -C "$(LOVE_API_ROOT)" rev-parse HEAD)" = "$(LOVE_API_REVISION)" || { echo "error: LOVE API checkout must be at $(LOVE_API_REVISION)"; exit 1; }
	@tmp="$$(mktemp -d)"; \
	trap 'rm -rf "$$tmp"' EXIT; \
	lua scripts/generate-love-api.lua "$(LOVE_API_ROOT)" "$$tmp/love_api.rs"; \
	lua scripts/generate-love-api-signatures.lua "$(LOVE_API_ROOT)" "$$tmp/love_api_signatures.rs"; \
	lua scripts/generate-love-api-extra.lua "$(LOVE_API_ROOT)" "$$tmp/love_api_extra.rs"; \
	rustfmt "$$tmp/love_api.rs" "$$tmp/love_api_signatures.rs" "$$tmp/love_api_extra.rs"; \
	diff -u crates/tua-core/src/love_api.rs "$$tmp/love_api.rs"; \
	diff -u crates/tua-core/src/love_api_signatures.rs "$$tmp/love_api_signatures.rs"; \
	diff -u crates/tua-core/src/love_api_extra.rs "$$tmp/love_api_extra.rs"

.PHONY: clippy
clippy: ## Run clippy with warnings denied.
	$(CARGO) clippy --workspace --all-targets -- -D warnings

.PHONY: verify
verify: fmt-check test clippy version-drift-check release-tool-test config-schema-drift-check llms-drift-check llm-language-eval-test extension-check json-check diff-check ## Run the main local verification suite without modifying files.

.PHONY: ci-check
ci-check: fmt-check clippy test build-examples editor-regression-smoke version-drift-check release-tool-test config-schema-drift-check llms-drift-check llm-language-eval-test extension-check json-check docs-build benchmark-smoke diff-check ## Run required CI checks that do not need an external catalog checkout or VS Code host.

.PHONY: diff-check
diff-check: ## Check the git diff for whitespace errors.
	git diff --check

.PHONY: docs
docs: ## Serve the Zensical documentation site locally.
	bash scripts/docs.sh serve

.PHONY: docs-build
docs-build: ## Build the Zensical documentation site.
	bash scripts/docs.sh build

.PHONY: logo
logo: ## Sync logo assets and render 256px PNGs for docs and the VS Code extension.
	@command -v $(RSVG_CONVERT) >/dev/null 2>&1 || { echo "error: $(RSVG_CONVERT) not found. Install it with: brew install librsvg"; exit 1; }
	mkdir -p $(VSCODE_DIR)/assets docs/assets
	cp $(LOGO_SVG) $(VSCODE_LOGO_SVG)
	cp $(LOGO_SVG) $(DOCS_ICON_SVG)
	$(RSVG_CONVERT) -w 256 -h 256 $(LOGO_SVG) -o $(VSCODE_LOGO_PNG)
	$(RSVG_CONVERT) -w 256 -h 256 $(LOGO_SVG) -o $(DOCS_ICON_PNG)

.PHONY: cli-help
cli-help: ## Print tua CLI help.
	$(TUA) --help

.PHONY: cli-version
cli-version: ## Print tua CLI version.
	$(TUA) --version

.PHONY: lsp
lsp: ## Run the raw stdio LSP server; it waits silently for an editor/client.
	$(TUA) lsp

.PHONY: lsp-smoke
lsp-smoke: ## Initialize and shutdown the LSP server to prove it responds.
	$(CARGO) build --package tua-cli
	node editors/vscode/scripts/lsp-smoke.js $(DEBUG_TUA)

.PHONY: lsp-features-smoke
lsp-features-smoke: ## Exercise LSP requests against examples/features with the debug server.
	$(CARGO) build --package tua-cli
	node editors/vscode/scripts/lsp-features-smoke.js $(DEBUG_TUA) examples/features

.PHONY: editor-regression-smoke
editor-regression-smoke: ## Gate typing freshness and closed/unopened module navigation through the real LSP server.
	$(CARGO) test -p tua-lsp editor_regression
	$(MAKE) lsp-features-smoke

.PHONY: profile-lsp
profile-lsp: ## Profile LSP request latency against small/medium/large benchmark fixtures.
	$(CARGO) build --package tua-cli
	TUA_PROFILE_REPEAT=$${TUA_PROFILE_REPEAT:-3} node editors/vscode/scripts/lsp-profile.js $(DEBUG_TUA) benchmarks/lsp/small benchmarks/lsp/medium benchmarks/lsp/large

.PHONY: profile-completion
profile-completion: ## Profile completion latency, ranking overhead, and completion cache behavior.
	$(CARGO) build --package tua-cli
	TUA_PROFILE_REPEAT=$${TUA_PROFILE_REPEAT:-20} node editors/vscode/scripts/lsp-completion-profile.js $(DEBUG_TUA) benchmarks/lsp/medium

.PHONY: profile-completion-phase1-gate
profile-completion-phase1-gate: ## Gate menu completion p95 at exactly 5% over a pre-Phase-1 report from this machine (set TUA_COMPLETION_BASELINE).
	@test -n "$(TUA_COMPLETION_BASELINE)" || { \
		echo "set TUA_COMPLETION_BASELINE to a pre-Phase-1 completion profile recorded on this machine" >&2; \
		exit 2; \
	}
	$(CARGO) build --package tua-cli
	TUA_PROFILE_REPEAT=$${TUA_PROFILE_REPEAT:-100} \
	TUA_COMPLETION_BASELINE="$(TUA_COMPLETION_BASELINE)" \
	TUA_COMPLETION_BASELINE_SLACK_MS=0 \
	TUA_COMPLETION_PROFILE_OUT=target/tua-speed/completion-phase1-gate.json \
	node editors/vscode/scripts/lsp-completion-profile.js $(DEBUG_TUA) benchmarks/lsp/medium

.PHONY: completion-ranking-smoke
completion-ranking-smoke: ## Check fixture-driven completion ranking order.
	$(CARGO) build --package tua-cli
	node editors/vscode/scripts/lsp-completion-ranking-smoke.js $(DEBUG_TUA) benchmarks/lsp/medium

.PHONY: ollama-completion-smoke
ollama-completion-smoke: ## Exercise an installed local Ollama model; set TUA_OLLAMA_MODEL.
	node scripts/ollama-completion-smoke.js

.PHONY: llm-language-eval-test
llm-language-eval-test: ## Verify the 40-task LLM corpus, references, and negative controls offline.
	$(CARGO) build --package tua-cli
	node --test scripts/llm-language-eval.test.js
	node scripts/llm-language-eval.js --reference --out target/llm-language/reference-report.json
	node scripts/llm-language-eval.js --mutants --out target/llm-language/mutant-report.json

.PHONY: llm-language-eval-ollama
llm-language-eval-ollama: ## Evaluate an installed local Ollama model across the language corpus.
	$(CARGO) build --package tua-cli
	node scripts/llm-language-eval.js --ollama --out target/llm-language/ollama-report.json

.PHONY: generate-lsp-benchmarks
generate-lsp-benchmarks: ## Generate deterministic representative LSP benchmark fixtures under target/.
	node scripts/generate-lsp-benchmarks.js

.PHONY: representative-benchmark-test
representative-benchmark-test: ## Verify representative LSP fixture sizes and deterministic content.
	node --test scripts/generate-lsp-benchmarks.test.js \
		editors/vscode/scripts/representative-diagnostics-compare.test.js

.PHONY: profile-representative
profile-representative: representative-benchmark-test ## Profile cold, warm, and contended LSP requests at representative scales.
	$(CARGO) build --release --package tua-cli
	node editors/vscode/scripts/lsp-representative-profile.js $(RELEASE_TUA)

.PHONY: representative-benchmark-smoke
representative-benchmark-smoke: representative-benchmark-test ## Gate completion freshness on CI-sized representative fixtures.
	$(CARGO) build --release --package tua-cli
	TUA_REPRESENTATIVE_FIXTURES=single-1000,project-100 \
	TUA_REPRESENTATIVE_COLD_REPEAT=1 \
	TUA_REPRESENTATIVE_WARM_REPEAT=2 \
	TUA_REPRESENTATIVE_CONTENTION_REPEAT=2 \
	TUA_REPRESENTATIVE_COMPLETION_P95_BUDGET_MS=3000 \
	TUA_REPRESENTATIVE_PROFILE_OUT=$${TUA_REPRESENTATIVE_PROFILE_OUT:-target/tua-speed/representative-profile-smoke.json} \
	node editors/vscode/scripts/lsp-representative-profile.js $(RELEASE_TUA)

.PHONY: representative-phase1-gate
representative-phase1-gate: ## Run the representative Phase 1 inline gate with strict internal and end-to-end latency budgets.
	TUA_REPRESENTATIVE_INLINE_SERVER_P95_BUDGET_MS=10 \
	TUA_REPRESENTATIVE_INLINE_WORKER_P95_BUDGET_MS=10 \
	TUA_REPRESENTATIVE_INLINE_BACKEND_P95_BUDGET_MS=10 \
	TUA_REPRESENTATIVE_INLINE_E2E_P95_BUDGET_MS=1500 \
	TUA_REPRESENTATIVE_INLINE_DELIVERY_P95_BUDGET_MS=250 \
	TUA_REPRESENTATIVE_PROFILE_OUT=target/tua-speed/representative-phase1-gate.json \
	$(MAKE) profile-representative

.PHONY: representative-diagnostics-baseline
representative-diagnostics-baseline: representative-benchmark-test ## Compare diagnostic freshness with a pre-Phase-1 Tua binary (set TUA_REPRESENTATIVE_BASELINE_TUA).
	@test -n "$(TUA_REPRESENTATIVE_BASELINE_TUA)" || { \
		echo "set TUA_REPRESENTATIVE_BASELINE_TUA to the pre-Phase-1 Tua binary" >&2; \
		exit 2; \
	}
	$(CARGO) build --release --package tua-cli
	node editors/vscode/scripts/representative-diagnostics-compare.js \
		"$(TUA_REPRESENTATIVE_BASELINE_TUA)" "$(RELEASE_TUA)"

.PHONY: profile-examples
profile-examples: ## Profile CLI and LSP speed across every bundled example project.
	$(CARGO) build --release --package tua-cli
	node scripts/profile-examples.js $(RELEASE_TUA)

.PHONY: profile-large-tables
profile-large-tables: ## Profile compiler/LSP latency and peak RSS for 1,000/10,000-field and deeply nested tables.
	$(CARGO) build --release --package tua-cli
	node scripts/profile-large-tables.js $(RELEASE_TUA)

.PHONY: profile-ecs-love
profile-ecs-love: ## Compare ECS against ECS_BASE_REF (default latest tag) under LOVE; ECS_LOVE_FLAGS=--jit forces JIT on.
	@test -n "$(ECS_BASE_REF)" || (echo "set ECS_BASE_REF to a git tag or commit"; exit 2)
	@mkdir -p target/tua-speed
	git show $(ECS_BASE_REF):vendor/runtime/tuaecs.lua > target/tua-speed/tuaecs.base.lua
	$(LOVE) benchmarks/ecs-love $(ECS_LOVE_FLAGS) $(CURDIR)/target/tua-speed/tuaecs.base.lua $(CURDIR)/vendor/runtime/tuaecs.lua

.PHONY: profile-store
profile-store: ## Compare the store runtime against STORE_BASE_REF (default latest tag) using LUA (default lua).
	@test -n "$(STORE_BASE_REF)" || (echo "set STORE_BASE_REF to a git tag or commit"; exit 2)
	@mkdir -p target/tua-speed
	git show $(STORE_BASE_REF):vendor/runtime/tuastore.lua > target/tua-speed/tuastore.base.lua
	$(LUA) benchmarks/store-runtime.lua target/tua-speed/tuastore.base.lua vendor/runtime/tuastore.lua

.PHONY: profile-sprite-love
profile-sprite-love: ## Compare sprite runtimes against SPRITE_BASE_REF under LOVE; SPRITE_LOVE_FLAGS=--jit forces JIT on.
	@test -n "$(SPRITE_BASE_REF)" || (echo "set SPRITE_BASE_REF to a git tag or commit"; exit 2)
	@mkdir -p target/tua-speed
	git show $(SPRITE_BASE_REF):vendor/runtime/anim8.lua > target/tua-speed/anim8.base.lua
	git show $(SPRITE_BASE_REF):vendor/runtime/tuasprite.lua > target/tua-speed/tuasprite.base.lua
	$(LOVE) benchmarks/sprite-love $(SPRITE_LOVE_FLAGS) $(CURDIR)/target/tua-speed/anim8.base.lua $(CURDIR)/target/tua-speed/tuasprite.base.lua $(CURDIR)/vendor/runtime/anim8.lua $(CURDIR)/vendor/runtime/tuasprite.lua

.PHONY: profile-examples-baseline
profile-examples-baseline: ## Write a local examples speed baseline for future comparisons.
	$(CARGO) build --release --package tua-cli
	TUA_SPEED_OUT=$${TUA_SPEED_OUT:-target/tua-speed/examples-baseline.json} node scripts/profile-examples.js $(RELEASE_TUA)

.PHONY: profile-examples-smoke
profile-examples-smoke: ## Run one quick examples speed profiling pass.
	$(CARGO) build --release --package tua-cli
	TUA_SPEED_WARMUP=0 TUA_SPEED_REPEAT=1 TUA_SPEED_OUT=$${TUA_SPEED_OUT:-target/tua-speed/examples-profile-smoke.json} node scripts/profile-examples.js $(RELEASE_TUA)

INLINE_EVAL_MANIFEST := benchmarks/inline-eval/manifest.toml
INLINE_EVAL_THRESHOLDS := benchmarks/inline-eval/thresholds.toml
INLINE_EVAL_SMOKE_EXPECTED := benchmarks/inline-eval/smoke/expected.json
INLINE_EVAL := $(CARGO) run --quiet --release --package tua-core --features test-fixture --example inline_eval --
INLINE_EVAL_DEBUG := $(CARGO) run --quiet --package tua-core --features test-fixture --example inline_eval --

.PHONY: inline-eval
inline-eval: ## Evaluate local inline-completion sources on the tuning sets (writes target/tua-speed/inline-eval-tune.json).
	$(INLINE_EVAL) --manifest $(INLINE_EVAL_MANIFEST) --set tune --out target/tua-speed/inline-eval-tune.json

.PHONY: inline-eval-phase2
inline-eval-phase2: ## Evaluate experimental Phase 2 exact retrieval on tuning data only.
	$(INLINE_EVAL) --manifest $(INLINE_EVAL_MANIFEST) --set tune --phase2-retrieval --thresholds $(INLINE_EVAL_THRESHOLDS) --out target/tua-speed/inline-eval-tune-phase2.json

.PHONY: inline-eval-phase3
inline-eval-phase3: ## Evaluate the offline Phase 3 n-gram experiment on tuning data only.
	$(INLINE_EVAL) --manifest $(INLINE_EVAL_MANIFEST) --set tune --phase3-ngram --thresholds $(INLINE_EVAL_THRESHOLDS) --out target/tua-speed/inline-eval-tune-phase3.json

.PHONY: inline-eval-phase3-smoke
inline-eval-phase3-smoke: ## Exercise the Phase 3 predictor on committed smoke fixtures without spending gate data.
	$(INLINE_EVAL_DEBUG) --manifest $(INLINE_EVAL_MANIFEST) --set phase3-smoke --max-positions 0 --phase3-ngram --thresholds $(INLINE_EVAL_THRESHOLDS) --require-correct-backend phase3-ngram --out target/tua-speed/inline-eval-smoke-phase3.json

.PHONY: inline-eval-smoke
inline-eval-smoke: ## Check the inline-completion evaluator against its committed smoke expectations (offline).
	$(INLINE_EVAL_DEBUG) --manifest $(INLINE_EVAL_MANIFEST) --set smoke --max-positions 0 --check-expected $(INLINE_EVAL_SMOKE_EXPECTED)

.PHONY: inline-eval-smoke-update
inline-eval-smoke-update: ## Rewrite the inline-completion smoke expectations after an intended change.
	$(INLINE_EVAL_DEBUG) --manifest $(INLINE_EVAL_MANIFEST) --set smoke --max-positions 0 --write-expected $(INLINE_EVAL_SMOKE_EXPECTED)

.PHONY: inline-editor-replay
inline-editor-replay: ## Replay exact tuning edits through real VS Code UI and the Tua inline provider (macOS only).
	@test -d $(VSCODE_DIR)/node_modules/@vscode/test-electron || { echo "error: extension dependencies missing; run make extension-deps first"; exit 1; }
	$(CARGO) build --package tua-cli
	$(NPM) --prefix $(VSCODE_DIR) run build
	node editors/vscode/scripts/inline-editor-replay.js $(DEBUG_TUA)

.PHONY: inline-eval-phase-gate-check
inline-eval-phase-gate-check: ## Check recorded Phase 1 prerequisites without refreshing expensive evidence.
	$(INLINE_EVAL_DEBUG) --check-phase-gate benchmarks/inline-eval/phase1-gate.toml

.PHONY: inline-eval-phase-gate
inline-eval-phase-gate: representative-phase1-gate ## Refresh performance evidence, then check all Phase 1 prerequisites.
	$(MAKE) inline-eval-phase-gate-check

.PHONY: inline-eval-holdout
inline-eval-holdout: ## Gate inline completion on the held-out Tua projects and frozen fixtures against committed thresholds.
	$(INLINE_EVAL) --manifest $(INLINE_EVAL_MANIFEST) --set holdout --thresholds $(INLINE_EVAL_THRESHOLDS) --out target/tua-speed/inline-eval-holdout.json

.PHONY: inline-eval-external
inline-eval-external: ## Fetch pinned third-party LOVE projects and gate inline completion on them (needs network; not in CI).
	node scripts/fetch-inline-eval-projects.js
	$(INLINE_EVAL) --manifest $(INLINE_EVAL_MANIFEST) --set external --thresholds $(INLINE_EVAL_THRESHOLDS) --out target/tua-speed/inline-eval-external.json

.PHONY: benchmark-smoke
benchmark-smoke: completion-ranking-smoke profile-examples-smoke representative-benchmark-smoke inline-eval-smoke inline-eval-phase3-smoke ## Exercise deterministic ranking, timing, completion-freshness, and inline-evaluation gates.

.PHONY: profile-all
profile-all: profile-lsp profile-completion profile-examples profile-representative ## Run all LSP, completion, representative, and bundled-example profiles.

.PHONY: check-example
check-example: ## Check one example project. Override with EXAMPLE=examples/love-full.
	cd $(EXAMPLE) && $(TUA) check

.PHONY: build-example
build-example: ## Build one example project. Override with EXAMPLE=examples/love-full.
	cd $(EXAMPLE) && $(TUA) build

.PHONY: build-examples
build-examples: build-love-minimal build-love-full build-love-tua build-audio-system build-features build-project-model ## Compile all bundled examples.

.PHONY: build-love-minimal
build-love-minimal: ## Compile examples/love-minimal to Lua.
	cd examples/love-minimal && $(TUA) build

.PHONY: build-love-full
build-love-full: ## Compile examples/love-full to Lua.
	cd examples/love-full && $(TUA) build

.PHONY: check-love-tua
check-love-tua: ## Check the mixed Tua/Lua bullet-hell example.
	cd examples/love-tua && $(TUA) check

.PHONY: build-love-tua
build-love-tua: ## Compile the mixed Tua/Lua bullet-hell example.
	cd examples/love-tua && $(TUA) build

.PHONY: build-audio-system
build-audio-system: ## Analyze and compile the first-class audio example.
	cd examples/audio-system && $(TUA) build

.PHONY: build-features
build-features: ## Compile examples/features to Lua.
	cd examples/features && $(TUA) build

.PHONY: build-project-model
build-project-model: ## Compile examples/project-model to Lua.
	cd examples/project-model && $(TUA) build

.PHONY: watch-example
watch-example: ## Watch one example project. Override with EXAMPLE=examples/love-full.
	cd $(EXAMPLE) && $(TUA) watch

.PHONY: lua-syntax
lua-syntax: ## Validate emitted example Lua syntax when LuaJIT/Lua is available.
	$(CARGO) test -p tua-cli --test lua_syntax -- --nocapture

.PHONY: love-test
love-test: ## Build-check LOVE examples; runtime validation is opt-in.
	$(CARGO) test -p tua-cli --test love_example -- --nocapture

.PHONY: love-runtime-test
love-runtime-test: ## Build and run LOVE example smoke tests with an installed LOVE executable.
	TUA_TEST_LOVE_RUNTIME=1 $(CARGO) test -p tua-cli --test love_example -- --nocapture

.PHONY: love-debugger-e2e
love-debugger-e2e: extension-deps ## Bind and hit a real source-mapped .tua breakpoint in LOVE.
	$(CARGO) build --package tua-cli
	TUA_BIN="$(CURDIR)/$(DEBUG_TUA)" TUA_LOVE_COMMAND="$(LOVE)" $(NPM) --prefix $(VSCODE_DIR) run test:love-debugger

.PHONY: run-love-minimal
run-love-minimal: build-love-minimal ## Build and run the minimal LOVE example.
	$(LOVE) examples/love-minimal/build

.PHONY: run-love-full
run-love-full: build-love-full ## Build and run the full LOVE example in smoke mode.
	TUA_LOVE_FULL_SMOKE=1 $(LOVE) examples/love-full/game

.PHONY: run-love-tua
run-love-tua: build-love-tua ## Build and run the bullet-hell LOVE/Tua showcase.
	$(LOVE) examples/love-tua/build

.PHONY: run-audio-system
run-audio-system: build-audio-system ## Build and run the first-class audio example.
	$(LOVE) examples/audio-system/build

.PHONY: smoke-love-tua
smoke-love-tua: build-love-tua ## Run the bullet-hell showcase for a short smoke pass.
	TUA_LOVE_TUA_SMOKE=1 $(LOVE) examples/love-tua/build

.PHONY: extension-deps
extension-deps: ## Install VS Code extension npm dependencies.
	$(NPM) install --prefix $(VSCODE_DIR)

.PHONY: extension-server
extension-server: ## Build and stage the tua server binary for the VS Code extension.
	$(CARGO) build --release --package tua-cli
	mkdir -p $(VSCODE_DIR)/bin
	cp $(RELEASE_TUA) $(VSCODE_TUA)

.PHONY: extension-clean-server
extension-clean-server: ## Remove the staged VS Code extension server binary.
	rm -rf $(VSCODE_DIR)/bin

.PHONY: extension-check
extension-check: ## Syntax-check the VS Code extension activation script.
	$(NPM) --prefix $(VSCODE_DIR) run check

.PHONY: extension-test
extension-test: extension-deps ## Run the VS Code extension-host integration tests.
	$(NPM) --prefix $(VSCODE_DIR) run test:vscode

.PHONY: extension-real-server-test
extension-real-server-test: extension-deps extension-server ## Run VS Code integration tests against the staged real tua server.
	TUA_REAL_SERVER_PATH="$(CURDIR)/$(VSCODE_TUA)" $(NPM) --prefix $(VSCODE_DIR) run test:vscode-real

.PHONY: json-check
json-check: ## Validate every tracked JSON file.
	node scripts/check-json.js

.PHONY: extension-package
extension-package: extension-deps extension-server json-check extension-check ## Build a VSIX with the bundled tua server.
	cd $(VSCODE_DIR) && $(VSCE) package --out tua-$(PROJECT_VERSION).vsix
	rm -rf $(VSCODE_DIR)/bin

.PHONY: extension-install
extension-install: extension-package ## Install the built VS Code extension with the code CLI.
	$(CODE) --install-extension $(VSIX) --force

.PHONY: extension-uninstall
extension-uninstall: ## Uninstall the VS Code extension with the code CLI.
	$(CODE) --uninstall-extension SolenodonteLabs.tua

.PHONY: extension-open-example
extension-open-example: ## Open the full LOVE example in VS Code for extension smoke testing.
	$(CODE) examples/love-full

.PHONY: extension-open-features
extension-open-features: ## Open the features example in VS Code for extension development.
	$(CODE) examples/features

.PHONY: extension-smoke
extension-smoke: extension-install extension-open-example ## Install the extension and open a .tua file.

.PHONY: extension-smoke-features
extension-smoke-features: extension-install extension-open-features ## Install the extension and open examples/features.

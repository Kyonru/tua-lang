.DEFAULT_GOAL := help

CARGO ?= cargo
NPM ?= npm
NPX ?= npx
CODE ?= code
LOVE ?= love
VSCE ?= $(NPX) --yes @vscode/vsce
RSVG_CONVERT ?= rsvg-convert
LOVE_API_ROOT ?=
LOVE_API_REVISION := $(shell sed -n '1p' scripts/love-api-revision.txt)

TUA := $(CARGO) run -q -p tua-cli --
VSCODE_DIR := editors/vscode
VSIX := $(VSCODE_DIR)/tua-0.0.1.vsix
EXAMPLE ?= examples/love-minimal
UNAME_S := $(shell uname -s 2>/dev/null)
EXE_SUFFIX := $(if $(filter MINGW% MSYS% CYGWIN%,$(UNAME_S)),.exe,)
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
verify: fmt test clippy extension-check json-check diff-check ## Run the main local verification suite.

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
	node editors/vscode/scripts/lsp-smoke.js target/debug/tua

.PHONY: lsp-features-smoke
lsp-features-smoke: ## Exercise LSP requests against examples/features with the debug server.
	$(CARGO) build --package tua-cli
	node editors/vscode/scripts/lsp-features-smoke.js target/debug/tua examples/features

.PHONY: profile-lsp
profile-lsp: ## Profile LSP request latency against small/medium/large benchmark fixtures.
	$(CARGO) build --package tua-cli
	TUA_PROFILE_REPEAT=$${TUA_PROFILE_REPEAT:-3} node editors/vscode/scripts/lsp-profile.js target/debug/tua benchmarks/lsp/small benchmarks/lsp/medium benchmarks/lsp/large

.PHONY: profile-completion
profile-completion: ## Profile completion latency, ranking overhead, and completion cache behavior.
	$(CARGO) build --package tua-cli
	TUA_PROFILE_REPEAT=$${TUA_PROFILE_REPEAT:-20} node editors/vscode/scripts/lsp-completion-profile.js target/debug/tua benchmarks/lsp/medium

.PHONY: completion-ranking-smoke
completion-ranking-smoke: ## Check fixture-driven completion ranking order.
	$(CARGO) build --package tua-cli
	node editors/vscode/scripts/lsp-completion-ranking-smoke.js target/debug/tua benchmarks/lsp/medium

.PHONY: profile-examples
profile-examples: ## Profile CLI and LSP speed across every bundled example project.
	$(CARGO) build --release --package tua-cli
	node scripts/profile-examples.js $(RELEASE_TUA)

.PHONY: profile-examples-baseline
profile-examples-baseline: ## Write a local examples speed baseline for future comparisons.
	$(CARGO) build --release --package tua-cli
	TUA_SPEED_OUT=$${TUA_SPEED_OUT:-target/tua-speed/examples-baseline.json} node scripts/profile-examples.js $(RELEASE_TUA)

.PHONY: profile-examples-smoke
profile-examples-smoke: ## Run one quick examples speed profiling pass.
	$(CARGO) build --release --package tua-cli
	TUA_SPEED_WARMUP=0 TUA_SPEED_REPEAT=1 TUA_SPEED_OUT=$${TUA_SPEED_OUT:-target/tua-speed/examples-profile-smoke.json} node scripts/profile-examples.js $(RELEASE_TUA)

.PHONY: check-example
check-example: ## Check one example project. Override with EXAMPLE=examples/love-full.
	cd $(EXAMPLE) && $(TUA) check

.PHONY: build-example
build-example: ## Build one example project. Override with EXAMPLE=examples/love-full.
	cd $(EXAMPLE) && $(TUA) build

.PHONY: build-examples
build-examples: build-love-minimal build-love-full build-features ## Compile all bundled examples.

.PHONY: build-love-minimal
build-love-minimal: ## Compile examples/love-minimal to Lua.
	cd examples/love-minimal && $(TUA) build

.PHONY: build-love-full
build-love-full: ## Compile examples/love-full to Lua.
	cd examples/love-full && $(TUA) build

.PHONY: build-features
build-features: ## Compile examples/features to Lua.
	cd examples/features && $(TUA) build

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

.PHONY: run-love-minimal
run-love-minimal: build-love-minimal ## Build and run the minimal LOVE example.
	$(LOVE) examples/love-minimal/build

.PHONY: run-love-full
run-love-full: build-love-full ## Build and run the full LOVE example in smoke mode.
	TUA_LOVE_FULL_SMOKE=1 $(LOVE) examples/love-full/game

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

.PHONY: json-check
json-check: ## Validate extension and workspace JSON metadata parse.
	node -e "for (const file of ['$(VSCODE_DIR)/package.json','$(VSCODE_DIR)/package-lock.json','$(VSCODE_DIR)/language-configuration.json','$(VSCODE_DIR)/icons/tua-icon-theme.json','$(VSCODE_DIR)/syntaxes/tua.tmLanguage.json','.vscode/launch.json','.vscode/tasks.json','.vscode/extensions.json']) JSON.parse(require('fs').readFileSync(file, 'utf8'));"

.PHONY: extension-package
extension-package: extension-deps extension-server json-check extension-check ## Build a VSIX with the bundled tua server.
	cd $(VSCODE_DIR) && $(VSCE) package --out tua-0.0.1.vsix
	rm -rf $(VSCODE_DIR)/bin

.PHONY: extension-install
extension-install: extension-package ## Install the built VS Code extension with the code CLI.
	$(CODE) --install-extension $(VSIX) --force

.PHONY: extension-uninstall
extension-uninstall: ## Uninstall the VS Code extension with the code CLI.
	$(CODE) --uninstall-extension tua-lang.tua

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

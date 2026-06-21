.DEFAULT_GOAL := help

CARGO ?= cargo
NPM ?= npm
NPX ?= npx
CODE ?= code
LOVE ?= love
VSCE ?= $(NPX) --yes @vscode/vsce

TUA := $(CARGO) run -q -p tua-cli --
VSCODE_DIR := editors/vscode
VSIX := $(VSCODE_DIR)/tua-0.0.1.vsix
EXAMPLE ?= examples/love-minimal

.PHONY: help
help: ## Show available targets.
	@awk 'BEGIN { FS = ":.*##"; printf "tua project targets:\n\n" } /^[a-zA-Z0-9_.-]+:.*##/ { printf "  %-28s %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

.PHONY: fmt
fmt: ## Format the Rust workspace.
	$(CARGO) fmt --all

.PHONY: test
test: ## Run the full Rust test suite.
	$(CARGO) test --workspace

.PHONY: clippy
clippy: ## Run clippy with warnings denied.
	$(CARGO) clippy --workspace --all-targets -- -D warnings

.PHONY: verify
verify: fmt test clippy extension-check json-check diff-check ## Run the main local verification suite.

.PHONY: diff-check
diff-check: ## Check the git diff for whitespace errors.
	git diff --check

.PHONY: cli-help
cli-help: ## Print tua CLI help.
	$(TUA) --help

.PHONY: cli-version
cli-version: ## Print tua CLI version.
	$(TUA) --version

.PHONY: lsp
lsp: ## Run the tua language server over stdio.
	$(TUA) lsp

.PHONY: check-example
check-example: ## Check one example project. Override with EXAMPLE=examples/love-full.
	cd $(EXAMPLE) && $(TUA) check

.PHONY: build-example
build-example: ## Build one example project. Override with EXAMPLE=examples/love-full.
	cd $(EXAMPLE) && $(TUA) build

.PHONY: build-examples
build-examples: build-love-minimal build-love-full ## Compile all bundled examples.

.PHONY: build-love-minimal
build-love-minimal: ## Compile examples/love-minimal to Lua.
	cd examples/love-minimal && $(TUA) build

.PHONY: build-love-full
build-love-full: ## Compile examples/love-full to Lua.
	cd examples/love-full && $(TUA) build

.PHONY: watch-example
watch-example: ## Watch one example project. Override with EXAMPLE=examples/love-full.
	cd $(EXAMPLE) && $(TUA) watch

.PHONY: lua-syntax
lua-syntax: ## Validate emitted example Lua syntax when LuaJIT/Lua is available.
	$(CARGO) test -p tua-cli --test lua_syntax -- --nocapture

.PHONY: love-test
love-test: ## Run LOVE example tests; skips gracefully if LOVE is unavailable.
	$(CARGO) test -p tua-cli --test love_example -- --nocapture

.PHONY: run-love-minimal
run-love-minimal: build-love-minimal ## Build and run the minimal LOVE example.
	$(LOVE) examples/love-minimal/build

.PHONY: run-love-full
run-love-full: build-love-full ## Build and run the full LOVE example in smoke mode.
	TUA_LOVE_FULL_SMOKE=1 $(LOVE) examples/love-full/game

.PHONY: extension-deps
extension-deps: ## Install VS Code extension npm dependencies.
	$(NPM) install --prefix $(VSCODE_DIR)

.PHONY: extension-check
extension-check: ## Syntax-check the VS Code extension activation script.
	$(NPM) --prefix $(VSCODE_DIR) run check

.PHONY: json-check
json-check: ## Validate extension JSON metadata and grammars parse.
	node -e "for (const file of ['$(VSCODE_DIR)/package.json','$(VSCODE_DIR)/package-lock.json','$(VSCODE_DIR)/language-configuration.json','$(VSCODE_DIR)/icons/tua-icon-theme.json','$(VSCODE_DIR)/syntaxes/tua.tmLanguage.json']) JSON.parse(require('fs').readFileSync(file, 'utf8'));"

.PHONY: extension-package
extension-package: extension-deps json-check extension-check ## Build a VSIX for the VS Code extension.
	cd $(VSCODE_DIR) && $(VSCE) package --out tua-0.0.1.vsix

.PHONY: extension-install
extension-install: extension-package ## Install the built VS Code extension with the code CLI.
	$(CODE) --install-extension $(VSIX) --force

.PHONY: extension-uninstall
extension-uninstall: ## Uninstall the VS Code extension with the code CLI.
	$(CODE) --uninstall-extension tua-lang.tua

.PHONY: extension-open-example
extension-open-example: ## Open the minimal example in VS Code for manual extension smoke testing.
	$(CODE) examples/love-minimal/src/main.tua

.PHONY: extension-smoke
extension-smoke: extension-install extension-open-example ## Install the extension and open a .tua file.

# Setup, installation and offline documentation audit

Bounded audit of revision/binary baseline in ../BASELINE.md. No product changes, installs, normal-profile writes, or network requests. One low-severity confirmed **documentation** defect; zero confirmed runtime defects in exercised workflows.

## Evidence and coverage

| Surface | Coverage | Outcome / limit |
| --- | --- | --- |
| Standalone onboarding | Executed absolute `target/release/tua --version`, `init`, `check`, `build`, repeated `init` in `setup_docs/fixture` | 0.6.0; all exit 0; repeated init reports files unchanged. `onboarding.json` |
| Quickstart program | Copied first Tua block from docs/quickstart.md into fixture; check/build; Lua 5.4 callback stub invokes update(1)/draw | All exit 0; x=200 and documented circle arguments. `quickstart.json`; generated fixture/build/main.lua. This is not a graphical LOVE launch. |
| Setup apply/restore | `node --test editors/vscode/test/unit/{setup-presets,setup-commands,annotation-guide,love-api-reference}.test.js` | 19/19 pass; inert activation, repeated apply/reload, explicit false/unset restoration, later edits, partial failure/retry, serialized clicks, mocked icon install failures. `unit-tests.log` |
| Theme | `node editors/vscode/scripts/theme-check.js` | 114 checks; text minimum 4.72:1, focus 3.89:1. `theme.log`; tests selected tokens and focus combinations, not exhaustive GUI accessibility. Theme explicitly documented as dark, not formal HC. |
| Manifest/settings/walkthrough | `node editors/vscode/scripts/manifest-check.js`; source inspection of presets/index.js and package.json | Exit 0. All five walkthrough media files/buttons resolve; preset scopes/restoration follow documented user-setting behavior. `walkthrough.json`, `manifest.log` |
| Offline guides | Actual registered command callbacks under VS Code mocks; annotation generator `--check`; HTMLParser link/resource scan | Annotation drift check exit 0; LOVE 11.5 reference 4,202 fragment links resolve against 1,112 named/id anchors; no remote src dependencies. `offline-anchors.json`, `annotation-drift.log` |
| Packaging | Existing local `editors/vscode/node_modules/.bin/vsce ls` | Exit 0; annotation guide, LOVE index/NOTICE, theme and five setup media included. `package-list.log`. Packaging list, not fresh VSIX installation. |
| Learning/navigation | Inspected installation, quickstart, learn, comments, annotations, releases, editor README, zensical navigation and ROADMAP | 39 relative file links across six focused pages resolve; `navigation.json`. Lua Basics → Quickstart → Learn → comments/interop paths exist. Public URLs/marketplace availability not probed. |
| Platform/GUI | Not exercised | No VS Code native walkthrough clicks, screen reader, keyboard-only webview, OS theme switching, macOS fresh install/quarantine, Windows/Linux installs or real registry installation. No claims about those paths. |

## SETUP-DOCS-01 — Setup README omits the shipped shader step

- Severity/category: low, confirmed documentation defect (not runtime).
- Reproduction: inspect `editors/vscode/README.md:138` and its four bullets, then enumerate `contributes.walkthroughs` entry `tua.setup` in package.json. `walkthrough.json` records the shipped five-step shape. The passing manifest check at `editors/vscode/scripts/manifest-check.js:270` explicitly requires five steps.
- Expected: the optional setup overview describes all current choices and their scope. Actual: “All four steps” omits shader associations, highlighting installation, validator checks and restore despite the shader step at `editors/vscode/package.json:2047`. README:168 also describes only the optional icon installation when explaining registry contact; the shader highlighting button also installs an extension (`editors/vscode/src/setup/index.js:104`, locate `setupInstallShaderHighlighting` if line numbers move).
- Impact: readers miss a useful setup option and do not learn its distinct workspace scope or optional registry download from the setup overview. Focused shader docs already explain these capabilities at docs/lsp.md:783 and assets/setup/shaders.md.
- Existing blind spot: manifest tests enforce five runtime steps/media; no check compares the README inventory or its install explanation to manifest actions.
- Smallest improvement: update the step count, add a shader bullet linked to the existing focused documentation, and mention both optional registry installations. Avoid duplicating validator details.
- Regression: include the walkthrough overview in documentation review whenever adding/removing a walkthrough step; a lightweight list comparison is optional.
- Known/new: not found as an explicit existing finding in searched prior audit Markdown or ROADMAP; no roadmap promise needed for this correction.

No unsupported runtime findings or speculative usability requests added. Shader validator behavior belongs to the shader lane. Initial exploratory regex anchor counting was discarded because it missed whitespace attributes and named anchors; the recorded final parser result recognizes both correctly.

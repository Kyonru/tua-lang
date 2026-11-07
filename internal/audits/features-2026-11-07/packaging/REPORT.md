# Packaging and distribution audit

Candidate 0.6.0, baseline 9d490dcd299bd25218c9fa4adc83e080e617453c. One confirmed medium defect; no high defects found in this bounded pass. Product files unchanged. Applied audit-lead, language-auditor, release-prep, security-auditor, and test-auditor instructions.

## Exercised and inspected

| Surface | Status / evidence |
| --- | --- |
| `.love` build, custom output directory containing spaces, archive names, assets/conf.lua | Passed actual CLI; `commands.json`, `followup.json`, `normal/` |
| Byte-identical rebuild | Passed; identical archive bytes across two runs |
| Exclude maps, source, config, hidden files, symlinks, dist | Passed representative in-place fixture; `in-place/`; archive contains only conf.lua/main.lua/secret.lua |
| Test source directory and `_spec.tua` | Passed; final archive contains conf.lua/level.txt/main.lua only. Initial `main.spec.tua` fixture was incorrectly named for the documented convention and legitimately shipped; corrected to `main_spec.tua` in followup |
| Missing Windows runtime option / absent macOS runtime path | Both return exit 1 with actionable diagnostics |
| Path-like package name | `../../escape` becomes `dist/-..-escape.love`, stays within dist |
| Windows fusion | Mock runtime only: executable equals original runtime bytes plus archive; DLL/license copied, lovec.exe omitted |
| Real LOVE 11.5a macOS fusion | Passed; requested name/identifier, embedded archive, ad hoc codesign and strict deep signature verification |
| Native `.love` and fused app launch | Passed outside restricted sandbox, output `PACKAGING_RUNTIME_OK`, exit 0; fused raw output `native-fused.log` |
| Server archive + candidate VSIX integrity | Passed `scripts/smoke_release.py`: exact matching executable bytes, manifest/version/entry point, extracted binaries init/check/build, rejection of invalid program |
| Release archive manifest/checksum | Passed: executable/README/LICENSE/CHANGELOG; SHA-256 matches sidecar; `followup.json` |
| Version synchronization and release packaging tool tests | Passed 0.6.0 synchronization; 7 Python packaging tests |
| Trusted CI/release/publication gates | Inspected `.github/workflows/ci.yml`, `release.yml`, `vscode-extension.yml`; not executed remotely |

`probe.py` preserves initial commands; `commands.json` preserves outputs. `followup.json` records corrected spec and Windows mock tests and artifact hashes. Existing CLI binary hash matches BASELINE.md. Native launch commands were `/Applications/LOVE.app/Contents/MacOS/love normal/dist/Star\ Dust.love` (absolute archive path) and `normal/dist/macos/Star\ Dust.app/Contents/MacOS/love`. Both first aborted with -6 in restricted execution; both succeeded after native execution escalation. This is an environment qualification, not a packaging defect.

## PACKAGING-01 — Fused macOS app retains LOVE document-handler claim

**Severity:** medium. **Category/status:** confirmed defect. **Known:** no matching prior finding found in ROADMAP.md or prior audit reports.

**Reproduction:** from `normal/`, run the absolute `target/release/tua package --target macos --love /Applications/LOVE.app --name 'Star Dust' --identifier org.example.packagingaudit`. Inspect generated `dist/macos/Star Dust.app/Contents/Info.plist` using `plutil -p` or Python plistlib. Exact command/output and parsed plist are in `commands.json`.

**Expected:** per `docs/packaging.md:75-78`, the generated app no longer claims `.love` files.

**Actual:** `UTExportedTypeDeclarations` is gone, but `CFBundleDocumentTypes[0]` still declares `CFBundleTypeExtensions = [love]`, `CFBundleTypeRole = Viewer`, `LSHandlerRank = Owner`, and `LSItemContentTypes = [org.love2d.love-game]`. Generic folder/document handlers also remain. This leaves the produced game advertising LOVE-file handling; actual Launch Services registration was deliberately not performed, so effects on user defaults are not asserted.

**Root:** `crates/tua-cli/src/package/mod.rs:299-302` removes only exported type declarations. **Test blind spot:** fixture/test at `:518-565` omits the actual runtime's `CFBundleDocumentTypes`, and assertions only check export declarations. Existing CLI packaging integration tests exercise `.love` and a synthetic Windows runtime, not real macOS metadata.

**Smallest improvement:** remove LOVE document handler declarations when creating a standalone game, using the shared plist transformation; preserve unrelated app metadata. Add a realistic XML plist fixture including exported types and document handlers and assert both are absent after transformation; parse the result and retain name/identifier assertions. Native install/profile changes are unnecessary for that regression test.

## Release gate assessment and limits

`release.yml:45-54` makes packaging depend on reusable trusted CI. Validation checks synchronized versions, release tools, tag/version, and changelog. Each of four target jobs checks the executable version, stages that same binary into VSIX, builds archives, checksums them, and executes `smoke_release.py` to verify matching bytes and behavior (`:94-144`). Linux additionally installs/exercises VSIX in the existing harness. Publication waits for build success and verifies combined checksums (`:161-179`). Marketplace/Open VSX workflow checks downloaded VSIX bytes against published SHA256SUMS before publication. Open/untrusted PR execution is intentionally absent and is not a finding.

No downloads, normal-profile installation, registration, signing identity use, tagging, pushes, or publication occurred. Only ad hoc signing of the disposable fused app occurred. No native Windows game launch, Windows/Linux/Intel server run, installer/Gatekeeper/notarization qualification, or live marketplace/attestation verification occurred. The candidate VSIX was not installed by this lane; its executable contents were exercised through the repository smoke script. Platform signing remains an explicitly documented ROADMAP item, not a new defect. Existing shared asset/build ownership findings were not duplicated. Tests/filters were representative, not exhaustive multi-root or custom-extension coverage.

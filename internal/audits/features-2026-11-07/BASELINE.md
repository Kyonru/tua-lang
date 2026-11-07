# Baseline

Revision: 9d490dcd299bd25218c9fa4adc83e080e617453c

Working tree (preserved):
```
 M CHANGELOG.md
 M Cargo.lock
 M Cargo.toml
 M docs/releases.md
 M editors/vscode/package-lock.json
 M editors/vscode/package.json
 M fuzz/Cargo.lock
```

Candidate version 0.6.0; release binary target/release/tua. Prior release checks: two existing core nominal assignment failures and inline-eval-smoke snapshot mismatch; Clippy clean, 169 extension tests pass, CLI tests 128 pass. Do not count those known failures as newly introduced by this audit. Full details: target/release-prep-0.6.0/README.md.

Release binary SHA-256: `cabb9291f29eba8a1d007eb248ffc40d800622cd927afcc1086c2527eb9feef4`

Lua: Lua 5.4.8  Copyright (C) 1994-2025 Lua.org, PUC-Rio

## Fresh shared checks

- `make runtime-test`: passed four standalone Lua suites (tween, camera, hot reload, Baton); see runtime-baseline.log. This does not cover every bundled helper.
- `RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -p tua-lsp`: 389 passed, zero failures; see lsp-baseline.log.
- Inventory: 105 registered extension commands and 74 settings saved in extension-surface.json.
- Trusted CI: `.github/workflows/ci.yml` runs on main pushes, manual dispatch and workflow_call; release.yml:45-53 invokes it before packaging. Platform tests run the workspace tests, Linux runs runtime-test with LuaJIT. Open PR execution is intentionally absent, not classified as a defect.

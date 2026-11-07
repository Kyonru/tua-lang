# Typing completion regression — 2026-11-08

The installed 0.6.0 server stalled on nested import analysis. In the zombie project,
completion in `src/core/state.tua` worked when opened alone (24 ms), but timed out
at 30 seconds after `src/main.tua` was opened first. The 0.5.1 server returned a
completion in main in 253 ms. Installed 0.6.0 matched the previous packaged binary.
VSCodium also logged a server-stop timeout and a later completion-provider error.

A process sample showed repeated `required_module_exports_for_path` →
`infer_module_exports` work. The earlier transitive-staleness repair (`0bffb26`)
disabled reuse for every module with imports. Recursive analysis consequently
repeated work, blocking requests from other open files. This was a regression,
not a disabled suggestion setting.

## Fix

The standalone filesystem cache now retains validated dependency inputs: source
contents and resolved import links, including initially missing targets. It checks
inputs before reuse and around inference. Cyclic/depth-truncated answers are not
cached; bounded walk-local reuse remains available when a complete dependency
snapshot cannot be retained. Project snapshots remain the authoritative project
query path.

Regression tests in `crates/tua-core/src/module_index/tests.rs` verify linear
inference work through ten dependency layers, reuse across unchanged requests,
transitive edits, new/deleted/preferred modules, and cycle-sensitive exports.
All 27 module-index tests pass. The existing same-size/same-timestamp edit test
and transitive invalidation test remain intact.

## Read-only editor reproduction

From the repository root:

```sh
node internal/audits/typing-completion-2026-11-08/probe.js \
  /absolute/path/to/tua /absolute/path/to/project \
  src/core/state.tua src/main.tua
```

The fixture assumes a store named `player` with an `aiming` field, as in the
reported project. The script opens main first and edits an unsaved state buffer;
it does not write game source or execute the game. Positions use exact UTF-16
buffer offsets. It reports server initialization separately from completion.

The first working cache draft still took 1.6–2.4 seconds for several requests.
The dependency-validated draft returned the first completion in 195 ms and
subsequent probes in 2–320 ms. These are individual local protocol measurements,
not a general latency guarantee or a desktop UI qualification. Final-tree replay
and verification results are recorded alongside this report.

## Final verification

- `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 make verify`:
  passed; 2,346 Rust tests, 172 extension tests, three explicitly ignored Rust tests.
- `make docs-build`: passed.
- `CARGO_INCREMENTAL=0 make extension-package`: passed; archive integrity passed.
- Final packaged-server replay: initialization 779 ms; first completion after
  opening main and state 197 ms; subsequent typing probes 2–302 ms. Correct
  candidates include `graphics`, store methods, `aiming`, `pointing`, and
  `gunPosition`. See [after.json](after.json) and [baseline timeout](before.log).
- The bundled server's SHA-256 matches the tested release executable:
  `9ea4fc50ab2b8e931094923dc9df74a77a60d54ea6c1d92d6ef727bd3e89cced`.
- Updated `editors/vscode/tua-0.6.0.vsix` SHA-256:
  `aaf6ec41798cb5f9517eb2b53369116d6946de68fc9edfa998ee70d918ebcfb3`.

An earlier full check hit the intermittent shader subprocess timeout
`headless_timeout_applies_per_process_not_per_batch`. The final complete rerun
passed; shader timeout settings and tests were not relaxed. Earlier extension
activation checks also encountered the previously documented mock-process startup
timeout; the final verification and packaging runs passed.

This verifies protocol behavior against the reported project and automated suites.
The updated VSIX still needs the user's installed-editor check. Reinstall the
0.6.0 package and reload the editor window to replace the old stalled server.

---
name: tua-game-packaging
description: Change or verify Tua game packaging, distribution targets, custom pipelines, generated GitHub Actions, or VS Code packaging flows. Use for .love, Windows, macOS, web, and custom game artifacts; not for publishing the Tua toolchain itself.
---

# Tua Game Packaging

Package the generated game for a specific target without changing the Tua
compiler's release process. Start with `docs/packaging.md` and `docs/cli.md`.

## Trace The Artifact

- Confirm project root, generated `main.lua`, copied assets, helper runtimes,
  `conf.lua`, and excluded source maps. Test with a helper-free and a helper-
  using game. Verify deterministic `.love` contents and path safety.
- Windows and macOS targets require the matching LÖVE runtime; web requires a
  compatible standalone player and host headers. A custom target runs only
  when selected, receives the built `.love` and staging paths, and must keep
  the previous package on failure. Do not treat a successful archive build as
  proof a target platform runs the game.
- The VS Code picker selects the active project's target and runs a task;
  preserve error output, cancellation, and artifact reveal. `tua actions init`
  creates a workflow only when absent; it relies on a published Tua CLI
  release matching the generated pin and does not need GitHub Packages.

## Verify And Document

- Exercise positive and failure paths for the changed target, inspect archive
  entries, and run on the platform when available. Record host or external
  runtime blockers accurately. Check custom commands and paths as untrusted
  project inputs.
- Update `docs/packaging.md`, command help, VS Code labels, examples, and
  regression tests for any user-visible change. Do not push, tag, publish,
  or edit release credentials without authorization.

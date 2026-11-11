# Documentation audit

## Sources and checks

The public navigation in `zensical.toml`, command overview in `docs/cli.md`, README, `examples/features/coverage.json`, and the CLI's `--help` output were compared at baseline `c08edbca`. `make docs-build` and `make feature-examples-check` passed. These checks establish valid navigation/buildable feature-tour mapping, not that every prose statement is true at runtime.

The feature navigation has 13 domain guides and the coverage map names all 13, including behavior trees, ECS, stores, and signals. The CLI help includes all command headings listed in `docs/cli.md`, including `shader-tools`, `api`, and `mcp`. The fresh-project CLI path described by the docs passed for init, check, lint, format check, build, Lua execution, and `.love` packaging. The unit-testing example passed three Busted cases.

## Unverified claims and limits

- Real LOVE launch, debugger mapping, shader validation in a graphical context, Windows fusion, and hot reload remain unqualified in this local run. A macOS fused app was packaged and its structure checked, but it was not launched. LOVE 11.5 is installed; even a one-line independent LOVE game aborts in this host, and the dummy SDL driver cannot create an OpenGL context. The docs build cannot qualify these runtime claims.
- The current source-only review has not checked every code sample, external link, VS Code screenshot, or exact diagnostic text. These remain pending until the corresponding investigation lanes report evidence.
- The `0.7.0` roadmap intentionally distinguishes scoped implemented contracts from runtime/long-session qualification; that distinction should be retained in any user-facing edits.

No documentation defect is confirmed by these initial checks. Specific corrections belong here with page, line, observed behavior, and test evidence when found.
